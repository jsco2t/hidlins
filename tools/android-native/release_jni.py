#!/usr/bin/env python3
"""SDK-only clipboard regression against the actual minified shipping APK.

No test code is added to that APK. Instrumentation is a separate, temporary
SDK-only APK; signing changes only scratch copies. Each device/server is owned
by this invocation, and every subprocess/boot/runtime wait has a deadline.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import socket
import subprocess
import tempfile
import time
import zipfile


ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "tools/android-native/release_jni"
OUTPUT = ROOT / "build/verification/release-clipboard"
APK_ROOT = ROOT / "app/build/app/outputs/apk"
MAPPING = ROOT / "app/build/app/outputs/mapping/release/mapping.txt"
RUNNER = "app.hidlins.releaseprobe/app.hidlins.releaseprobe.ReleaseClipboardInstrumentation"
SUCCESS = "HIDLINS_RELEASE_CLIPBOARD 1 passed, 0 failed"
FAILURE = re.compile(r"HIDLINS_RELEASE_CLIPBOARD 0 passed, 1 failed stage=([a-z-]+) code=([A-Z0-9_]+)")
OFFLINE_PERMISSIONS = ("android.permission.NEARBY_WIFI_DEVICES",
                       "android.permission.ACCESS_LOCAL_NETWORK")


def checked(args: list[str], env: dict[str, str], timeout: int = 60,
            *, diagnostic_output: bool = True) -> str:
    result = subprocess.run(args, cwd=ROOT, env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout)
    if result.returncode:
        detail = "\n" + result.stdout if diagnostic_output else "; application output suppressed"
        raise RuntimeError(f"tool failed: {Path(args[0]).name} ({result.returncode}){detail}")
    return result.stdout


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def offline_permissions(output: str, api: int) -> list[str]:
    declared = set(re.findall(r"(?m)^\s*permission:\s*(android\.permission\.[A-Z_]+)\s*$", output))
    if api >= 33 and OFFLINE_PERMISSIONS[0] not in declared:
        raise RuntimeError("SDK did not enumerate the required nearby-device runtime permission")
    return [name for name in OFFLINE_PERMISSIONS if name in declared]


def verify_offline_denial(output: str, permissions: list[str]) -> None:
    for name in permissions:
        found = re.search(r"(?m)^\s*" + re.escape(name)
                          + r": granted=false, flags=\[([^\]]*)\]", output)
        tokens = set(re.findall(r"[A-Z_]+", found[1])) if found else set()
        if not {"USER_SET", "USER_FIXED"}.issubset(tokens):
            raise RuntimeError("offline permission denial was not established")


def mapped_field(mapping: str, owner: str, name: str) -> str | None:
    active = False
    for line in mapping.splitlines():
        if line and not line.startswith((" ", "#")):
            active = line.startswith(owner + " -> ")
        if active:
            found = re.fullmatch(r"\s+\S+ " + re.escape(name) + r" -> (\S+)", line)
            if found:
                return found[1]
    return None


def mapping_arguments(variant: str, text: str) -> dict[str, str]:
    fields = {"servicesField": ("app.hidlins.MainActivity", "platformServices"),
              "clipboardField": ("app.hidlins.HidlinsPlatformServices", "clipboard"),
              "ttlField": ("app.hidlins.ClipboardController", "pendingTtlSeconds")}
    result = {}
    for argument, (owner, name) in fields.items():
        value = name if variant == "debug" else mapped_field(text, owner, name)
        # The pre-fix release legitimately prunes TTL. Let the real callback
        # lookup demonstrate the ABI defect before requiring this probe field.
        if value is None and argument == "ttlField":
            continue
        if value is None or not re.fullmatch(r"[A-Za-z_$][A-Za-z0-9_$]*", value):
            raise RuntimeError(f"missing/invalid release mapping for {owner}.{name}")
        result[argument] = value
    return result


def apk_payload(path: Path) -> dict[str, str]:
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        if len(names) != len(set(names)):
            raise RuntimeError("APK contains ambiguous duplicate entries")
        return {name: hashlib.sha256(archive.read(name)).hexdigest()
                for name in names if not re.fullmatch(
                    r"META-INF/(MANIFEST\.MF|[^/]+\.(SF|RSA|DSA|EC))", name, re.IGNORECASE)}


def require_release_payload(path: Path) -> None:
    with zipfile.ZipFile(path) as archive:
        required = ("lib/arm64-v8a/libapp.so", "lib/x86_64/libapp.so",
                    "assets/flutter_assets/AssetManifest.bin")
        for name in required:
            try:
                present = archive.getinfo(name).file_size > 0
            except KeyError:
                present = False
            if not present:
                raise RuntimeError("release APK lacks the canonical Flutter application payload")


def parse_result(output: str) -> dict[str, str | bool]:
    success = output.count(SUCCESS)
    failures = FAILURE.findall(output)
    # Instrumentation can exit the adb command with zero even when its test
    # failed. Require one complete, unambiguous runner result and Android code.
    if success == 1 and not failures and "INSTRUMENTATION_CODE: -1" in output:
        return {"passed": True}
    if success == 0 and len(failures) == 1 and "INSTRUMENTATION_CODE: 0" in output:
        return {"passed": False, "stage": failures[0][0], "code": failures[0][1]}
    raise RuntimeError("instrumentation did not return one complete clipboard result")


def has_callback_definition(dump: str, descriptor: str) -> bool:
    """Accept a real public instance method, never a name/string reference."""
    for block in re.split(r"(?m)^Class #", dump):
        owner = re.search(r"Class descriptor\s*: '([^']+)'", block)
        if owner is None or owner[1] != descriptor:
            continue
        virtual = block.split("  Virtual methods", 1)
        if len(virtual) != 2:
            return False
        for method in re.split(r"(?m)^    #\d+\s+:", virtual[1]):
            if (re.search(r"name\s*: 'receive'", method)
                    and re.search(r"type\s*: '\(\[B\)Z'", method)):
                access = re.search(r"access\s*: (0x[0-9a-fA-F]+)", method)
                return access is not None and int(access[1], 16) & 0x9 == 0x1
    return False


def inspect_apk(apk: Path, sdk: Path, env: dict[str, str], mapping: Path = MAPPING) -> None:
    # Concrete class names may change; read its actual R8 mapping, then verify
    # method definitions in DEX, independently of the reflective runtime test.
    mapped = re.search(r"(?m)^app\.hidlins\.ClipboardController -> (\S+):$", mapping.read_text())
    if mapped is None:
        raise RuntimeError("clipboard implementation absent from release mapping")
    descriptor = "L" + mapped[1].replace(".", "/") + ";"
    found = False
    with zipfile.ZipFile(apk) as archive, tempfile.TemporaryDirectory(prefix="hidlins-dex-") as temporary:
        for name in archive.namelist():
            if re.fullmatch(r"classes\d*\.dex", name):
                path = Path(temporary) / name
                path.write_bytes(archive.read(name))
                dump = checked([str(sdk / "build-tools/36.0.0/dexdump"), str(path)], env)
                found |= has_callback_definition(dump, descriptor)
    if not found:
        raise RuntimeError("shipping DEX lacks public instance receive([B)Z callback")


def tools(environment: dict[str, str]) -> tuple[Path, Path, dict[str, str]]:
    sdk_value = environment.get("ANDROID_SDK_ROOT") or environment.get("ANDROID_HOME")
    java_value = environment.get("JAVA_HOME")
    if not sdk_value or not java_value:
        raise RuntimeError("set ANDROID_SDK_ROOT and JAVA_HOME to the approved cached toolchains")
    sdk, java = Path(sdk_value).resolve(), Path(java_value).resolve()
    build = sdk / "build-tools/36.0.0"
    required = [sdk / "platform-tools/adb", sdk / "emulator/emulator",
                sdk / "platforms/android-36/android.jar", java / "bin/javac",
                java / "bin/keytool"] + [build / tool for tool in
                ("aapt", "apksigner", "d8", "zipalign", "dexdump")]
    if any(not path.is_file() for path in required):
        raise RuntimeError("missing approved SDK36/JDK build tools; no automatic download")
    env = dict(environment, JAVA_HOME=str(java), ANDROID_SDK_ROOT=str(sdk),
               PATH=str(java / "bin") + os.pathsep + environment.get("PATH", ""))
    return sdk, java, env


def stop_owned_build(process: subprocess.Popen) -> None:
    # The leader can already have exited while an ordinary descendant remains.
    # Signal the owned group even in that case, then reap the direct child.
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait(timeout=5)


def build_shipping(env: dict[str, str]) -> None:
    # Flutter invokes Gradle itself. Keep that canonical build offline without
    # changing tracked configuration or any other project's Gradle settings.
    policy = ROOT / ".gradle/android/init.d/hidlins-release-jni-offline.gradle"
    policy.parent.mkdir(parents=True, exist_ok=True)
    with policy.open("x") as output:
        output.write("gradle.startParameter.offline = true\n")
    process = None
    try:
        # The build and emulator run sequentially. Do not retain a multi-GB
        # compilation daemon alongside the subsequently started Android guest.
        build_env = dict(env, GRADLE_OPTS=env.get("GRADLE_OPTS", "") + " -Dorg.gradle.daemon=false")
        process = subprocess.Popen(
            ["make", "-f", "tools/android-native/release-jni-tests.mk", "build"],
            cwd=ROOT, env=build_env, start_new_session=True)
        status = process.wait(timeout=3600)
        if status:
            raise subprocess.CalledProcessError(status, process.args)
    finally:
        if process is not None:
            stop_owned_build(process)
        policy.unlink()


def sign_apk(source: Path, destination: Path, build: Path, scratch: Path,
             env: dict[str, str]) -> None:
    checked([str(build / "apksigner"), "sign", "--ks", str(scratch / "probe.p12"),
             "--ks-pass", "file:" + str(scratch / "signing-password"),
             "--v1-signing-enabled", "false", "--v2-signing-enabled", "true",
             "--v3-signing-enabled", "false", "--v4-signing-enabled", "false",
             "--out", str(destination), str(source)], env)
    checked([str(build / "apksigner"), "verify", str(destination)], env)
    if apk_payload(source) != apk_payload(destination):
        raise RuntimeError("signing changed APK executable/resource payloads")


def package_probe(sdk: Path, java: Path, scratch: Path, env: dict[str, str]) -> Path:
    build = sdk / "build-tools/36.0.0"
    android = sdk / "platforms/android-36/android.jar"
    classes, dex = scratch / "classes", scratch / "dex"
    classes.mkdir()
    dex.mkdir()
    checked([str(java / "bin/javac"), "--release", "17", "-Xlint:all", "-Werror",
             "-cp", str(android), "-d", str(classes),
             str(FIXTURE / "ReleaseClipboardInstrumentation.java")], env)
    checked([str(build / "d8"), "--min-api", "29", "--lib", str(android),
             "--output", str(dex)] + [str(p) for p in sorted(classes.rglob("*.class"))], env)
    unsigned = scratch / "probe-unsigned.apk"
    checked([str(build / "aapt"), "package", "-f", "-M", str(FIXTURE / "AndroidManifest.xml"),
             "-I", str(android), "-F", str(unsigned)], env)
    with zipfile.ZipFile(unsigned, "a", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.write(dex / "classes.dex", "classes.dex")
    aligned = scratch / "probe-aligned.apk"
    checked([str(build / "zipalign"), "-f", "4", str(unsigned), str(aligned)], env)
    password = scratch / "signing-password"
    password.touch(mode=0o600)
    password.write_text(os.urandom(24).hex() + "\n")
    checked([str(java / "bin/keytool"), "-genkeypair", "-noprompt", "-storetype", "PKCS12",
             "-keystore", str(scratch / "probe.p12"), "-storepass:file", str(password),
             "-alias", "probe", "-keyalg", "RSA", "-keysize", "2048", "-validity", "2",
             "-dname", "CN=Hidlins Temporary Clipboard Regression"], env)
    signed = scratch / "probe.apk"
    sign_apk(aligned, signed, build, scratch, env)
    return signed


def stop_owned(process: subprocess.Popen) -> None:
    if process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)


def run_device(entry: dict, apk: Path, probe: Path, arguments: dict[str, str],
               sdk: Path, scratch: Path, env: dict[str, str], mode: str) -> dict:
    server, console = 5039, 5584
    serial = f"emulator-{console}"
    for port in (server, console, console + 1):
        deadline = time.monotonic() + 15
        while True:
            try:
                with socket.socket() as listener:
                    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                    listener.bind(("127.0.0.1", port))
                break
            except OSError:
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.1)
    user = scratch / f"android-user-{entry['api']}"
    user.mkdir()
    sdk_temporary = scratch / f"sdk-tmp-{entry['api']}"
    sdk_temporary.mkdir(mode=0o700)
    avd_home = env.get("ANDROID_AVD_HOME")
    if not avd_home or not (Path(avd_home) / (entry["avd"] + ".ini")).is_file():
        raise RuntimeError("set ANDROID_AVD_HOME to the provisioned API29/API36 matrix")
    owned = dict(env, ANDROID_USER_HOME=str(user), ANDROID_ADB_SERVER_PORT=str(server),
                 ADB_SERVER_SOCKET=f"tcp:127.0.0.1:{server}",
                 ANDROID_TMP=str(sdk_temporary), TMPDIR=str(sdk_temporary))
    adb = [str(sdk / "platform-tools/adb"), "-P", str(server)]
    device = adb + ["-s", serial]
    process = None
    installed_app = installed_probe = False
    try:
        checked(adb + ["start-server"], owned)
        devices = checked(adb + ["devices"], owned).splitlines()[1:]
        if any(line.strip() for line in devices):
            raise RuntimeError("dedicated clipboard adb server has an existing device")
        acceleration = "off" if platform.system() == "Linux" and not os.access(
            "/dev/kvm", os.R_OK | os.W_OK) else "auto"
        boot = OUTPUT / f"api{entry['api']}-boot.log"
        with boot.open("w") as log:
            process = subprocess.Popen(
                [str(sdk / "emulator/emulator"), "-avd", entry["avd"], "-read-only",
                 "-no-window", "-no-audio", "-no-snapshot", "-no-boot-anim", "-accel", acceleration,
                 "-gpu", "swiftshader", "-cores", "2", "-memory", "1536", "-port", str(console)],
                env=owned, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            deadline, progress = time.monotonic() + 600, 0.0
            while True:
                if process.poll() is not None:
                    raise RuntimeError(f"owned emulator exited; inspect {boot}")
                state = subprocess.run(device + ["shell", "getprop", "sys.boot_completed"],
                                       env=owned, text=True, stdout=subprocess.PIPE,
                                       stderr=subprocess.DEVNULL, timeout=15)
                if state.returncode == 0 and state.stdout.strip() == "1":
                    break
                if time.monotonic() >= deadline:
                    raise RuntimeError(f"owned emulator boot timed out; inspect {boot}")
                if time.monotonic() >= progress:
                    print(f"Waiting for API{entry['api']} ({acceleration} acceleration)", flush=True)
                    progress = time.monotonic() + 20
                time.sleep(2)
            actual = {"api": checked(device + ["shell", "getprop", "ro.build.version.sdk"], owned).strip(),
                      "abi": checked(device + ["shell", "getprop", "ro.product.cpu.abi"], owned).strip(),
                      "avd": checked(device + ["emu", "avd", "name"], owned).splitlines()[0]}
            if actual != {"api": str(entry["api"]), "abi": entry["abi"], "avd": entry["avd"]}:
                raise RuntimeError("owned emulator identity does not match the required matrix")
            checked(device + ["install", "-t", str(apk)], owned, timeout=180)
            installed_app = True
            checked(device + ["install", "-t", str(probe)], owned, timeout=120)
            installed_probe = True
            # This journey is offline. A newly installed app otherwise may
            # request nearby-device access after unlock and move focus to the
            # system permission activity. Model the user's existing denial in
            # this disposable guest; grant no network capability to the app.
            declared = checked(device + ["shell", "pm", "list", "permissions", "-d", "-g"], owned)
            denied = offline_permissions(declared, entry["api"])
            for permission in denied:
                checked(device + ["shell", "pm", "revoke", "app.hidlins", permission], owned)
                checked(device + ["shell", "pm", "set-permission-flags", "app.hidlins", permission,
                                  "user-set", "user-fixed"], owned)
            verify_offline_denial(checked(device + ["shell", "dumpsys", "package", "app.hidlins"], owned), denied)
            # Debug installation can outlast the display timeout on a cold
            # software guest. Match the canonical native runner's order.
            checked(device + ["shell", "input", "keyevent", "KEYCODE_WAKEUP"], owned)
            checked(device + ["shell", "wm", "dismiss-keyguard"], owned)
            command = device + ["shell", "am", "instrument", "-w", "-r", "-e", "mode", mode]
            for name, value in arguments.items():
                command += ["-e", name, value]
            raw = checked(command + [RUNNER], owned, timeout=900 if mode == "full" else 300,
                          diagnostic_output=False)
            result = dict(parse_result(raw), runtime=actual, mode=mode, apk_sha256=digest(apk),
                          offline_permissions_denied=denied,
                          owned_sdk_overlays=sum(1 for p in sdk_temporary.rglob("*.qcow2") if p.is_file()))
            # Persist fixed status only; never log a UI tree, clipboard, exception
            # message, instrumentation bundle or user/fixture secret value.
            label = f"api{entry['api']}-{mode}"
            (OUTPUT / (label + ".json")).write_text(json.dumps(result, indent=2) + "\n")
            print("RELEASE_CLIPBOARD", json.dumps(result), flush=True)
            return result
    finally:
        if process is not None:
            for installed, package in ((installed_probe, "app.hidlins.releaseprobe"),
                                       (installed_app, "app.hidlins")):
                if installed:
                    try:
                        checked(device + ["uninstall", package], owned, timeout=30)
                    except (RuntimeError, subprocess.TimeoutExpired):
                        pass
            stop_owned(process)
        try:
            checked(adb + ["kill-server"], owned, timeout=15)
        except (RuntimeError, subprocess.TimeoutExpired):
            pass


def parse_arguments(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("test", "probe", "inspect"))
    parser.add_argument("--variant", choices=("debug", "release"), default="release")
    parser.add_argument("--api", type=int, choices=(29, 36))
    parser.add_argument("--mode", choices=("native", "abi", "full"), default="full")
    parser.add_argument("--apk", type=Path, help="probe-only, explicitly identified control APK")
    parser.add_argument("--mapping", type=Path, help="probe-only, mapping from that same control build")
    options = parser.parse_args(argv)
    if options.action == "test" and (options.variant != "release" or options.api is not None
                                     or options.mode != "full" or options.apk or options.mapping):
        parser.error("test always verifies the canonical shipping release, full journey, both APIs")
    if options.action == "inspect" and (options.variant != "release" or options.apk or options.mapping):
        parser.error("inspect always verifies the canonical shipping release")
    if options.apk and options.variant == "release" and not options.mapping:
        parser.error("a control release APK requires its matching --mapping")
    return options


def main() -> int:
    options = parse_arguments()
    sdk, java, env = tools(dict(os.environ))
    OUTPUT.mkdir(parents=True, exist_ok=True)
    if options.action == "test":
        # Canonical main.dart, release minification and production native
        # prerequisites. No test target, injected source or custom keep rule.
        build_shipping(env)
    variant = options.variant
    apk = options.apk or APK_ROOT / variant / ("app-debug.apk" if variant == "debug" else "app-release-unsigned.apk")
    if variant == "release":
        require_release_payload(apk)
    if options.action == "inspect":
        inspect_apk(apk, sdk, env)
        print("Shipping release JNI callback definition verified", flush=True)
        return 0
    original = digest(apk)
    mapping = options.mapping or MAPPING
    arguments = mapping_arguments(variant, mapping.read_text() if variant == "release" else "")
    matrix = json.loads((ROOT / "tools/android-native/emulator-matrix.json").read_text())
    machine = {"amd64": "x86_64", "aarch64": "arm64"}.get(platform.machine().lower(), platform.machine().lower())
    entries = [entry for entry in matrix["entries"] if entry["host_arch"] == machine
               and entry["api"] in ((options.api,) if options.api else (29, 36))]
    if len(entries) != (1 if options.api else 2):
        raise RuntimeError("the host must support both required Android matrix entries")
    results = []
    with tempfile.TemporaryDirectory(prefix="hidlins-release-clipboard-") as temporary:
        scratch = Path(temporary)
        probe = package_probe(sdk, java, scratch, env)
        signed = scratch / "shipping.apk"
        sign_apk(apk, signed, sdk / "build-tools/36.0.0", scratch, env)
        for entry in entries:
            results.append(run_device(entry, signed, probe, arguments, sdk, scratch, env, options.mode))
    if digest(apk) != original:
        raise RuntimeError("canonical shipping APK changed during runtime verification")
    report = {"variant": variant, "apk": str(apk.resolve()), "apk_sha256": original,
              "control": bool(options.apk), "temporary_root_removed": not scratch.exists(),
              "results": results}
    (OUTPUT / "matrix.json").write_text(json.dumps(report, indent=2) + "\n")
    if options.action == "test" and all(result["passed"] for result in results):
        inspect_apk(apk, sdk, env)
    return 0 if all(result["passed"] for result in results) else 1


if __name__ == "__main__":
    def interrupted(_signum, _frame):
        # Let run_device's finally stop only the owned emulator/server on a
        # pipeline halt; the emulator deliberately has its own process group.
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, interrupted)
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit("clipboard runtime verification cancelled; owned cleanup attempted") from None
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        raise SystemExit(f"clipboard test infrastructure error (not behavioral red): {error}") from error

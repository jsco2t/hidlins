#!/usr/bin/env python3
"""Prove Android NSD across the emulator's genuine shared virtual Wi-Fi LAN."""

from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re
import selectors
import secrets
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from typing import IO

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools" / "local-sync-tests"))
from mobile_scenario import (  # noqa: E402
    flutter_output_passed,
    require_android_lan_topology,
    require_local_endpoint,
)

SDK_ROOT = pathlib.Path(
    os.environ.get("ANDROID_SDK_ROOT")
    or os.environ.get("ANDROID_HOME")
    or pathlib.Path.home() / "Library" / "Android" / "sdk"
)
ADB = SDK_ROOT / "platform-tools" / "adb"
EMULATOR = SDK_ROOT / "emulator" / "emulator"
RESULT = ROOT / "build" / "verification" / "android" / "lan-discovery.json"
APP_APK = ROOT / "app" / "build" / "app" / "outputs" / "flutter-apk" / "app-debug.apk"
TEST_APK = (
    ROOT
    / "app"
    / "build"
    / "app"
    / "outputs"
    / "apk"
    / "androidTest"
    / "debug"
    / "app-debug-androidTest.apk"
)
REMOTE_ROOT = "/data/local/tmp/hidlins-lan-scenario"
PORT = 37371
CLIENT_SERIAL = "emulator-5554"
AUTHORITY_SERIAL = "emulator-5556"
REGISTRAR_COMPONENT = "app.hidlins.test/app.hidlins.ScenarioNsdRegistrarActivity"
REGISTRAR_STOP_ACTION = "app.hidlins.test.STOP_NSD_REGISTRAR"
REGISTRAR_TAG = "HidlinsNsdRegistrar"
REGISTRAR_READY = "HIDLINS_NSD_REGISTRAR_READY"
REGISTRAR_ERROR = "HIDLINS_NSD_REGISTRAR_ERROR"
DISCOVERY_MARKER = re.compile(
    r"HIDLINS_DISCOVERY_CANDIDATE=([^:\s]+):(\d+):(\d+)"
)
CLI_CONNECTION_MARKER = "HIDLINS_DISCOVERY_CLI_NOISE_CONNECTED=true"
CLI_TCP_MARKER = "HIDLINS_DISCOVERY_CLI_TCP_CONNECTED=true"


def run(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, text=True, capture_output=True, check=False, **kwargs)
    if result.returncode != 0:
        detail = (result.stdout + result.stderr)[-4000:]
        raise RuntimeError(f"command failed ({command[0]}): {detail}")
    return result


def adb(serial: str, *args: str, **kwargs: object) -> subprocess.CompletedProcess[str]:
    return run([str(ADB), "-s", serial, *args], **kwargs)


def selected_avd(command: str) -> tuple[str, str, str]:
    output = run(
        [sys.executable, str(ROOT / "tools/android-native/emulator_matrix.py"), command]
    ).stdout.strip()
    rows = [row.split("\t") for row in output.splitlines() if row]
    if command in {"select", "authority"}:
        rows = [row for row in rows if row[1] == "36"]
    if len(rows) != 1 or len(rows[0]) != 5:
        raise RuntimeError("Android emulator matrix did not select one current image")
    return rows[0][0], rows[0][1], rows[0][2]


def emulator_version() -> str:
    output = run([str(EMULATOR), "-version"]).stdout
    match = re.search(r"Android emulator version (\d+\.\d+\.\d+)", output)
    if match is None:
        raise RuntimeError("Android Emulator version could not be determined")
    return match.group(1)


def connected_emulators() -> list[str]:
    output = run([str(ADB), "devices"]).stdout
    return [
        fields[0]
        for line in output.splitlines()[1:]
        if len(fields := line.split()) >= 2
        and fields[0].startswith("emulator-")
        and fields[1] in {"device", "offline"}
    ]


def boot(
    avd: str,
    serial: str,
    port: int,
    log: IO[str],
    extra_args: tuple[str, ...] = (),
) -> subprocess.Popen[str]:
    process = subprocess.Popen(
        [
            str(EMULATOR),
            "-avd",
            avd,
            "-port",
            str(port),
            "-no-window",
            "-no-audio",
            "-no-snapshot",
            "-wipe-data",
            "-no-boot-anim",
            *extra_args,
        ],
        stdout=log,
        stderr=subprocess.STDOUT,
        text=True,
    )
    deadline = time.monotonic() + 240
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"Android emulator exited during boot: {avd}")
        status = subprocess.run(
            [str(ADB), "-s", serial, "shell", "getprop", "sys.boot_completed"],
            text=True,
            capture_output=True,
            check=False,
        )
        if status.returncode == 0 and status.stdout.strip() == "1":
            return process
        time.sleep(1)
    raise RuntimeError(f"Android emulator did not boot: {avd}")


def wlan_address(serial: str) -> tuple[str, int]:
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        output = adb(
            serial, "shell", "ip", "-4", "-o", "addr", "show", "wlan0"
        ).stdout
        match = re.search(r"\binet (\d+\.\d+\.\d+\.\d+)/(\d+)\b", output)
        if match is not None:
            return match.group(1), int(match.group(2))
        time.sleep(1)
    raise RuntimeError(f"{serial} did not obtain a wlan0 DHCP address")


def sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def build_authority() -> pathlib.Path:
    output = run([str(ROOT / "tools/android-native/build.sh"), "scenario-build"]).stdout
    path = pathlib.Path(output.strip().splitlines()[-1])
    if not path.is_file():
        raise RuntimeError("Android scenario authority binary was not produced")
    return path


def build_test_apk() -> pathlib.Path:
    run(
        [
            str(ROOT / "tools/android-native/gradle.sh"),
            "-Ptarget-platform=android-arm64,android-x64",
            ":app:assembleDebug",
            ":app:assembleDebugAndroidTest",
        ],
        timeout=300,
    )
    if not TEST_APK.is_file():
        raise RuntimeError("Android instrumentation APK was not produced")
    return TEST_APK


def registrar_start_arguments(*, port: int, kind: str) -> list[str]:
    """Build authority-side registrar arguments without accepting an address."""

    if port < 1 or port > 65_535:
        raise ValueError("registrar port is outside the valid range")
    if kind not in {"pairing", "trusted"}:
        raise ValueError("registrar service kind is invalid")
    return [
        "shell",
        "am",
        "start",
        "-W",
        "-n",
        REGISTRAR_COMPONENT,
        "--ei",
        "port",
        str(port),
        "--es",
        "kind",
        kind,
    ]


def install_registrar(serial: str, test_apk: pathlib.Path) -> None:
    adb(serial, "install", "-r", "-g", str(test_apk))
    declared = adb(serial, "shell", "pm", "list", "permissions").stdout
    for permission in (
        "android.permission.NEARBY_WIFI_DEVICES",
        "android.permission.ACCESS_LOCAL_NETWORK",
    ):
        if permission in declared:
            adb(
                serial,
                "shell",
                "pm",
                "grant",
                "--user",
                "0",
                "app.hidlins.test",
                permission,
            )


def start_registrar(serial: str, *, port: int, kind: str) -> str:
    adb(serial, "logcat", "-c")
    adb(serial, *registrar_start_arguments(port=port, kind=kind))
    expected = f"{REGISTRAR_READY}={kind}:{port}"
    deadline = time.monotonic() + 30
    output = ""
    while time.monotonic() < deadline:
        output = adb(serial, "logcat", "-d", "-s", f"{REGISTRAR_TAG}:I", "*:S").stdout
        if expected in output:
            return output
        if REGISTRAR_ERROR in output:
            raise RuntimeError("Android test registrar reported registration failure")
        time.sleep(0.25)
    raise RuntimeError("Android test registrar did not report readiness")


def stop_registrar(serial: str) -> None:
    adb(
        serial,
        "shell",
        "am",
        "start",
        "-W",
        "-a",
        REGISTRAR_STOP_ACTION,
        "-n",
        REGISTRAR_COMPONENT,
        timeout=30,
    )
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        output = adb(serial, "logcat", "-d", "-s", f"{REGISTRAR_TAG}:I", "*:S").stdout
        if "HIDLINS_NSD_REGISTRAR_STOPPED" in output:
            return
        time.sleep(0.1)
    raise RuntimeError("Android test registrar did not confirm unregistration")


def create_authority(serial: str, binary: pathlib.Path, password: str) -> None:
    adb(serial, "install", "-r", "-g", str(APP_APK))
    adb(serial, "shell", "mkdir", "-p", REMOTE_ROOT)
    adb(serial, "push", str(binary), f"{REMOTE_ROOT}/hidlins")
    adb(serial, "shell", "chmod", "700", f"{REMOTE_ROOT}/hidlins")
    result = subprocess.run(
        [
            str(ADB), "-s", serial, "shell", f"{REMOTE_ROOT}/hidlins",
            "--registry", f"{REMOTE_ROOT}/vaults.toml", "--format", "json",
            "vault", "create", "--id", "authority", "--path",
            f"{REMOTE_ROOT}/authority.kdbx", "--no-recovery-warning",
        ],
        input=f"{password}\n{password}\n",
        text=True,
        capture_output=True,
        check=False,
        timeout=120,
    )
    if result.returncode != 0:
        raise RuntimeError("authority vault creation failed")


def authority_command(serial: str, address: str) -> list[str]:
    """Run the real CLI independently from the client-only application UID."""

    return [
        str(ADB), "-s", serial, "shell", f"{REMOTE_ROOT}/hidlins",
        "--registry", f"{REMOTE_ROOT}/vaults.toml", "--format", "json",
        "sync", "serve", "--vault", "authority", "--address", address,
        "--port", str(PORT), "--pairing-window",
    ]


def start_authority(
    serial: str, address: str, password: str
) -> subprocess.Popen[str]:
    process = subprocess.Popen(
        authority_command(serial, address),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1,
    )
    assert process.stdin is not None and process.stdout is not None
    # The smoke client rejects the SAS after the XX handshake. Preloading the
    # matching authority rejection proves the real CLI accepted the discovered
    # connection without creating trust or transferring a vault.
    process.stdin.write(f"{password}\nno\n")
    process.stdin.flush()
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    ready = selector.select(timeout=60)
    selector.close()
    if not ready:
        process.terminate()
        raise RuntimeError("authority CLI did not report readiness")
    payload = json.loads(process.stdout.readline())
    if payload.get("status") != "serving" or payload.get("endpoint") != f"{address}:{PORT}":
        process.terminate()
        raise RuntimeError("authority CLI reported an unexpected listener")
    return process


def run_discovery(serial: str) -> tuple[str, int, int, str]:
    if not APP_APK.is_file():
        raise RuntimeError("shipping Android debug APK is missing")
    with tempfile.TemporaryDirectory(prefix="hidlins-android-client.") as scratch:
        shipping_apk = pathlib.Path(scratch) / "shipping.apk"
        scenario_apk = pathlib.Path(scratch) / "scenario.apk"
        shutil.copy2(APP_APK, shipping_apk)
        build = run(
            [
                "flutter", "build", "apk", "--debug", "--no-pub",
            "--target=integration_test/mobile_local_sync_scenario_test.dart",
            "--dart-define=HIDLINS_SCENARIO_PHASE=discovery",
            "--android-project-arg=target-platform=android-arm64,android-x64",
            ],
            cwd=ROOT / "app",
            timeout=300,
        )
        print(build.stdout)
        shutil.copy2(APP_APK, scenario_apk)
        shutil.copy2(shipping_apk, APP_APK)
        # `-g` is Android's instrumentation-grade equivalent of accepting all
        # runtime permission prompts at install time. It changes permission
        # state only; it supplies no route or discovery result.
        adb(serial, "install", "-r", "-g", str(scenario_apk))

    declared = adb(serial, "shell", "pm", "list", "permissions").stdout
    permissions = (
        "android.permission.NEARBY_WIFI_DEVICES",
        "android.permission.ACCESS_LOCAL_NETWORK",
    )
    for permission in permissions:
        if permission in declared:
            adb(
                serial,
                "shell",
                "pm",
                "grant",
                "--user",
                "0",
                "app.hidlins",
                permission,
            )
            checked = adb(
                serial,
                "shell",
                "pm",
                "check-permission",
                "--user",
                "0",
                permission,
                "app.hidlins",
            ).stdout.strip()
            if checked != "granted":
                raise RuntimeError(f"Android did not grant required permission {permission}")

    adb(serial, "logcat", "-c")
    adb(serial, "shell", "am", "start", "-W", "-n", "app.hidlins/.MainActivity")
    for permission in permissions:
        if permission in declared:
            adb(
                serial,
                "shell",
                "pm",
                "grant",
                "--user",
                "0",
                "app.hidlins",
                permission,
            )
    deadline = time.monotonic() + 180
    output = ""
    while time.monotonic() < deadline:
        output = adb(serial, "logcat", "-d", "-s", "flutter:I").stdout
        if flutter_output_passed(output) or "Some tests failed" in output or "[E]" in output:
            break
        time.sleep(0.25)
    print(output)
    if not flutter_output_passed(output):
        raise RuntimeError("shipping Android client discovery scenario failed")
    matches = DISCOVERY_MARKER.findall(output)
    if not matches:
        raise RuntimeError("shipping Android client did not report an NSD candidate")
    if CLI_TCP_MARKER not in output:
        raise RuntimeError("shipping Android client did not reach the discovered CLI socket")
    if CLI_CONNECTION_MARKER not in output:
        raise RuntimeError("shipping Android client did not connect to the discovered CLI")
    unique = {(address, int(port), int(scope)) for address, port, scope in matches}
    if len(unique) != 1:
        raise RuntimeError("Android NSD resolved an ambiguous candidate set")
    address, port, scope = unique.pop()
    return address, port, scope, output


def stop_emulator(serial: str) -> None:
    subprocess.run(
        [str(ADB), "-s", serial, "emu", "kill"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )


def main() -> None:
    if os.environ.get("HIDLINS_ANDROID_STRICT") != "1":
        raise SystemExit("HIDLINS_ANDROID_STRICT=1 is required; discovery has no fallback")
    for tool in (ADB, EMULATOR):
        if not tool.is_file():
            raise SystemExit(f"required Android tool is unavailable: {tool}")
    if connected_emulators():
        raise SystemExit("Android LAN discovery requires no pre-connected emulators")

    client_avd, api, abi = selected_avd("select")
    authority_avd, authority_api, authority_abi = selected_avd("authority")
    if (api, abi) != (authority_api, authority_abi) or client_avd == authority_avd:
        raise RuntimeError("client and authority AVDs do not share one distinct pinned image")

    binary = build_authority()
    test_apk = build_test_apk()
    version = emulator_version()
    password = secrets.token_urlsafe(32)
    processes: list[subprocess.Popen[str]] = []
    authority: subprocess.Popen[str] | None = None
    registrar_started = False
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="hidlins-android-lan."))
    try:
        with (scratch / "client-emulator.log").open("w", encoding="utf-8") as client_log, (
            scratch / "authority-emulator.log"
        ).open("w", encoding="utf-8") as authority_log:
            processes.append(boot(client_avd, CLIENT_SERIAL, 5554, client_log))
            processes.append(boot(authority_avd, AUTHORITY_SERIAL, 5556, authority_log))
            client_address, prefix = wlan_address(CLIENT_SERIAL)
            authority_address, authority_prefix = wlan_address(AUTHORITY_SERIAL)
            if prefix != authority_prefix:
                raise RuntimeError("Android wlan0 prefix lengths differ")
            topology = require_android_lan_topology(
                emulator_version=version,
                client_serial=CLIENT_SERIAL,
                authority_serial=AUTHORITY_SERIAL,
                client_address=client_address,
                authority_address=authority_address,
                prefix_length=prefix,
            )
            create_authority(AUTHORITY_SERIAL, binary, password)
            install_registrar(AUTHORITY_SERIAL, test_apk)
            authority = start_authority(AUTHORITY_SERIAL, authority_address, password)
            registrar_started = True
            start_registrar(AUTHORITY_SERIAL, port=PORT, kind="pairing")
            discovered_address, discovered_port, scope, _ = run_discovery(CLIENT_SERIAL)
            require_local_endpoint(discovered_address, discovered_port)
            if (discovered_address, discovered_port, scope) != (
                authority_address, PORT, 0
            ):
                raise RuntimeError("NSD result did not match the authority wlan0 listener")

            RESULT.parent.mkdir(parents=True, exist_ok=True)
            RESULT.write_text(
                json.dumps(
                    {
                        "schema_version": 1,
                        "emulator_version": version,
                        "image": {"api": int(api), "abi": abi},
                        "client": {"avd": client_avd, "serial": CLIENT_SERIAL, "wlan0": client_address},
                        "authority": {
                            "avd": authority_avd,
                            "serial": AUTHORITY_SERIAL,
                            "wlan0": authority_address,
                            "listener_port": PORT,
                            "binary_sha256": sha256(binary),
                            "execution_model": "standalone-guest-cli",
                            "mobile_server_enabled": False,
                        },
                        "network": str(topology.network),
                        "native_discovery": {
                            "address": discovered_address,
                            "port": discovered_port,
                            "scope_id": scope,
                            "rust_policy_validated": True,
                            "noise_connection_to_cli": True,
                            "registrar": "androidTest-NsdManager",
                            "registrar_received_address": False,
                            "registrar_test_only": True,
                        },
                        "client_endpoint_input": False,
                        "sync_proxy": False,
                        "sync_route_injected": False,
                    },
                    indent=2,
                    sort_keys=True,
                )
                + "\n",
                encoding="utf-8",
            )
            print(f"  OK: Android NSD resolved {authority_address}:{PORT} across {topology.network}")
            print(f"  evidence: {RESULT}")
    finally:
        password = ""
        if registrar_started:
            try:
                stop_registrar(AUTHORITY_SERIAL)
            except RuntimeError:
                pass
        if authority is not None:
            subprocess.run(
                [str(ADB), "-s", AUTHORITY_SERIAL, "shell",
                 "pkill", "-INT", "-x", "hidlins"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
            try:
                authority.wait(timeout=10)
            except subprocess.TimeoutExpired:
                authority.terminate()
        for serial in (CLIENT_SERIAL, AUTHORITY_SERIAL):
            subprocess.run(
                [str(ADB), "-s", serial, "uninstall", "app.hidlins.test"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
            subprocess.run(
                [str(ADB), "-s", serial, "uninstall", "app.hidlins"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
            subprocess.run(
                [str(ADB), "-s", serial, "shell", "rm", "-rf", REMOTE_ROOT],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
            stop_emulator(serial)
        for process in processes:
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.terminate()
        shutil.rmtree(scratch)


if __name__ == "__main__":
    main()

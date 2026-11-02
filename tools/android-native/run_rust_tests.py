#!/usr/bin/env python3
"""Run the atomic recovery regressions on the host and real API29/API36 Bionic."""

from __future__ import annotations

import json
import os
from pathlib import Path
import platform
import shlex
import signal
import socket
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parents[2]
TEST = "atomic_cleanup_regressions"


def build_test(target: str | None, environment: dict[str, str]) -> Path:
    flags = f"--offline --locked -p hidlins-core --test {TEST} --no-run --message-format=json"
    if target:
        flags += f" --target {target}"
    result = subprocess.run(
        ["make", "test", f"CARGO_FLAGS={flags}"], cwd=ROOT, env=environment,
        text=True, stdout=subprocess.PIPE, timeout=1200,
    )
    executable = None
    for line in result.stdout.splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (item.get("reason") == "compiler-artifact"
                and item.get("target", {}).get("name") == TEST
                and item.get("executable")):
            executable = Path(item["executable"])
    if result.returncode or executable is None or not executable.is_file():
        raise RuntimeError(f"could not build {TEST} for {target or 'host'}")
    return executable


def checked(args: list[str], environment: dict[str, str], timeout: int = 30) -> str:
    result = subprocess.run(
        args, env=environment, text=True, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, timeout=timeout,
    )
    if result.returncode:
        raise RuntimeError(f"command failed ({result.returncode}): {shlex.join(args)}\n{result.stderr}")
    return result.stdout.strip()


def stop_process(process: subprocess.Popen) -> None:
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)


def run_device(
    entry: dict, executable: Path, sdk: Path, avd_home: Path,
    environment: dict[str, str], results: Path, scratch: Path,
) -> int:
    adb = str(sdk / "platform-tools/adb")
    emulator = str(sdk / "emulator/emulator")
    serial = "emulator-5582"
    # Own a separate adb server and read-only AVD instance; never stop or wipe
    # a developer's existing device/server/data image.
    env = dict(environment, ANDROID_AVD_HOME=str(avd_home),
               ANDROID_USER_HOME=str(scratch / "android-user"),
               ANDROID_ADB_SERVER_PORT="5038", ADB_SERVER_SOCKET="tcp:127.0.0.1:5038")
    (scratch / "android-user").mkdir(exist_ok=True)
    for port in [5038, 5582, 5583]:
        deadline = time.monotonic() + 15
        while True:
            try:
                with socket.socket() as probe:
                    probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
                    probe.bind(("127.0.0.1", port))
                break
            except OSError:
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.1)
    checked([adb, "-P", "5038", "start-server"], env)
    existing = checked([adb, "-P", "5038", "devices"], env).splitlines()[1:]
    if any(line.strip() for line in existing):
        raise RuntimeError("atomic matrix requires its dedicated adb server to have no attached devices")
    acceleration = "off" if platform.system() == "Linux" and not os.access(
        "/dev/kvm", os.R_OK | os.W_OK
    ) else "auto"
    boot_log = results / f"api{entry['api']}-boot.log"
    remote = f"/data/local/tmp/hidlins-atomic-{scratch.name}"
    process = None
    try:
        with boot_log.open("w") as output:
            process = subprocess.Popen(
                [emulator, "-avd", entry["avd"], "-read-only", "-no-window",
                 "-no-audio", "-no-snapshot", "-no-boot-anim", "-accel", acceleration,
                 "-gpu", "swiftshader", "-cores", "2", "-memory", "1536", "-port", "5582"],
                env=env, stdout=output, stderr=subprocess.STDOUT, start_new_session=True,
            )
            deadline = time.monotonic() + 600
            next_progress = time.monotonic()
            while True:
                if process.poll() is not None:
                    raise RuntimeError(f"{entry['avd']} exited during boot; inspect {boot_log}")
                probe = subprocess.run(
                    [adb, "-P", "5038", "-s", serial, "shell", "getprop", "sys.boot_completed"],
                    env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=15,
                )
                if probe.returncode == 0 and probe.stdout.strip() == "1":
                    break
                if time.monotonic() > deadline:
                    raise RuntimeError(f"{entry['avd']} did not boot; inspect {boot_log}")
                if time.monotonic() >= next_progress:
                    print(f"Waiting for {entry['avd']} boot ({acceleration} acceleration)", flush=True)
                    next_progress = time.monotonic() + 20
                time.sleep(2)
            prefix = [adb, "-P", "5038", "-s", serial]
            api = checked(prefix + ["shell", "getprop", "ro.build.version.sdk"], env)
            abi = checked(prefix + ["shell", "getprop", "ro.product.cpu.abi"], env)
            avd = checked(prefix + ["emu", "avd", "name"], env).splitlines()[0]
            if api != str(entry["api"]) or abi != entry["abi"] or avd != entry["avd"]:
                raise RuntimeError(f"runtime identity mismatch: {api}/{abi}/{avd}")
            print(f"ANDROID_RUNTIME api={api} abi={abi} avd={avd}", flush=True)
            checked(prefix + ["shell", "mkdir", "-m", "700", remote], env)
            checked(prefix + ["push", str(executable), remote + "/regressions"], env, timeout=120)
            checked(prefix + ["shell", "chmod", "700", remote + "/regressions"], env)
            command = (
                f"TMPDIR={shlex.quote(remote)} "
                f"{shlex.quote(remote + '/regressions')} --test-threads=1"
            )
            test = subprocess.run(
                prefix + ["shell", command], env=env, text=True,
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=900,
            )
            (results / f"api{api}-tests.log").write_text(test.stdout)
            print(test.stdout, end="", flush=True)
            # A build/loader/missing-tool failure cannot count as test-forward
            # evidence. Rust's harness must actually report its test outcome.
            if "test result:" not in test.stdout:
                raise RuntimeError(f"API{api} test harness did not execute")
            return test.returncode
    finally:
        if process is not None:
            try:
                subprocess.run(
                    [adb, "-P", "5038", "-s", serial, "shell", "rm", "-rf", remote],
                    env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15,
                )
            except subprocess.TimeoutExpired:
                pass
            stop_process(process)
        subprocess.run([adb, "-P", "5038", "kill-server"], env=env,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15)


def main() -> int:
    env = dict(os.environ)
    cached_sdk = ROOT / ".codex-tmp/pr4-android/sdk"
    sdk = Path(env.get("ANDROID_SDK_ROOT") or env.get("ANDROID_HOME") or cached_sdk).resolve()
    for tool in [sdk / "platform-tools/adb", sdk / "emulator/emulator"]:
        if not tool.is_file():
            raise RuntimeError(f"missing cached Android tool: {tool}; set ANDROID_SDK_ROOT")
    machine = platform.machine().lower()
    architecture = {"amd64": "x86_64", "aarch64": "arm64"}.get(machine, machine)
    targets = {"x86_64": ("x86_64-linux-android", "linux-x86_64"),
               "arm64": ("aarch64-linux-android", "darwin-x86_64" if platform.system() == "Darwin" else "linux-x86_64")}
    if architecture not in targets:
        raise RuntimeError(f"unsupported Android matrix host: {machine}")
    target, host_tag = targets[architecture]
    if platform.system() == "Darwin":
        host_tag = "darwin-x86_64"
    matrix = json.loads((ROOT / "tools/android-native/emulator-matrix.json").read_text())
    entries = [entry for entry in matrix["entries"]
               if entry["host_arch"] == architecture
               and entry["api"] in (matrix["minimum_api"], matrix["current_api"])]
    if len(entries) != 2:
        raise RuntimeError("matrix must name both minimum and current Android API")
    configured = Path(env.get("ANDROID_AVD_HOME") or Path.home() / ".android/avd")
    cached = ROOT / ".codex-tmp/pr4-android/acceptance-avds"
    avd_home = next((p for p in [configured, cached]
                     if all((p / (entry["avd"] + ".ini")).is_file() for entry in entries)), None)
    if avd_home is None:
        raise RuntimeError("missing API29/API36 AVDs; set ANDROID_AVD_HOME or provision the cached matrix")
    contract = json.loads((ROOT / "tools/android-native/artifact-contract.json").read_text())
    compiler = sdk / "ndk" / contract["ndk_version"] / "toolchains/llvm/prebuilt" / host_tag / "bin" / f"{target}29-clang"
    if not compiler.is_file():
        raise RuntimeError(f"missing pinned NDK compiler: {compiler}")
    env["ANDROID_SDK_ROOT"] = str(sdk)
    android_env = dict(env)
    android_env[f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"] = str(compiler)
    results = ROOT / "build/verification/atomic-recovery"
    results.mkdir(parents=True, exist_ok=True)
    host = build_test(None, env)
    android = build_test(target, android_env)
    host_test = subprocess.run([str(host), "--test-threads=1"], cwd=ROOT,
                               env=env, text=True, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, timeout=900)
    (results / "host-tests.log").write_text(host_test.stdout)
    print(host_test.stdout, end="", flush=True)
    if "test result:" not in host_test.stdout:
        raise RuntimeError("host test harness did not execute")
    statuses = {"host": host_test.returncode}
    with tempfile.TemporaryDirectory(prefix="hidlins-atomic-native-") as temporary:
        for entry in entries:
            statuses[f"api{entry['api']}"] = run_device(
                entry, android, sdk, avd_home, android_env, results, Path(temporary),
            )
    (results / "matrix.json").write_text(json.dumps(statuses, indent=2) + "\n")
    print("ATOMIC_MATRIX_RESULTS", json.dumps(statuses), flush=True)
    return 1 if any(statuses.values()) else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        raise SystemExit(f"atomic matrix infrastructure error (not behavioral red): {error}") from error

#!/usr/bin/env python3
"""Exercise the shipping offline journey with fresh, undecided permissions."""
from __future__ import annotations

import contextlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys
import tempfile
import threading

import release_jni
from emulator_matrix import NATIVE_APIS, client_entries

ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / '.codex-tmp/android-startup-discovery'


class FreshPermissionPolicy:
    def __init__(self, sdk_permissions):
        self.sdk_permissions = sdk_permissions
        self.permissions: list[str] = []
        self.api: int | None = None
        self.verified = False

    def declared(self, output: str, api: int) -> list[str]:
        # Keep the existing SDK enumeration admission checks. This fixture
        # models a new install, so it never pre-sets a grant or user denial.
        self.permissions = self.sdk_permissions(output, api)
        self.api = api
        return []

    def verify(self, output: str) -> None:
        if self.api is None:
            raise RuntimeError('SDK permissions were not enumerated before package inspection')
        for permission in self.permissions:
            found = re.search(r'(?m)^\s*' + re.escape(permission)
                              + r': granted=(true|false), flags=\[([^\]]*)\]', output)
            flags = set(re.findall(r'[A-Z_]+', found[2])) if found else set()
            if not found or found[1] != 'false' or flags & {'USER_SET', 'USER_FIXED'}:
                raise RuntimeError('fresh runtime permission state was not established')
        self.verified = True


@contextlib.contextmanager
def fresh_permissions(driver):
    original = driver.offline_permissions
    policy = FreshPermissionPolicy(original)
    driver.offline_permissions = policy.declared
    try:
        yield policy
    finally:
        driver.offline_permissions = original


def diagnostics(device: list[str], env: dict[str, str]) -> dict[str, str]:
    # Reuse the existing secret-safe classifier. Raw window titles, trees and
    # exception messages never leave the classifier or enter an evidence file.
    script = '''source "$1"
shift
startup_adb=("$@")
startup_diagnostic_adb() { "${startup_adb[@]}" "$@"; }
hidlins_device_ui_diagnostics startup_diagnostic_adb
'''
    result = subprocess.run(
        [sys.executable, str(ROOT / 'tools/android-native/run_with_timeout.py'),
         '15', 'bash', '-c', script, 'startup-diagnostics',
         str(ROOT / 'tools/android-native/instrumentation-result.sh'), *device],
        cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL, timeout=20, check=False)
    allowed = {'focus-window', 'wakefulness', 'error-owner', 'error-kind'}
    return {key: value for line in result.stdout.splitlines()
            if '=' in line for key, value in [line.split('=', 1)] if key in allowed}


@contextlib.contextmanager
def runtime_adapter(driver, policy: FreshPermissionPolicy, facts: dict):
    original = driver.checked
    prepared = False

    def checked(args, env, timeout=60, *, diagnostic_output=True):
        nonlocal prepared
        adb = Path(args[0]).name == 'adb'
        if adb and 'install' in args and not prepared:
            device = args[:args.index('install')]
            if '5039' not in device or 'emulator-5584' not in device:
                raise RuntimeError('fresh setup requires the verified owned emulator')
            # run_device verifies API/ABI/AVD identity before this first install.
            # Reset only our packages inside its disposable read-only overlay.
            for package in ('app.hidlins.releaseprobe', 'app.hidlins.test', 'app.hidlins'):
                try:
                    original(device + ['uninstall', package], env, timeout=30,
                             diagnostic_output=False)
                except RuntimeError:
                    pass
            prepared = True
        instrument = adb and any(args[i:i + 3] == ['shell', 'am', 'instrument']
                                 for i in range(len(args) - 2))
        stop = threading.Event()
        watcher = None
        if instrument:
            device = args[:args.index('shell')]

            def observe():
                while not stop.is_set():
                    try:
                        state = diagnostics(device, env)
                        if 'focus-window' in state:
                            facts['diagnostic_samples'] += 1
                            if state['focus-window'] == 'permission-dialog':
                                facts['permission_dialog_observed'] = True
                    except (OSError, subprocess.TimeoutExpired):
                        pass
                    stop.wait(1)

            watcher = threading.Thread(target=observe, daemon=True,
                                       name='hidlins-startup-permission-observer')
            watcher.start()
        try:
            output = original(args, env, timeout=timeout, diagnostic_output=diagnostic_output)
            if adb and args[-3:] == ['dumpsys', 'package', 'app.hidlins']:
                policy.verify(output)
            return output
        finally:
            stop.set()
            if watcher is not None:
                watcher.join(timeout=25)
                if watcher.is_alive():
                    raise RuntimeError('permission observer did not settle')

    driver.checked = checked
    try:
        yield
    finally:
        driver.checked = original


def main() -> int:
    sdk, java, env = release_jni.tools(dict(os.environ))
    OUTPUT.mkdir(parents=True, exist_ok=True)
    release_jni.OUTPUT = OUTPUT
    # The default release/JNI fixture remains untouched. Its declared SDK
    # checks and complete shipping journey run here with a different initial
    # permission state, on both required emulator APIs.
    release_jni.build_shipping(env)
    apk = release_jni.APK_ROOT / 'release/app-release-unsigned.apk'
    release_jni.require_release_payload(apk)
    original_hash = release_jni.digest(apk)
    arguments = release_jni.mapping_arguments('release', release_jni.MAPPING.read_text())
    results = []
    with tempfile.TemporaryDirectory(prefix='hidlins-startup-', dir=ROOT / '.codex-tmp') as temporary:
        scratch = Path(temporary)
        probe = release_jni.package_probe(sdk, java, scratch, env)
        signed = scratch / 'shipping.apk'
        release_jni.sign_apk(apk, signed, sdk / 'build-tools/36.0.0', scratch, env)
        for entry in client_entries(platform.machine(), NATIVE_APIS):
            facts = {'diagnostic_samples': 0, 'permission_dialog_observed': False}
            with fresh_permissions(release_jni) as policy:
                with runtime_adapter(release_jni, policy, facts):
                    result = release_jni.run_device(entry, signed, probe, arguments,
                                                   sdk, scratch, env, 'full')
                if not policy.verified or facts['diagnostic_samples'] == 0:
                    raise RuntimeError('fresh permission observations were not established')
            result.update(facts, fresh_permissions_verified=True)
            if result['passed'] and facts['permission_dialog_observed']:
                result.update(passed=False, stage='startup-discovery',
                              code='UNEXPECTED_PERMISSION_DIALOG')
            results.append(result)
            (OUTPUT / f"api{entry['api']}-startup.json").write_text(json.dumps(result, indent=2) + '\n')
            print('STARTUP_DISCOVERY', json.dumps(result), flush=True)
    if release_jni.digest(apk) != original_hash:
        raise RuntimeError('canonical shipping APK changed during runtime verification')
    report = {'canonical_apk_sha256': original_hash, 'temporary_root_removed': not scratch.exists(),
              'results': results}
    (OUTPUT / 'matrix.json').write_text(json.dumps(report, indent=2) + '\n')
    return 0 if all(result['passed'] for result in results) else 1


if __name__ == '__main__':
    def interrupted(_signum, _frame):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, interrupted)
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit('startup verification cancelled; owned cleanup attempted') from None
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        raise SystemExit(f'startup verification infrastructure error (not behavioral red): {error}') from error

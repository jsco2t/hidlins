#!/usr/bin/env python3
"""Exercise the actual device runner with owned executable SDK fixtures."""

import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

FAKE_TOOL = r'''
import json, os, signal, sys, time
from pathlib import Path
config = json.loads(Path(os.environ['RUNNER_FIXTURE_CONFIG']).read_text())
kind = Path(sys.argv[0]).name
args = sys.argv[1:]
event = dict(kind=kind, args=args, pid=os.getpid(), serial=os.environ.get('ANDROID_SERIAL'))
with open(os.environ['RUNNER_FIXTURE_EVENTS'], 'ab', buffering=0) as out:
    out.write((json.dumps(event) + '\n').encode())
pid_file = Path(os.environ['RUNNER_FIXTURE_EMULATOR_PID'])
mode = config['mode']
if kind == 'emulator':
    pid_file.write_text(str(os.getpid()))
    if mode == 'early-exit':
        print('FATAL: fixture emulator exited before attachment', flush=True)
        sys.exit(7)
    # Finite even if a defective runner fails to clean up.
    time.sleep(30)
    sys.exit(0)
if args[:1] == ['-s']:
    args = args[2:]
if mode == 'adb-stall':
    time.sleep(2)
    sys.exit(1)
attached = config['existing'] or (pid_file.exists() and mode not in ('early-exit', 'unattached'))
if args == ['get-state']:
    if attached:
        print('device')
        sys.exit(0)
    sys.exit(1)
if args == ['wait-for-device']:
    if not attached:
        time.sleep(30)
    sys.exit(0)
if args[:1] == ['shell'] and not attached:
    sys.exit(1)
if args[:2] == ['shell', 'getprop']:
    values = {'sys.boot_completed': '0' if mode == 'boot-stall' else '1',
              'ro.build.version.sdk': config.get('api', '29'),
              'ro.product.cpu.abi': config.get('abi', 'x86_64')}
    print(values.get(args[2], ''))
elif args[:3] == ['shell', 'am', 'instrument']:
    count = config['case_count']
    if mode == 'instrumentation-failure':
        print(f'HIDLINS_DEVICE_TESTS {count - 1} passed, 1 failed\nFAIL fixture\nINSTRUMENTATION_CODE: 0')
    else:
        print(f'HIDLINS_DEVICE_TESTS {count} passed, 0 failed\nINSTRUMENTATION_CODE: -1')
elif args[:3] == ['shell', 'dumpsys', '-t']:
    if args[-1] == 'window':
        print('mCurrentFocus=app.hidlins/fixture-private-window-marker')
    else:
        print('mWakefulness=Awake')
else:
    print('Success')
'''


class RunnerStartupTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='hidlins-runner-startup-')
        self.addCleanup(temporary.cleanup)
        self.temp = Path(temporary.name).resolve()
        self.repo = self.temp / 'checkout'
        tools = self.repo / 'tools/android-native'
        tools.mkdir(parents=True)
        for name in ('run_app_tests.sh', 'run_with_timeout.py', 'instrumentation-result.sh'):
            shutil.copy2(ROOT / 'tools/android-native' / name, tools / name)
        gradle = tools / 'gradle.sh'
        gradle.write_text('#!/usr/bin/env bash\nexit 0\n')
        gradle.chmod(0o755)
        self.sdk = self.temp / 'SDK with spaces'
        for relative in ('platform-tools/adb', 'emulator/emulator'):
            path = self.sdk / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('#!' + sys.executable + '\n' + FAKE_TOOL)
            path.chmod(0o755)
        for relative in ('debug/app-debug.apk', 'androidTest/debug/app-debug-androidTest.apk'):
            path = self.repo / 'app/build/app/outputs/apk' / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'fixture APK; fake adb never executes it')
        # Fixture output follows the declared native case inventory. Other
        # policy tests independently preserve the original and added cases.
        cases = (ROOT / 'app/android/app/src/androidTest/java/app/hidlins/HidlinsAndroidTest.java').read_text()
        self.config = dict(mode='success', existing=True,
                           case_count=len(re.findall(r'runCase\("', cases)))
        self.config_file = self.temp / 'config.json'
        self.events_file = self.temp / 'events.jsonl'
        self.pid_file = self.temp / 'emulator.pid'
        self.env = dict(os.environ, ANDROID_SDK_ROOT=str(self.sdk), ANDROID_HOME=str(self.sdk),
                        ANDROID_SERIAL='fixture-connected', ANDROID_AVD_HOME=str(self.temp / 'avds'),
                        HIDLINS_ANDROID_AVD='fixture-avd', HIDLINS_ANDROID_EXPECTED_API='29',
                        HIDLINS_ANDROID_EXPECTED_ABI='x86_64', HIDLINS_ANDROID_BOOT_TIMEOUT_SECONDS='1',
                        HIDLINS_ANDROID_ADB_TIMEOUT_SECONDS='0.2', HIDLINS_ANDROID_EMULATOR_PORT='5580',
                        RUNNER_FIXTURE_CONFIG=str(self.config_file),
                        RUNNER_FIXTURE_EVENTS=str(self.events_file),
                        RUNNER_FIXTURE_EMULATOR_PID=str(self.pid_file), TMPDIR=str(self.temp))

    @staticmethod
    def alive(pid):
        status = subprocess.run(['ps', '-p', str(pid), '-o', 'stat='],
                                capture_output=True, text=True, timeout=2)
        return status.returncode == 0 and bool(status.stdout.strip()) and not status.stdout.lstrip().startswith('Z')

    def run_runner(self, mode='success', existing=True):
        self.config.update(mode=mode, existing=existing)
        self.config_file.write_text(json.dumps(self.config))
        if not existing:
            self.env.pop('ANDROID_SERIAL', None)
        process = subprocess.Popen(['bash', 'tools/android-native/run_app_tests.sh'],
                                   cwd=self.repo, env=self.env, text=True,
                                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        timed_out = False
        child_alive_at_exit = False
        try:
            try:
                output, _ = process.communicate(timeout=8)
            except subprocess.TimeoutExpired:
                timed_out = True
                os.killpg(process.pid, signal.SIGKILL)
                output, _ = process.communicate(timeout=3)
            if self.pid_file.exists():
                child_alive_at_exit = self.alive(int(self.pid_file.read_text()))
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=3)
            if self.pid_file.exists():
                pid = int(self.pid_file.read_text())
                if self.alive(pid):
                    os.kill(pid, signal.SIGKILL)
        events = [json.loads(line) for line in self.events_file.read_text().splitlines()] if self.events_file.exists() else []
        self.assertFalse(timed_out, 'Actual runner exceeded the configured startup bound; it is waiting on a dead/unavailable device\n' + output)
        return process.returncode, output, events, child_alive_at_exit

    def assert_startup_failure(self, mode):
        code, output, events, alive = self.run_runner(mode, existing=False)
        self.assertNotEqual(code, 0, output)
        self.assertFalse(alive, 'Runner left its spawned emulator running')
        commands = [event['args'] for event in events if event['kind'] == 'adb']
        self.assertFalse(any('install' in args or 'uninstall' in args for args in commands),
                         'Startup failure must not install or uninstall applications')
        self.assertTrue(any(event['kind'] == 'emulator' for event in events))
        return output, events

    def test_emulator_early_exit_fails_promptly_with_fresh_boot_diagnostic(self):
        output, _ = self.assert_startup_failure('early-exit')
        self.assertIn('exited', output.lower())
        self.assertIn('fixture emulator exited before attachment', output)

    def test_unattached_emulator_is_bounded_and_owned_child_is_stopped(self):
        _, events = self.assert_startup_failure('unattached')
        emulator = next(event for event in events if event['kind'] == 'emulator')
        self.assertIn('-port', emulator['args'])
        port = emulator['args'][emulator['args'].index('-port') + 1]
        after_launch = events[events.index(emulator) + 1:]
        self.assertTrue(after_launch)
        self.assertTrue(all(event['serial'] == 'emulator-' + port for event in after_launch if event['kind'] == 'adb'))

    def test_boot_stall_respects_total_startup_bound(self):
        self.assert_startup_failure('boot-stall')

    def test_stalled_initial_adb_is_bounded(self):
        self.assert_startup_failure('adb-stall')

    def test_existing_connected_success_preserves_protocol_and_device(self):
        code, output, events, _ = self.run_runner()
        self.assertEqual(code, 0, output)
        self.assertIn(f'HIDLINS_DEVICE_TESTS {self.config["case_count"]} passed, 0 failed', output)
        self.assertFalse(any(event['kind'] == 'emulator' for event in events))
        self.assertFalse(any(event['args'][:2] == ['emu', 'kill'] for event in events))
        self.assertTrue(all(event['serial'] == 'fixture-connected' for event in events))

    def test_instrumentation_failure_stays_failure_with_safe_diagnostics(self):
        code, output, _, _ = self.run_runner('instrumentation-failure')
        self.assertNotEqual(code, 0, output)
        self.assertIn('did not report success', output)
        self.assertNotIn('fixture-private-window-marker', output)

    def test_wrong_api_or_abi_is_rejected_before_install(self):
        for key, value in (('api', '36'), ('abi', 'arm64-v8a')):
            with self.subTest(key=key):
                self.config[key] = value
                code, output, events, _ = self.run_runner()
                self.assertNotEqual(code, 0, output)
                self.assertIn('expected Android', output)
                self.assertFalse(any('install' in event['args'] for event in events))
                self.config.pop(key)
                self.events_file.unlink()


if __name__ == '__main__':
    unittest.main()

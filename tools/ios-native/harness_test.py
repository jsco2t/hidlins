#!/usr/bin/env python3
"""Unit tests for deterministic iPhone/iPad simulator selection."""

from __future__ import annotations

import json
import os
from pathlib import Path
import signal
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import select_simulators

ROOT = Path(__file__).resolve().parents[2]


class IntegrationHarnessTests(unittest.TestCase):
    def test_sdk_fixtures_ignore_the_parent_full_scenario_flag(self) -> None:
        with mock.patch.dict(os.environ, HIDLINS_MOBILE_SYNC_SCENARIOS='1'):
            self.run_vm_fixture(expected=0)
            self.assertEqual(os.environ['HIDLINS_MOBILE_SYNC_SCENARIOS'], '1')

    def test_vm_attach_does_not_require_a_unified_log_notification(self) -> None:
        self.run_vm_fixture(expected=0)

    def test_attached_driver_failure_is_preserved_without_retry(self) -> None:
        self.run_vm_fixture(expected=9, driver_status=9)

    def test_build_failure_prevents_simulator_launch(self) -> None:
        self.run_vm_fixture(expected=7, build_status=7)

    def test_missing_vm_service_times_out_and_cleans_up(self) -> None:
        self.run_vm_fixture(expected=124, unavailable=True)

    def test_unrelated_http_service_cannot_satisfy_vm_readiness(self) -> None:
        self.run_vm_fixture(expected=124, vm_type='OtherService')

    def run_vm_fixture(self, *, expected: int, driver_status: int = 0,
                       build_status: int = 0, unavailable: bool = False,
                       vm_type: str = 'VM') -> None:
        # Reproduce a successful app launch whose VM URL never reaches unified
        # logging. The VM itself is reachable; waiting for its log line hangs.
        with tempfile.TemporaryDirectory(prefix='hidlins-ios-vm-') as temporary:
            root = Path(temporary)
            fixture = root / 'devices.json'
            fixture.write_text(json.dumps({'devices': {
                'com.apple.CoreSimulator.SimRuntime.iOS-26-5': [
                    {'udid': 'iPhone', 'name': 'iPhone', 'isAvailable': True}],
                'com.apple.CoreSimulator.SimRuntime.iOS-26-4': [
                    {'udid': 'iPad', 'name': 'iPad', 'isAvailable': True}],
            }}))
            server = root / 'vm.py'
            server.write_text('''import http.server, json, os, sys
class VM(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = json.dumps({'result': {'type': os.environ['TEST_VM_TYPE']}}).encode()
        self.send_response(200)
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *args): pass
http.server.HTTPServer(('127.0.0.1', int(sys.argv[1])), VM).serve_forever()
''')
            xcrun = root / 'xcrun'
            xcrun.write_text(f'#!{sys.executable}\n' + '''import json, os, signal, subprocess, sys
from pathlib import Path
with open(os.environ['TEST_LOG'], 'a') as log:
    log.write(json.dumps(['xcrun', *sys.argv[1:]]) + '\\n')
action = sys.argv[2]
if action == 'list':
    print(Path(os.environ['TEST_DEVICES']).read_text())
elif action == 'spawn':
    sys.exit(1)
elif action == 'launch' and os.environ['TEST_UNAVAILABLE'] == '0':
    port = next(arg.split('=', 1)[1] for arg in sys.argv if arg.startswith('--vm-service-port='))
    process = subprocess.Popen([sys.executable, os.environ['TEST_SERVER'], port],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    Path(os.environ['TEST_PID']).write_text(str(process.pid))
elif action == 'terminate':
    try:
        os.kill(int(Path(os.environ['TEST_PID']).read_text()), signal.SIGTERM)
    except (FileNotFoundError, ProcessLookupError): pass
''')
            flutter = root / 'flutter'
            flutter.write_text(f'#!{sys.executable}\n' + '''import json, os, socket, subprocess, sys, time, urllib.request
from pathlib import Path
with open(os.environ['TEST_LOG'], 'a') as log:
    log.write(json.dumps(['flutter', *sys.argv[1:]]) + '\\n')
if os.environ.get('HIDLINS_MOBILE_SYNC_SCENARIOS') == '1':
    sys.exit(37)  # SDK fixtures must never enter the real CLI-authority journey.
if sys.argv[1] == 'test':
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    vm = subprocess.Popen([sys.executable, os.environ['TEST_SERVER'], str(port)],
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    Path(os.environ['TEST_PID']).write_text(str(vm.pid))
    print('Waiting for VM Service port to be available...', flush=True)
    time.sleep(60)
elif sys.argv[1] == 'build':
    sys.exit(int(os.environ['TEST_BUILD_STATUS']))
elif sys.argv[1] == 'drive':
    uri = next(arg.split('=', 1)[1] for arg in sys.argv if arg.startswith('--use-existing-app='))
    with urllib.request.urlopen(uri + 'getVM', timeout=1) as response:
        assert json.load(response)['result']['type'] == 'VM'
    print('All tests passed.' if os.environ['TEST_DRIVER_STATUS'] == '0' else 'Some tests failed.')
    sys.exit(int(os.environ['TEST_DRIVER_STATUS']))
''')
            for executable in (xcrun, flutter):
                executable.chmod(0o755)
            env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ['PATH'],
                       TEST_LOG=str(root / 'commands'), TEST_DEVICES=str(fixture),
                       TEST_SERVER=str(server), TEST_PID=str(root / 'pid'),
                       TEST_UNAVAILABLE=str(int(unavailable)),
                       TEST_VM_TYPE=vm_type,
                       TEST_DRIVER_STATUS=str(driver_status), TEST_BUILD_STATUS=str(build_status),
                       # The real mobile gate sets this on its nested make.
                       # These SDK fixtures cover UI/bridge orchestration only.
                       HIDLINS_MOBILE_SYNC_SCENARIOS='0',
                       HIDLINS_IOS_TEST_TIMEOUT_SECONDS='2' if expected == 124 else '8')
            process = subprocess.Popen(['bash', str(ROOT / 'tools/ios-native/integration.sh')],
                                       env=env, text=True, stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE, start_new_session=True)
            try:
                output, error = process.communicate(timeout=40)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate()
                self.fail('iOS harness did not bound or clean up its child processes')
            finally:
                try:
                    os.kill(int((root / 'pid').read_text()), signal.SIGTERM)
                except (FileNotFoundError, ProcessLookupError):
                    pass
            self.assertEqual(process.returncode, expected, output + error)
            commands = [json.loads(line) for line in (root / 'commands').read_text().splitlines()]
            drives = [command for command in commands if command[:2] == ['flutter', 'drive']]
            self.assertEqual(len(drives), 4 if expected == 0 else int(driver_status != 0))
            self.assertFalse(any(command[:2] == ['flutter', 'test'] for command in commands))
            devices = ('iPhone', 'iPad') if expected == 0 else ('iPhone',)
            for device in devices:
                self.assertIn(['xcrun', 'simctl', 'shutdown', device], commands)
            launches = [command for command in commands if command[:3] == ['xcrun', 'simctl', 'launch']]
            if build_status:
                self.assertEqual(launches, [])
            else:
                self.assertTrue(launches)
                for command in launches:
                    self.assertIn('--vm-service-host=127.0.0.1', command)
                    self.assertIn('--disable-vm-service-publication', command)
                for device in devices:
                    self.assertIn(['xcrun', 'simctl', 'terminate', device, 'app.hidlins.ios'], commands)

    def test_implicit_engine_registers_sdk_plugins(self) -> None:
        source = (ROOT / 'app/ios/Runner/AppDelegate.swift').read_text()
        callback = source[source.index('func didInitializeImplicitFlutterEngine'):]
        self.assertIn('GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)', callback)

    def test_sdk_registrant_is_exposed_to_swift_and_compiled_with_the_app(self) -> None:
        bridge = (ROOT / 'app/ios/Runner/Runner-Bridging-Header.h').read_text()
        self.assertIn('#import "GeneratedPluginRegistrant.h"', bridge)
        project = (ROOT / 'app/ios/Runner.xcodeproj/project.pbxproj').read_text()
        implementation = re.search(
            r'([A-F0-9]{24}) /\* GeneratedPluginRegistrant.m \*/ = '
            r'\{isa = PBXFileReference;[^\n]*path = GeneratedPluginRegistrant.m;', project)
        self.assertIsNotNone(implementation)
        build_file = re.search(
            r'([A-F0-9]{24}) /\* GeneratedPluginRegistrant.m in Sources \*/ = '
            r'\{isa = PBXBuildFile; fileRef = ' + implementation.group(1), project)
        self.assertIsNotNone(build_file)
        app_sources = re.search(
            r'isa = PBXSourcesBuildPhase;[^}]*files = \(([^)]*AppDelegate.swift in Sources[^)]*)\)',
            project)
        self.assertIsNotNone(app_sources)
        self.assertIn(build_file.group(1), app_sources.group(1))

    def test_native_test_failure_preserves_the_assertion_diagnostic(self) -> None:
        with tempfile.TemporaryDirectory(prefix='hidlins-ios-native-log-') as temporary:
            root = Path(temporary)
            flutter = root / 'flutter'
            flutter.write_text('#!/bin/bash\nexit 0\n')
            xcodebuild = root / 'xcodebuild'
            xcodebuild.write_text('''#!/bin/bash
for argument in "$@"; do
  if [[ "$argument" == -quiet ]]; then exit 65; fi
done
echo 'XCTAssertNotEqual failed: clipboard expiry assertion at RunnerTests.swift'
exit 65
''')
            for executable in (flutter, xcodebuild):
                executable.chmod(0o755)
            env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ['PATH'])
            result = subprocess.run(
                ['bash', str(ROOT / 'tools/ios-native/run.sh'), 'simulator'],
                env=env, capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 65, result.stdout + result.stderr)
            self.assertIn('clipboard expiry assertion at RunnerTests.swift',
                          result.stdout + result.stderr)

    def test_hung_flutter_child_fails_and_shuts_down_owned_simulator(self) -> None:
        with tempfile.TemporaryDirectory(prefix='hidlins-ios-integration-') as temporary:
            root = Path(temporary)
            devices = {'devices': {}}
            for runtime, kind in (('26-5', 'iPhone'), ('26-4', 'iPad')):
                devices['devices']['com.apple.CoreSimulator.SimRuntime.iOS-' + runtime] = [
                    {'udid': kind, 'name': kind, 'isAvailable': True, 'state': 'Shutdown'}]
            fixture = root / 'devices.json'
            fixture.write_text(json.dumps(devices))
            xcrun = root / 'xcrun'
            xcrun.write_text('''#!/bin/bash
printf '%s\\n' "$*" >> "$TEST_LOG"
case "$2" in
  list) cat "$TEST_DEVICES" ;;
  spawn) exit 1 ;;
esac
''')
            flutter = root / 'flutter'
            flutter.write_text('#!/bin/bash\nexec sleep 60\n')
            for executable in (xcrun, flutter):
                executable.chmod(0o755)
            env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ['PATH'],
                       TEST_LOG=str(root / 'commands'), TEST_DEVICES=str(fixture),
                       HIDLINS_IOS_TEST_TIMEOUT_SECONDS='0.1')
            process = subprocess.Popen(['bash', str(ROOT / 'tools/ios-native/integration.sh')],
                                       env=env, text=True, stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE, start_new_session=True)
            try:
                output, error = process.communicate(timeout=3)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate()
                self.fail('iOS integration harness did not enforce the child timeout')
            self.assertEqual(process.returncode, 124, output + error)
            self.assertIn('timeout', output)
            self.assertIn('simctl shutdown iPhone', (root / 'commands').read_text())


class SimulatorSelectionTests(unittest.TestCase):
    def test_matrix_uses_distinct_newest_phone_and_tablet_runtimes(self) -> None:
        candidates = [
            ("com.apple.CoreSimulator.SimRuntime.iOS-18-6", {"name": "iPhone 16", "state": "Shutdown"}),
            ("com.apple.CoreSimulator.SimRuntime.iOS-18-6", {"name": "iPad Air", "state": "Shutdown"}),
            ("com.apple.CoreSimulator.SimRuntime.iOS-26-4", {"name": "iPhone 17", "state": "Shutdown"}),
            ("com.apple.CoreSimulator.SimRuntime.iOS-26-4", {"name": "iPad Pro", "state": "Shutdown"}),
            ("com.apple.CoreSimulator.SimRuntime.iOS-26-5", {"name": "iPhone 17 Pro", "state": "Shutdown"}),
            ("com.apple.CoreSimulator.SimRuntime.iOS-26-5", {"name": "iPad mini", "state": "Shutdown"}),
        ]

        matrix = select_simulators.select_matrix(candidates)

        self.assertEqual(select_simulators.runtime_version(matrix[0][0]), (26, 5))
        self.assertIn("iPhone", str(matrix[0][1]["name"]))
        self.assertEqual(select_simulators.runtime_version(matrix[1][0]), (26, 4))
        self.assertIn("iPad", str(matrix[1][1]["name"]))

    def test_matrix_rejects_single_runtime(self) -> None:
        candidates = [
            ("com.apple.CoreSimulator.SimRuntime.iOS-26-5", {"name": "iPhone 17 Pro", "state": "Shutdown"}),
            ("com.apple.CoreSimulator.SimRuntime.iOS-26-5", {"name": "iPad mini", "state": "Shutdown"}),
        ]

        with self.assertRaisesRegex(SystemExit, "at least two"):
            select_simulators.select_matrix(candidates)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Unit tests for deterministic iPhone/iPad simulator selection."""

from __future__ import annotations

import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest

import select_simulators

ROOT = Path(__file__).resolve().parents[2]


class IntegrationHarnessTests(unittest.TestCase):
    def test_implicit_engine_registers_sdk_plugins(self) -> None:
        source = (ROOT / 'app/ios/Runner/AppDelegate.swift').read_text()
        callback = source[source.index('func didInitializeImplicitFlutterEngine'):]
        self.assertIn('GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)', callback)

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

#!/usr/bin/env python3
"""Negative controls for Android matrix and result tooling."""

from __future__ import annotations

import copy
import pathlib
import subprocess
import sys
import unittest

import emulator_matrix
import redact_log
import run_with_timeout


class EmulatorMatrixTests(unittest.TestCase):
    def setUp(self) -> None:
        self.entries = emulator_matrix.load()

    def matrix(self) -> dict[str, object]:
        return {
            "schema_version": 1,
            "minimum_api": 29,
            "current_api": 36,
            "shared_lan_minimum_api": 30,
            "entries": copy.deepcopy(self.entries),
        }

    def test_complete_matrix_is_accepted(self) -> None:
        self.assertEqual(len(emulator_matrix.validate(self.matrix())), 6)

    def test_native_matrix_keeps_api29_while_lan_uses_api30_and_current(self) -> None:
        for host in ("arm64", "x86_64"):
            for command, expected in (("select", [29, 36]), ("lan", [30, 36])):
                with self.subTest(host=host, command=command):
                    result = subprocess.run(
                        [sys.executable, str(pathlib.Path(emulator_matrix.__file__)),
                         command, '--host', host], capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    rows = [line.split('\t') for line in result.stdout.splitlines()]
                    self.assertEqual([int(row[1]) for row in rows], expected)
                    self.assertEqual([row[3] for row in rows], ['phone', 'tablet'])

    def test_missing_lan_minimum_is_rejected(self) -> None:
        data = self.matrix()
        data['entries'] = [entry for entry in self.entries if entry['api'] != 30]
        with self.assertRaises(SystemExit):
            emulator_matrix.validate(data)

    def test_legacy_api_cannot_be_admitted_as_the_shared_lan_minimum(self) -> None:
        data = self.matrix()
        data['shared_lan_minimum_api'] = 29
        with self.assertRaises(SystemExit):
            emulator_matrix.validate(data)

    def test_missing_abi_is_rejected(self) -> None:
        data = self.matrix()
        data["entries"] = [entry for entry in self.entries if entry["abi"] != "x86_64"]
        with self.assertRaises(SystemExit):
            emulator_matrix.validate(data)

    def test_stale_current_api_is_rejected(self) -> None:
        data = self.matrix()
        data["current_api"] = 33
        with self.assertRaises(SystemExit):
            emulator_matrix.validate(data)

    def test_single_form_factor_is_rejected(self) -> None:
        data = self.matrix()
        for entry in data["entries"]:
            entry["form_factor"] = "phone"
        with self.assertRaises(SystemExit):
            emulator_matrix.validate(data)

    def test_every_client_uses_distinct_matching_authority_and_replacement_avds(self) -> None:
        authorities = emulator_matrix.scenario_entries("authority", "arm64")
        replacements = emulator_matrix.scenario_entries("replacement", "arm64")
        self.assertEqual([entry["api"] for entry in authorities], [30, 36])
        self.assertEqual([entry["api"] for entry in replacements], [30, 36])
        self.assertEqual(
            [entry["avd"] for entry in authorities],
            ["hidlins-api30-authority", "hidlins-api36-tablet-authority"],
        )
        self.assertEqual(
            [entry["avd"] for entry in replacements],
            [
                "hidlins-api30-authority-replacement",
                "hidlins-api36-tablet-authority-replacement",
            ],
        )

    def test_matrix_cleanup_waits_for_adb_disconnect_before_full_scenario(self) -> None:
        source = (
            pathlib.Path(__file__).with_name("run_emulator_suite.sh")
        ).read_text(encoding="utf-8")
        cleanup = source.index("cleanup_emulator()")
        self.assertIn('wait_for_disconnect "$CURRENT_SERIAL"', source)
        self.assertIn('grep -Eq "^${serial}[[:space:]]"', source)
        self.assertLess(
            source.index('wait_for_disconnect "$CURRENT_SERIAL"', cleanup),
            source.index('CURRENT_SERIAL=""', cleanup),
        )
        self.assertLess(
            source.index("done 3<\"$MATRIX\""),
            source.index("android_mobile_scenario.py"),
        )

    def test_android_test_authority_keeps_the_explicit_wlan_listener(self) -> None:
        runtime = (
            pathlib.Path(__file__).parents[2]
            / "crates"
            / "hidlins-sync"
            / "src"
            / "server"
            / "runtime.rs"
        ).read_text(encoding="utf-8")
        listener_loop = runtime[
            runtime.index('name("hidlins-sync-listener"') : runtime.index(
                "let accepted =", runtime.index('name("hidlins-sync-listener"')
            )
        ]
        self.assertNotIn("android-scenario-authority", listener_loop)
        self.assertIn("refresh_desktop_listener", listener_loop)

    def test_android_test_registrar_is_the_only_scenario_publisher(self) -> None:
        runtime = (
            pathlib.Path(__file__).parents[2]
            / "crates"
            / "hidlins-sync"
            / "src"
            / "server"
            / "runtime.rs"
        ).read_text(encoding="utf-8")
        self.assertNotIn("android-scenario-authority", runtime)


class RedactionTests(unittest.TestCase):
    def test_test_secret_markers_are_removed(self) -> None:
        values = redact_log.sensitive_values()
        sanitized = redact_log.redact("integration-master-marker", values)
        self.assertNotIn("integration-master-marker", sanitized)

    def test_flutter_device_failure_markers_fail_closed(self) -> None:
        self.assertTrue(redact_log.has_flutter_device_failure("Some tests failed."))
        self.assertTrue(redact_log.has_flutter_device_failure("Failure Details:"))
        self.assertFalse(
            redact_log.has_flutter_device_failure("All tests passed on the device.")
        )


class ProcessTimeoutTests(unittest.TestCase):
    def test_successful_child_status_is_preserved(self) -> None:
        self.assertEqual(
            run_with_timeout.run([sys.executable, "-c", "pass"], 1), 0
        )

    def test_stalled_child_is_terminated(self) -> None:
        self.assertEqual(
            run_with_timeout.run(
                [sys.executable, "-c", "import time; time.sleep(2)"], 0.01
            ),
            124,
        )

if __name__ == "__main__":
    unittest.main()

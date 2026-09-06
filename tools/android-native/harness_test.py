#!/usr/bin/env python3
"""Negative controls for Android matrix and result tooling."""

from __future__ import annotations

import copy
import sys
import tempfile
import unittest
from pathlib import Path

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
            "entries": copy.deepcopy(self.entries),
        }

    def test_complete_matrix_is_accepted(self) -> None:
        self.assertEqual(len(emulator_matrix.validate(self.matrix())), 4)

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


class RedactionTests(unittest.TestCase):
    def test_test_markers_and_protected_config_values_are_removed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            defines = Path(directory) / "defines.json"
            defines.write_text(
                '{"HIDLINS_TEST_S3_ACCESS_KEY":"access-canary",'
                '"HIDLINS_TEST_S3_SECRET_KEY":"secret-canary"}',
                encoding="utf-8",
            )
            values = redact_log.sensitive_values(defines)
            sanitized = redact_log.redact(
                "integration-master-marker access-canary secret-canary", values
            )
            self.assertNotIn("integration-master-marker", sanitized)
            self.assertNotIn("access-canary", sanitized)
            self.assertNotIn("secret-canary", sanitized)

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

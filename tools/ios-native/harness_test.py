#!/usr/bin/env python3
"""Unit tests for the iOS simulator selection and protected S3 config tools."""

from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path

import secure_s3_config
import select_simulators


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


class SecureConfigTests(unittest.TestCase):
    def test_real_config_requires_private_source_and_writes_private_defines(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.json"
            output = root / "defines.json"
            source.write_text(
                json.dumps(
                    {
                        "endpoint": "https://s3.example.invalid",
                        "bucket": "fixture",
                        "region": "us-east-1",
                        "path_style": False,
                        "access_key_id": "fixture-access-marker",
                        "secret_access_key": "fixture-secret-marker",
                    }
                ),
                encoding="utf-8",
            )
            source.chmod(0o600)

            config = secure_s3_config.read_real(source)
            secure_s3_config.write_defines(output, config)

            self.assertEqual(output.stat().st_mode & 0o777, 0o600)
            defines = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(
                defines["HIDLINS_TEST_S3_SECRET_KEY"],
                "fixture-secret-marker",
            )
            self.assertTrue(defines["HIDLINS_TEST_S3_KEY"].endswith(".kdbx"))

    def test_real_config_rejects_group_readable_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.json"
            source.write_text("{}", encoding="utf-8")
            source.chmod(0o640)
            with self.assertRaises(SystemExit):
                secure_s3_config.read_real(source)

    def test_real_config_rejects_non_https_endpoint(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.json"
            source.write_text(
                json.dumps(
                    {
                        "endpoint": "http://s3.example.invalid",
                        "bucket": "fixture",
                        "region": "us-east-1",
                        "path_style": False,
                        "access_key_id": "fixture-access-marker",
                        "secret_access_key": "fixture-secret-marker",
                    }
                ),
                encoding="utf-8",
            )
            source.chmod(0o600)
            with self.assertRaises(SystemExit):
                secure_s3_config.read_real(source)

    def test_real_config_rejects_missing_keys(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.json"
            source.write_text(
                json.dumps(
                    {
                        "endpoint": "https://s3.example.invalid",
                        "bucket": "fixture",
                        "region": "us-east-1",
                        "path_style": False,
                        "access_key_id": "fixture-access-marker",
                    }
                ),
                encoding="utf-8",
            )
            source.chmod(0o600)
            with self.assertRaises(SystemExit):
                secure_s3_config.read_real(source)

    def test_real_config_rejects_unexpected_keys(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.json"
            source.write_text(
                json.dumps(
                    {
                        "endpoint": "https://s3.example.invalid",
                        "bucket": "fixture",
                        "region": "us-east-1",
                        "path_style": False,
                        "access_key_id": "fixture-access-marker",
                        "secret_access_key": "fixture-secret-marker",
                        "unexpected": "must-not-be-ignored",
                    }
                ),
                encoding="utf-8",
            )
            source.chmod(0o600)
            with self.assertRaises(SystemExit):
                secure_s3_config.read_real(source)

    def test_real_config_rejects_empty_credential_field(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "source.json"
            source.write_text(
                json.dumps(
                    {
                        "endpoint": "https://s3.example.invalid",
                        "bucket": "fixture",
                        "region": "us-east-1",
                        "path_style": False,
                        "access_key_id": "fixture-access-marker",
                        "secret_access_key": "",
                    }
                ),
                encoding="utf-8",
            )
            source.chmod(0o600)
            with self.assertRaises(SystemExit):
                secure_s3_config.read_real(source)

    def test_android_emulator_endpoint_is_rewritten_without_exposing_credentials(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "defines.json"
            secure_s3_config.write_defines(
                output,
                {
                    "endpoint": "http://127.0.0.1:9000",
                    "bucket": "fixture",
                    "region": "us-east-1",
                    "path_style": True,
                    "access_key_id": "fixture-access-marker",
                    "secret_access_key": "fixture-secret-marker",
                },
                key_prefix="hidlins-android-alpha",
                endpoint_host="10.0.2.2",
            )
            defines = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(
                defines["HIDLINS_TEST_S3_ENDPOINT"],
                "http://10.0.2.2:9000",
            )
            self.assertTrue(
                defines["HIDLINS_TEST_S3_KEY"].startswith("hidlins-android-alpha/")
            )


if __name__ == "__main__":
    os.umask(0o077)
    unittest.main()

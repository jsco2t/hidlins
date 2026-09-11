#!/usr/bin/env python3
"""Unit tests for deterministic iPhone/iPad simulator selection."""

from __future__ import annotations

import unittest

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


if __name__ == "__main__":
    unittest.main()

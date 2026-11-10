#!/usr/bin/env python3
"""The bootstrap must provision every simulator target used by Cargokit."""
from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]


class SimulatorTargetTests(unittest.TestCase):
    def test_shipping_simulator_architectures_are_declared_for_bootstrap(self):
        source = (ROOT / 'app/rust_builder/cargokit/build_tool/lib/src/target.dart').read_text()
        targets = {triple for triple, body in re.findall(
            r"Target\(\s*rust:\s*'([^']+)'(.*?)\)", source, re.S)
            if re.search(r"darwinPlatform:\s*'iphonesimulator'", body)}
        self.assertEqual(targets, {'aarch64-apple-ios-sim', 'x86_64-apple-ios'},
                         'The actual Cargokit simulator matrix must be inspected')
        configured = set(tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['targets'])
        self.assertFalse(targets - configured,
                         'Missing simulator bootstrap targets: ' + str(sorted(targets - configured)))

    def test_exact_compiler_and_existing_target_matrix_are_preserved(self):
        toolchain = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']
        self.assertRegex(toolchain['channel'], r'^\d+\.\d+\.\d+$')
        self.assertTrue({'x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu',
                         'aarch64-apple-darwin', 'x86_64-apple-darwin',
                         'aarch64-apple-ios', 'aarch64-apple-ios-sim',
                         'aarch64-linux-android', 'x86_64-linux-android'} <= set(toolchain['targets']))


if __name__ == '__main__':
    unittest.main(verbosity=2)

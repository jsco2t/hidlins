#!/usr/bin/env python3
"""Require portable fixture stubs and real rustfmt rejection through Make."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import test_tui_formatting as fixture


class TuiFormatFixtureHostTests(unittest.TestCase):
    def setUp(self):
        self.original = fixture.TuiFormattingTests()
        self.original.setUp()
        self.addCleanup(self.original.doCleanups)

    def test_native_make_can_run_the_controlled_cargo_noop(self):
        self.original.fixture.write_text('fn main() {}\n')
        result = self.original.run_make('fmt-check')
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_invalid_include_reaches_the_real_formatter_diagnostic(self):
        self.original.fixture.write_text('fn main( {\n')
        result = self.original.run_make('fmt-check')
        output = result.stdout + result.stderr
        self.assertNotEqual(result.returncode, 0, output)
        self.assertIn('error:', output)
        self.assertIn('app_format_probe_regressions.rs', output)
        self.assertNotIn('No such file or directory', output)


if __name__ == '__main__':
    unittest.main(verbosity=2)

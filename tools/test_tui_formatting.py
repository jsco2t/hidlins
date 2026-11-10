#!/usr/bin/env python3
"""Verify canonical formatting reaches external TUI include! modules."""

import pathlib
import shlex
import subprocess
import sys
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


class TuiFormattingTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="hidlins-tui-format-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = pathlib.Path(self.temporary.name)
        for source in (
            "tools/noise-erasure-tests/probe.rs",
            "tools/noise-erasure-tests/observer.rs",
        ):
            destination = self.directory / source
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((ROOT / source).read_bytes())
        self.fixture = (
            self.directory
            / "crates/hidlins-tui/src/app_format_probe_regressions.rs"
        )
        self.fixture.parent.mkdir(parents=True)

    def run_make(self, target):
        # Cargo never discovers include! sources. Isolate that existing half
        # of the workflow; the real Makefile and real rustfmt must handle the
        # owned external probe. No repository file is mutated by this test.
        return subprocess.run(
            [
                "make",
                "--no-print-directory",
                "-f",
                str(ROOT / "Makefile"),
                target,
                "CARGO=" + shlex.join([sys.executable, "-c", "pass"]),
            ],
            cwd=self.directory,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )

    def test_formatted_include_module_passes(self):
        self.fixture.write_text("fn main() {}\n")
        result = self.run_make("fmt-check")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_malformed_include_module_fails_check_without_rewriting(self):
        original = "fn main( ){ }\n"
        self.fixture.write_text(original)
        result = self.run_make("fmt-check")
        self.assertNotEqual(
            result.returncode, 0, "fmt-check ignored the malformed include! module"
        )
        self.assertEqual(self.fixture.read_text(), original)

    def test_format_repairs_include_module(self):
        self.fixture.write_text("fn main( ){ }\n")
        result = self.run_make("fmt")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.fixture.read_text(), "fn main() {}\n")
        checked = self.run_make("fmt-check")
        self.assertEqual(checked.returncode, 0, checked.stdout + checked.stderr)


if __name__ == "__main__":
    unittest.main()

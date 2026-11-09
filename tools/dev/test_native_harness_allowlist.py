#!/usr/bin/env python3
"""Run the actual Dart boundary scanner against isolated harness fixtures."""

from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
ALLOWLIST = Path("tools/dev/dart-native-test-allowlist.txt")
BROAD_SEARCH = Path("app/test_bridge/broad_search_performance_test.dart")


class NativeHarnessAllowlist(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="hidlins-native-allowlist-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for directory in ("lib", "test", "test_bridge", "integration_test"):
            (self.root / "app" / directory).mkdir(parents=True)
        (self.root / "tools/dev").mkdir(parents=True)
        shutil.copyfile(ROOT / ALLOWLIST, self.root / ALLOWLIST)

        # Execute the production functions and scanner verbatim. Other boundary
        # checks need real build metadata; the full gate runs separately.
        source = (ROOT / "tools/dev/boundary-check.sh").read_text()
        prefix = source[: source.index("# 1. frb config exposes only hidlins-api.")]
        start = source.index("# 2. Dart must reach Rust only through the generated bridge.")
        end = source.index("# Fixed platform capability exception", start)
        self.script = self.root / "tools/dev/boundary-slice.sh"
        self.script.write_text(prefix + source[start:end] + '\nexit "$FAIL"\n')
        for line in (self.root / ALLOWLIST).read_text().splitlines():
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            capability, name = line.split()
            self.assertIn(capability, ("ffi", "process"))
            path = Path(name)
            self.assertFalse(path.is_absolute())
            self.assertNotIn("..", path.parts)
            self.write(
                path,
                "import 'dart:ffi';\n"
                if capability == "ffi"
                else "void main() { Process.run('fixture', []); }\n",
            )

    def write(self, path, content):
        destination = self.root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(content)

    def scan(self, expected):
        result = subprocess.run(
            ["bash", str(self.script)],
            cwd=self.root,
            capture_output=True,
            text=True,
            timeout=15,
        )
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        return result.stdout + result.stderr

    def test_existing_exact_reviewed_harnesses_are_admitted(self):
        self.scan(0)

    def test_actual_broad_search_harness_is_admitted(self):
        self.write(BROAD_SEARCH, (ROOT / BROAD_SEARCH).read_text())
        self.scan(0)

    def test_unlisted_test_ffi_is_rejected(self):
        self.write("app/test_bridge/unreviewed.dart", "import 'dart:ffi';\n")
        self.assertIn("exact reviewed allowlist", self.scan(1))

    def test_production_ffi_is_rejected(self):
        self.write("app/lib/unreviewed.dart", "import 'dart:ffi';\n")
        self.assertIn("exact reviewed allowlist", self.scan(1))

    def test_unlisted_test_process_is_rejected(self):
        self.write("app/test/unreviewed.dart", "void main() { Process.run('fixture', []); }\n")
        self.assertIn("exact reviewed allowlist", self.scan(1))

    def test_stale_extra_allowlist_entry_is_rejected(self):
        path = self.root / ALLOWLIST
        path.write_text(path.read_text() + "ffi app/test_bridge/removed.dart\n")
        self.assertIn("exact reviewed allowlist", self.scan(1))

    def test_production_io_is_rejected_with_both_quote_styles(self):
        for quote in ("'", '"'):
            with self.subTest(quote=quote):
                self.write("app/lib/unreviewed.dart", f"import {quote}dart:io{quote};\n")
                self.assertIn("file/socket I/O belongs to the Rust core", self.scan(1))


if __name__ == "__main__":
    unittest.main(verbosity=2)

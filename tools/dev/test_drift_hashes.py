#!/usr/bin/env python3
"""Run the canonical drift recipes with disposable generators and tool paths."""

from __future__ import annotations

import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
TARGETS = ("api-gen-check", "completions-check", "pub-vendor-check")


def recipe(source: str, target: str) -> str:
    match = re.search(
        rf"(?m)^{re.escape(target)}:[^\n]*\n(?:(?:\t[^\n]*|[ \t]*)\n)*",
        source,
    )
    if match is None:
        raise AssertionError(f"canonical recipe missing: {target}")
    return match.group()


class DriftRecipeTests(unittest.TestCase):
    def exercise(self, target: str, mode: str, hash_tool: str = "absent"):
        with tempfile.TemporaryDirectory(prefix="hidlins drift gates ") as temporary:
            root = Path(temporary)
            binaries = root / "bin"
            binaries.mkdir()
            for name in ("bash", "make", "find", "sort", "xargs", "cat", "python3"):
                executable = shutil.which(name)
                self.assertIsNotNone(executable, f"fixture prerequisite: {name}")
                (binaries / name).symlink_to(executable)
            git = binaries / "git"
            git.write_text("#!/bin/bash\nexit 0\n")
            git.chmod(0o755)
            if hash_tool == "failed":
                broken = binaries / "sha256sum"
                broken.write_text("#!/bin/bash\nexit 1\n")
                broken.chmod(0o755)
            elif hash_tool == "working":
                executable = shutil.which("sha256sum")
                if executable is not None:
                    (binaries / "sha256sum").symlink_to(executable)
                else:
                    # The fixture itself must run on hosts without GNU tools.
                    control = binaries / "sha256sum"
                    control.write_text(
                        f"#!{sys.executable}\n"
                        "import hashlib,pathlib,sys\n"
                        "if len(sys.argv)==1:\n"
                        " print(hashlib.sha256(sys.stdin.buffer.read()).hexdigest()+'  -')\n"
                        "else:\n"
                        " for name in sys.argv[1:]:\n"
                        "  try: print(hashlib.sha256(pathlib.Path(name).read_bytes()).hexdigest()+'  '+name)\n"
                        "  except OSError: sys.exit(1)\n"
                    )
                    control.chmod(0o755)

            outputs = {
                "api-gen-check": "crates/api/frb_generated.rs",
                "completions-check": "shell-completions/hidlins.bash",
                "pub-vendor-check": "app/pubspec.lock",
            }
            for name in (
                *outputs.values(),
                "app/lib/bridge/bindings.dart",
                "shell-completions/_hidlins",
                "shell-completions/hidlins.fish",
            ):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("original bytes\n")
            for name in ("app/vendor-pub/hosted",):
                (root / name / "pub.dev/package-1.0.0").mkdir(parents=True)
            # The recipe retains its exact two production cache paths.
            cache = root / "app/rust_builder/cargokit/build_tool/vendor-pub/hosted"
            (cache / "pub.dev/package-1.0.0").mkdir(parents=True)
            tools = root / "tools/dev"
            tools.mkdir(parents=True)
            (tools / "pub-vendor-hash.sh").write_text("#!/bin/bash\nexit 0\n")
            (tools / "pub-vendor-hash.sh").chmod(0o755)
            helper = ROOT / "tools/dev/content_fingerprint.py"
            if helper.exists():
                shutil.copyfile(helper, tools / helper.name)

            protected = root / outputs[target]
            if target == "api-gen-check":
                protected = root / "app/lib/bridge/unreadable.dart"
                protected.write_text("unchanged\n")
            elif target == "completions-check":
                protected = root / "shell-completions/unreadable"
                protected.write_text("unchanged\n")
            if mode == "unreadable_before":
                protected.chmod(0)
            if mode == "unreadable_directory":
                protected = root / {
                    "api-gen-check": "app/lib/bridge/hidden",
                    "completions-check": "shell-completions/hidden",
                    "pub-vendor-check": "app/vendor-pub/hosted/pub.dev",
                }[target]
                protected.mkdir(exist_ok=True)
                (protected / "child").write_text("unreadable child\n")
                protected.chmod(0)

            unusual = root / (
                "app/lib/bridge/line\nbreak.dart"
                if target == "api-gen-check"
                else "shell-completions/line\nbreak"
            )
            if mode == "newline_change":
                unusual.write_text("before\n")
            generator = root / "generator.py"
            generator.write_text(
                "from pathlib import Path\nimport os,sys\n"
                "mode=os.environ['FIXTURE_MODE']\n"
                "if mode=='fail': sys.exit(23)\n"
                "p=Path(os.environ['FIXTURE_OUTPUT'])\n"
                "if mode=='change': p.write_text(p.read_text()+'changed\\n')\n"
                "if mode=='unreadable_after': Path(os.environ['FIXTURE_PROTECTED']).chmod(0)\n"
                "if mode=='newline_change': Path(os.environ['FIXTURE_UNUSUAL']).write_text('after\\n')\n"
            )
            source = (ROOT / "Makefile").read_text()
            makefile = (
                "SHELL := /bin/bash\n"
                "FRB_RUST_OUT := crates/api/frb_generated.rs\n"
                "FRB_DART_OUT := app/lib/bridge/\n"
                + recipe(source, target)
                + "\n.PHONY: api-gen completions app-deps pub-vendor-hash-test\n"
                + "api-gen completions app-deps:\n"
                + f"\t{shlex.quote(sys.executable)} generator.py\n"
                + "pub-vendor-hash-test:\n\t@true\n"
            )
            (root / "Makefile").write_text(makefile)
            environment = dict(os.environ)
            for key in ("MAKEFLAGS", "MFLAGS", "MAKELEVEL", "MAKEFILES"):
                environment.pop(key, None)
            environment.update(
                PATH=str(binaries),
                FIXTURE_MODE=mode,
                FIXTURE_OUTPUT=outputs[target],
                FIXTURE_PROTECTED=str(protected.relative_to(root)),
                FIXTURE_UNUSUAL=str(unusual.relative_to(root)),
            )
            try:
                return subprocess.run(
                    ["make", "--no-print-directory", target],
                    cwd=root,
                    env=environment,
                    text=True,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    timeout=20,
                )
            finally:
                # Restore fixture permissions so TemporaryDirectory can clean up.
                protected.chmod(0o700 if protected.is_dir() else 0o600)

    def test_changed_output_is_rejected_when_hash_command_fails(self):
        for target in TARGETS:
            with self.subTest(target=target):
                result = self.exercise(target, "change", "failed")
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_changed_output_is_rejected_without_gnu_hash(self):
        for target in TARGETS:
            with self.subTest(target=target):
                result = self.exercise(target, "change")
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_unchanged_outputs_work_without_gnu_hash(self):
        for target in TARGETS:
            with self.subTest(target=target):
                result = self.exercise(target, "noop")
                self.assertEqual(result.returncode, 0, result.stdout)

    def test_generator_failure_never_reports_clean(self):
        for target in TARGETS:
            with self.subTest(target=target):
                result = self.exercise(target, "fail")
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_unreadable_input_before_generation_fails_closed(self):
        for target in TARGETS:
            with self.subTest(target=target):
                result = self.exercise(target, "unreadable_before", "working")
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_unreadable_input_after_generation_fails_closed(self):
        for target in TARGETS:
            with self.subTest(target=target):
                result = self.exercise(target, "unreadable_after", "working")
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_directory_enumeration_failure_is_not_a_clean_signature(self):
        for target in TARGETS:
            with self.subTest(target=target):
                result = self.exercise(target, "unreadable_directory", "working")
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_literal_newline_filename_drift_is_rejected(self):
        for target in TARGETS[:2]:
            with self.subTest(target=target):
                result = self.exercise(target, "newline_change", "working")
                self.assertNotEqual(result.returncode, 0, result.stdout)


if __name__ == "__main__":
    unittest.main(verbosity=2)

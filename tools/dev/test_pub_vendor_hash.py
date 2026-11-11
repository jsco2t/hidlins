#!/usr/bin/env python3
"""Exercise pub-cache integrity policy in disposable real Git repositories."""
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("pub-vendor-hash.sh")
TREES = (
    "app/vendor-pub/hosted",
    "app/vendor-pub/hosted-hashes",
    "app/rust_builder/cargokit/build_tool/vendor-pub/hosted",
    "app/rust_builder/cargokit/build_tool/vendor-pub/hosted-hashes",
)


class PubVendorHashTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="hidlins-pub-hash-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.script = self.root / "tools/dev/pub-vendor-hash.sh"
        self.script.parent.mkdir(parents=True)
        shutil.copyfile(SCRIPT, self.script)
        self.manifest = self.script.with_name("pub-vendor.SHA256SUMS")
        self.git("init", "-q")
        (self.root / ".gitignore").write_text("**/.dart_tool/\n")
        self.inputs = []
        for i, tree in enumerate(TREES):
            path = self.root / tree / "pub.dev" / f"fixture-{i}" / "source.dart"
            path.parent.mkdir(parents=True)
            path.write_text(f"reviewed input {i}\n")
            self.inputs.append(path)
        self.git("add", ".")
        self.run_hash("generate", success=True)
        self.git("add", ".")
        self.run_hash("verify", success=True)

    def git(self, *args):
        return subprocess.run(
            ["git", "-C", str(self.root), *args],
            check=True, capture_output=True, text=True,
        )

    def run_hash(self, mode, *, success):
        result = subprocess.run(
            ["bash", str(self.script), mode], cwd=self.root,
            capture_output=True, text=True, timeout=20,
        )
        if success:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def test_each_reviewed_tree_detects_modified_payload(self):
        for path in self.inputs:
            with self.subTest(path=path.relative_to(self.root)):
                original = path.read_bytes()
                path.write_bytes(original + b"modified\n")
                self.run_hash("verify", success=False)
                path.write_bytes(original)
                self.run_hash("verify", success=True)

    def test_each_reviewed_tree_detects_deleted_payload(self):
        for path in self.inputs:
            with self.subTest(path=path.relative_to(self.root)):
                original = path.read_bytes()
                path.unlink()
                self.run_hash("verify", success=False)
                path.write_bytes(original)

    def test_each_reviewed_tree_rejects_untracked_source(self):
        for path in self.inputs:
            extra = path.with_name("unexpected.dart")
            extra.write_text("unreviewed\n")
            self.run_hash("verify", success=False)
            extra.unlink()

    def test_ignored_execution_artifacts_do_not_change_reviewed_manifest(self):
        before = self.manifest.read_bytes()
        for path in self.inputs:
            artifact = path.parent / ".dart_tool" / "package_config.json"
            artifact.parent.mkdir()
            artifact.write_text('{"runtime":true}\n')
        self.run_hash("verify", success=True)
        self.run_hash("generate", success=True)
        self.assertEqual(self.manifest.read_bytes(), before)

    def test_tracked_ignored_files_remain_protected(self):
        artifact = self.inputs[0].parent / ".dart_tool" / "reviewed.json"
        artifact.parent.mkdir()
        artifact.write_text("reviewed\n")
        self.git("add", "-f", str(artifact))
        self.run_hash("generate", success=True)
        self.run_hash("verify", success=True)
        artifact.write_text("changed\n")
        self.run_hash("verify", success=False)
        artifact.unlink()
        self.run_hash("verify", success=False)

    def test_generation_includes_intentional_new_input_until_staged(self):
        extra = self.inputs[0].with_name("new file.dart")
        extra.write_bytes(b"intentional\n")
        self.run_hash("generate", success=True)
        expected = hashlib.sha256(extra.read_bytes()).hexdigest()
        self.assertIn(
            f"{expected}  {extra.relative_to(self.root)}\n",
            self.manifest.read_text(),
        )
        self.run_hash("verify", success=False)
        self.git("add", str(extra))
        self.run_hash("verify", success=True)

    def test_generation_handles_intentional_removal_without_accepting_unstaged_drift(self):
        removed = self.inputs[0]
        removed.unlink()
        self.run_hash("generate", success=True)
        self.assertNotIn(str(removed.relative_to(self.root)), self.manifest.read_text())
        self.run_hash("verify", success=False)
        self.git("add", "-u")
        self.run_hash("verify", success=True)

    def test_all_symlinks_are_rejected_even_inside_ignored_runtime(self):
        for ignored in (False, True):
            with self.subTest(ignored=ignored):
                parent = self.inputs[0].parent
                if ignored:
                    parent = parent / ".dart_tool"
                    parent.mkdir(exist_ok=True)
                link = parent / "link.dart"
                link.symlink_to(self.inputs[1])
                self.run_hash("verify", success=False)
                self.run_hash("generate", success=False)
                link.unlink()

    def test_missing_cache_tree_fails_both_modes(self):
        shutil.rmtree(self.root / TREES[-1])
        self.run_hash("verify", success=False)
        self.run_hash("generate", success=False)


if __name__ == "__main__":
    unittest.main(verbosity=2)

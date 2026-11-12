#!/usr/bin/env python3
"""Exact renderer geometry patch replay and closed-source negative controls."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("hidlins_geometry_vendor", ROOT / "tools/dev/vendor.py")
if spec is None or spec.loader is None:
    raise RuntimeError("cannot load maintained vendor patches")
vendor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vendor)
RELATIVE = "src/paragraph.rs"


class GeometryPatchTests(unittest.TestCase):
    def fixture(self, directory: str) -> Path:
        destination = Path(directory)
        shutil.copytree(ROOT / "vendor/ratatui-widgets", destination / "ratatui-widgets")
        return destination

    def snapshot(self, destination: Path) -> dict[str, bytes]:
        return {
            str(path.relative_to(destination)): path.read_bytes()
            for path in destination.rglob("*") if path.is_file()
        }

    def apply(self, destination: Path) -> None:
        with patch.object(vendor, "VENDOR", destination):
            vendor.patch_ratatui_geometry()

    def pristine(self, destination: Path) -> None:
        path = destination / "ratatui-widgets" / RELATIVE
        text = path.read_text()
        before, after = vendor.RATATUI_GEOMETRY_EDIT
        self.assertEqual(text.count(after), 1)
        text = text.replace(after, before, 1)
        self.assertEqual(
            hashlib.sha256(text.encode()).hexdigest(), vendor.RATATUI_GEOMETRY_HASHES[0]
        )
        path.write_text(text)
        vendor.refresh_checksum(destination / "ratatui-widgets")

    def test_committed_complete_idempotent_and_checksummed(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-geometry-") as directory:
            target = self.fixture(directory)
            self.apply(target)
            before = self.snapshot(target)
            data = json.loads((target / "ratatui-widgets/.cargo-checksum.json").read_text())
            self.assertEqual(data["files"][RELATIVE], vendor.RATATUI_GEOMETRY_HASHES[1])
            self.apply(target)
            self.assertEqual(before, self.snapshot(target))

    def test_exact_pristine_replays_identically(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-geometry-") as directory:
            target = self.fixture(directory)
            before = self.snapshot(target)
            self.pristine(target)
            self.apply(target)
            self.assertEqual(before, self.snapshot(target))

    def test_target_drift_is_rejected_without_writing(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-geometry-") as directory:
            target = self.fixture(directory)
            self.pristine(target)
            path = target / "ratatui-widgets" / RELATIVE
            path.write_text(path.read_text() + "\n// unknown drift\n")
            before = self.snapshot(target)
            with self.assertRaises(RuntimeError):
                self.apply(target)
            self.assertEqual(before, self.snapshot(target))

    def test_wrong_version_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-geometry-") as directory:
            target = self.fixture(directory)
            path = target / "ratatui-widgets/Cargo.toml"
            path.write_text(path.read_text().replace('version = "0.3.2"', 'version = "0.3.3"', 1))
            before = self.snapshot(target)
            with self.assertRaises(RuntimeError):
                self.apply(target)
            self.assertEqual(before, self.snapshot(target))

    def test_untouched_drift_does_not_rebless_or_partially_patch(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-geometry-") as directory:
            target = self.fixture(directory)
            self.pristine(target)
            path = target / "ratatui-widgets/src/lib.rs"
            path.write_text(path.read_text() + "\n// unknown drift\n")
            before = self.snapshot(target)
            with self.assertRaises(RuntimeError):
                self.apply(target)
            self.assertEqual(before, self.snapshot(target))

    def test_inventory_package_and_target_checksum_tamper_rejected(self) -> None:
        for kind in ["inventory", "package", "target"]:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory(prefix="hidlins-geometry-") as directory:
                target = self.fixture(directory)
                path = target / "ratatui-widgets/.cargo-checksum.json"
                data = json.loads(path.read_text())
                if kind == "inventory":
                    del data["files"]["src/lib.rs"]
                elif kind == "package":
                    data["package"] = "0" * 64
                else:
                    data["files"][RELATIVE] = "0" * 64
                path.write_text(json.dumps(data))
                before = self.snapshot(target)
                with self.assertRaises(RuntimeError):
                    self.apply(target)
                self.assertEqual(before, self.snapshot(target))


if __name__ == "__main__":
    unittest.main()

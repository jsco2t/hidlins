#!/usr/bin/env python3
"""Regression tests for audited vendored-source patch guards."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
VENDOR_SCRIPT = ROOT / "tools" / "dev" / "vendor.py"

spec = importlib.util.spec_from_file_location("hidlins_vendor", VENDOR_SCRIPT)
if spec is None or spec.loader is None:
    raise RuntimeError(f"cannot load {VENDOR_SCRIPT}")
vendor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vendor)


class KeepassPatchReplayTests(unittest.TestCase):
    def test_string_value_patch_replays_from_original_annotation(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-keepass-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "keepass"
            shutil.copytree(ROOT / "vendor" / "keepass", crate)
            relative = Path("src/format/xml_db/entry.rs")
            reviewed = (crate / relative).read_text()
            prefix, block = reviewed.split("pub struct StringValue {", 1)
            value, suffix = block.split("    value: Option<String>,", 1)
            # Reconstruct this upstream annotation even when the committed
            # tree already contains the reviewed fix. The other audited
            # patches remain applied; no downloads are needed.
            value = value.replace('rename = "$text"', 'rename = "$value"', 1)
            if 'with = "cs_opt_string"' not in value:
                value = value.replace(
                    '        rename = "$value",\n',
                    '        rename = "$value",\n        with = "cs_opt_string",\n',
                    1,
                )
            (crate / relative).write_text(
                prefix + "pub struct StringValue {" + value
                + "    value: Option<String>," + suffix
            )
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_keepass()
                self.assertEqual((crate / relative).read_text(), reviewed)
                first = {
                    path.relative_to(crate): path.read_bytes()
                    for path in crate.rglob("*") if path.is_file()
                }
                vendor.patch_keepass()
                second = {
                    path.relative_to(crate): path.read_bytes()
                    for path in crate.rglob("*") if path.is_file()
                }
                self.assertEqual(first, second)


class QuickXmlTextPatchReplayTests(unittest.TestCase):
    def test_text_map_patch_replays_and_preserves_other_bytes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-xml-patch-") as temporary:
            vendor_dir = Path(temporary)
            crate = vendor_dir / "quick-xml"
            shutil.copytree(ROOT / "vendor" / "quick-xml", crate)
            source = crate / "src/de/map.rs"
            reviewed = source.read_bytes()
            fixed = (
                b"        if self.has_text_field {\r\n"
                b"            Ok(())\r\n"
                b"        } else {\r\n"
                b"            self.de.skip_whitespaces()\r\n"
                b"        }\r\n"
            )
            original = b"        self.de.skip_whitespaces()\r\n"
            source.write_bytes(reviewed.replace(fixed, original, 1))
            with patch.object(vendor, "VENDOR", vendor_dir):
                vendor.patch_quick_xml()
                self.assertEqual(source.read_bytes(), reviewed)
                first = (crate / ".cargo-checksum.json").read_bytes()
                vendor.patch_quick_xml()
                self.assertEqual(source.read_bytes(), reviewed)
                self.assertEqual((crate / ".cargo-checksum.json").read_bytes(), first)


class SnowPatchGuardTests(unittest.TestCase):
    def copy_snow(self, parent: Path, name: str) -> Path:
        destination = parent / name
        shutil.copytree(ROOT / "vendor" / "snow", destination)
        return destination

    def test_committed_patched_tree_passes(self) -> None:
        vendor.verify_snow_patch(ROOT / "vendor" / "snow")

    def test_compiled_feature_set_is_exact(self) -> None:
        metadata = json.loads(
            subprocess.check_output(
                ["cargo", "metadata", "--offline", "--locked", "--format-version", "1"],
                cwd=ROOT,
            )
        )
        packages = {package["id"]: package for package in metadata["packages"]}
        snow_node = next(
            node
            for node in metadata["resolve"]["nodes"]
            if packages[node["id"]]["name"] == "snow"
        )
        self.assertEqual(
            set(snow_node["features"]),
            {
                "chacha20poly1305",
                "curve25519-dalek",
                "default-resolver",
                "getrandom",
                "sha2",
                "std",
                "use-chacha20poly1305",
                "use-curve25519",
                "use-getrandom",
                "use-sha2",
            },
        )

    def test_unselected_ring_graph_is_absent(self) -> None:
        lock = (ROOT / "Cargo.lock").read_text()
        self.assertNotIn('name = "ring"', lock)
        self.assertFalse((ROOT / "vendor" / "ring").exists())
        self.assertNotIn('"ring?/std"', (ROOT / "vendor/snow/Cargo.toml").read_text())

    def test_vendor_pruning_uses_full_lock_membership(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-vendor-prune-") as temporary:
            root = Path(temporary)
            vendor_dir = root / "vendor"
            vendor_dir.mkdir()
            retained = vendor_dir / "target-only"
            retained.mkdir()
            (retained / "Cargo.toml").write_text(
                '[package]\nname = "target-only"\nversion = "1.2.3"\n'
            )
            stale = vendor_dir / "stale"
            stale.mkdir()
            (stale / "Cargo.toml").write_text(
                '[package]\nname = "stale"\nversion = "9.9.9"\n'
            )
            lock_path = root / "Cargo.lock"
            lock_path.write_text(
                'version = 4\n\n[[package]]\nname = "target-only"\n'
                'version = "1.2.3"\nsource = "registry+https://example.invalid/index"\n'
            )

            vendor.prune_unlocked_vendor(vendor_dir, lock_path)

            self.assertTrue(retained.is_dir())
            self.assertFalse(stale.exists())

    def test_vendor_pruning_uses_union_of_isolated_lockfiles(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-vendor-union-") as temporary:
            root = Path(temporary)
            vendor_dir = root / "vendor"
            vendor_dir.mkdir()
            for name, version in (("production", "1.0.0"), ("fuzz-only", "2.0.0")):
                crate = vendor_dir / name
                crate.mkdir()
                (crate / "Cargo.toml").write_text(
                    f'[package]\nname = "{name}"\nversion = "{version}"\n'
                )
            production_lock = root / "Cargo.lock"
            production_lock.write_text(
                'version = 4\n\n[[package]]\nname = "production"\n'
                'version = "1.0.0"\nsource = "registry+https://example.invalid/index"\n'
            )
            fuzz_lock = root / "fuzz.lock"
            fuzz_lock.write_text(
                'version = 4\n\n[[package]]\nname = "fuzz-only"\n'
                'version = "2.0.0"\nsource = "registry+https://example.invalid/index"\n'
            )

            vendor.prune_unlocked_vendor(
                vendor_dir, production_lock, additional_lock_paths=(fuzz_lock,)
            )

            self.assertTrue((vendor_dir / "production").is_dir())
            self.assertTrue((vendor_dir / "fuzz-only").is_dir())

    def test_missing_zeroization_patch_fails(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-snow-patch-") as temporary:
            snow = self.copy_snow(Path(temporary), "unpatched")
            builder = snow / "src" / "builder.rs"
            source = builder.read_text()
            builder.write_text(source.replace("#[derive(ZeroizeOnDrop)]\n", "", 1))

            with self.assertRaisesRegex(RuntimeError, "src/builder.rs"):
                vendor.verify_snow_patch(snow)

    def test_source_drift_fails(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-snow-drift-") as temporary:
            snow = self.copy_snow(Path(temporary), "drifted")
            types = snow / "src" / "types.rs"
            types.write_text(types.read_text() + "\n")

            with self.assertRaisesRegex(RuntimeError, "src/types.rs"):
                vendor.verify_snow_patch(snow)

    def test_unknown_version_fails(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-snow-version-") as temporary:
            snow = self.copy_snow(Path(temporary), "wrong-version")
            manifest = snow / "Cargo.toml"
            manifest.write_text(
                manifest.read_text().replace('version = "0.10.0"', 'version = "0.10.1"', 1)
            )

            with self.assertRaisesRegex(RuntimeError, "audited for 0.10.0"):
                vendor.verify_snow_patch(snow)


if __name__ == "__main__":
    unittest.main()

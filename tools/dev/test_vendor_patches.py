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


ROOT = Path(__file__).resolve().parents[2]
VENDOR_SCRIPT = ROOT / "tools" / "dev" / "vendor.py"

spec = importlib.util.spec_from_file_location("hidlins_vendor", VENDOR_SCRIPT)
if spec is None or spec.loader is None:
    raise RuntimeError(f"cannot load {VENDOR_SCRIPT}")
vendor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vendor)


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

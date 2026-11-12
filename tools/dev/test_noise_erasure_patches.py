#!/usr/bin/env python3
"""Replay and fail-closed guards for the selected Noise SHA erasure patch."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import re
import shutil
import tempfile
import tomllib
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("hidlins_vendor", ROOT / "tools/dev/vendor.py")
if spec is None or spec.loader is None:
    raise RuntimeError("cannot load vendor patch implementation")
vendor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vendor)
CRATES = ("snow", "sha2", "digest", "block-buffer")


class NoiseErasurePatchTests(unittest.TestCase):
    def fixture(self, directory: str) -> Path:
        destination = Path(directory)
        for name in CRATES:
            shutil.copytree(ROOT / "vendor" / name, destination / name)
        return destination

    def apply_and_check(self, destination: Path) -> None:
        with patch.object(vendor, "VENDOR", destination):
            vendor.patch_snow()
        manifest = tomllib.loads((destination / "snow/Cargo.toml").read_text())
        self.assertEqual(manifest["dependencies"]["sha2"]["version"], "=0.11.0")
        self.assertEqual(manifest["dependencies"]["sha2"]["features"], ["zeroize"])
        self.assertNotIn("sha2?/std", manifest["features"]["std"])
        for filename in ("Cargo.toml", "Cargo.toml.orig"):
            digest = tomllib.loads((destination / "digest" / filename).read_text())
            buffer = tomllib.loads((destination / "block-buffer" / filename).read_text())
            self.assertIn("common/zeroize", digest["features"]["zeroize"])
            self.assertIn("hybrid-array/zeroize", buffer["features"]["zeroize"])
        source = (destination / "snow/src/resolvers/default.rs").read_text()
        self.assertEqual(source.count("core::mem::take(&mut self.hasher).finalize_into("), 2)
        self.assertIn("#[derive(Default, ZeroizeOnDrop)]\nstruct HashSHA256", source)
        self.assertIn("#[derive(Default, ZeroizeOnDrop)]\nstruct HashSHA512", source)
        self.assertNotIn("impl Drop for HashSHA256", source)
        self.assertNotIn("impl Drop for HashSHA512", source)
        self.assertIn("zeroize::Zeroizing::new(full_res)", (destination / "digest/src/block_api/ct_variable.rs").read_text())
        self.assertIn("zeroize::Zeroizing::new(buf)", (destination / "block-buffer/src/lib.rs").read_text())
        self.assertNotIn("&[b.0]", (destination / "sha2/src/block_api.rs").read_text())
        before = {name: (destination / name / ".cargo-checksum.json").read_bytes() for name in CRATES}
        with patch.object(vendor, "VENDOR", destination):
            vendor.patch_snow()
        self.assertEqual(before, {name: (destination / name / ".cargo-checksum.json").read_bytes() for name in CRATES})

    def test_committed_selected_erasure_patch_is_complete_and_idempotent(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-noise-erasure-replay-") as temporary:
            self.apply_and_check(self.fixture(temporary))

    def test_prior_and_raw_snow_std_feature_edges_upgrade_without_fallback(self) -> None:
        for raw in (False, True):
            with self.subTest(raw=raw), tempfile.TemporaryDirectory(prefix="hidlins-noise-std-replay-") as temporary:
                destination = self.fixture(temporary)
                values = ["getrandom/std", "subtle/std", "ring/std", "blake2/std", "sha2/std"] if raw else [
                    "getrandom?/std", "subtle/std", "blake2?/std", "sha2?/std",
                ]
                for filename in ("Cargo.toml", "Cargo.toml.orig"):
                    path = destination / "snow" / filename
                    source = path.read_text()
                    if filename.endswith(".orig"):
                        source, count = re.subn(r"^std = \[.*\]$", "std = [" + ", ".join('"' + value + '"' for value in values) + "]", source, flags=re.MULTILINE)
                    else:
                        source, count = re.subn(r"^std = \[\n.*?\n\]", "std = [\n" + "".join('    "' + value + '",\n' for value in values) + "]", source, flags=re.MULTILINE | re.DOTALL)
                    self.assertEqual(count, 1)
                    path.write_text(source)
                self.apply_and_check(destination)

    def test_native_output_finalization_replays_from_prior_reset_implementation(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-noise-output-replay-") as temporary:
            destination = self.fixture(temporary)
            path = destination / "snow/src/resolvers/default.rs"
            source = path.read_text()
            for size in (32, 64):
                after = f"        let mut hash = Zeroizing::new([0_u8; {size}]);\n        core::mem::take(&mut self.hasher).finalize_into((&mut *hash).into());"
                source = source.replace(after, "        let hash = self.hasher.finalize_reset();", 1)
            path.write_text(source)
            self.apply_and_check(destination)

    def test_dependency_scratch_and_borrowed_padding_patches_replay(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-noise-scratch-replay-") as temporary:
            destination = self.fixture(temporary)
            path = destination / "digest/src/block_api/ct_variable.rs"
            source = path.read_text()
            after = '''        let full_res = Array::<u8, T::OutputSize>::default();
        #[cfg(feature = "zeroize")]
        let mut full_res = zeroize::Zeroizing::new(full_res);
        #[cfg(not(feature = "zeroize"))]
        let mut full_res = full_res;'''
            path.write_text(source.replace(after, "        let mut full_res = Default::default();", 1))
            path = destination / "block-buffer/src/lib.rs"
            after = '''        let buf = self.pad_with_zeros();
        #[cfg(feature = "zeroize")]
        let mut buf = zeroize::Zeroizing::new(buf);
        #[cfg(not(feature = "zeroize"))]
        let mut buf = buf;'''
            path.write_text(path.read_text().replace(after, "        let mut buf = self.pad_with_zeros();", 1))
            path = destination / "sha2/src/block_api.rs"
            path.write_text(path.read_text().replace("Array::cast_slice_to_core(core::slice::from_ref(b))", "&[b.0]"))
            self.apply_and_check(destination)

    def test_each_dependency_source_drift_is_rejected(self) -> None:
        for name, filename in (("sha2", "src/block_api.rs"), ("digest", "src/block_api/ct_variable.rs"), ("block-buffer", "src/lib.rs")):
            with self.subTest(crate=name), tempfile.TemporaryDirectory(prefix="hidlins-noise-drift-") as temporary:
                destination = self.fixture(temporary)
                path = destination / name / filename
                path.write_text(path.read_text() + "\n// unexpected source drift\n")
                with patch.object(vendor, "VENDOR", destination), self.assertRaises(RuntimeError):
                    vendor.patch_snow()

    def test_each_dependency_version_drift_is_rejected(self) -> None:
        for name in ("sha2", "digest", "block-buffer"):
            with self.subTest(crate=name), tempfile.TemporaryDirectory(prefix="hidlins-noise-version-") as temporary:
                destination = self.fixture(temporary)
                path = destination / name / "Cargo.toml"
                source = path.read_text()
                version = tomllib.loads(source)["package"]["version"]
                path.write_text(source.replace(f'version = "{version}"', 'version = "9.99.99"', 1))
                with patch.object(vendor, "VENDOR", destination), self.assertRaises(RuntimeError):
                    vendor.patch_snow()


if __name__ == "__main__":
    unittest.main()

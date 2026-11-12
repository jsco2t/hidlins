#!/usr/bin/env python3
"""Replay and closed-context guards for untrusted KDBX4 KDF admission."""

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
spec = importlib.util.spec_from_file_location("hidlins_kdf_vendor", ROOT / "tools/dev/vendor.py")
if spec is None or spec.loader is None:
    raise RuntimeError("cannot load maintained vendor patches")
vendor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vendor)
RELATIVE = "src/format/kdbx4/parse.rs"
METHOD = "    pub fn kdbx4_kdf_config(data: &[u8]) -> Result<KdfConfig, DatabaseOpenError>"


class KdfAdmissionPatchTests(unittest.TestCase):
    def fixture(self, directory: str) -> Path:
        destination = Path(directory)
        shutil.copytree(ROOT / "vendor/keepass", destination / "keepass")
        return destination

    def apply_and_check(self, destination: Path) -> None:
        with patch.object(vendor, "VENDOR", destination):
            vendor.patch_keepass()
        source = (destination / "keepass" / RELATIVE).read_text()
        self.assertIn(METHOD, source)
        self.assertIn("let (header, _) = parse_outer_header(data)", source)
        self.assertIn("Untrusted metadata", source)
        checksum_path = destination / "keepass/.cargo-checksum.json"
        checksums = json.loads(checksum_path.read_text())
        self.assertEqual(checksums["files"][RELATIVE], hashlib.sha256(source.encode()).hexdigest())
        before = checksum_path.read_bytes()
        with patch.object(vendor, "VENDOR", destination):
            vendor.patch_keepass()
        self.assertEqual(checksum_path.read_bytes(), before)

    def test_committed_header_patch_is_complete_and_idempotent(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-kdf-replay-") as directory:
            self.apply_and_check(self.fixture(directory))

    def test_prior_exact_header_source_replays(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-kdf-prior-") as directory:
            destination = self.fixture(directory)
            path = destination / "keepass" / RELATIVE
            source = path.read_text()
            # Restore the exact original input even when the independent
            # scalar-bounds patch is present in the committed fixture.
            source = source.replace("fn read_u32_field(bytes: &[u8]) -> Option<u32> {\n    bytes.get(..4).map(LittleEndian::read_u32)\n}\n\n", "")
            for kind in ("Outer", "Inner"):
                source = source.replace(f"read_u32_field(entry_buffer).ok_or(Kdbx4{kind}HeaderError::UnexpectedEof)?", "LittleEndian::read_u32(entry_buffer)")
            marker = "impl Database {\n    /// Inspect the untrusted KDBX4 KDF configuration without deriving a key."
            if marker in source:
                start = source.index(marker)
                end = source.index("/// Open, decrypt and parse", start)
                source = source[:start] + source[end:]
            self.assertEqual(hashlib.sha256(source.encode()).hexdigest(), "7b44f1b0952dbccacd40da33b660c7c6237415f4a7b365de863e29c5b206412e")
            path.write_text(source)
            self.apply_and_check(destination)

    def test_unknown_header_source_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-kdf-drift-") as directory:
            destination = self.fixture(directory)
            path = destination / "keepass" / RELATIVE
            path.write_text(path.read_text() + "\n// unexpected source drift\n")
            with patch.object(vendor, "VENDOR", destination), self.assertRaises(RuntimeError):
                vendor.patch_keepass()

    def test_wrong_keepass_version_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-kdf-version-") as directory:
            destination = self.fixture(directory)
            path = destination / "keepass/Cargo.toml"
            source = path.read_text()
            self.assertIn('version = "0.13.25"', source)
            path.write_text(source.replace('version = "0.13.25"', 'version = "0.13.26"', 1))
            with patch.object(vendor, "VENDOR", destination), self.assertRaises(RuntimeError):
                vendor.patch_keepass()


if __name__ == "__main__":
    unittest.main()

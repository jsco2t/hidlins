#!/usr/bin/env python3
"""Replay and closed-context checks for maintained numeric parser bounds."""

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
spec = importlib.util.spec_from_file_location("hidlins_bounds_vendor", ROOT / "tools/dev/vendor.py")
if spec is None or spec.loader is None:
    raise RuntimeError("cannot load maintained vendor patches")
vendor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vendor)
HEADER = "src/format/kdbx4/parse.rs"
DICTIONARY = "src/format/variant_dictionary.rs"
READ_U32 = """fn read_u32_field(bytes: &[u8]) -> Option<u32> {
    bytes.get(..4).map(LittleEndian::read_u32)
}

"""
WIDTHS = """            let width = match value_type {
                U32_TYPE_ID | I32_TYPE_ID => 4,
                U64_TYPE_ID | I64_TYPE_ID => 8,
                _ => 0,
            };
            if value_buffer.len() < width {
                return Err(VariantDictionaryError::UnexpectedEof);
            }

"""


class HeaderBoundsPatchTests(unittest.TestCase):
    def fixture(self, directory: str) -> Path:
        destination = Path(directory)
        shutil.copytree(ROOT / "vendor/keepass", destination / "keepass")
        return destination

    def apply_and_check(self, destination: Path) -> None:
        with patch.object(vendor, "VENDOR", destination):
            vendor.patch_keepass()
        source = (destination / "keepass" / HEADER).read_text()
        self.assertIn(READ_U32, source)
        for kind in ("Outer", "Inner"):
            self.assertIn(f"read_u32_field(entry_buffer).ok_or(Kdbx4{kind}HeaderError::UnexpectedEof)?", source)
        self.assertIn(WIDTHS, (destination / "keepass" / DICTIONARY).read_text())
        checksum = destination / "keepass/.cargo-checksum.json"
        values = json.loads(checksum.read_text())["files"]
        for relative in (HEADER, DICTIONARY):
            self.assertEqual(values[relative], hashlib.sha256((destination / "keepass" / relative).read_bytes()).hexdigest())
        before = checksum.read_bytes()
        with patch.object(vendor, "VENDOR", destination):
            vendor.patch_keepass()
        self.assertEqual(checksum.read_bytes(), before)

    def test_committed_numeric_bounds_are_complete_and_idempotent(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-bounds-replay-") as directory:
            self.apply_and_check(self.fixture(directory))

    def test_prior_exact_parser_sources_replay(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-bounds-prior-") as directory:
            destination = self.fixture(directory)
            path = destination / "keepass" / HEADER
            source = path.read_text().replace(READ_U32, "")
            for kind in ("Outer", "Inner"):
                source = source.replace(f"read_u32_field(entry_buffer).ok_or(Kdbx4{kind}HeaderError::UnexpectedEof)?", "LittleEndian::read_u32(entry_buffer)")
            self.assertEqual(hashlib.sha256(source.encode()).hexdigest(), "47d33e7e51262f283f4b01458ec3bc41bd5fbdc95c58f61193a846bbc4606b90")
            path.write_text(source)
            path = destination / "keepass" / DICTIONARY
            path.write_text(path.read_text().replace(WIDTHS, ""))
            self.apply_and_check(destination)

    def test_unknown_parser_drift_is_rejected(self) -> None:
        for relative in (HEADER, DICTIONARY):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory(prefix="hidlins-bounds-drift-") as directory:
                destination = self.fixture(directory)
                path = destination / "keepass" / relative
                path.write_text(path.read_text() + "\n// unexpected source drift\n")
                with patch.object(vendor, "VENDOR", destination), self.assertRaises(RuntimeError):
                    vendor.patch_keepass()

    def test_wrong_keepass_version_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="hidlins-bounds-version-") as directory:
            destination = self.fixture(directory)
            path = destination / "keepass/Cargo.toml"
            source = path.read_text()
            self.assertIn('version = "0.13.25"', source)
            path.write_text(source.replace('version = "0.13.25"', 'version = "0.13.26"', 1))
            with patch.object(vendor, "VENDOR", destination), self.assertRaises(RuntimeError):
                vendor.patch_keepass()


if __name__ == "__main__":
    unittest.main()

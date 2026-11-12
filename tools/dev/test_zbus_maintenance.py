#!/usr/bin/env python3
"""Pin the maintained D-Bus integration's existing local-codec/patch contract."""
from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('hidlins_vendor_dbus', ROOT / 'tools/dev/vendor.py')
assert spec is not None and spec.loader is not None
vendor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(vendor)
CRATE = ROOT / 'vendor/zbus'

LOWER_HEX = '''geteuid()
                    .as_raw()
                    .to_string()
                    .bytes()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()'''


def files(directory: Path) -> dict[str, str]:
    return {str(p.relative_to(directory)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in directory.rglob('*') if p.is_file() and p.name != '.cargo-checksum.json'}


def restore_pristine(crate: Path) -> None:
    version = tomllib.loads((crate / 'Cargo.toml').read_text())['package']['version']
    if version not in ('5.15.0', '6.2.1'):
        raise AssertionError(f'Unaudited zbus fixture version: {version}')
    normalized = crate / 'Cargo.toml'
    text = normalized.read_text()
    declaration = '[dependencies.hex]\nversion = "0.4.3"\n'
    if version == '6.2.1':
        declaration += 'optional = true\n'
    assert '[dependencies.hex]' not in text
    text = text.replace('\n[dependencies.', '\n' + declaration + '\n[dependencies.', 1)
    normalized.write_text(text)
    original = crate / 'Cargo.toml.orig'
    text = original.read_text()
    declaration = 'hex.workspace = true\n' if version == '5.15.0' else 'hex = { workspace = true, optional = true }\n'
    assert '[dependencies]\n' in text
    original.write_text(text.replace('[dependencies]\n', '[dependencies]\n' + declaration, 1))
    if version == '6.2.1':
        for manifest in (normalized, original):
            text = manifest.read_text()
            assert '    "dep:hex",\n' not in text
            assert text.count('    "dep:ordered-stream",\n') == 1
            manifest.write_text(text.replace('    "dep:ordered-stream",\n', '    "dep:hex",\n    "dep:ordered-stream",\n', 1))
    module = crate / 'src/connection/handshake/mod.rs'
    text = module.read_text()
    assert text.count('mod hex;\n') == 1
    module.write_text(text.replace('mod hex;\n', '', 1))
    command = crate / 'src/connection/handshake/command.rs'
    text = command.read_text()
    assert text.count('\nuse super::hex;\n') == 1
    text = text.replace('\nuse super::hex;\n', '', 1)
    if version == '5.15.0':
        assert text.count('hex::DecodeError') == 2
        text = text.replace('hex::DecodeError', 'hex::FromHexError')
    command.write_text(text)
    (crate / 'src/connection/handshake/hex.rs').unlink()
    issue = crate / 'tests/issue/issue_813.rs'
    text = issue.read_text()
    assert text.count(LOWER_HEX) == 1
    issue.write_text(text.replace(LOWER_HEX, 'hex::encode(geteuid().as_raw().to_string())', 1))
    vendor.refresh_checksum(crate)


class ZbusMaintenanceTests(unittest.TestCase):
    def test_codec_behavior_including_all_bytes_and_error_positions(self) -> None:
        source = CRATE / 'src/connection/handshake/hex.rs'
        with tempfile.TemporaryDirectory(prefix='hidlins-dbus-codec-') as temporary:
            directory = Path(temporary)
            harness = directory / 'codec.rs'
            harness.write_text('#[path = ' + json.dumps(str(source)) + ']\nmod codec;\n' + r'''
#[test]
fn all_bytes_round_trip_in_both_cases() {
    let bytes: Vec<u8> = (0..=255).collect();
    let encoded = codec::encode(&bytes);
    assert_eq!(encoded.len(), 512);
    assert!(encoded.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    assert_eq!(codec::decode(&encoded).unwrap(), bytes);
    assert_eq!(codec::decode(encoded.to_ascii_uppercase()).unwrap(), bytes);
}
#[test]
fn empty_input_is_valid() {
    assert_eq!(codec::encode([]), "");
    assert_eq!(codec::decode("").unwrap(), Vec::<u8>::new());
}
#[test]
fn malformed_input_retains_exact_error_and_index() {
    use codec::DecodeError::{InvalidCharacter, OddLength};
    assert_eq!(codec::decode("a"), Err(OddLength));
    assert_eq!(OddLength.to_string(), "odd number of hexadecimal digits");
    for (input, byte, index) in [("g0", b'g', 0), ("0g", b'g', 1), ("00g0", b'g', 2), ("000g", b'g', 3)] {
        let error = InvalidCharacter { byte, index };
        assert_eq!(codec::decode(input), Err(error.clone()));
        assert_eq!(error.to_string(), format!("invalid hexadecimal character 0x{byte:02x} at index {index}"));
    }
    assert_eq!(codec::decode([0xff, b'0']), Err(InvalidCharacter { byte: 0xff, index: 0 }));
}
''')
            binary = directory / 'codec-tests'
            subprocess.run(['rustc', '--test', '--edition=2021', '-D', 'warnings', str(harness), '-o', str(binary)], cwd=ROOT, check=True)
            subprocess.run([str(binary), '--test-threads=1'], cwd=ROOT, check=True)

    def test_pristine_patch_replay_preserves_all_other_bytes_and_is_idempotent(self) -> None:
        with tempfile.TemporaryDirectory(prefix='hidlins-dbus-replay-') as temporary:
            directory = Path(temporary)
            crate = directory / 'zbus'
            shutil.copytree(CRATE, crate)
            expected = files(crate)
            restore_pristine(crate)
            pristine = files(crate)
            changed = {name for name in pristine.keys() | expected.keys() if pristine.get(name) != expected.get(name)}
            self.assertEqual(changed, {'Cargo.toml', 'Cargo.toml.orig', 'src/connection/handshake/mod.rs', 'src/connection/handshake/command.rs', 'src/connection/handshake/hex.rs', 'tests/issue/issue_813.rs'})
            with patch.object(vendor, 'VENDOR', directory):
                vendor.patch_zbus()
                self.assertEqual(files(crate), expected)
                checksum = (crate / '.cargo-checksum.json').read_bytes()
                vendor.patch_zbus()
                self.assertEqual(files(crate), expected)
                self.assertEqual((crate / '.cargo-checksum.json').read_bytes(), checksum)
            self.assertEqual((crate / 'src/connection/handshake/hex.rs').read_text(), vendor.HEX_CODEC)
            checksum = json.loads((crate / '.cargo-checksum.json').read_text())
            self.assertEqual(checksum['files'], expected)
            self.assertNotIn('[dependencies.hex]', (crate / 'Cargo.toml').read_text())
            self.assertNotIn('"dep:hex"', (crate / 'Cargo.toml').read_text())

    def test_unreviewed_patch_context_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix='hidlins-dbus-context-') as temporary:
            directory = Path(temporary)
            crate = directory / 'zbus'
            shutil.copytree(CRATE, crate)
            restore_pristine(crate)
            command = crate / 'src/connection/handshake/command.rs'
            command.write_text(command.read_text().replace('// The plain-text', '// Changed context', 1))
            with patch.object(vendor, 'VENDOR', directory), self.assertRaises(RuntimeError):
                vendor.patch_zbus()

    def test_unreviewed_existing_codec_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory(prefix='hidlins-dbus-codec-drift-') as temporary:
            directory = Path(temporary)
            crate = directory / 'zbus'
            shutil.copytree(CRATE, crate)
            source = crate / 'src/connection/handshake/hex.rs'
            source.write_text(source.read_text() + '// unexpected codec drift\n')
            with patch.object(vendor, 'VENDOR', directory), self.assertRaises(RuntimeError):
                vendor.patch_zbus()


if __name__ == '__main__':
    unittest.main(verbosity=2)

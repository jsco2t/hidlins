#!/usr/bin/env python3
"""Re-vendor Cargo dependencies and reapply Hidlins' narrow source patches.

Cargo verifies vendored files against each crate's `.cargo-checksum.json`.  The
patches remove the general-purpose `hex` crate where upstream packages only
need tiny, well-specified encode/decode operations, remove Snow's unselected
Ring feature edge from the lock/vendor graph, and add the minimal Keepass API
needed to repair historical attachment pool references safely. Keeping the
patch application here makes `make vendor` reproducible instead of relying on
manual edits under `vendor/`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib


ROOT = Path(__file__).resolve().parents[2]
VENDOR = ROOT / "vendor"

# Whole-file digests turn the context replacements below into a closed audit
# boundary. Any upstream source drift, partial patch, or unaudited edit fails
# before Cargo is allowed to consume Snow.
SNOW_PATCHED_SHA256 = {
    "Cargo.toml": "c972cd82d6c1eafc84e8e3cd0bb9aa289d76617dbc44bedb08c00b03813116e7",
    "Cargo.toml.orig": "9d54d07e0f1750b11c4c319089fec1187f7786855610319e73d0ed71e9f3b294",
    "src/builder.rs": "f968573c898ae39d2aa47460b59943cf773795756b7a804dc4028f14cd11cc55",
    "src/types.rs": "42d3c0d258279b60057f45c06d385e559edd173988f62f4a38f23ad4c58f7099",
    "src/cipherstate.rs": "58dc28c667e522e1d066014c773ef7590f4fe626515b5f07f4a260c070374fbe",
    "src/symmetricstate.rs": "595dd6ac6579b164df5fffa5e2ca787681580c8dfdcd6e4aee7df50395014885",
    "src/resolvers/default.rs": "c895c06a824cb3bb91afa61923d5745af60c6351b14b8d6d96165eb8562e3c4b",
    "src/handshakestate.rs": "b90c570b8b85c8dbb5dbcdee79b31f91cf01d03ede92e460cb17965e0544a2f4",
    "src/transportstate.rs": "4e4b09c43fcdf73b33f8a0f76a4a50c4e44d42985bde436d3bea0b8df413a892",
    "src/stateless_transportstate.rs": "a0c2595ca6ca39787340ce59116d3339b31cd4f3cd0a29f1caa367bd02b52c18",
}


# Exact reviewed RustCrypto manifest and modified-source boundary.
NOISE_HASH_PATCHED_SHA256 = {
    "sha2": {
        "Cargo.toml": "bb187fe9b788b66dc91c717ace2a41b5da5518672a302696ab79bf4619d510a9",
        "Cargo.toml.orig": "a89bc798ce45a942b765694d92169e86b551bf03ea69c8c445739ae147c0cd70",
        "src/block_api.rs": "1f45c294429a4dd76eb0256950941631da8639129d6d31e383c81953c2d7fc42"
    },
    "digest": {
        "Cargo.toml": "5fe7b1d9713b7dd9e2f5d3b4d2a3eb9105f0b0fd7798fc91efbe0073f55bcacd",
        "Cargo.toml.orig": "7a9e0927d6d83aaaaf0da7accc3f9360141179b3593ad3654c195996d14285a9",
        "src/block_api/ct_variable.rs": "8831dc4588b2a56dfc9183d12befc2e09d765b97d823ce782b626a4cb5ee874c"
    },
    "block-buffer": {
        "Cargo.toml": "688149b168d0a66185047ae55426fa32f042f1215837768a5bebe4f727ea6368",
        "Cargo.toml.orig": "675f8563d8c0abcd874447e3453fb3e0542691203f3a25d1e958e778a438b2d1",
        "src/lib.rs": "6c95a5cc113d6f95cd6992a34a53ad98cfd2f568675cc8332191ce4bcb9af98d"
    }
}


KEEPASS_HEX = """use std::fmt;

/// Error returned when a hexadecimal key contains malformed input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HexDecodeError {
    /// Hexadecimal input must contain two characters per decoded byte.
    OddLength,
    /// A byte outside the ASCII hexadecimal ranges was encountered.
    InvalidCharacter { byte: u8, index: usize },
}

impl fmt::Display for HexDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OddLength => formatter.write_str("odd number of hexadecimal digits"),
            Self::InvalidCharacter { byte, index } => write!(
                formatter,
                "invalid hexadecimal character 0x{byte:02x} at index {index}"
            ),
        }
    }
}

impl std::error::Error for HexDecodeError {}

pub(super) fn decode(input: impl AsRef<[u8]>) -> Result<Vec<u8>, HexDecodeError> {
    let input = input.as_ref();
    if input.len() % 2 != 0 {
        return Err(HexDecodeError::OddLength);
    }

    let mut decoded = Vec::with_capacity(input.len() / 2);
    for (pair_index, pair) in input.chunks_exact(2).enumerate() {
        let index = pair_index * 2;
        let high = nibble(pair[0]).ok_or(HexDecodeError::InvalidCharacter {
            byte: pair[0],
            index,
        })?;
        let low = nibble(pair[1]).ok_or(HexDecodeError::InvalidCharacter {
            byte: pair[1],
            index: index + 1,
        })?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

const fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode, HexDecodeError};

    #[test]
    fn decodes_mixed_case_and_all_nibbles() {
        assert_eq!(
            decode(b"0123456789aBcDeF").expect("valid hexadecimal"),
            [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]
        );
    }

    #[test]
    fn rejects_odd_length() {
        assert_eq!(decode(b"0"), Err(HexDecodeError::OddLength));
    }

    #[test]
    fn reports_invalid_character_index() {
        assert_eq!(
            decode(b"00xz"),
            Err(HexDecodeError::InvalidCharacter {
                byte: b'x',
                index: 2,
            })
        );
    }
}
"""


HEX_CODEC = """use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DecodeError {
    OddLength,
    InvalidCharacter { byte: u8, index: usize },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OddLength => formatter.write_str("odd number of hexadecimal digits"),
            Self::InvalidCharacter { byte, index } => write!(
                formatter,
                "invalid hexadecimal character 0x{byte:02x} at index {index}"
            ),
        }
    }
}

pub(crate) fn encode(input: impl AsRef<[u8]>) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let input = input.as_ref();
    let mut output = String::with_capacity(input.len().saturating_mul(2));
    for byte in input {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

pub(crate) fn decode(input: impl AsRef<[u8]>) -> Result<Vec<u8>, DecodeError> {
    let input = input.as_ref();
    if input.len() % 2 != 0 {
        return Err(DecodeError::OddLength);
    }
    input
        .chunks_exact(2)
        .enumerate()
        .map(|(pair_index, pair)| {
            let index = pair_index * 2;
            let high = nibble(pair[0]).ok_or(DecodeError::InvalidCharacter {
                byte: pair[0],
                index,
            })?;
            let low = nibble(pair[1]).ok_or(DecodeError::InvalidCharacter {
                byte: pair[1],
                index: index + 1,
            })?;
            Ok((high << 4) | low)
        })
        .collect()
}

const fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{decode, encode, DecodeError};

    #[test]
    fn codec_covers_all_nibbles_and_mixed_case() {
        assert_eq!(encode([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]), "0123456789abcdef");
        assert_eq!(decode("0123456789aBcDeF").unwrap(), [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
    }

    #[test]
    fn decoder_rejects_malformed_input() {
        assert_eq!(decode("0"), Err(DecodeError::OddLength));
        assert_eq!(
            decode("00xz"),
            Err(DecodeError::InvalidCharacter { byte: b'x', index: 2 })
        );
    }
}
"""



def package_version(crate_dir: Path) -> str:
    manifest = (crate_dir / "Cargo.toml").read_text()
    match = re.search(r"(?ms)^\[package\].*?^version = \"([^\"]+)\"", manifest)
    if match is None:
        raise RuntimeError(f"cannot determine package version from {crate_dir / 'Cargo.toml'}")
    return match.group(1)


def require_version(crate_dir: Path, expected: str) -> None:
    actual = package_version(crate_dir)
    if actual != expected:
        raise RuntimeError(
            f"refusing to patch {crate_dir.name} {actual}; patch is audited for {expected}"
        )


def replace_once(path: Path, before: str, after: str) -> None:
    text = path.read_text()
    if not after and before not in text:
        return
    if text.count(after) == 1:
        return
    if text.count(before) != 1:
        context = before.splitlines()[0][:80]
        raise RuntimeError(
            f"expected exactly one audited patch context in {path}: {context!r}"
        )
    path.write_text(text.replace(before, after, 1))


def replace_exact_count(path: Path, before: str, after: str, count: int) -> None:
    """Replace an audited context that intentionally occurs `count` times."""
    text = path.read_text()
    if text.count(after) == count:
        return
    if text.count(before) != count:
        raise RuntimeError(f"expected exactly {count} audited patch contexts in {path}")
    path.write_text(text.replace(before, after))


def replace_one_of(path: Path, before: tuple[str, ...], after: str) -> None:
    """Upgrade one exact upstream/prior-patch context, retaining closed replay."""
    text = path.read_text()
    if text.count(after) == 1:
        return
    matching = [candidate for candidate in before if candidate in text]
    if len(matching) != 1 or text.count(matching[0]) != 1:
        raise RuntimeError(f"expected one reviewed patch generation in {path}")
    path.write_text(text.replace(matching[0], after, 1))


def insert_once(path: Path, marker: str, insertion: str) -> None:
    """Insert audited text exactly once before a stable upstream marker."""
    text = path.read_text()
    insertion_count = text.count(insertion)
    if insertion_count == 1:
        return
    if insertion_count != 0:
        raise RuntimeError(f"audited insertion appears more than once in {path}")
    if text.count(marker) != 1:
        raise RuntimeError(f"expected exactly one audited insertion marker in {path}")
    path.write_text(text.replace(marker, insertion + marker, 1))


def ensure_file(path: Path, expected: str) -> None:
    if path.exists():
        if path.read_text() != expected:
            raise RuntimeError(f"refusing to overwrite unexpected patched file {path}")
        return
    path.write_text(expected)


def refresh_checksum(crate_dir: Path) -> None:
    checksum_path = crate_dir / ".cargo-checksum.json"
    checksum = json.loads(checksum_path.read_text())
    files: dict[str, str] = {}
    for path in sorted(crate_dir.rglob("*")):
        if path.is_file() and path != checksum_path:
            relative = path.relative_to(crate_dir).as_posix()
            files[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    checksum_path.write_text(
        json.dumps({"files": files, "package": checksum["package"]}, separators=(",", ":"))
    )


def verify_snow_patch(crate_dir: Path) -> None:
    """Reject every Snow tree outside the exact reviewed patched source set."""
    require_version(crate_dir, "0.10.0")
    for relative, expected in SNOW_PATCHED_SHA256.items():
        path = crate_dir / relative
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != expected:
            raise RuntimeError(
                f"Snow patch digest mismatch for {relative}: expected {expected}, got {actual}"
            )


def verify_noise_hash_patch() -> None:
    """Reject partial erasure patches or unreviewed selected RustCrypto sources."""
    for name, version in (("sha2", "0.11.0"), ("digest", "0.11.3"), ("block-buffer", "0.12.0")):
        crate_dir = VENDOR / name
        require_version(crate_dir, version)
        for relative, expected in NOISE_HASH_PATCHED_SHA256[name].items():
            actual = hashlib.sha256((crate_dir / relative).read_bytes()).hexdigest()
            if actual != expected:
                raise RuntimeError(f"Noise hash patch digest mismatch for {name}/{relative}")


def patch_noise_hash_dependencies() -> None:
    """Use upstream owned-state erasure and protect its remaining output copies."""
    for name, version in (("sha2", "0.11.0"), ("digest", "0.11.3"), ("block-buffer", "0.12.0")):
        require_version(VENDOR / name, version)
    digest = VENDOR / "digest"
    replace_once(
        digest / "Cargo.toml",
        '    "block-buffer?/zeroize",\n',
        '    "block-buffer?/zeroize",\n    "common/zeroize",\n',
    )
    replace_once(
        digest / "Cargo.toml.orig",
        'zeroize = ["dep:zeroize", "block-buffer?/zeroize"]',
        'zeroize = ["dep:zeroize", "block-buffer?/zeroize", "common/zeroize"]',
    )
    replace_once(
        digest / "src/block_api/ct_variable.rs",
        '        let mut full_res = Default::default();',
        '        let full_res = Array::<u8, T::OutputSize>::default();\n'
        '        #[cfg(feature = "zeroize")]\n'
        '        let mut full_res = zeroize::Zeroizing::new(full_res);\n'
        '        #[cfg(not(feature = "zeroize"))]\n'
        '        let mut full_res = full_res;',
    )
    buffer = VENDOR / "block-buffer"
    features = '[features]\nzeroize = ["dep:zeroize", "hybrid-array/zeroize"]\n\n'
    replace_once(buffer / "Cargo.toml", '[lib]\n', features + '[lib]\n')
    replace_once(buffer / "Cargo.toml.orig", '[dev-dependencies]\n', features + '[dev-dependencies]\n')
    replace_once(
        buffer / "src/lib.rs",
        '        let mut buf = self.pad_with_zeros();',
        '        let buf = self.pad_with_zeros();\n'
        '        #[cfg(feature = "zeroize")]\n'
        '        let mut buf = zeroize::Zeroizing::new(buf);\n'
        '        #[cfg(not(feature = "zeroize"))]\n'
        '        let mut buf = buf;',
    )
    replace_exact_count(
        VENDOR / "sha2/src/block_api.rs",
        '&[b.0]',
        'Array::cast_slice_to_core(core::slice::from_ref(b))',
        2,
    )
    verify_noise_hash_patch()
    for name in NOISE_HASH_PATCHED_SHA256:
        refresh_checksum(VENDOR / name)


def patch_quick_xml() -> None:
    crate_dir = VENDOR / "quick-xml"
    require_version(crate_dir, "0.41.0")
    source = crate_dir / "src/de/map.rs"
    # Respect explicitly requested text content while retaining indentation
    # skipping for element maps. Preserve this pinned source's CRLF bytes.
    before = b"        self.de.skip_whitespaces()\r\n"
    after = (
        b"        if self.has_text_field {\r\n"
        b"            Ok(())\r\n"
        b"        } else {\r\n"
        b"            self.de.skip_whitespaces()\r\n"
        b"        }\r\n"
    )
    data = source.read_bytes()
    if data.count(after) != 1:
        if data.count(before) != 1:
            raise RuntimeError(f"expected exactly one audited text-map context in {source}")
        source.write_bytes(data.replace(before, after, 1))
    refresh_checksum(crate_dir)


KEEPASS_KDF_HEADER_ORIGINAL_SHA256 = "2bbeb2534f63800c5be1264281146b9f000a31498dab8c91d1e524d64babc121"
KEEPASS_KDF_HEADER_PATCHED_SHA256 = "499df7f1ff5487a513ce6e5c1a0e31bfd2e8b8f228fa60f3e533a1f3c2f91991"
KEEPASS_KDF_HEADER_API = """impl Database {
    /// Inspect the untrusted KDBX4 KDF configuration without deriving a key.
    ///
    /// Untrusted metadata is suitable only for resource admission. Callers
    /// must still open the database and authenticate its complete contents.
    pub fn kdbx4_kdf_config(data: &[u8]) -> Result<KdfConfig, DatabaseOpenError> {
        if !matches!(DatabaseVersion::parse(data)?, DatabaseVersion::KDB4(_)) {
            return Err(DatabaseOpenError::UnsupportedVersion);
        }
        let (header, _) = parse_outer_header(data)
            .map_err(|error| DatabaseOpenError::Format(DatabaseFormatError::Kdbx4(Kdbx4OpenError::OuterHeader(error))))?;
        Ok(header.kdf_config)
    }
}

"""


KEEPASS_HEADER_SOURCE_STATES = ('7b44f1b0952dbccacd40da33b660c7c6237415f4a7b365de863e29c5b206412e', '47d33e7e51262f283f4b01458ec3bc41bd5fbdc95c58f61193a846bbc4606b90', '2bbeb2534f63800c5be1264281146b9f000a31498dab8c91d1e524d64babc121', '499df7f1ff5487a513ce6e5c1a0e31bfd2e8b8f228fa60f3e533a1f3c2f91991')
KEEPASS_DICTIONARY_SOURCE_STATES = ('afea03a11895893d7306a8a0c9d0c4eedab86bfc307d3d64368e19ed9866f5b0', '31e54efa657f70a91b65401a697de02cafae55b50db5b3bdc1905204c376e115')
KEEPASS_READ_U32_FIELD = """fn read_u32_field(bytes: &[u8]) -> Option<u32> {
    bytes.get(..4).map(LittleEndian::read_u32)
}

"""
KEEPASS_DICTIONARY_WIDTHS = """            let width = match value_type {
                U32_TYPE_ID | I32_TYPE_ID => 4,
                U64_TYPE_ID | I64_TYPE_ID => 8,
                _ => 0,
            };
            if value_buffer.len() < width {
                return Err(VariantDictionaryError::UnexpectedEof);
            }

"""


def patch_keepass_header_bounds(crate_dir: Path) -> None:
    require_version(crate_dir, "0.13.25")
    header = crate_dir / "src/format/kdbx4/parse.rs"
    dictionary = crate_dir / "src/format/variant_dictionary.rs"
    if hashlib.sha256(header.read_bytes()).hexdigest() not in KEEPASS_HEADER_SOURCE_STATES:
        raise RuntimeError("refusing unexpected KeePass header bounds source")
    if hashlib.sha256(dictionary.read_bytes()).hexdigest() not in KEEPASS_DICTIONARY_SOURCE_STATES:
        raise RuntimeError("refusing unexpected KeePass dictionary bounds source")
    replace_once(header, "fn parse_outer_header(", KEEPASS_READ_U32_FIELD + "fn parse_outer_header(")
    for config, kind in (("CompressionConfig", "Outer"), ("InnerCipherConfig", "Inner")):
        replace_once(
            header,
            config + "::try_from(LittleEndian::read_u32(entry_buffer))",
            config + "::try_from(read_u32_field(entry_buffer).ok_or(Kdbx4" + kind + "HeaderError::UnexpectedEof)?)",
        )
    replace_once(dictionary, "            let value = match value_type {", KEEPASS_DICTIONARY_WIDTHS + "            let value = match value_type {")
    if hashlib.sha256(header.read_bytes()).hexdigest() not in (KEEPASS_KDF_HEADER_ORIGINAL_SHA256, KEEPASS_KDF_HEADER_PATCHED_SHA256):
        raise RuntimeError("KeePass header bounds patch hash mismatch")
    if hashlib.sha256(dictionary.read_bytes()).hexdigest() != KEEPASS_DICTIONARY_SOURCE_STATES[1]:
        raise RuntimeError("KeePass dictionary bounds patch hash mismatch")


def patch_keepass_kdf_header(crate_dir: Path) -> None:
    require_version(crate_dir, "0.13.25")
    path = crate_dir / "src/format/kdbx4/parse.rs"
    source = path.read_text()
    actual = hashlib.sha256(source.encode()).hexdigest()
    if actual == KEEPASS_KDF_HEADER_PATCHED_SHA256:
        return
    if actual != KEEPASS_KDF_HEADER_ORIGINAL_SHA256:
        raise RuntimeError("refusing unexpected KeePass KDF header source")
    anchor = "/// Open, decrypt and parse"
    if source.count(anchor) != 1:
        raise RuntimeError("KeePass KDF header insertion context changed")
    source = source.replace(anchor, KEEPASS_KDF_HEADER_API + anchor, 1)
    if hashlib.sha256(source.encode()).hexdigest() != KEEPASS_KDF_HEADER_PATCHED_SHA256:
        raise RuntimeError("KeePass KDF header patch hash mismatch")
    path.write_text(source)


def patch_keepass() -> None:
    crate_dir = VENDOR / "keepass"
    require_version(crate_dir, "0.13.25")
    patch_keepass_header_bounds(crate_dir)
    patch_keepass_kdf_header(crate_dir)
    replace_once(
        crate_dir / "src/db/types/value.rs",
        "use zeroize::Zeroize;",
        "use zeroize::{Zeroize, ZeroizeOnDrop};",
    )
    replace_once(
        crate_dir / "src/db/types/value.rs",
        "impl<T: Zeroize + Display> std::fmt::Display for Value<T> {",
        """// Hidlins: wipe owned unprotected values without changing KDBX protection flags.
impl<T: Zeroize> Drop for Value<T> {
    fn drop(&mut self) {
        if let Value::Unprotected(data) = self {
            data.zeroize();
        }
        // Protected values are wiped by SecretBox's own destructor.
    }
}

impl<T: Zeroize> ZeroizeOnDrop for Value<T> {}

impl<T: Zeroize + Display> std::fmt::Display for Value<T> {""",
    )
    # Entry strings are literal user data, including whitespace-only text.
    # Keep cs_opt_string's empty-metadata handling for its other callers.
    replace_once(
        crate_dir / "src/format/xml_db/entry.rs",
        '''pub struct StringValue {
    #[serde(default, rename = "@Protected", with = "cs_bool")]
    protected: bool,

    #[serde(
        default,
        rename = "$value",
        with = "cs_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    value: Option<String>,
}''',
        '''pub struct StringValue {
    #[serde(default, rename = "@Protected", with = "cs_bool")]
    protected: bool,

    #[serde(
        default,
        rename = "$text",
        skip_serializing_if = "Option::is_none"
    )]
    value: Option<String>,
}''',
    )
    replace_once(
        crate_dir / "Cargo.toml",
        '[dependencies.hex]\nversion = "0.4"\n\n',
        "",
    )
    replace_once(
        crate_dir / "Cargo.toml.orig",
        'hex = { version = "0.4" }\n',
        "",
    )
    replace_once(
        crate_dir / "src/key/mod.rs",
        "use crate::crypt::calculate_sha256;\n\npub type KeyElement",
        "use crate::crypt::calculate_sha256;\n\nmod hex;\n\npub use hex::HexDecodeError;\n\npub type KeyElement",
    )
    replace_once(
        crate_dir / "src/key/yubikey.rs",
        "use cipher::InvalidLength;\nuse hex::FromHexError;\nuse thiserror::Error;",
        "use cipher::InvalidLength;\nuse thiserror::Error;",
    )
    replace_once(
        crate_dir / "src/key/yubikey.rs",
        "use crate::key::KeyElement;\n\n/// Represents",
        "use crate::key::KeyElement;\n\nuse super::hex::{decode, HexDecodeError};\n\n/// Represents",
    )
    replace_once(
        crate_dir / "src/key/yubikey.rs",
        "let secret_bytes = hex::decode(secret)?;",
        "let secret_bytes = decode(secret)?;",
    )
    replace_once(
        crate_dir / "src/key/yubikey.rs",
        "Hex(#[from] FromHexError),",
        "Hex(#[from] HexDecodeError),",
    )
    replace_once(
        crate_dir / "src/db/types/entry.rs",
        """    pub fn id(&self) -> EntryId {
        self.id
    }

    /// Get the icon of this entry, if it exists""",
        """    pub fn id(&self) -> EntryId {
        self.id
    }

    /// Get the identifier of the group containing this entry.
    pub fn parent_id(&self) -> GroupId {
        self.parent
    }

    /// Get the previous parent-group identifier, if one is recorded.
    pub fn previous_parent_id(&self) -> Option<GroupId> {
        self.previous_parent_group
    }

    /// Get the icon of this entry, if it exists""",
    )
    replace_once(
        crate_dir / "src/db/types/entry.rs",
        """    pub fn attachment(&self, id: AttachmentId) -> Option<AttachmentRef<'_>> {
        self.attachments
            .values()
            .find(|&attachment_id| *attachment_id == id)
            .cloned()
            .map(move |attachment_id| AttachmentRef::new(self.database, attachment_id))
    }

    /// Get a reference to an attachment by name, if it exists.
    pub fn attachment_by_name(&self, name: &str) -> Option<AttachmentRef<'_>> {
        self.attachments
            .get(name)
            .cloned()
            .map(move |attachment_id| AttachmentRef::new(self.database, attachment_id))
    }

    /// Get an iterator over the attachments of this entry.
    pub fn attachments(&self) -> impl Iterator<Item = AttachmentRef<'_>> {
        self.attachments
            .values()
            .cloned()
            .map(move |attachment_id| AttachmentRef::new(self.database, attachment_id))
    }

    /// Get an iterator over the (name, attachment) pairs of this entry.
    ///
    /// Useful when callers need both the attachment's filename (the key under
    /// which it is stored on the entry) and its data, since [`AttachmentRef`]
    /// itself does not expose the per-entry name.
    pub fn attachments_named(&self) -> impl Iterator<Item = (&str, AttachmentRef<'_>)> {
        self.attachments.iter().map(move |(name, &attachment_id)| {
            (name.as_str(), AttachmentRef::new(self.database, attachment_id))
        })
    }""",
        """    pub fn attachment(&self, id: AttachmentId) -> Option<AttachmentRef<'_>> {
        self.attachments
            .values()
            .find(|&attachment_id| *attachment_id == id)
            .filter(|attachment_id| self.database.attachments.contains_key(attachment_id))
            .copied()
            .map(move |attachment_id| AttachmentRef::new(self.database, attachment_id))
    }

    /// Get a reference to an attachment by name, if it exists and resolves in
    /// this database's binary pool.
    pub fn attachment_by_name(&self, name: &str) -> Option<AttachmentRef<'_>> {
        self.attachments
            .get(name)
            .filter(|attachment_id| self.database.attachments.contains_key(attachment_id))
            .copied()
            .map(move |attachment_id| AttachmentRef::new(self.database, attachment_id))
    }

    /// Get an iterator over the attachments of this entry that resolve in this
    /// database's binary pool.
    pub fn attachments(&self) -> impl Iterator<Item = AttachmentRef<'_>> {
        self.attachments.values().filter_map(move |attachment_id| {
            self.database
                .attachments
                .contains_key(attachment_id)
                .then(|| AttachmentRef::new(self.database, *attachment_id))
        })
    }

    /// Get an iterator over every attachment name, including references that
    /// do not currently resolve in this database's binary pool.
    pub fn attachment_names(&self) -> impl Iterator<Item = &str> {
        self.attachments.keys().map(String::as_str)
    }

    /// Get an iterator over the resolved (name, attachment) pairs of this entry.
    ///
    /// Useful when callers need both the attachment's filename (the key under
    /// which it is stored on the entry) and its data, since [`AttachmentRef`]
    /// itself does not expose the per-entry name. Unresolved pool references
    /// are skipped instead of constructing an [`AttachmentRef`] that would
    /// panic when dereferenced.
    pub fn attachments_named(&self) -> impl Iterator<Item = (&str, AttachmentRef<'_>)> {
        self.attachments.iter().filter_map(move |(name, attachment_id)| {
            self.database.attachments.contains_key(attachment_id).then(|| {
                (name.as_str(), AttachmentRef::new(self.database, *attachment_id))
            })
        })
    }""",
    )
    replace_once(
        crate_dir / "src/db/types/entry.rs",
        "    pub(crate) fn historical(&mut self, index: usize) -> Option<EntryMut<'_>> {",
        "    pub fn historical(&mut self, index: usize) -> Option<EntryMut<'_>> {",
    )
    replace_once(
        crate_dir / "src/db/types/entry.rs",
        "let entries: HashSet<(EntryId, Option<usize>)> = vec![(self.id, None)].into_iter().collect();",
        """let entries: HashSet<(EntryId, Option<usize>)> =
            vec![(self.id, self.history_index)].into_iter().collect();""",
    )
    replace_once(
        crate_dir / "src/db/types/entry.rs",
        """        let id = self.id;

        // remove the attachment reference from this entry
        if let Some(attachment_id) = self.attachments.remove(name) {
            if let Some(mut attachment) = self.database.attachment_mut(attachment_id) {
                attachment.entries.retain(|&(entry_id, _)| entry_id != id);""",
        """        let id = self.id;
        let history_index = self.history_index;

        // remove the attachment reference from this entry
        if let Some(attachment_id) = self.attachments.remove(name) {
            if let Some(mut attachment) = self.database.attachment_mut(attachment_id) {
                attachment.entries.retain(|&(entry_id, entry_history_index)| {
                    !(entry_id == id && entry_history_index == history_index)
                });""",
    )
    replace_once(
        crate_dir / "src/db/types/entry.rs",
        """        let id = self.id;

        // remove the attachment reference from this entry
        let mut names_to_remove = Vec::new();""",
        """        let id = self.id;
        let history_index = self.history_index;

        // remove the attachment reference from this entry
        let mut names_to_remove = Vec::new();""",
    )
    replace_once(
        crate_dir / "src/db/types/entry.rs",
        """        if let Some(mut attachment) = self.database.attachment_mut(attachment_id) {
            attachment.entries.retain(|&(entry_id, _)| entry_id != id);

            // if this was the last entry referencing the attachment, remove it from the database""",
        """        if let Some(mut attachment) = self.database.attachment_mut(attachment_id) {
            attachment.entries.retain(|&(entry_id, entry_history_index)| {
                !(entry_id == id && entry_history_index == history_index)
            });

            // if this was the last entry referencing the attachment, remove it from the database""",
    )
    insert_once(
        crate_dir / "src/db/types/mod.rs",
        """    /// Get an immutable reference to the root group of the database.
    pub fn root(&self) -> GroupRef<'_> {""",
        """    /// Rebuild attachment-to-entry back-references from every current and
    /// historical entry's attachment map.
    ///
    /// KDBX stores entry-to-binary references, so the reverse index must be
    /// reconstructed after parsing or history replacement. Missing pool ids
    /// are ignored; callers can then repair those dangling references through
    /// the entry APIs without risking deletion of a binary still referenced by
    /// another current or historical version.
    pub fn rebuild_attachment_references(&mut self) {
        for attachment in self.attachments.values_mut() {
            attachment.entries.clear();
        }

        let mut references = Vec::new();
        for (entry_id, entry) in &self.entries {
            references.extend(
                entry
                    .attachments
                    .values()
                    .copied()
                    .map(|attachment_id| (attachment_id, *entry_id, None)),
            );
            if let Some(history) = &entry.history {
                for (index, historical) in history.entries.iter().enumerate() {
                    references.extend(
                        historical
                            .attachments
                            .values()
                            .copied()
                            .map(|attachment_id| (attachment_id, *entry_id, Some(index))),
                    );
                }
            }
        }

        for (attachment_id, entry_id, history_index) in references {
            if let Some(attachment) = self.attachments.get_mut(&attachment_id) {
                attachment.entries.insert((entry_id, history_index));
            }
        }
    }

""",
    )
    replace_once(
        crate_dir / "src/db/merge.rs",
        """        merge_icons(self, other, &mut log)?;
        merge_groups(self, other, &mut log)?;

        Ok(log)""",
        """        merge_icons(self, other, &mut log)?;
        merge_groups(self, other, &mut log)?;
        self.rebuild_attachment_references();

        Ok(log)""",
    )
    replace_once(
        crate_dir / "src/format/xml_db/mod.rs",
        """        db.attachments = attachments;
        db.custom_icons = custom_icons;

        // Re-populate CustomIcon back-reference sets.""",
        """        db.attachments = attachments;
        db.custom_icons = custom_icons;
        db.rebuild_attachment_references();

        // Re-populate CustomIcon back-reference sets.""",
    )
    replace_once(
        crate_dir / "src/db/types/entry.rs",
        """            entry.history.get_or_insert_default().add_entry(historical);
        }
    }
}""",
        """            entry.history.get_or_insert_default().add_entry(historical);
        }
        self.database.rebuild_attachment_references();
    }
}""",
    )
    replace_once(
        crate_dir / "src/db/merge.rs",
        """                return Err(MergeError::EntryModificationTimeNotUpdated(id));
            }
            continue;""",
        """                return Err(MergeError::EntryModificationTimeNotUpdated(id));
            }
            // Hidlins: equal current versions can carry different prior secrets.
            if dest_entry.history != source_history {
                let empty_history = History::default();
                dest_entry.history = Some(merge_history(
                    dest_entry.history.as_ref().unwrap_or(&empty_history),
                    source_history.as_ref().unwrap_or(&empty_history),
                    log,
                )?);
            }
            continue;""",
    )
    replace_once(
        crate_dir / "src/db/merge.rs",
        "db::{CustomIconId, Entry, EntryId, Group, GroupId, GroupRef, History, MoveGroupError, Times}",
        "db::{Attachment, AttachmentId, CustomIconId, Entry, EntryId, Group, GroupId, GroupRef, History, MoveGroupError, Times}",
    )
    replace_once(
        crate_dir / "src/db/merge.rs",
        """    /// A group could not be moved to its merged location.""",
        """    /// An entry history references attachment bytes absent from the source.
    #[error("Entry {0} has an unresolved historical attachment.")]
    HistoryAttachmentMissing(EntryId),

    /// A group could not be moved to its merged location.""",
    )
    replace_once(
        crate_dir / "src/db/merge.rs",
        """    for &id in dest_entries.intersection(&source_entries) {
        #[allow(clippy::unwrap_used)] // id is guaranteed to exist in both dest and source
        let mut dest_entry = dest_db.entry_mut(id).unwrap();

        #[allow(clippy::unwrap_used)] // id is guaranteed to exist in both dest and source
        let source_entry = source_db.entry(id).unwrap();""",
        """    for &id in dest_entries.intersection(&source_entries) {
        #[allow(clippy::unwrap_used)] // id is guaranteed to exist in both dest and source
        let source_entry = source_db.entry(id).unwrap();
        let source_history = import_history(dest_db, source_db, &source_entry)?;

        #[allow(clippy::unwrap_used)] // id is guaranteed to exist in both dest and source
        let mut dest_entry = dest_db.entry_mut(id).unwrap();""",
    )
    replace_once(
        crate_dir / "src/db/merge.rs",
        "let source_history = source_entry.history.clone().unwrap_or_else(|| {",
        "let source_history = source_history.unwrap_or_else(|| {",
    )
    insert_once(
        crate_dir / "src/db/merge.rs",
        "/// Merge two histories together, returning the merged history.",
        """// Hidlins: historical attachment ids belong to the source pool.
// Import their bytes before history comparison, reusing an equal destination
// value (including its protection flag). This makes repeated unions idempotent
// instead of treating equal bytes at different pool ids as distinct versions.
fn import_history(
    dest_db: &mut Database,
    source_db: &Database,
    source: &Entry,
) -> Result<Option<History>, MergeError> {
    let Some(mut history) = source.history.clone() else {
        return Ok(None);
    };
    for past in &mut history.entries {
        // Prefer the pool ids already used by this exact destination version.
        // Other versions or entries can own equal bytes at different ids.
        let corresponding = dest_db.entries.get(&source.id())
            .and_then(|entry| entry.history.as_ref())
            .and_then(|existing| existing.entries.iter().find_map(|candidate| {
                (past.times == candidate.times
                    && same_entry_properties(past, candidate)
                    && past.history == candidate.history)
                    .then(|| candidate.attachments.clone())
            }));
        for (name, source_id) in &mut past.attachments {
            let source_attachment = source_db.attachments.get(source_id)
                .ok_or(MergeError::HistoryAttachmentMissing(source.id()))?;
            let preferred = corresponding.as_ref().and_then(|ids| ids.get(name))
                .copied().filter(|id| dest_db.attachments.get(id)
                    .is_some_and(|attachment| attachment.data == source_attachment.data));
            let existing = preferred.or_else(|| dest_db.attachments.values()
                .filter(|attachment| attachment.data == source_attachment.data)
                .map(Attachment::id).min_by_key(AttachmentId::id));
            let destination_id = if let Some(id) = existing {
                id
            } else {
                let id = AttachmentId::next_free(dest_db);
                dest_db.attachments.insert(id, Attachment {
                    id,
                    entries: HashSet::new(),
                    data: source_attachment.data.clone(),
                });
                id
            };
            *source_id = destination_id;
        }
    }
    Ok(Some(history))
}

""",
    )
    replace_once(
        crate_dir / "src/db/merge.rs",
        "    Ok(History { entries })\n}",
        """    // Hidlins: a timestamp can contain several distinct secret versions.
    // Pairwise merging can repeat one of them when one replica has a subset.
    // Remove only exact duplicates, retaining all different metadata/content.
    // Distinct timestamps cannot contain exact duplicate versions.
    if entries.windows(2).all(|pair| {
        pair[0].times.last_modification != pair[1].times.last_modification
    }) {
        return Ok(History { entries });
    }
    let mut unique: Vec<Entry> = Vec::with_capacity(entries.len());
    for entry in entries {
        // The merge emits descending timestamps. Compare only the equal-time
        // suffix so ordinary histories retain linear deduplication cost.
        let duplicate = unique.iter().rev()
            .take_while(|prior| prior.times.last_modification == entry.times.last_modification)
            .any(|prior| prior == &entry);
        if !duplicate {
            unique.push(entry);
        }
    }
    Ok(History { entries: unique })
}""",
    )
    replace_once(
        crate_dir / "src/db/merge.rs",
        """            dest_entry.times.last_modification = source_entry.times.last_modification;
            dest_entry.fields = source_entry.fields.clone();""",
        """            dest_entry.times.last_modification = source_entry.times.last_modification;
            dest_entry.times.expires = source_entry.times.expires;
            dest_entry.times.expiry = source_entry.times.expiry;
            dest_entry.fields = source_entry.fields.clone();""",
    )
    replace_once(
        crate_dir / "src/db/merge.rs",
        """fn have_entries_diverged(a: &Entry, b: &Entry) -> bool {
    let new_times = Times::default();

    let mut a = a.clone();
    a.times = new_times.clone();
    a.history = None;

    let mut b = b.clone();
    b.times = new_times.clone();
    b.history = None;

    !a.eq(&b)
}""",
        """fn have_entries_diverged(a: &Entry, b: &Entry) -> bool {
    !same_entry_properties(a, b)
        || a.times.expires != b.times.expires
        || a.times.expiry != b.times.expiry
        || a.attachments != b.attachments
}""",
    )
    replace_once(
        crate_dir / "src/db/merge.rs",
        "/// Check if two entries are dissimilar, ignoring their timestamps.",
        "/// Check entry content, ignoring ordering/access times but retaining expiration.",
    )
    # Preserve newer deep descendants without repeatedly walking overlapping subtrees.
    for before, after in [
        ("use std::{collections::HashSet, ops::Deref};", "use std::{collections::{HashMap, HashSet}, ops::Deref};"),
        ("fn get_last_update(group: GroupRef<'_>) -> Option<NaiveDateTime> {", "fn get_last_update(\n    group: GroupRef<'_>,\n    cache: &mut HashMap<crate::db::GroupId, Option<NaiveDateTime>>,\n) -> Option<NaiveDateTime> {\n    let id = group.id();\n    if let Some(update) = cache.get(&id) {\n        return *update;\n    }"),
        ("    group\n        .entries()\n", "    let latest = group\n        .entries()\n"),
        ("        .chain(\n            group\n                .groups()\n                .filter_map(|g| g.times.last_modification.or(g.times.location_changed)),\n        )", "        .chain(group.groups().filter_map(|child| get_last_update(child, cache)))"),
        ("        .chain(last_update)\n        .max()\n}", "        .chain(last_update)\n        .max();\n    cache.insert(id, latest);\n    latest\n}"),
        ("    let source_groups = source_db.groups.keys().cloned().collect::<HashSet<_>>();", "    let source_groups = source_db.groups.keys().cloned().collect::<HashSet<_>>();\n    // Visit each requested subtree only once, even when ancestors overlap.\n    let mut source_updates = HashMap::new();\n    let mut dest_updates = HashMap::new();"),
        ("get_last_update(source);", "get_last_update(source, &mut source_updates);"),
        ("get_last_update(dest.as_ref());", "get_last_update(dest.as_ref(), &mut dest_updates);"),
    ]:
        replace_once(crate_dir / "src/db/merge.rs", before, after)
    insert_once(
        crate_dir / "src/db/merge.rs",
        "        merge_icons(self, other, &mut log)?;",
        """        // Hidlins: propagate deletions even when neither side has the object.
        // Concrete timestamps supersede unknown dates; never move a marker back.
        for (id, deleted_at) in &other.deleted_objects {
            self.deleted_objects.entry(*id)
                .and_modify(|existing| *existing = (*existing).max(*deleted_at))
                .or_insert(*deleted_at);
        }

""",
    )
    replace_exact_count(
        crate_dir / "src/db/merge.rs",
        "dest_db.deleted_objects.insert(id.uuid(), *deletion_time);",
        "dest_db.deleted_objects.entry(id.uuid())\n                .and_modify(|existing| *existing = (*existing).max(*deletion_time))\n                .or_insert(*deletion_time);",
        3,
    )
    # A move can be newer than the content clock; Option::or is not ordering.
    for before, after in [
        ("group.times.last_modification.or(group.times.location_changed)", "group.times.last_modification.max(group.times.location_changed)"),
        ("e.times.last_modification.or(e.times.location_changed)", "e.times.last_modification.max(e.times.location_changed)"),
        (".last_modification\n                .or(source_entry.times.location_changed)", ".last_modification\n                .max(source_entry.times.location_changed)"),
        (".last_modification\n                .or(dest_entry.times.location_changed)", ".last_modification\n                .max(dest_entry.times.location_changed)"),
    ]:
        replace_once(crate_dir / "src/db/merge.rs", before, after)
    replace_once(
        crate_dir / "src/db/merge.rs",
        "            dest_group.times = source.times.clone();",
        "            dest_group.times = source.times.clone();\n            dest_group.previous_parent_group = source.previous_parent_group;",
    )
    insert_once(
        crate_dir / "src/db/merge.rs",
        "/// Check entry content, ignoring ordering/access times but retaining expiration.",
        """// Hidlins: compare borrowed Entry properties without cloning secrets/history.
// This is the complete keepass 0.13.25 Entry field set except times, history
// and attachment references, which each caller compares with its own mask.
fn same_entry_properties(a: &Entry, b: &Entry) -> bool {
    a.id == b.id
        && a.parent == b.parent
        && a.fields == b.fields
        && a.autotype == b.autotype
        && a.tags == b.tags
        && a.custom_data == b.custom_data
        && a.icon == b.icon
        && a.foreground_color == b.foreground_color
        && a.background_color == b.background_color
        && a.override_url == b.override_url
        && a.quality_check == b.quality_check
        && a.previous_parent_group == b.previous_parent_group
}

""",
    )
    ensure_file(crate_dir / "src/key/hex.rs", KEEPASS_HEX)
    refresh_checksum(crate_dir)



def patch_zbus() -> None:
    crate_dir = VENDOR / "zbus"
    require_version(crate_dir, "6.2.1")
    replace_once(
        crate_dir / "Cargo.toml",
        '[dependencies.hex]\nversion = "0.4.3"\noptional = true\n\n',
        "",
    )
    replace_once(crate_dir / "Cargo.toml.orig", "hex = { workspace = true, optional = true }\n", "")
    for manifest in ("Cargo.toml", "Cargo.toml.orig"):
        replace_once(crate_dir / manifest, '    "dep:hex",\n', "")
    replace_once(
        crate_dir / "src/connection/handshake/mod.rs",
        "mod common;\n#[cfg(feature = \"p2p\")]",
        "mod common;\nmod hex;\n#[cfg(feature = \"p2p\")]",
    )
    replace_once(
        crate_dir / "src/connection/handshake/command.rs",
        "use crate::{Error, Guid, OwnedGuid, Result, conn::AuthMechanism};\n\n// The plain-text",
        "use crate::{Error, Guid, OwnedGuid, Result, conn::AuthMechanism};\n\nuse super::hex;\n\n// The plain-text",
    )
    ensure_file(crate_dir / "src/connection/handshake/hex.rs", HEX_CODEC)
    replace_once(
        crate_dir / "tests/issue/issue_813.rs",
        "hex::encode(geteuid().as_raw().to_string())",
        "geteuid()\n                    .as_raw()\n                    .to_string()\n                    .bytes()\n                    .map(|byte| format!(\"{byte:02x}\"))\n                    .collect::<String>()",
    )
    refresh_checksum(crate_dir)



def patch_snow() -> None:
    """Apply Hidlins' audited secret-erasure patch to Snow 0.10.0."""
    crate_dir = VENDOR / "snow"
    require_version(crate_dir, "0.10.0")
    replace_once(
        crate_dir / "Cargo.toml",
        '[dependencies.sha2]\nversion = "0.10"\noptional = true\ndefault-features = false',
        '[dependencies.sha2]\nversion = "=0.11.0"\nfeatures = ["zeroize"]\noptional = true\ndefault-features = false',
    )
    replace_once(
        crate_dir / "Cargo.toml.orig",
        'sha2 = { version = "0.10", optional = true, default-features = false }',
        'sha2 = { version = "=0.11.0", optional = true, default-features = false, features = ["zeroize"] }',
    )

    replace_once(
        crate_dir / "Cargo.toml",
        '[dependencies.subtle]\nversion = "2.4"\ndefault-features = false\n',
        '[dependencies.subtle]\nversion = "2.4"\ndefault-features = false\n\n'
        '[dependencies.zeroize]\nversion = "1.8"\nfeatures = ["derive"]\n'
        'default-features = false\n',
    )
    replace_once(
        crate_dir / "Cargo.toml.orig",
        'subtle = { version = "2.4", default-features = false }\n',
        'subtle = { version = "2.4", default-features = false }\n'
        'zeroize = { version = "1.8", default-features = false, features = ["derive"] }\n',
    )
    replace_one_of(
        crate_dir / "Cargo.toml",
        ('std = [\n'
        '    "getrandom/std",\n'
        '    "subtle/std",\n'
        '    "ring/std",\n'
        '    "blake2/std",\n'
        '    "sha2/std",\n'
        ']\n',
        'std = [\n'
        '    "getrandom?/std",\n'
        '    "subtle/std",\n'
        '    "blake2?/std",\n'
        '    "sha2?/std",\n'
        ']\n'),
        'std = [\n'
        '    "getrandom?/std",\n'
        '    "subtle/std",\n'
        '    "blake2?/std",\n'
        ']\n',
    )
    replace_one_of(
        crate_dir / "Cargo.toml.orig",
        ('std = ["getrandom/std", "subtle/std", "ring/std", "blake2/std", "sha2/std"]\n',
         'std = ["getrandom?/std", "subtle/std", "blake2?/std", "sha2?/std"]\n'),
        'std = ["getrandom?/std", "subtle/std", "blake2?/std"]\n',
    )
    replace_once(
        crate_dir / "src/builder.rs",
        'use subtle::ConstantTimeEq;\n',
        'use subtle::ConstantTimeEq;\nuse zeroize::{ZeroizeOnDrop, Zeroizing};\n',
    )
    replace_once(
        crate_dir / "src/builder.rs",
        'pub struct Keypair {\n',
        '#[derive(ZeroizeOnDrop)]\npub struct Keypair {\n',
    )
    replace_once(
        crate_dir / "src/builder.rs",
        '        let mut psks = [None::<[u8; PSKLEN]>; 10];\n',
        '        let mut psks: [Option<Zeroizing<[u8; PSKLEN]>>; 10] =\n'
        '            core::array::from_fn(|_| None);\n',
    )
    replace_once(
        crate_dir / "src/builder.rs",
        '                psks[i] = Some(k);\n',
        '                psks[i] = Some(Zeroizing::new(k));\n',
    )
    replace_once(
        crate_dir / "src/builder.rs",
        '            &psks,\n',
        '            psks,\n',
    )
    replace_once(
        crate_dir / "src/types.rs",
        '    constants::{CIPHERKEYLEN, MAXBLOCKLEN, MAXHASHLEN, TAGLEN},\n'
        '    Error,\n};\n',
        '    constants::{CIPHERKEYLEN, MAXBLOCKLEN, MAXDHLEN, MAXHASHLEN, TAGLEN},\n'
        '    Error,\n};\nuse zeroize::Zeroizing;\n',
    )
    replace_once(
        crate_dir / "src/types.rs",
        '    fn dh_len(&self) -> usize {\n'
        '        self.pub_len()\n'
        '    }\n'
        '}\n',
        '    fn dh_len(&self) -> usize {\n'
        '        self.pub_len()\n'
        '    }\n\n'
        '    /// Erase the private key held by this implementation.\n'
        '    fn zeroize(&mut self) {\n'
        '        let zeros = [0_u8; MAXDHLEN];\n'
        '        self.set(&zeros[..self.priv_len()]);\n'
        '    }\n'
        '}\n',
    )
    replace_once(
        crate_dir / "src/types.rs",
        '        let mut ciphertext = [0; CIPHERKEYLEN + TAGLEN];\n',
        '        let mut ciphertext = Zeroizing::new([0; CIPHERKEYLEN + TAGLEN]);\n',
    )
    replace_once(
        crate_dir / "src/types.rs",
        '        let mut key = [0_u8; CIPHERKEYLEN];\n'
        '        key.copy_from_slice(&ciphertext[..CIPHERKEYLEN]);\n\n'
        '        self.set(&key);\n'
        '    }\n'
        '}\n',
        '        let mut key = Zeroizing::new([0_u8; CIPHERKEYLEN]);\n'
        '        key.copy_from_slice(&ciphertext[..CIPHERKEYLEN]);\n\n'
        '        self.set(&*key);\n'
        '    }\n\n'
        '    /// Erase the active cipher key held by this implementation.\n'
        '    fn zeroize(&mut self) {\n'
        '        self.set(&[0_u8; CIPHERKEYLEN]);\n'
        '    }\n'
        '}\n',
    )
    for before, after in (
        (
            '        let mut ipad = [0x36_u8; MAXBLOCKLEN];\n',
            '        let mut ipad = Zeroizing::new([0x36_u8; MAXBLOCKLEN]);\n',
        ),
        (
            '        let mut opad = [0x5c_u8; MAXBLOCKLEN];\n',
            '        let mut opad = Zeroizing::new([0x5c_u8; MAXBLOCKLEN]);\n',
        ),
        (
            '        let mut inner_output = [0_u8; MAXHASHLEN];\n',
            '        let mut inner_output = Zeroizing::new([0_u8; MAXHASHLEN]);\n',
        ),
        (
            '        let mut temp_key = [0_u8; MAXHASHLEN];\n',
            '        let mut temp_key = Zeroizing::new([0_u8; MAXHASHLEN]);\n',
        ),
        (
            '        let mut in2 = [0_u8; MAXHASHLEN + 1];\n',
            '        let mut in2 = Zeroizing::new([0_u8; MAXHASHLEN + 1]);\n',
        ),
        (
            '        let mut in3 = [0_u8; MAXHASHLEN + 1];\n',
            '        let mut in3 = Zeroizing::new([0_u8; MAXHASHLEN + 1]);\n',
        ),
    ):
        replace_once(crate_dir / "src/types.rs", before, after)
    for before, after in (
        ("&mut ciphertext);", "&mut *ciphertext);"),
        ("        self.set(&key);\n", "        self.set(&*key);\n"),
        ("        self.result(&mut inner_output);\n", "        self.result(&mut *inner_output);\n"),
        ("&mut temp_key);", "&mut *temp_key);"),
    ):
        replace_once(crate_dir / "src/types.rs", before, after)
    replace_exact_count(
        crate_dir / "src/types.rs",
        "self.hmac(&temp_key,",
        "self.hmac(&*temp_key,",
        3,
    )
    replace_once(
        crate_dir / "src/cipherstate.rs",
        '    pub fn set_nonce(&mut self, nonce: u64) {\n'
        '        self.n = nonce;\n'
        '    }\n'
        '}\n\n'
        'pub(crate) struct CipherStates',
        '    pub fn set_nonce(&mut self, nonce: u64) {\n'
        '        self.n = nonce;\n'
        '    }\n\n'
        '    fn zeroize(&mut self) {\n'
        '        self.cipher.zeroize();\n'
        '        self.n = 0;\n'
        '        self.has_key = false;\n'
        '    }\n'
        '}\n\n'
        'pub(crate) struct CipherStates',
    )
    replace_once(
        crate_dir / "src/cipherstate.rs",
        '    pub fn rekey_responder_manually(&mut self, key: &[u8; CIPHERKEYLEN]) {\n'
        '        self.1.rekey_manually(key);\n'
        '    }\n'
        '}\n\n'
        'pub(crate) struct StatelessCipherState',
        '    pub fn rekey_responder_manually(&mut self, key: &[u8; CIPHERKEYLEN]) {\n'
        '        self.1.rekey_manually(key);\n'
        '    }\n\n'
        '    pub(crate) fn zeroize(&mut self) {\n'
        '        self.0.zeroize();\n'
        '        self.1.zeroize();\n'
        '    }\n'
        '}\n\n'
        'pub(crate) struct StatelessCipherState',
    )
    replace_once(
        crate_dir / "src/cipherstate.rs",
        '    pub fn rekey_manually(&mut self, key: &[u8; CIPHERKEYLEN]) {\n'
        '        self.cipher.set(key);\n'
        '    }\n'
        '}\n\n'
        '/// Validates that a nonce value',
        '    pub fn rekey_manually(&mut self, key: &[u8; CIPHERKEYLEN]) {\n'
        '        self.cipher.set(key);\n'
        '    }\n\n'
        '    fn zeroize(&mut self) {\n'
        '        self.cipher.zeroize();\n'
        '        self.has_key = false;\n'
        '    }\n'
        '}\n\n'
        '/// Validates that a nonce value',
    )
    replace_once(
        crate_dir / "src/cipherstate.rs",
        'impl StatelessCipherStates {\n'
        '    pub fn rekey_initiator(&mut self) {\n'
        '        self.0.rekey();\n'
        '    }\n\n'
        '    pub fn rekey_initiator_manually(&mut self, key: &[u8; CIPHERKEYLEN]) {\n'
        '        self.0.rekey_manually(key);\n'
        '    }\n\n'
        '    pub fn rekey_responder(&mut self) {\n'
        '        self.1.rekey();\n'
        '    }\n\n'
        '    pub fn rekey_responder_manually(&mut self, key: &[u8; CIPHERKEYLEN]) {\n'
        '        self.1.rekey_manually(key);\n'
        '    }\n'
        '}\n',
        'impl StatelessCipherStates {\n'
        '    pub fn rekey_initiator(&mut self) {\n'
        '        self.0.rekey();\n'
        '    }\n\n'
        '    pub fn rekey_initiator_manually(&mut self, key: &[u8; CIPHERKEYLEN]) {\n'
        '        self.0.rekey_manually(key);\n'
        '    }\n\n'
        '    pub fn rekey_responder(&mut self) {\n'
        '        self.1.rekey();\n'
        '    }\n\n'
        '    pub fn rekey_responder_manually(&mut self, key: &[u8; CIPHERKEYLEN]) {\n'
        '        self.1.rekey_manually(key);\n'
        '    }\n\n'
        '    pub(crate) fn zeroize(&mut self) {\n'
        '        self.0.zeroize();\n'
        '        self.1.zeroize();\n'
        '    }\n'
        '}\n',
    )
    replace_once(
        crate_dir / "src/symmetricstate.rs",
        '    types::Hash,\n};\n\n#[derive(Copy, Clone)]\n',
        '    types::Hash,\n};\nuse zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};\n\n'
        '#[derive(Clone, Zeroize, ZeroizeOnDrop)]\n',
    )
    replace_once(
        crate_dir / "src/symmetricstate.rs",
        '        let mut hkdf_output = ([0_u8; MAXHASHLEN], [0_u8; MAXHASHLEN]);\n'
        '        self.hasher.hkdf(\n'
        '            &self.inner.ck[..hash_len],\n'
        '            data,\n'
        '            2,\n'
        '            &mut hkdf_output.0,\n'
        '            &mut hkdf_output.1,\n'
        '            &mut [],\n'
        '        );\n\n'
        '        // TODO(mcginty): use `split_array_ref` once stable to avoid memory inefficiency\n'
        '        let mut cipher_key = [0_u8; CIPHERKEYLEN];\n'
        '        cipher_key.copy_from_slice(&hkdf_output.1[..CIPHERKEYLEN]);\n\n'
        '        self.inner.ck = hkdf_output.0;\n'
        '        self.cipherstate.set(&cipher_key, 0);\n',
        '        let mut hkdf_output_1 = Zeroizing::new([0_u8; MAXHASHLEN]);\n'
        '        let mut hkdf_output_2 = Zeroizing::new([0_u8; MAXHASHLEN]);\n'
        '        self.hasher.hkdf(\n'
        '            &self.inner.ck[..hash_len],\n'
        '            data,\n'
        '            2,\n'
        '            &mut *hkdf_output_1,\n'
        '            &mut *hkdf_output_2,\n'
        '            &mut [],\n'
        '        );\n\n'
        '        // TODO(mcginty): use `split_array_ref` once stable to avoid memory inefficiency\n'
        '        let mut cipher_key = Zeroizing::new([0_u8; CIPHERKEYLEN]);\n'
        '        cipher_key.copy_from_slice(&hkdf_output_2[..CIPHERKEYLEN]);\n\n'
        '        self.inner.ck.copy_from_slice(&*hkdf_output_1);\n'
        '        self.cipherstate.set(&*cipher_key, 0);\n',
    )
    replace_once(
        crate_dir / "src/symmetricstate.rs",
        '        let mut hkdf_output = ([0_u8; MAXHASHLEN], [0_u8; MAXHASHLEN], [0_u8; MAXHASHLEN]);\n'
        '        self.hasher.hkdf(\n'
        '            &self.inner.ck[..hash_len],\n'
        '            data,\n'
        '            3,\n'
        '            &mut hkdf_output.0,\n'
        '            &mut hkdf_output.1,\n'
        '            &mut hkdf_output.2,\n'
        '        );\n'
        '        self.inner.ck = hkdf_output.0;\n'
        '        self.mix_hash(&hkdf_output.1[..hash_len]);\n\n'
        '        // TODO(mcginty): use `split_array_ref` once stable to avoid memory inefficiency\n'
        '        let mut cipher_key = [0_u8; CIPHERKEYLEN];\n'
        '        cipher_key.copy_from_slice(&hkdf_output.2[..CIPHERKEYLEN]);\n'
        '        self.cipherstate.set(&cipher_key, 0);\n',
        '        let mut hkdf_output_1 = Zeroizing::new([0_u8; MAXHASHLEN]);\n'
        '        let mut hkdf_output_2 = Zeroizing::new([0_u8; MAXHASHLEN]);\n'
        '        let mut hkdf_output_3 = Zeroizing::new([0_u8; MAXHASHLEN]);\n'
        '        self.hasher.hkdf(\n'
        '            &self.inner.ck[..hash_len],\n'
        '            data,\n'
        '            3,\n'
        '            &mut *hkdf_output_1,\n'
        '            &mut *hkdf_output_2,\n'
        '            &mut *hkdf_output_3,\n'
        '        );\n'
        '        self.inner.ck.copy_from_slice(&*hkdf_output_1);\n'
        '        self.mix_hash(&hkdf_output_2[..hash_len]);\n\n'
        '        // TODO(mcginty): use `split_array_ref` once stable to avoid memory inefficiency\n'
        '        let mut cipher_key = Zeroizing::new([0_u8; CIPHERKEYLEN]);\n'
        '        cipher_key.copy_from_slice(&hkdf_output_3[..CIPHERKEYLEN]);\n'
        '        self.cipherstate.set(&*cipher_key, 0);\n',
    )
    replace_once(
        crate_dir / "src/symmetricstate.rs",
        '        let mut hkdf_output = ([0_u8; MAXHASHLEN], [0_u8; MAXHASHLEN]);\n'
        '        self.split_raw(&mut hkdf_output.0, &mut hkdf_output.1);\n\n'
        '        // TODO(mcginty): use `split_array_ref` once stable to avoid memory inefficiency\n'
        '        let mut cipher_keys = ([0_u8; CIPHERKEYLEN], [0_u8; CIPHERKEYLEN]);\n'
        '        cipher_keys.0.copy_from_slice(&hkdf_output.0[..CIPHERKEYLEN]);\n'
        '        cipher_keys.1.copy_from_slice(&hkdf_output.1[..CIPHERKEYLEN]);\n'
        '        child1.set(&cipher_keys.0, 0);\n'
        '        child2.set(&cipher_keys.1, 0);\n',
        '        let mut hkdf_output_1 = Zeroizing::new([0_u8; MAXHASHLEN]);\n'
        '        let mut hkdf_output_2 = Zeroizing::new([0_u8; MAXHASHLEN]);\n'
        '        self.split_raw(&mut *hkdf_output_1, &mut *hkdf_output_2);\n\n'
        '        // TODO(mcginty): use `split_array_ref` once stable to avoid memory inefficiency\n'
        '        let mut cipher_key_1 = Zeroizing::new([0_u8; CIPHERKEYLEN]);\n'
        '        let mut cipher_key_2 = Zeroizing::new([0_u8; CIPHERKEYLEN]);\n'
        '        cipher_key_1.copy_from_slice(&hkdf_output_1[..CIPHERKEYLEN]);\n'
        '        cipher_key_2.copy_from_slice(&hkdf_output_2[..CIPHERKEYLEN]);\n'
        '        child1.set(&*cipher_key_1, 0);\n'
        '        child2.set(&*cipher_key_2, 0);\n',
    )
    replace_once(
        crate_dir / "src/symmetricstate.rs",
        '    pub(crate) fn checkpoint(&mut self) -> SymmetricStateData {\n'
        '        self.inner\n'
        '    }\n',
        '    pub(crate) fn checkpoint(&mut self) -> SymmetricStateData {\n'
        '        self.inner.clone()\n'
        '    }\n',
    )
    replace_one_of(
        crate_dir / "src/resolvers/default.rs",
        ('    types::{Cipher, Dh, Hash, Random},\n};\n\n',
         '    types::{Cipher, Dh, Hash, Random},\n};\nuse zeroize::{Zeroize, ZeroizeOnDrop};\n\n'),
        '    types::{Cipher, Dh, Hash, Random},\n};\n'
        'use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};\n\n',
    )
    for feature, name in (
        ('use-curve25519', 'Dh25519'),
        ('p256', 'P256'),
        ('use-aes-gcm', 'CipherAesGcm'),
        ('use-chacha20poly1305', 'CipherChaChaPoly'),
        ('use-xchacha20poly1305', 'CipherXChaChaPoly'),
    ):
        replace_once(
            crate_dir / "src/resolvers/default.rs",
            f'#[cfg(feature = "{feature}")]\n#[derive(Default)]\nstruct {name} {{',
            f'#[cfg(feature = "{feature}")]\n'
            f'#[derive(Default, Zeroize, ZeroizeOnDrop)]\nstruct {name} {{',
        )
    sha_source = crate_dir / "src/resolvers/default.rs"
    for bits in (256, 512):
        replace_once(
            sha_source,
            f'#[derive(Default)]\nstruct HashSHA{bits}',
            f'#[derive(Default, ZeroizeOnDrop)]\nstruct HashSHA{bits}',
        )
        # The prior reset-only destructor is absent in raw upstream and in the
        # completed patch. Its exact known body is the only removable variant.
        prior_drop = (
            '#[cfg(feature = "use-sha2")]\n'
            f'impl Drop for HashSHA{bits} {{\n'
            '    fn drop(&mut self) {\n'
            '        ShaDigest::reset(&mut self.hasher);\n'
            '    }\n}\n\n'
        )
        text = sha_source.read_text()
        if text.count(prior_drop) > 1:
            raise RuntimeError("duplicate prior SHA destructor")
        if prior_drop in text:
            sha_source.write_text(text.replace(prior_drop, '', 1))
        text = sha_source.read_text()
        start = text.index(f'impl Hash for HashSHA{bits} {{')
        end = text.index('\n}\n', start) + 3
        body = text[start:end]
        before = '        let hash = self.hasher.finalize_reset();\n        copy_slices!(hash.as_slice(), out);'
        after = (
            f'        let mut hash = Zeroizing::new([0_u8; {bits // 8}]);\n'
            '        core::mem::take(&mut self.hasher).finalize_into((&mut *hash).into());\n'
            '        copy_slices!(hash.as_slice(), out);'
        )
        if body.count(after) != 1:
            if body.count(before) != 1:
                raise RuntimeError("unexpected selected SHA finalization source")
            sha_source.write_text(text[:start] + body.replace(before, after, 1) + text[end:])
    replace_once(
        crate_dir / "src/handshakestate.rs",
        '    fmt,\n};\n',
        '    fmt,\n};\nuse zeroize::Zeroizing;\n',
    )
    replace_once(
        crate_dir / "src/handshakestate.rs",
        '    pub(crate) psks: [Option<[u8; PSKLEN]>; 10],\n',
        '    pub(crate) psks: [Option<Zeroizing<[u8; PSKLEN]>>; 10],\n',
    )
    replace_once(
        crate_dir / "src/handshakestate.rs",
        '        psks: &[Option<[u8; PSKLEN]>; 10],\n',
        '        psks: [Option<Zeroizing<[u8; PSKLEN]>>; 10],\n',
    )
    replace_once(
        crate_dir / "src/handshakestate.rs",
        '            psks: *psks,\n',
        '            psks,\n',
    )
    replace_once(
        crate_dir / "src/handshakestate.rs",
        '    fn dh(&self, token: DhToken) -> Result<[u8; MAXDHLEN], Error> {\n'
        '        let mut dh_out = [0_u8; MAXDHLEN];\n',
        '    fn dh(&self, token: DhToken) -> Result<Zeroizing<[u8; MAXDHLEN]>, Error> {\n'
        '        let mut dh_out = Zeroizing::new([0_u8; MAXDHLEN]);\n',
    )
    replace_exact_count(
        crate_dir / "src/handshakestate.rs",
        '                Token::Psk(n) => match self.psks[usize::from(n)] {\n',
        '                Token::Psk(n) => match self.psks[usize::from(n)].as_ref() {\n',
        2,
    )
    for filename, state_name in (
        ("transportstate.rs", "TransportState"),
        ("stateless_transportstate.rs", "StatelessTransportState"),
    ):
        replace_once(
            crate_dir / "src" / filename,
            f'impl TryFrom<HandshakeState> for {state_name} {{\n',
            f'impl Drop for {state_name} {{\n'
            '    fn drop(&mut self) {\n'
            '        self.cipherstates.zeroize();\n'
            '    }\n'
            '}\n\n'
            f'impl TryFrom<HandshakeState> for {state_name} {{\n',
        )
    replace_once(
        crate_dir / "src/handshakestate.rs",
        '        self.psks[location] = Some(new_psk);\n',
        '        self.psks[location] = Some(Zeroizing::new(new_psk));\n',
    )
    replace_once(
        crate_dir / "src/handshakestate.rs",
        '        dh.dh(&**key, &mut dh_out)?;\n',
        '        dh.dh(&**key, &mut *dh_out)?;\n',
    )
    replace_exact_count(
        crate_dir / "src/handshakestate.rs",
        '                        self.symmetricstate.mix_key_and_hash(&psk);\n',
        '                        self.symmetricstate.mix_key_and_hash(&psk[..]);\n',
        2,
    )
    patch_noise_hash_dependencies()
    verify_snow_patch(crate_dir)
    refresh_checksum(crate_dir)



# Exact pristine/patched ratatui-core 0.1.2 source boundary.
RATATUI_MEMORY_HASHES = {'Cargo.toml': ('ba34c802d26ec0b8c22bb7e6fd9b736de53d49f999f6806e166bfb1f7c8aad39',
                '255a4e62049d25f43bbcf25affdc19158aba3716f16f5efd6b817827b0c2a05e'),
 'src/buffer/cell.rs': ('c483c4139658804e633f264e8d302951e76546db52245a19388697f1c51333f9',
                        'a2fe375795fbf5eef60d1d6d513b42b874a32610c68f97ec1971f715c71c1dbc'),
 'src/buffer/buffer.rs': ('e0afea68c43a04f2b64192da0af9894eb55771e6eff1c74ca5db4985da350444',
                          'f1f5ea75c9da7c604505f1ba4b55d3df014a85310591c9769e25b639b86e8a70')}

RATATUI_MEMORY_EDITS = {'Cargo.toml': [['[dependencies.compact_str]\nversion = "0.9"\ndefault-features = false',
                 '[dependencies.compact_str]\n'
                 'version = "0.9"\n'
                 'features = ["zeroize"]\n'
                 'default-features = false'],
                ['[dependencies.bitflags]\n',
                 '[dependencies.zeroize]\n'
                 'version = "=1.8.2"\n'
                 'default-features = false\n'
                 '\n'
                 '[dependencies.bitflags]\n']],
 'src/buffer/cell.rs': [['/// Cell diffing options\n',
                         '// Hidlins: erase both inline and heap symbols on every ownership release.\n'
                         '// Keep Drop on this private field wrapper so Cell::new/EMPTY remain const.\n'
                         '#[repr(transparent)]\n'
                         '#[derive(Clone, Default)]\n'
                         '#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]\n'
                         '#[cfg_attr(feature = "serde", serde(transparent))]\n'
                         'struct CellSymbol(CompactString);\n'
                         '\n'
                         'impl CellSymbol {\n'
                         '    #[inline]\n'
                         "    const fn const_new(symbol: &'static str) -> Self {\n"
                         '        Self(CompactString::const_new(symbol))\n'
                         '    }\n'
                         '\n'
                         '    #[inline]\n'
                         '    fn new(symbol: &str) -> Self {\n'
                         '        Self(CompactString::new(symbol))\n'
                         '    }\n'
                         '\n'
                         '    #[inline]\n'
                         '    fn push_str(&mut self, symbol: &str) {\n'
                         '        let required = self\n'
                         '            .0\n'
                         '            .len()\n'
                         '            .checked_add(symbol.len())\n'
                         '            .expect("cell symbol capacity overflow");\n'
                         '        if required > self.0.capacity() {\n'
                         '            let capacity = required.max(self.0.capacity().saturating_mul(2));\n'
                         '            let mut replacement = Self(CompactString::with_capacity(capacity));\n'
                         '            replacement.0.push_str(self.0.as_str());\n'
                         '            core::mem::swap(self, &mut replacement);\n'
                         '            // replacement erases the original allocation before it is freed.\n'
                         '        }\n'
                         '        self.0.push_str(symbol);\n'
                         '    }\n'
                         '}\n'
                         '\n'
                         'impl core::ops::Deref for CellSymbol {\n'
                         '    type Target = CompactString;\n'
                         '\n'
                         '    #[inline]\n'
                         '    fn deref(&self) -> &Self::Target {\n'
                         '        &self.0\n'
                         '    }\n'
                         '}\n'
                         '\n'
                         'impl core::fmt::Debug for CellSymbol {\n'
                         "    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result "
                         '{\n'
                         '        core::fmt::Debug::fmt(&self.0, formatter)\n'
                         '    }\n'
                         '}\n'
                         '\n'
                         'impl zeroize::ZeroizeOnDrop for CellSymbol {}\n'
                         '\n'
                         'impl Drop for CellSymbol {\n'
                         '    #[inline]\n'
                         '    fn drop(&mut self) {\n'
                         '        zeroize::Zeroize::zeroize(&mut self.0);\n'
                         '    }\n'
                         '}\n'
                         '\n'
                         '/// Cell diffing options\n'],
                        ['symbol: Option<CompactString>', 'symbol: Option<CellSymbol>'],
                        ['Some(CompactString::const_new(symbol))', 'Some(CellSymbol::const_new(symbol))'],
                        ['Some(CompactString::new(merged_symbol))', 'Some(CellSymbol::new(merged_symbol))'],
                        ['Some(CompactString::new(symbol))', 'Some(CellSymbol::new(symbol))'],
                        ['Some(CompactString::new(ch.encode_utf8(&mut buf)))',
                         'Some(CellSymbol::new(ch.encode_utf8(&mut buf)))'],
                        ['Some(CompactString::const_new("あ"))', 'Some(CellSymbol::const_new("あ"))'],
                        ['    pub fn set_symbol', '    #[inline]\n    pub fn set_symbol'],
                        ['    pub(crate) fn append_symbol', '    #[inline]\n    pub(crate) fn append_symbol'],
                        ['    pub fn set_char', '    #[inline]\n    pub fn set_char'],
                        ['    pub fn reset', '    #[inline]\n    pub fn reset']],
 'src/buffer/buffer.rs': [['        if self.content.len() > length {\n'
                           '            self.content.truncate(length);\n'
                           '        } else {\n'
                           '            self.content.resize(length, Cell::EMPTY);\n'
                           '        }',
                           '        self.resize_content(length);'],
                          ['    /// Reset all cells in the buffer\n',
                           '    // Vec::resize may relocate live inline symbols without dropping their old\n'
                           '    // storage. Clone into a final allocation and drop the populated old vector\n'
                           '    // first, so its CellSymbol owners erase that storage before release.\n'
                           '    fn resize_content(&mut self, length: usize) {\n'
                           '        if length > self.content.capacity() {\n'
                           '            let capacity = '
                           'length.max(self.content.capacity().saturating_mul(2));\n'
                           '            let mut replacement = Vec::with_capacity(capacity);\n'
                           '            replacement.extend(self.content.iter().cloned());\n'
                           '            self.content = replacement;\n'
                           '        }\n'
                           '        self.content.resize(length, Cell::EMPTY);\n'
                           '    }\n'
                           '\n'
                           '    /// Reset all cells in the buffer\n'],
                          ['self.content.resize(area.area() as usize, Cell::EMPTY);',
                           'self.resize_content(area.area() as usize);']]}

def patch_ratatui_memory() -> None:
    """Erase rendered symbols, preserving public Cell and Buffer contracts.

    Validate the entire edit set before any write. Accept exactly pristine or
    exactly patched files; reject partial patches and checksum drift. No new
    package/version is introduced: enable compact_str's existing zeroize support
    and bind the already-locked, reviewed zeroize 1.8.2 directly.
    """
    crate_dir = VENDOR / "ratatui-core"
    require_version(crate_dir, "0.1.2")
    checksums = json.loads((crate_dir / ".cargo-checksum.json").read_text())
    if checksums["package"] != "cbb175c433c8e28a809d1f5773a2ae96e68c0ce40db865cbab1020bf33ae479c":
        raise RuntimeError("unexpected ratatui-core package checksum")
    files = {path.relative_to(crate_dir).as_posix(): path for path in crate_dir.rglob("*")
             if path.is_file() and path.name != ".cargo-checksum.json"}
    if set(files) != set(checksums["files"]):
        raise RuntimeError("ratatui-core checksum inventory drift")
    generations = set()
    staged = {}
    for relative, path in files.items():
        contents = path.read_bytes()
        digest = hashlib.sha256(contents).hexdigest()
        if relative not in RATATUI_MEMORY_HASHES:
            if digest != checksums["files"][relative]:
                raise RuntimeError(f"ratatui-core untouched source checksum drift: {relative}")
            continue
        before_hash, after_hash = RATATUI_MEMORY_HASHES[relative]
        if digest == before_hash:
            generations.add("pristine")
            text = contents.decode()
            for before, after in RATATUI_MEMORY_EDITS[relative]:
                if text.count(before) != 1:
                    raise RuntimeError(f"ratatui-core audited context drift: {relative}")
                text = text.replace(before, after, 1)
            staged[relative] = text.encode()
        elif digest == after_hash:
            generations.add("patched")
            staged[relative] = contents
        else:
            raise RuntimeError(f"ratatui-core reviewed source drift: {relative}")
        if hashlib.sha256(staged[relative]).hexdigest() != after_hash:
            raise RuntimeError(f"ratatui-core patch output drift: {relative}")
    if len(generations) != 1:
        raise RuntimeError("ratatui-core partial patch rejected")
    for relative, contents in staged.items():
        (crate_dir / relative).write_bytes(contents)
    refresh_checksum(crate_dir)


# Exact existing ratatui-widgets geometry query source boundary.
RATATUI_GEOMETRY_HASHES = ('337cc24ce4e07dac5de0e30e0ecb3922580b83354032ed035a359c293b62a4db', '8740395431e475ca4ab6ffb6d8eb8afbfac162570092d6d10c48c74a25046d5d')
RATATUI_GEOMETRY_PACKAGE = '66e3d19bcc9130ca376277d93b60767ff121ace3be06f5f95f81dd68956407d1'
RATATUI_GEOMETRY_EDIT = ('    /// Calculates the shortest line width needed to avoid any word being wrapped or truncated.\n',
 '    /// Finds the unscrolled content row containing a rendered byte of one span.\n'
 '    ///\n'
 '    /// `width` is the actual inner text width; block and scroll offsets are not\n'
 '    /// included. Uses the same complete input and word wrapper as rendering.\n'
 '    /// Returns `None` without wrapping, for invalid/empty/end positions, zero\n'
 '    /// width, or when the target grapheme is filtered, overwide, or zero-width.\n'
 '    ///\n'
 '    /// This query borrows text. The composer may process an entire logical line\n'
 '    /// before yielding its first wrapped row, just as normal Paragraph drawing.\n'
 '    #[instability::unstable(\n'
 '        feature = "rendered-line-info",\n'
 '        issue = "https://github.com/ratatui/ratatui/issues/293"\n'
 '    )]\n'
 '    pub fn wrapped_row_for_span(\n'
 '        &self,\n'
 '        width: u16,\n'
 '        logical_line: usize,\n'
 '        span_index: usize,\n'
 '        byte_offset: usize,\n'
 '    ) -> Option<usize> {\n'
 '        if width == 0 {\n'
 '            return None;\n'
 '        }\n'
 '        let Wrap { trim } = self.wrap?;\n'
 '        let target = self.text.lines.get(logical_line)?.spans.get(span_index)?;\n'
 '        if byte_offset >= target.content.len() {\n'
 '            return None;\n'
 '        }\n'
 '        let address = target.content.as_ptr().wrapping_add(byte_offset).addr();\n'
 '        let marker = Style::default().add_modifier(ratatui_core::style::Modifier::REVERSED);\n'
 '        let styled = self.text.iter().enumerate().map(|(line_index, line)| {\n'
 '            let graphemes = line\n'
 '                .spans\n'
 '                .iter()\n'
 '                .enumerate()\n'
 '                .flat_map(move |(index, span)| {\n'
 '                    let tagged = line_index == logical_line && index == span_index;\n'
 '                    // Reflow depends only on symbols/widths. Query-only tags retain\n'
 '                    // source-span identity even for identical shared static text.\n'
 '                    span.styled_graphemes(Style::default())\n'
 '                        .map(move |grapheme| StyledGrapheme {\n'
 '                            symbol: grapheme.symbol,\n'
 '                            style: if tagged { marker } else { Style::default() },\n'
 '                        })\n'
 '                });\n'
 '            (graphemes, line.alignment.unwrap_or(self.alignment))\n'
 '        });\n'
 '        let mut composer = WordWrapper::new(styled, width, trim);\n'
 '        let mut row = 0;\n'
 '        while let Some(line) = composer.next_line() {\n'
 '            if line.graphemes.iter().any(|grapheme| {\n'
 '                let start = grapheme.symbol.as_ptr().addr();\n'
 '                grapheme.style == marker\n'
 '                    && grapheme.symbol.cell_width() > 0\n'
 '                    && address >= start\n'
 '                    && address < start.saturating_add(grapheme.symbol.len())\n'
 '            }) {\n'
 '                return Some(row);\n'
 '            }\n'
 '            row += 1;\n'
 '        }\n'
 '        None\n'
 '    }\n'
 '\n'
 '    /// Calculates the shortest line width needed to avoid any word being wrapped or truncated.\n')


def patch_ratatui_geometry() -> None:
    """Expose exact borrowed span geometry through the existing word composer."""
    crate_dir = VENDOR / "ratatui-widgets"
    require_version(crate_dir, "0.3.2")
    checksum = json.loads((crate_dir / ".cargo-checksum.json").read_text())
    if checksum["package"] != RATATUI_GEOMETRY_PACKAGE:
        raise RuntimeError("unexpected ratatui-widgets package checksum")
    files = {p.relative_to(crate_dir).as_posix(): p for p in crate_dir.rglob("*")
             if p.is_file() and p.name != ".cargo-checksum.json"}
    if set(files) != set(checksum["files"]):
        raise RuntimeError("ratatui-widgets checksum inventory drift")
    relative = "src/paragraph.rs"
    before_hash, after_hash = RATATUI_GEOMETRY_HASHES
    if checksum["files"][relative] not in RATATUI_GEOMETRY_HASHES:
        raise RuntimeError("ratatui-widgets target checksum drift")
    staged = None
    for name, path in files.items():
        contents = path.read_bytes()
        digest = hashlib.sha256(contents).hexdigest()
        if name != relative:
            if digest != checksum["files"][name]:
                raise RuntimeError(f"ratatui-widgets untouched source checksum drift: {name}")
            continue
        if digest == before_hash:
            text = contents.decode()
            before, after = RATATUI_GEOMETRY_EDIT
            if text.count(before) != 1:
                raise RuntimeError("ratatui-widgets audited geometry context drift")
            staged = text.replace(before, after, 1).encode()
        elif digest == after_hash:
            staged = contents
        else:
            raise RuntimeError("ratatui-widgets reviewed paragraph source drift")
    if staged is None or hashlib.sha256(staged).hexdigest() != after_hash:
        raise RuntimeError("ratatui-widgets geometry patch output drift")
    # Every source/checksum precondition is checked before the single write.
    (crate_dir / relative).write_bytes(staged)
    refresh_checksum(crate_dir)

def patch_vendor() -> None:
    patch_ratatui_geometry()
    patch_ratatui_memory()
    patch_quick_xml()
    patch_keepass()
    patch_zbus()
    patch_snow()


def prune_unlocked_vendor(
    vendor_dir: Path = VENDOR,
    lock_path: Path = ROOT / "Cargo.lock",
    additional_lock_paths: tuple[Path, ...] = (),
) -> None:
    """Remove crate directories absent from all approved lock graphs."""
    active: set[tuple[str, str]] = set()
    for approved_lock_path in (lock_path, *additional_lock_paths):
        lock = tomllib.loads(approved_lock_path.read_text())
        active.update(
            (package["name"], package["version"])
            for package in lock["package"]
            if package.get("source", "").startswith("registry+")
        )

    for directory in vendor_dir.iterdir():
        if not directory.is_dir():
            continue
        manifest = directory / "Cargo.toml"
        if not manifest.is_file():
            raise RuntimeError(f"refusing to prune non-crate vendor directory {directory}")
        package = tomllib.loads(manifest.read_text())["package"]
        if (package["name"], package["version"]) in active:
            continue
        shutil.rmtree(directory)


def revendor() -> None:
    lock_path = ROOT / "Cargo.lock"
    fuzz_lock_path = ROOT / "fuzz" / "Cargo.lock"
    approved_lock = lock_path.read_bytes()
    approved_fuzz_lock = fuzz_lock_path.read_bytes() if fuzz_lock_path.is_file() else None
    environment = os.environ.copy()
    environment["CARGO_NET_OFFLINE"] = "false"
    try:
        subprocess.run(
            ["cargo", "vendor", "--sync", "fuzz/Cargo.toml"],
            cwd=ROOT,
            env=environment,
            check=True,
        )
    finally:
        # Registry manifests still declare `hex`; preserve the reviewed lock
        # produced from our patched manifests even if cargo vendor fails.
        lock_path.write_bytes(approved_lock)
        if approved_fuzz_lock is not None:
            fuzz_lock_path.write_bytes(approved_fuzz_lock)
    patch_vendor()
    prune_unlocked_vendor(additional_lock_paths=(fuzz_lock_path,))
    subprocess.run(
        ["cargo", "metadata", "--offline", "--locked", "--no-deps"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.DEVNULL,
    )


def seed_fuzz_lock() -> None:
    """Seed the isolated lock from production pins, then resolve fuzz-only edges.

    Cargo workspaces cannot share a lockfile. Starting from the reviewed
    production lock avoids needlessly selecting newer duplicate versions for
    the path dependency graph while Cargo removes unrelated workspace members.
    """
    fuzz_lock = ROOT / "fuzz" / "Cargo.lock"
    fuzz_lock.write_bytes((ROOT / "Cargo.lock").read_bytes())
    subprocess.run(
        [
            "cargo",
            "metadata",
            "--manifest-path",
            "fuzz/Cargo.toml",
            "--offline",
            "--format-version",
            "1",
            "--features",
            "fuzzing",
        ],
        cwd=ROOT,
        check=True,
        stdout=subprocess.DEVNULL,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--patch-only",
        action="store_true",
        help="reapply/verify the audited patches without downloading crates",
    )
    parser.add_argument(
        "--check-snow-dir",
        type=Path,
        help="verify an existing Snow tree without modifying it",
    )
    parser.add_argument(
        "--seed-fuzz-lock",
        action="store_true",
        help="re-resolve the fuzz graph starting from production package pins",
    )
    args = parser.parse_args()
    try:
        if args.check_snow_dir is not None:
            verify_snow_patch(args.check_snow_dir)
        elif args.seed_fuzz_lock:
            seed_fuzz_lock()
        elif args.patch_only:
            patch_vendor()
            fuzz_lock = ROOT / "fuzz" / "Cargo.lock"
            additional = (fuzz_lock,) if fuzz_lock.is_file() else ()
            prune_unlocked_vendor(additional_lock_paths=additional)
        else:
            revendor()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print("Vendored Cargo dependencies and audited source patches are consistent.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

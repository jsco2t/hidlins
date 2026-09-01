#!/usr/bin/env python3
"""Re-vendor Cargo dependencies and reapply Hidlins' narrow source patches.

Cargo verifies vendored files against each crate's `.cargo-checksum.json`.  The
patches remove the general-purpose `hex` crate where upstream packages only
need tiny, well-specified encode/decode operations and add the minimal Keepass
API needed to repair historical attachment pool references safely. Keeping the
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


ROOT = Path(__file__).resolve().parents[2]
VENDOR = ROOT / "vendor"


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


FRB_ENCODING = """pub(crate) fn encode_lower(input: impl AsRef<[u8]>) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";

    let input = input.as_ref();
    let mut encoded = String::with_capacity(input.len().saturating_mul(2));
    for byte in input {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::encode_lower;

    #[test]
    fn matches_macro_metadata_wire_format() {
        assert_eq!(encode_lower("#[frb(sync)]"), "235b6672622873796e63295d");
        assert_eq!(encode_lower(""), "");
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


TERM_HEX_DECODER = """#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DecodeError {
    OddLength,
    InvalidCharacter { byte: u8, index: usize },
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
    use super::{decode, DecodeError};

    #[test]
    fn decodes_all_nibbles_and_mixed_case() {
        assert_eq!(
            decode("0123456789aBcDeF").unwrap(),
            [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]
        );
    }

    #[test]
    fn rejects_malformed_input() {
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
    if after in text and before not in text:
        return
    if text.count(before) != 1:
        raise RuntimeError(f"expected exactly one audited patch context in {path}")
    path.write_text(text.replace(before, after, 1))


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


def patch_keepass() -> None:
    crate_dir = VENDOR / "keepass"
    require_version(crate_dir, "0.13.25")
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
    ensure_file(crate_dir / "src/key/hex.rs", KEEPASS_HEX)
    refresh_checksum(crate_dir)


def patch_flutter_rust_bridge_macros() -> None:
    crate_dir = VENDOR / "flutter_rust_bridge_macros"
    require_version(crate_dir, "2.12.0")
    replace_once(
        crate_dir / "Cargo.toml",
        '[dependencies.hex]\nversion = "0.4.3"\n\n',
        "",
    )
    replace_once(crate_dir / "Cargo.toml.orig", 'hex = "0.4.3"\n', "")
    replace_once(
        crate_dir / "src/components/mod.rs",
        "pub(crate) mod converter;\npub(crate) mod encoder;",
        "pub(crate) mod converter;\npub(crate) mod encoding;\npub(crate) mod encoder;",
    )
    replace_once(
        crate_dir / "src/components/encoder.rs",
        "use quote::quote;\n\n// This is surely executed",
        "use quote::quote;\n\nuse super::encoding::encode_lower;\n\n// This is surely executed",
    )
    replace_once(
        crate_dir / "src/components/encoder.rs",
        'format!(r#"frb_encoded({})"#, hex::encode(data))',
        'format!(r#"frb_encoded({})"#, encode_lower(data))',
    )
    replace_once(
        crate_dir / "src/components/attr_external.rs",
        "use syn::{ImplItem, Item, ItemImpl};\n\n// This is surely executed",
        "use syn::{ImplItem, Item, ItemImpl};\n\nuse super::encoding::encode_lower;\n\n// This is surely executed",
    )
    replace_once(
        crate_dir / "src/components/attr_external.rs",
        "hex::encode(original_self_ty_string),",
        "encode_lower(original_self_ty_string),",
    )
    ensure_file(crate_dir / "src/components/encoding.rs", FRB_ENCODING)
    refresh_checksum(crate_dir)


def patch_zbus() -> None:
    crate_dir = VENDOR / "zbus"
    require_version(crate_dir, "5.15.0")
    replace_once(
        crate_dir / "Cargo.toml",
        '[dependencies.hex]\nversion = "0.4.3"\n\n',
        "",
    )
    replace_once(crate_dir / "Cargo.toml.orig", "hex.workspace = true\n", "")
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
    replace_once(
        crate_dir / "src/connection/handshake/command.rs",
        "impl From<hex::FromHexError> for Error {\n    fn from(e: hex::FromHexError) -> Self {",
        "impl From<hex::DecodeError> for Error {\n    fn from(e: hex::DecodeError) -> Self {",
    )
    ensure_file(crate_dir / "src/connection/handshake/hex.rs", HEX_CODEC)
    replace_once(
        crate_dir / "tests/issue/issue_813.rs",
        "hex::encode(geteuid().as_raw().to_string())",
        "geteuid()\n                    .as_raw()\n                    .to_string()\n                    .bytes()\n                    .map(|byte| format!(\"{byte:02x}\"))\n                    .collect::<String>()",
    )
    refresh_checksum(crate_dir)


def patch_termwiz() -> None:
    crate_dir = VENDOR / "termwiz"
    require_version(crate_dir, "0.23.3")
    replace_once(
        crate_dir / "Cargo.toml",
        '[dependencies.hex]\nversion = "0.4"\n\n',
        "",
    )
    replace_once(crate_dir / "Cargo.toml.orig", "hex.workspace = true\n", "")
    replace_once(
        crate_dir / "src/lib.rs",
        "pub mod escape;",
        "pub mod escape;\nmod hex;",
    )
    replace_once(
        crate_dir / "src/escape/parser/mod.rs",
        "let decoded = hex::decode(&self.current)",
        "let decoded = crate::hex::decode(&self.current)",
    )
    ensure_file(crate_dir / "src/hex.rs", TERM_HEX_DECODER)
    refresh_checksum(crate_dir)


def patch_vendor() -> None:
    patch_keepass()
    patch_flutter_rust_bridge_macros()
    patch_zbus()
    patch_termwiz()


def lock_contains_hex(lock_bytes: bytes) -> bool:
    return re.search(rb'(?m)^name = "hex"$', lock_bytes) is not None


def prune_hex_if_unlocked(lock_bytes: bytes) -> None:
    if lock_contains_hex(lock_bytes):
        return
    hex_dir = VENDOR / "hex"
    if not hex_dir.exists():
        return
    manifest = (hex_dir / "Cargo.toml").read_text()
    if 'name = "hex"' not in manifest or 'version = "0.4.3"' not in manifest:
        raise RuntimeError(f"refusing to remove unexpected directory {hex_dir}")
    shutil.rmtree(hex_dir)


def revendor() -> None:
    lock_path = ROOT / "Cargo.lock"
    approved_lock = lock_path.read_bytes()
    environment = os.environ.copy()
    environment["CARGO_NET_OFFLINE"] = "false"
    try:
        subprocess.run(["cargo", "vendor"], cwd=ROOT, env=environment, check=True)
    finally:
        # Registry manifests still declare `hex`; preserve the reviewed lock
        # produced from our patched manifests even if cargo vendor fails.
        lock_path.write_bytes(approved_lock)
    patch_vendor()
    prune_hex_if_unlocked(approved_lock)
    subprocess.run(
        ["cargo", "metadata", "--offline", "--locked", "--no-deps"],
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
    args = parser.parse_args()
    try:
        if args.patch_only:
            patch_vendor()
            prune_hex_if_unlocked((ROOT / "Cargo.lock").read_bytes())
        else:
            revendor()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    print("Vendored Cargo dependencies and audited source patches are consistent.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

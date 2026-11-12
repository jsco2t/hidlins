//! Malformed numeric header fields must return errors rather than unwind.

use std::panic::{catch_unwind, AssertUnwindSafe};

use hidlins_core::{Database, KdfParams, MasterPassword, NoRecoveryConfirmed, Vault};
use hidlins_sync::{
    backup::backup_path_for,
    protocol::RemoteVersion,
    server::{AuthoritativeVault, ServerError, StagedUpload},
};
use sha2::{Digest, Sha256};

fn password() -> MasterPassword {
    MasterPassword::new("synthetic malformed header fixture".to_string())
}

fn outer_field(bytes: &[u8], wanted: u8) -> (usize, usize, usize) {
    let mut at = 12;
    loop {
        let kind = bytes[at];
        let length = usize::try_from(u32::from_le_bytes(
            bytes[at + 1..at + 5].try_into().unwrap(),
        ))
        .unwrap();
        if kind == wanted {
            return (at, at + 5, length);
        }
        assert_ne!(kind, 0, "generated fixture contains requested header field");
        at += 5 + length;
    }
}

fn shortened_compression(original: &[u8], keep: usize) -> Vec<u8> {
    let mut bytes = original.to_vec();
    let (at, start, length) = outer_field(&bytes, 3);
    assert_eq!(length, 4);
    bytes[at + 1..at + 5].copy_from_slice(&u32::try_from(keep).unwrap().to_le_bytes());
    bytes.drain(start + keep..start + length);
    bytes
}

fn shortened_numeric(original: &[u8], kind: u8, keep: usize) -> Vec<u8> {
    let mut bytes = original.to_vec();
    let (outer, start, length) = outer_field(&bytes, 11);
    let field = [5, 1, 0, 0, 0, b'I', 8, 0, 0, 0];
    let relative = bytes[start..start + length]
        .windows(field.len())
        .position(|part| part == field)
        .unwrap();
    let entry = start + relative;
    bytes[entry] = kind;
    bytes[entry + 6..entry + 10].copy_from_slice(&u32::try_from(keep).unwrap().to_le_bytes());
    bytes.drain(entry + 10 + keep..entry + 18);
    bytes[outer + 1..outer + 5]
        .copy_from_slice(&u32::try_from(length - (8 - keep)).unwrap().to_le_bytes());
    bytes
}

fn check_widths(kind: Option<u8>, width: usize) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.kdbx");
    let master = password();
    let mut vault = Vault::create(
        &path,
        &master,
        None,
        KdfParams {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut failures = Vec::new();
    for keep in 0..width {
        let bytes = match kind {
            Some(kind) => shortened_numeric(&before, kind, keep),
            None => shortened_compression(&before, keep),
        };
        let metadata = catch_unwind(|| Database::kdbx4_kdf_config(&bytes));
        if !matches!(metadata, Ok(Err(_))) {
            failures.push(format!("metadata width {keep}"));
        }
        let mut stage = StagedUpload::new(
            &path,
            Some(RemoteVersion::new(Sha256::digest(&before).into())),
            bytes.len() as u64,
            Sha256::digest(&bytes).into(),
        )
        .unwrap();
        stage.write_chunk(0, &bytes).unwrap();
        let stage = stage.finish(1).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            AuthoritativeVault::new(&mut vault, &master, None).commit(&stage)
        }));
        if !matches!(result, Ok(Err(ServerError::InvalidUpload))) {
            failures.push(format!("upload width {keep}"));
        }
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!backup_path_for(&path).exists());
    }
    assert!(
        failures.is_empty(),
        "short numeric fields must be rejected without panic: {failures:?}"
    );
}

#[test]
fn short_compression_fields_are_errors() {
    check_widths(None, 4);
}

#[test]
fn short_u32_kdf_fields_are_errors() {
    check_widths(Some(0x04), 4);
}

#[test]
fn short_u64_kdf_fields_are_errors() {
    check_widths(Some(0x05), 8);
}

#[test]
fn short_i32_kdf_fields_are_errors() {
    check_widths(Some(0x0c), 4);
}

#[test]
fn short_i64_kdf_fields_are_errors() {
    check_widths(Some(0x0d), 8);
}

#[test]
fn valid_header_configuration_remains_compatible() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.kdbx");
    let vault = Vault::create(
        &path,
        &password(),
        None,
        KdfParams {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    assert_eq!(
        Database::kdbx4_kdf_config(&std::fs::read(path).unwrap()).unwrap(),
        vault.database().config.kdf_config
    );
}

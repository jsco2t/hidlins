mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{Keyfile, NoRecoveryConfirmed, Vault, VaultError};

fn create(path: &Path, keyfile: &Keyfile) -> Result<Vault, VaultError> {
    Vault::create(
        path,
        &master("creation-regression-fixture"),
        Some(keyfile),
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
}

#[test]
fn missing_keyfile_does_not_reserve_the_vault_and_corrected_retry_succeeds() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    let keyfile_path = env.tempdir().join("missing.key");
    let keyfile = Keyfile::Path(keyfile_path.clone());

    assert!(matches!(
        create(&path, &keyfile),
        Err(VaultError::Io { .. })
    ));
    assert!(
        !path.exists(),
        "failed credential setup must not reserve a vault"
    );

    std::fs::write(&keyfile_path, b"synthetic keyfile fixture").unwrap();
    drop(create(&path, &keyfile).expect("retry with a readable keyfile"));
    Vault::open(
        &path,
        &master("creation-regression-fixture"),
        Some(&keyfile),
    )
    .expect("created vault reopens with the same credentials");
}

#[test]
fn directory_keyfile_does_not_reserve_the_vault() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    let keyfile = Keyfile::Path(env.tempdir().to_path_buf());

    assert!(matches!(
        create(&path, &keyfile),
        Err(VaultError::Io { .. })
    ));
    assert!(!path.exists(), "a directory is not a readable keyfile");
}

#[test]
fn unreadable_keyfile_does_not_reserve_the_vault() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    let keyfile_path = env.tempdir().join("unreadable.key");
    std::fs::write(&keyfile_path, b"synthetic keyfile fixture").unwrap();
    std::fs::set_permissions(&keyfile_path, std::fs::Permissions::from_mode(0o0)).unwrap();
    let result = create(&path, &Keyfile::Path(keyfile_path.clone()));
    std::fs::set_permissions(&keyfile_path, std::fs::Permissions::from_mode(0o600)).unwrap();

    assert!(matches!(result, Err(VaultError::Io { .. })));
    assert!(
        !path.exists(),
        "unreadable credentials must not reserve a vault"
    );
}

#[test]
fn absent_vault_cannot_be_used_as_its_own_keyfile() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");

    assert!(matches!(
        create(&path, &Keyfile::Path(path.clone())),
        Err(VaultError::Io { .. })
    ));
    assert!(
        !path.exists(),
        "a reservation must never become keyfile material"
    );
}

#[test]
fn existing_destination_is_preserved_even_with_a_missing_keyfile() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    let original = b"preexisting destination fixture";
    std::fs::write(&path, original).unwrap();

    assert!(matches!(
        create(&path, &Keyfile::Path(env.tempdir().join("missing.key"))),
        Err(VaultError::PathExists { .. })
    ));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn non_xml_keyfile_bytes_remain_valid() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    // KeePass treats arbitrary bytes, including malformed XML, as a bare
    // keyfile. Credential validation must preserve that supported format.
    let keyfile = Keyfile::Bytes(b"<KeyFile>not closed".to_vec());

    drop(create(&path, &keyfile).expect("bare keyfiles remain supported"));
    Vault::open(
        &path,
        &master("creation-regression-fixture"),
        Some(&keyfile),
    )
    .expect("bare-keyfile vault reopens");
}

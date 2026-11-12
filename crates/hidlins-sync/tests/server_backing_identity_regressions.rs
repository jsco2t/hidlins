//! Authoritative server I/O must use the file identity protected by Vault's lock.
#![cfg(unix)]

mod common;

use common::sync_env::{vault_entry_titles, SyncTestEnv};
use hidlins_core::Vault;
use hidlins_sync::backup::backup_path_for;
use hidlins_sync::protocol::RemoteVersion;
use hidlins_sync::server::{AuthoritativeVault, ServerError, StagedUpload};
use sha2::{Digest, Sha256};

fn version(bytes: &[u8]) -> RemoteVersion {
    RemoteVersion::new(Sha256::digest(bytes).into())
}

fn retargeted_vault(original: &SyncTestEnv, other: &SyncTestEnv) -> (tempfile::TempDir, Vault) {
    let directory = tempfile::tempdir().unwrap();
    let alias = directory.path().join("alias.kdbx");
    std::os::unix::fs::symlink(original.vault_path(), &alias).unwrap();
    let vault = Vault::open(&alias, &original.master(), None).unwrap();
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(other.vault_path(), &alias).unwrap();
    (directory, vault)
}

#[test]
fn head_hashes_the_locked_backing_file_after_alias_retargeting() {
    let original = SyncTestEnv::new("head-original");
    let other = SyncTestEnv::new("head-other");
    let expected = version(&std::fs::read(original.vault_path()).unwrap());
    let (_directory, mut vault) = retargeted_vault(&original, &other);
    let master = original.master();
    let host = AuthoritativeVault::new(&mut vault, &master, None);
    assert_eq!(host.head().unwrap(), expected);
    assert_eq!(host.path(), original.vault_path());
}

#[test]
fn fetch_returns_only_the_locked_backing_file_after_alias_retargeting() {
    let original = SyncTestEnv::new("fetch-original");
    let other = SyncTestEnv::new("fetch-other");
    let expected = std::fs::read(original.vault_path()).unwrap();
    let (_directory, mut vault) = retargeted_vault(&original, &other);
    let master = original.master();
    let host = AuthoritativeVault::new(&mut vault, &master, None);
    let mut snapshot = host.fetch(None).unwrap().unwrap();
    assert_eq!(snapshot.version(), version(&expected));
    assert_eq!(snapshot.length(), expected.len() as u64);
    let mut actual = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let count = snapshot.read_chunk(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        actual.extend_from_slice(&buffer[..count]);
    }
    assert_eq!(actual, expected);
    assert!(host.fetch(Some(version(&expected))).unwrap().is_none());
}

#[test]
fn commit_cas_backup_and_response_share_the_locked_backing_file() {
    let original = SyncTestEnv::new("commit-original");
    original.add_entry("base");
    let remote = original.clone_to("commit-remote");
    remote.add_entry("remote addition");
    let before = std::fs::read(original.vault_path()).unwrap();
    let upload_bytes = std::fs::read(remote.vault_path()).unwrap();
    let other = SyncTestEnv::new("commit-other");
    let other_before = std::fs::read(other.vault_path()).unwrap();
    let (directory, mut vault) = retargeted_vault(&original, &other);
    let master = original.master();
    {
        let mut host = AuthoritativeVault::new(&mut vault, &master, None);
        let mut stage = StagedUpload::new(
            host.path(),
            Some(version(&before)),
            upload_bytes.len() as u64,
            Sha256::digest(&upload_bytes).into(),
        )
        .unwrap();
        stage.write_chunk(0, &upload_bytes).unwrap();
        let stage = stage.finish(1).unwrap();
        let committed = host.commit(&stage).unwrap();
        assert_eq!(
            committed,
            version(&std::fs::read(original.vault_path()).unwrap())
        );
        assert_eq!(host.head().unwrap(), committed);
    }
    assert!(vault_entry_titles(&vault).contains(&"remote addition".to_string()));
    assert_eq!(
        std::fs::read(backup_path_for(original.vault_path())).unwrap(),
        before
    );
    assert_eq!(std::fs::read(other.vault_path()).unwrap(), other_before);
    assert!(!backup_path_for(&directory.path().join("alias.kdbx")).exists());
    assert_eq!(vault.path(), directory.path().join("alias.kdbx"));
}

#[test]
fn retargeted_alias_version_cannot_authorize_a_backing_file_commit() {
    let original = SyncTestEnv::new("cas-original");
    let before = std::fs::read(original.vault_path()).unwrap();
    let other = SyncTestEnv::new("cas-other");
    let other_before = std::fs::read(other.vault_path()).unwrap();
    let (_directory, mut vault) = retargeted_vault(&original, &other);
    let master = original.master();
    let mut host = AuthoritativeVault::new(&mut vault, &master, None);
    let mut stage = StagedUpload::new(
        host.path(),
        Some(version(&other_before)),
        before.len() as u64,
        Sha256::digest(&before).into(),
    )
    .unwrap();
    stage.write_chunk(0, &before).unwrap();
    let stage = stage.finish(1).unwrap();
    assert_eq!(host.commit(&stage), Err(ServerError::StaleVersion));
    assert_eq!(std::fs::read(original.vault_path()).unwrap(), before);
    assert_eq!(std::fs::read(other.vault_path()).unwrap(), other_before);
    assert!(!backup_path_for(original.vault_path()).exists());
}

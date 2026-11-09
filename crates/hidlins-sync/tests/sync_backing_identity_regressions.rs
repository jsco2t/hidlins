//! Sync must share the stable file identity used by Vault's lock and saves.
#![cfg(unix)]

mod common;

use common::sync_env::SyncTestEnv;
use hidlins_core::Vault;
use hidlins_sync::backup::backup_path_for;
use hidlins_sync::sync::run_state_machine;
use hidlins_sync::transport::SyncTransport;
use hidlins_sync::{MemoryTransport, SyncOptions, SyncOutcome};

#[test]
fn push_uses_open_backing_file_after_alias_is_retargeted() {
    let original = SyncTestEnv::new("original");
    original.add_entry("owned original fixture");
    let other = SyncTestEnv::new("other");
    other.add_entry("unrelated fixture");
    let expected = std::fs::read(original.vault_path()).unwrap();
    let other_before = std::fs::read(other.vault_path()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let alias = directory.path().join("alias.kdbx");
    std::os::unix::fs::symlink(original.vault_path(), &alias).unwrap();
    let mut vault = Vault::open(&alias, &original.master(), None).unwrap();
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(other.vault_path(), &alias).unwrap();
    let mut transport = MemoryTransport::new();
    let (outcome, pointers) = run_state_machine(
        &mut vault,
        &original.master(),
        None,
        &mut transport,
        None,
        None,
        SyncOptions::default(),
    )
    .unwrap();
    assert!(matches!(
        outcome,
        SyncOutcome::Pushed {
            is_first_seed: true
        }
    ));
    assert!(
        transport.fetch_if_changed(None).unwrap().unwrap().bytes == expected,
        "push must upload the encrypted file owned by the open handle"
    );
    assert_eq!(pointers.local_sha256, Some(original.local_sha()));
    assert_eq!(std::fs::read(other.vault_path()).unwrap(), other_before);
    assert_eq!(vault.path(), alias, "logical display path stays compatible");
}

#[test]
fn merge_backup_upload_and_digest_follow_the_open_backing_file() {
    let original = SyncTestEnv::new("merge-original");
    original.add_entry("base");
    let base_sha = original.local_sha();
    let remote = original.clone_to("remote");
    remote.add_entry("remote addition");
    original.add_entry("local addition");
    let before = std::fs::read(original.vault_path()).unwrap();
    let other = SyncTestEnv::new("unrelated");
    let other_before = std::fs::read(other.vault_path()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let alias = directory.path().join("alias.kdbx");
    std::os::unix::fs::symlink(original.vault_path(), &alias).unwrap();
    let mut vault = Vault::open(&alias, &original.master(), None).unwrap();
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(other.vault_path(), &alias).unwrap();
    let mut transport = MemoryTransport::new();
    transport
        .commit_conditional(&std::fs::read(remote.vault_path()).unwrap(), None)
        .unwrap();
    let (outcome, pointers) = run_state_machine(
        &mut vault,
        &original.master(),
        None,
        &mut transport,
        Some("stale".into()),
        Some(&base_sha),
        SyncOptions::default(),
    )
    .unwrap();
    assert!(matches!(outcome, SyncOutcome::Merged { .. }));
    assert!(
        transport.fetch_if_changed(None).unwrap().unwrap().bytes
            == std::fs::read(original.vault_path()).unwrap(),
        "commit must upload exactly the file the merge saved"
    );
    assert_eq!(pointers.local_sha256, Some(original.local_sha()));
    assert_eq!(
        std::fs::read(backup_path_for(original.vault_path())).unwrap(),
        before,
        "backup must be a sibling of the backing file and contain its pre-merge bytes"
    );
    assert_eq!(std::fs::read(other.vault_path()).unwrap(), other_before);
    assert!(
        !backup_path_for(&alias).exists(),
        "do not snapshot the retargetable alias"
    );
}

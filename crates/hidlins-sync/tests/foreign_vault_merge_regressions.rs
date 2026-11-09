//! A valid encrypted foreign vault is a controlled identity error, not a panic.
mod common;

use common::sync_env::SyncTestEnv;
use hidlins_core::{Database, VaultError};
use hidlins_sync::sync::run_state_machine;
use hidlins_sync::transport::SyncTransport;
use hidlins_sync::{reconcile, MemoryTransport, SyncError, SyncOptions};

#[test]
fn public_reconcile_rejects_a_foreign_root_without_mutating_either_database() {
    let mut local = Database::new();
    local
        .root_mut()
        .add_entry()
        .set_unprotected("Title", "local fixture");
    let before = local.clone();
    let remote = Database::new();
    let remote_before = remote.clone();
    assert!(
        reconcile(&mut local, &remote).is_err(),
        "unrelated roots must fail closed"
    );
    assert!(
        local == before,
        "identity rejection must not mutate local data"
    );
    assert!(remote == remote_before);
}

#[test]
fn diverged_sync_rejects_readable_foreign_vault_with_an_identity_error() {
    let local = SyncTestEnv::new("local");
    local.add_entry("base");
    let base_sha = local.local_sha();
    local.add_entry("local edit");
    let foreign = SyncTestEnv::new("foreign");
    foreign.add_entry("foreign secret fixture");
    let mut transport = MemoryTransport::new();
    let foreign_bytes = std::fs::read(foreign.vault_path()).unwrap();
    transport.commit_conditional(&foreign_bytes, None).unwrap();
    let before = std::fs::read(local.vault_path()).unwrap();
    let mut vault = local.open();
    let before_database = vault.database().clone();
    let error = run_state_machine(
        &mut vault,
        &local.master(),
        None,
        &mut transport,
        Some("stale".into()),
        Some(&base_sha),
        SyncOptions::default(),
    )
    .unwrap_err();
    assert!(
        matches!(
            error,
            SyncError::Vault(VaultError::DatabaseIdentityMismatch { .. })
        ),
        "foreign readable KDBX must produce the established identity error"
    );
    assert!(vault.database() == &before_database);
    assert!(std::fs::read(local.vault_path()).unwrap() == before);
    assert!(transport.fetch_if_changed(None).unwrap().unwrap().bytes == foreign_bytes);
}

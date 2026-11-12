//! US-045 — sync failure leaves the vault fully usable locally with a clear,
//! actionable error (FR-044; impl plan §8.4.3). Covers the authority
//! unreachable failure mode deterministically. The local vault must be unharmed.

#![allow(clippy::doc_markdown)]

mod common;

use common::sync_env::{open_vault, SyncTestEnv};
use hidlins_sync::sync::run_state_machine;
use hidlins_sync::transport::memory::MemoryTransport;
use hidlins_sync::{backup, SyncError, SyncOptions};

#[test]
fn remote_unreachable_leaves_local_vault_intact() {
    let dev = SyncTestEnv::new("work");
    dev.add_entry("local-only");
    let pre_bytes = std::fs::read(dev.vault_path()).unwrap();

    // A transport whose version check fails → SyncError::RemoteUnreachable.
    let mut transport = MemoryTransport::new();
    transport.set_fail_head(true);

    {
        let mut vault = open_vault(&dev);
        let err = run_state_machine(
            &mut vault,
            &dev.master(),
            None,
            &mut transport,
            None,
            None,
            SyncOptions::default(),
        )
        .expect_err("version-check failure must surface an error");
        assert!(
            matches!(err, SyncError::RemoteUnreachable { .. }),
            "got {err:?}"
        );
    }

    // The local vault is byte-identical and still opens; no `.kdbx.bak` was
    // written (we failed before any merge work).
    assert_eq!(
        std::fs::read(dev.vault_path()).unwrap(),
        pre_bytes,
        "local vault unchanged"
    );
    assert!(
        !backup::backup_path_for(dev.vault_path()).exists(),
        "no .kdbx.bak on a pre-merge failure"
    );
    let _ = open_vault(&dev); // still opens cleanly with the same password
}

//! US-042 — conditional commit on save (FR-042; impl plan §8.4.3). When the
//! local vault changed but the remote did not, the orchestrator uploads via a
//! conditional commit against the last-synced version (the `Pushed` outcome).

#![allow(clippy::doc_markdown)]

mod common;

use common::sync_env::{open_vault, SyncTestEnv};
use hidlins_sync::sync::run_state_machine;
use hidlins_sync::transport::memory::MemoryTransport;
use hidlins_sync::transport::SyncTransport;
use hidlins_sync::{SyncOptions, SyncOutcome};

#[test]
fn local_changed_remote_unchanged_pushes_conditionally() {
    let dev = SyncTestEnv::new("work");
    dev.add_entry("base");

    // Seed the remote with the synced state.
    let mut transport = MemoryTransport::new();
    let synced_version = {
        let bytes = std::fs::read(dev.vault_path()).unwrap();
        transport
            .commit_conditional(&bytes, None)
            .expect("seed remote")
    };
    let synced_sha = dev.local_sha();

    // Local edit; remote untouched.
    dev.add_entry("local-edit");

    let mut vault = open_vault(&dev);
    let (outcome, pointers) = run_state_machine(
        &mut vault,
        &dev.master(),
        None,
        &mut transport,
        Some(synced_version.clone().0),
        Some(&synced_sha),
        SyncOptions::default(),
    )
    .expect("push sync ok");

    assert!(
        matches!(
            outcome,
            SyncOutcome::Pushed {
                is_first_seed: false
            }
        ),
        "steady-state push expected, got {outcome:?}"
    );

    // The remote advanced past the last-synced version (the conditional commit
    // landed) and the new pointer was recorded.
    let head = transport.head().expect("head ok").expect("remote present");
    assert_ne!(
        head, synced_version,
        "conditional commit must advance the remote version"
    );
    assert_eq!(pointers.remote_version.as_deref(), Some(head.0.as_str()));
}

#[test]
fn conditional_commit_against_stale_version_is_rejected() {
    // A commit claiming a stale version is rejected; this is the
    // compare-and-swap primitive the orchestrator relies on.
    let mut transport = MemoryTransport::new();
    let v1 = transport.commit_conditional(b"v1", None).expect("seed");
    let _v2 = transport
        .commit_conditional(b"v2", Some(&v1))
        .expect("advance");
    let err = transport
        .commit_conditional(b"v3", Some(&v1))
        .expect_err("stale authority version must be rejected");
    assert!(matches!(
        err,
        hidlins_sync::MemoryTransportError::PreconditionFailed
    ));
}

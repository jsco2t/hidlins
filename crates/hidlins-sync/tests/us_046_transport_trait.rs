//! US-046 — the `SyncTransport` abstraction proof (FR-046; impl plan §8.4.3).
//!
//! The orchestrator drives a full merge cycle through a transport it knows
//! only as `impl SyncTransport` — here `MemoryTransport`, with no provider-specific, no
//! authentication implementation, no network, and no filesystem authority. That the disjoint-merge
//! scenario (US-043) works unchanged through this substitute transport is the
//! evidence that sync logic is transport-agnostic.

#![allow(clippy::doc_markdown)]

mod common;

use common::sync_env::{open_vault, vault_entry_titles, SyncTestEnv};
use hidlins_sync::sync::run_state_machine;
use hidlins_sync::transport::memory::MemoryTransport;
use hidlins_sync::transport::{SyncSnapshot, SyncTransport, SyncVersion};
use hidlins_sync::{SyncOptions, SyncOutcome};

/// Compile-time evidence that the orchestrator entry point is generic over
/// the trait, not bound to the production `provider-specificTransport`. If this fn compiles,
/// any `SyncTransport` (a Phase-4 NFS/WebDAV impl included) plugs in.
fn assert_accepts_any_transport<T: SyncTransport>(_t: &T)
where
    T::Error: Into<hidlins_sync::SyncError>,
{
}

#[test]
fn memory_transport_drives_a_full_merge_cycle() {
    let dev_a = SyncTestEnv::new("work");
    dev_a.add_entry("base");
    let base_sha = dev_a.local_sha();

    let dev_b = dev_a.clone_to("work-b");
    dev_b.add_entry("from-b");

    let mut transport = MemoryTransport::new();
    {
        let b_bytes = std::fs::read(dev_b.vault_path()).unwrap();
        transport
            .commit_conditional(&b_bytes, None)
            .expect("device B push");
    }
    assert_accepts_any_transport(&transport);

    dev_a.add_entry("from-a");
    let mut vault = open_vault(&dev_a);
    let (outcome, _) = run_state_machine(
        &mut vault,
        &dev_a.master(),
        None,
        &mut transport,
        Some("stale-version".to_string()),
        Some(&base_sha),
        SyncOptions::default(),
    )
    .expect("merge through MemoryTransport ok");

    assert!(
        matches!(outcome, SyncOutcome::Merged { .. }),
        "got {outcome:?}"
    );
    let titles = vault_entry_titles(&vault);
    for want in ["base", "from-a", "from-b"] {
        assert!(
            titles.contains(&want.to_string()),
            "missing {want}: {titles:?}"
        );
    }
}

#[test]
fn memory_transport_honours_the_trait_contract() {
    // The trait's documented promises, exercised directly: head None on
    // empty; conditional GET 304-equivalent on a matching version;
    // precondition failure on a stale conditional commit.
    let mut t = MemoryTransport::new();
    assert_eq!(t.head().unwrap(), None, "empty transport → head None");

    let v1 = t.commit_conditional(b"v1", None).unwrap();
    assert_eq!(
        t.fetch_if_changed(Some(&v1))
            .unwrap()
            .map(|s: SyncSnapshot| s.bytes),
        None,
        "matching version → no snapshot (304-equivalent)"
    );

    let stale = SyncVersion("memv-stale".to_string());
    assert!(
        t.commit_conditional(b"v2", Some(&stale)).is_err(),
        "stale conditional commit must fail the precondition"
    );
}

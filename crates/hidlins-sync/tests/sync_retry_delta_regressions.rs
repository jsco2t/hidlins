//! Successful retry summaries describe the whole cycle's net local changes.
mod common;

use common::sync_env::SyncTestEnv;
use hidlins_sync::sync::run_state_machine;
use hidlins_sync::transport::{SyncSnapshot, SyncTransport, SyncVersion};
use hidlins_sync::{MemoryTransport, MemoryTransportError, SyncOptions, SyncOutcome};

struct RacingAuthority {
    inner: MemoryTransport,
    advance_before_commit: Option<Vec<u8>>,
}

impl SyncTransport for RacingAuthority {
    type Error = MemoryTransportError;

    fn head(&mut self) -> Result<Option<SyncVersion>, Self::Error> {
        self.inner.head()
    }

    fn fetch_if_changed(
        &mut self,
        previous: Option<&SyncVersion>,
    ) -> Result<Option<SyncSnapshot>, Self::Error> {
        self.inner.fetch_if_changed(previous)
    }

    fn commit_conditional(
        &mut self,
        bytes: &[u8],
        expected: Option<&SyncVersion>,
    ) -> Result<SyncVersion, Self::Error> {
        if let Some(racing_bytes) = self.advance_before_commit.take() {
            let current = self.inner.head()?;
            self.inner
                .commit_conditional(&racing_bytes, current.as_ref())?;
        }
        // This performs the actual CAS comparison, rejecting the first
        // attempt's stale version rather than fabricating an error result.
        self.inner.commit_conditional(bytes, expected)
    }
}

fn retry_case(remove_first_import: bool) {
    let local = SyncTestEnv::new("local");
    local.add_entry("base");
    let baseline = local.local_sha();
    let first = local.clone_to("first-authority");
    let first_import = first.seed_entry("first import");
    let next = first.clone_to("next-authority");
    let second_import = if remove_first_import {
        let mut vault = next.open();
        vault.purge_entry(first_import).unwrap();
        vault.save().unwrap();
        None
    } else {
        Some(next.seed_entry("second import"))
    };
    local.add_entry("independent local edit");
    let mut inner = MemoryTransport::new();
    inner
        .commit_conditional(&std::fs::read(first.vault_path()).unwrap(), None)
        .unwrap();
    let mut authority = RacingAuthority {
        inner,
        advance_before_commit: Some(std::fs::read(next.vault_path()).unwrap()),
    };
    let mut vault = local.open();
    let (outcome, pointers) = run_state_machine(
        &mut vault,
        &local.master(),
        None,
        &mut authority,
        Some("stale".into()),
        Some(&baseline),
        SyncOptions::default(),
    )
    .unwrap();
    let SyncOutcome::Merged { delta, attempts } = outcome else {
        panic!("expected a merge")
    };
    assert_eq!(attempts, 2);
    assert!(delta.modified.is_empty());
    assert!(
        delta.removed.is_empty(),
        "an entry absent before and after is not a net removal"
    );
    if let Some(second) = second_import {
        assert_eq!(
            delta.added.len(),
            2,
            "both attempts' imports belong in the net result"
        );
        assert!(delta.added.contains(&first_import));
        assert!(delta.added.contains(&second));
        assert!(vault.get_entry(first_import).is_ok());
        assert!(vault.get_entry(second).is_ok());
    } else {
        assert!(
            delta.added.is_empty(),
            "an imported-then-purged entry cancels out"
        );
        assert!(vault.get_entry(first_import).is_err());
    }
    assert_eq!(pointers.local_sha256, Some(local.local_sha()));
    assert!(
        authority.fetch_if_changed(None).unwrap().unwrap().bytes
            == std::fs::read(local.vault_path()).unwrap()
    );
}

#[test]
fn successful_retry_reports_imports_from_both_attempts() {
    retry_case(false);
}

#[test]
fn retry_delta_cancels_an_import_removed_by_the_next_authority_version() {
    retry_case(true);
}

//! Callers may retain a Vault after a sync error and save another local edit.
mod common;

use common::sync_env::SyncTestEnv;
use hidlins_core::{EntryBuilder, Vault};
use hidlins_sync::sync::run_state_machine;
use hidlins_sync::transport::{SyncSnapshot, SyncTransport, SyncVersion};
use hidlins_sync::{MemoryTransport, MemoryTransportError, SyncError, SyncOptions};

struct Authority {
    inner: MemoryTransport,
    racing: Option<Vec<u8>>,
}

impl SyncTransport for Authority {
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
        if let Some(racing) = self.racing.take() {
            let current = self.inner.head()?;
            self.inner.commit_conditional(&racing, current.as_ref())?;
        }
        self.inner.commit_conditional(bytes, expected)
    }
}

fn retained_merge_error(retry: bool) {
    const CONFLICT_TIME: i64 = 1_800_000_000;
    let local = SyncTestEnv::new("local");
    let shared = local.seed_entry("base");
    let base_sha = local.local_sha();
    let first = local.clone_to("first");
    let first_import = first.seed_entry("first import");
    let later = first.clone_to("later");
    let later_import = later.seed_entry("partial later import");
    if retry {
        later.edit_entry_at(shared, "remote conflict", CONFLICT_TIME);
    } else {
        first.edit_entry_at(shared, "remote conflict", CONFLICT_TIME);
    }
    local.edit_entry_at(shared, "local conflict", CONFLICT_TIME);
    let original = std::fs::read(local.vault_path()).unwrap();
    let mut inner = MemoryTransport::new();
    inner
        .commit_conditional(&std::fs::read(first.vault_path()).unwrap(), None)
        .unwrap();
    let mut authority = Authority {
        inner,
        racing: retry.then(|| std::fs::read(later.vault_path()).unwrap()),
    };
    let mut vault = local.open();
    let error = run_state_machine(
        &mut vault,
        &local.master(),
        None,
        &mut authority,
        Some("stale".into()),
        Some(&base_sha),
        SyncOptions::default(),
    )
    .unwrap_err();
    assert!(matches!(error, SyncError::Unresolvable { .. }));
    let saved_bytes = std::fs::read(local.vault_path()).unwrap();
    let saved_database = Vault::open_from_bytes(&saved_bytes, &local.master(), None).unwrap();
    assert!(
        vault.database() == &saved_database,
        "retained handle must match the last complete saved database after reconciliation fails"
    );
    assert_eq!(
        vault.get_entry(first_import).is_ok(),
        retry,
        "a prior successful attempt stays saved; a failed attempt contributes nothing"
    );
    assert!(vault.get_entry(later_import).is_err());
    if !retry {
        assert!(saved_bytes == original);
    }
    let root = vault.root_group_uuid();
    vault
        .add_entry(
            root,
            EntryBuilder::credential("local work after error").build(),
        )
        .unwrap();
    vault.save().unwrap();
    let reopened = Vault::open_from_bytes(
        &std::fs::read(local.vault_path()).unwrap(),
        &local.master(),
        None,
    )
    .unwrap();
    assert!(reopened == *vault.database());
    assert!(
        common::sync_env::vault_entry_titles(&vault).contains(&"local work after error".to_owned())
    );
}

#[test]
fn failed_first_merge_leaves_retained_live_database_unchanged() {
    retained_merge_error(false);
}

#[test]
fn failed_retry_restores_the_previous_attempts_successfully_saved_database() {
    retained_merge_error(true);
}

#[cfg(debug_assertions)]
mod atomic_replace {
    use super::*;
    use hidlins_core::MasterPassword;
    use sha2::{Digest as _, Sha256};
    use std::path::PathBuf;
    use std::process::Command;

    fn replace_error(case: &str, phase: &str, installed: bool) {
        if let Some(root) = std::env::var_os("HIDLINS_SYNC_REPLACE_ERROR_ROOT") {
            let root = PathBuf::from(root);
            let master = MasterPassword::new(common::sync_env::TEST_PASSWORD.to_owned());
            let path = root.join("local.kdbx");
            let before = std::fs::read(&path).unwrap();
            let sha = common::encoding::encode_lower(&Sha256::digest(&before));
            let mut vault = Vault::open(&path, &master, None).unwrap();
            let mut transport = MemoryTransport::new();
            transport
                .commit_conditional(&std::fs::read(root.join("remote.kdbx")).unwrap(), None)
                .unwrap();
            let result = run_state_machine(
                &mut vault,
                &master,
                None,
                &mut transport,
                Some("stale".into()),
                Some(&sha),
                SyncOptions::default(),
            );
            assert!(result.is_err(), "atomic fault must be observed");
            let canonical = std::fs::read(&path).unwrap();
            let database = Vault::open_from_bytes(&canonical, &master, None).unwrap();
            assert!(
                vault.database() == &database,
                "fast-replace errors must leave live and canonical state coherent"
            );
            assert_eq!(
                common::sync_env::vault_entry_titles(&vault)
                    .contains(&"remote addition".to_owned()),
                installed
            );
            assert_eq!(canonical == before, !installed);
            return;
        }
        let local = SyncTestEnv::new("local");
        local.add_entry("base");
        let remote = local.clone_to("remote");
        remote.add_entry("remote addition");
        let root = local.vault_path().parent().unwrap();
        std::fs::copy(remote.vault_path(), root.join("remote.kdbx")).unwrap();
        let signals = root.join("signals");
        std::fs::create_dir_all(signals.join(phase)).unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", case, "--nocapture"])
            .env("HIDLINS_SYNC_REPLACE_ERROR_ROOT", root)
            .env("HIDLINS_ATOMIC_FAULT_TARGET", local.vault_path())
            .env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", &signals)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "atomic replacement child failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn failure_before_fast_replace_rename_restores_the_old_live_database() {
        replace_error(
            "atomic_replace::failure_before_fast_replace_rename_restores_the_old_live_database",
            "temp_written",
            false,
        );
    }

    #[test]
    fn failure_after_fast_replace_rename_keeps_the_installed_live_database() {
        replace_error(
            "atomic_replace::failure_after_fast_replace_rename_keeps_the_installed_live_database",
            "rename_done",
            true,
        );
    }
}

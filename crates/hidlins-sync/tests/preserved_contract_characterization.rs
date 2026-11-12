//! Executable evidence for retained sync contracts cited by the test matrix.
mod common;

use common::sync_env::SyncTestEnv;
use hidlins_core::{KdfParams, NoRecoveryConfirmed, Vault, VaultRegistry};
use hidlins_sync::{
    backup,
    config::local::LocalSyncConfig,
    identity::SyncRole,
    protocol::RemoteVersion,
    sync::run_state_machine,
    transport::{SyncSnapshot, SyncTransport, SyncVersion},
    MemoryTransport, MemoryTransportError, SyncError, SyncOptions, SyncOutcome,
};

fn diverged() -> (SyncTestEnv, MemoryTransport, String, Vec<u8>) {
    let local = SyncTestEnv::new("local");
    local.add_entry("base");
    let baseline = local.local_sha();
    let remote = local.clone_to("authority");
    remote.add_entry("authority edit");
    local.add_entry("local edit");
    let before = std::fs::read(local.vault_path()).unwrap();
    let mut transport = MemoryTransport::new();
    transport
        .commit_conditional(&std::fs::read(remote.vault_path()).unwrap(), None)
        .unwrap();
    (local, transport, baseline, before)
}

struct AlwaysAdvancing {
    inner: MemoryTransport,
    fetches: usize,
    commits: usize,
}

impl SyncTransport for AlwaysAdvancing {
    type Error = MemoryTransportError;

    fn head(&mut self) -> Result<Option<SyncVersion>, Self::Error> {
        self.inner.head()
    }

    fn fetch_if_changed(
        &mut self,
        previous: Option<&SyncVersion>,
    ) -> Result<Option<SyncSnapshot>, Self::Error> {
        self.fetches += 1;
        self.inner.fetch_if_changed(previous)
    }

    fn commit_conditional(
        &mut self,
        bytes: &[u8],
        expected: Option<&SyncVersion>,
    ) -> Result<SyncVersion, Self::Error> {
        self.commits += 1;
        assert!(
            self.commits <= 3,
            "sync retries exceeded the test safety bound"
        );
        let current = self.inner.fetch_if_changed(None)?.unwrap();
        self.inner
            .commit_conditional(&current.bytes, Some(&current.version))?;
        // Use the actual CAS primitive against the now-stale fetched version.
        self.inner.commit_conditional(bytes, expected)
    }
}

#[test]
fn default_cas_exhausts_after_two_real_races_and_preserves_original_backup() {
    let (local, inner, baseline, before) = diverged();
    let mut authority = AlwaysAdvancing {
        inner,
        fetches: 0,
        commits: 0,
    };
    let mut vault = local.open();
    let error = run_state_machine(
        &mut vault,
        &local.master(),
        None,
        &mut authority,
        Some("previous authority version".into()),
        Some(&baseline),
        SyncOptions::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SyncError::ConditionalCommitExhausted { attempts: 2 }
    ));
    assert_eq!((authority.fetches, authority.commits), (2, 2));
    assert_eq!(
        std::fs::read(backup::backup_path_for(local.vault_path())).unwrap(),
        before
    );
    drop(vault);
    let reopened = local.open();
    let titles = common::sync_env::vault_entry_titles(&reopened);
    assert!(titles.contains(&"local edit".to_string()));
    assert!(titles.contains(&"authority edit".to_string()));
}

fn kdf_mismatch(diverged: bool) {
    let local = SyncTestEnv::new("local");
    local.add_entry("base");
    let baseline = local.local_sha();
    let remote = local.clone_to("authority");
    remote.add_entry("authority edit");
    let template_directory = tempfile::tempdir().unwrap();
    let template = Vault::create(
        &template_directory.path().join("kdf-template.kdbx"),
        &local.master(),
        None,
        KdfParams {
            memory_kib: 2_048,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    {
        let mut authority = remote.open();
        authority.database_mut().config.kdf_config = template.database().config.kdf_config.clone();
        authority.save().unwrap();
        assert_eq!(
            authority.database().root().id().uuid(),
            local.open().database().root().id().uuid(),
            "the mismatch fixture must retain the same logical vault"
        );
    }
    if diverged {
        local.add_entry("local edit");
    }
    let before = std::fs::read(local.vault_path()).unwrap();
    let mut transport = MemoryTransport::new();
    transport
        .commit_conditional(&std::fs::read(remote.vault_path()).unwrap(), None)
        .unwrap();
    let remote_before = transport.fetch_if_changed(None).unwrap().unwrap().bytes;
    let mut vault = local.open();
    let error = run_state_machine(
        &mut vault,
        &local.master(),
        None,
        &mut transport,
        Some("previous authority version".into()),
        Some(&baseline),
        SyncOptions::default(),
    )
    .unwrap_err();
    assert!(matches!(error, SyncError::MasterPasswordMismatch));
    assert_eq!(std::fs::read(local.vault_path()).unwrap(), before);
    assert_eq!(
        transport.fetch_if_changed(None).unwrap().unwrap().bytes,
        remote_before
    );
    let backup = backup::backup_path_for(local.vault_path());
    if diverged {
        assert_eq!(std::fs::read(backup).unwrap(), before);
    } else {
        assert!(!backup.exists(), "fast replace validates KDF before backup");
    }
}

#[test]
fn fast_replace_rejects_same_vault_kdf_mismatch_without_replacing_local_bytes() {
    kdf_mismatch(false);
}

#[test]
fn diverged_merge_rejects_same_vault_kdf_mismatch_and_keeps_pre_merge_backup() {
    kdf_mismatch(true);
}

#[test]
fn successful_divergence_returns_both_current_pointers_for_noop_resync() {
    let (local, mut transport, baseline, _) = diverged();
    let mut vault = local.open();
    let (outcome, pointers) = run_state_machine(
        &mut vault,
        &local.master(),
        None,
        &mut transport,
        Some("previous authority version".into()),
        Some(&baseline),
        SyncOptions::default(),
    )
    .unwrap();
    assert!(matches!(outcome, SyncOutcome::Merged { attempts: 1, .. }));
    assert_eq!(
        pointers.remote_version,
        transport.head().unwrap().map(|version| version.0)
    );
    assert_eq!(pointers.local_sha256, Some(local.local_sha()));
    let (next, unchanged) = run_state_machine(
        &mut vault,
        &local.master(),
        None,
        &mut transport,
        pointers.remote_version.clone(),
        pointers.local_sha256.as_deref(),
        SyncOptions::default(),
    )
    .unwrap();
    assert!(matches!(next, SyncOutcome::AlreadyInSync));
    assert_eq!(unchanged.remote_version, pointers.remote_version);
    assert_eq!(unchanged.local_sha256, pointers.local_sha256);
}

#[test]
fn local_configuration_persists_both_distinct_divergence_versions_together() {
    let mut local = SyncTestEnv::new("local");
    let name = local.name().to_string();
    let paths = local.registry().paths().clone();
    let mut config = LocalSyncConfig::create(&name, SyncRole::Client, &local.master()).unwrap();
    let remote = RemoteVersion::new([11; 32]);
    let encrypted_local = RemoteVersion::new([22; 32]);
    config.set_sync_versions(Some(remote), Some(encrypted_local));
    config.persist(local.registry_mut(), &name).unwrap();
    let reloaded = VaultRegistry::load(paths).unwrap();
    let persisted = LocalSyncConfig::from_vault_entry(reloaded.get(&name).unwrap()).unwrap();
    assert_eq!(persisted.last_synced_remote_version(), Some(remote));
    assert_eq!(persisted.last_synced_local_version(), Some(encrypted_local));
}

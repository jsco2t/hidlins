#![allow(dead_code)]

use std::sync::mpsc;

use hidlins_api::dto::LockEvent;
use hidlins_api::event::{EventSink, MpscEventSink};
use hidlins_core::{HidlinsPaths, KdfParams, MasterPassword, NoRecoveryConfirmed, Vault};
use tempfile::TempDir;

pub struct TestEnv {
    paths: HidlinsPaths,
    tempdir: TempDir,
}

impl TestEnv {
    pub fn new() -> Self {
        let tempdir = TempDir::new().expect("test env: create tempdir");
        let state = tempdir.path().join("state");
        let paths = HidlinsPaths::with_state_dir(state);
        Self { paths, tempdir }
    }

    pub fn paths(&self) -> &HidlinsPaths {
        &self.paths
    }

    pub fn paths_clone(&self) -> HidlinsPaths {
        HidlinsPaths::with_state_dir(self.paths.state_dir().to_path_buf())
    }

    pub fn tempdir(&self) -> &std::path::Path {
        self.tempdir.path()
    }
}

pub fn master(value: &str) -> MasterPassword {
    MasterPassword::new(value.to_string())
}

pub fn fast_kdf() -> KdfParams {
    KdfParams {
        memory_kib: 1_024,
        iterations: 1,
        parallelism: 1,
    }
}

pub fn create_test_vault(env: &TestEnv, name: &str, password: &str) -> std::path::PathBuf {
    let paths = env.paths();
    paths.ensure_exists().expect("ensure state dir");
    let vault_path = paths.state_dir().join(format!("{name}.kdbx"));
    let master = master(password);
    let _vault = Vault::create(
        &vault_path,
        &master,
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .expect("create test vault");
    vault_path
}

/// Creates a keyfile-protected vault, returning `(vault_path, keyfile_bytes)`.
///
/// The keyfile is written to disk (so the `KeyfileRef::Path` arm can be
/// exercised) and its bytes returned (so the mobile `KeyfileRef::Bytes` arm
/// can be too).
pub fn create_test_vault_with_keyfile(
    env: &TestEnv,
    name: &str,
    password: &str,
) -> (std::path::PathBuf, std::path::PathBuf, Vec<u8>) {
    use hidlins_core::Keyfile;

    let paths = env.paths();
    paths.ensure_exists().expect("ensure state dir");
    let vault_path = paths.state_dir().join(format!("{name}.kdbx"));
    let keyfile_path = paths.state_dir().join(format!("{name}.key"));

    let keyfile_bytes: Vec<u8> = (0u8..=255).collect();
    std::fs::write(&keyfile_path, &keyfile_bytes).expect("write keyfile");

    let master = master(password);
    let keyfile = Keyfile::Path(keyfile_path.clone());
    let _vault = Vault::create(
        &vault_path,
        &master,
        Some(&keyfile),
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .expect("create keyfile-protected test vault");

    (vault_path, keyfile_path, keyfile_bytes)
}

pub fn register_vault(env: &TestEnv, name: &str, vault_path: &std::path::Path) {
    use hidlins_core::{RegisteredVault, VaultRegistry};
    let mut registry = VaultRegistry::load(env.paths_clone()).expect("load registry");
    let rv = RegisteredVault {
        name: name.to_string(),
        path: vault_path.to_path_buf(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        keyfile_path: None,
        extra: toml::Table::new(),
    };
    registry.register(rv).expect("register vault");
    registry.save().expect("save registry");
}

pub struct RecordingLockSink {
    rx: mpsc::Receiver<LockEvent>,
}

impl RecordingLockSink {
    pub fn new() -> (Box<dyn EventSink<LockEvent>>, Self) {
        let (tx, rx) = mpsc::channel();
        let sink: Box<dyn EventSink<LockEvent>> = Box::new(MpscEventSink::new(tx));
        (sink, Self { rx })
    }

    pub fn try_recv(&self) -> Option<LockEvent> {
        self.rx.try_recv().ok()
    }

    pub fn drain(&self) -> Vec<LockEvent> {
        let mut events = Vec::new();
        while let Ok(e) = self.rx.try_recv() {
            events.push(e);
        }
        events
    }

    /// True once the session has dropped its sink (stream closed).
    ///
    /// Panics on a non-empty buffer instead of silently consuming an event:
    /// a quiet `try_recv` here would make a later `drain()` mysteriously
    /// miss whatever it swallowed. Call `drain()` first.
    pub fn is_closed(&self) -> bool {
        match self.rx.try_recv() {
            Err(mpsc::TryRecvError::Disconnected) => true,
            Err(mpsc::TryRecvError::Empty) => false,
            Ok(event) => panic!("is_closed() with buffered event {event:?} — drain() first"),
        }
    }
}

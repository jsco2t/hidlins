//! A post-rename registry error must not delete the registered ciphertext.
#![cfg(debug_assertions)]

use std::{path::Path, process::Command, time::Instant};

use hidlins_core::{
    HidlinsPaths, KdfParams, MasterPassword, NoRecoveryConfirmed, Vault, VaultRegistry,
};
use hidlins_sync::{
    client::{import_paired_vault, ImportError},
    config::local::LocalSyncConfig,
    identity::{PublicIdentity, SyncRole},
    noise::NoiseKeypair,
    pairing::PairingWindow,
};

fn password() -> MasterPassword {
    MasterPassword::new("synthetic import registry fixture".to_string())
}

fn active_config() -> LocalSyncConfig {
    let mut client = LocalSyncConfig::create("imported", SyncRole::Client, &password()).unwrap();
    let server = PublicIdentity::new(NoiseKeypair::generate().unwrap().public_key());
    let now = Instant::now();
    let mut window = PairingWindow::open_at(now);
    let sas = window.begin_candidate(server, [9; 32], now).unwrap();
    let confirmation = window.confirm(sas, now).unwrap();
    let transaction = confirmation
        .transaction(server, client.identity().public_key())
        .unwrap();
    client
        .prepare_server(
            &transaction,
            &confirmation,
            "Server".to_string(),
            1000,
            1180,
        )
        .unwrap();
    client.activate_server(&transaction, 1001).unwrap();
    client
}

fn paths(directory: &Path) -> HidlinsPaths {
    HidlinsPaths::with_state_dir(directory.join("state"))
}

#[test]
#[ignore = "child-only exact-target fault helper; inert without its explicit fixture environment"]
fn import_worker() {
    let Some(directory) = std::env::var_os("HIDLINS_IMPORT_COMMIT_FIXTURE") else {
        return;
    };
    let directory = Path::new(&directory);
    let source = directory.join("source.kdbx");
    drop(
        Vault::create(
            &source,
            &password(),
            None,
            KdfParams {
                memory_kib: 1024,
                iterations: 1,
                parallelism: 1,
            },
            NoRecoveryConfirmed::yes(),
        )
        .unwrap(),
    );
    let mut registry = VaultRegistry::with_paths(paths(directory));
    let result = import_paired_vault(
        &std::fs::read(source).unwrap(),
        &directory.join("imported.kdbx"),
        "imported",
        &password(),
        None,
        &active_config(),
        &mut registry,
    );
    if std::env::var_os("HIDLINS_ATOMIC_FAULT_TARGET").is_some() {
        assert!(
            matches!(result, Err(ImportError::Registry)),
            "fault must be an actual registry write error"
        );
    } else {
        result.unwrap();
    }
}

fn run_import(directory: &Path, phase: Option<&str>) {
    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args(["--exact", "import_worker", "--ignored", "--nocapture"]);
    child.env("HIDLINS_IMPORT_COMMIT_FIXTURE", directory);
    if let Some(phase) = phase {
        let signals = directory.join("signals");
        std::fs::create_dir(&signals).unwrap();
        // Earlier phase signals are files. This phase's directory forces a
        // real I/O error exactly before/after the registry's atomic rename.
        std::fs::create_dir(signals.join(phase)).unwrap();
        child.env(
            "HIDLINS_ATOMIC_FAULT_TARGET",
            paths(directory).vaults_toml(),
        );
        child.env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", signals);
    }
    let output = child.output().unwrap();
    assert!(
        output.status.success(),
        "isolated helper failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn post_rename_error_preserves_the_complete_registered_vault() {
    let directory = tempfile::tempdir().unwrap();
    run_import(directory.path(), Some("rename_done"));
    let registry = VaultRegistry::load(paths(directory.path())).unwrap();
    let entry = registry
        .get("imported")
        .expect("registry rename committed before error");
    assert!(
        entry.path.is_file(),
        "post-commit error must not delete the registered vault"
    );
    assert_eq!(
        std::fs::read(&entry.path).unwrap(),
        std::fs::read(directory.path().join("source.kdbx")).unwrap()
    );
    drop(Vault::open(&entry.path, &password(), None).unwrap());
}

#[test]
fn pre_rename_error_removes_only_the_unregistered_import() {
    let directory = tempfile::tempdir().unwrap();
    run_import(directory.path(), Some("fsync_done"));
    let registry = VaultRegistry::load(paths(directory.path())).unwrap();
    assert!(registry.get("imported").is_none());
    assert!(!directory.path().join("imported.kdbx").exists());
}

#[test]
fn successful_import_keeps_complete_registration_and_ciphertext() {
    let directory = tempfile::tempdir().unwrap();
    run_import(directory.path(), None);
    let registry = VaultRegistry::load(paths(directory.path())).unwrap();
    let entry = registry.get("imported").unwrap();
    assert_eq!(
        std::fs::read(&entry.path).unwrap(),
        std::fs::read(directory.path().join("source.kdbx")).unwrap()
    );
    drop(Vault::open(&entry.path, &password(), None).unwrap());
}

//! Live registry state must track the rename commit, including durability errors.
#![cfg(debug_assertions)]

use std::{path::Path, process::Command};

use hidlins_core::{HidlinsPaths, RegisteredVault, VaultError, VaultRegistry};

fn paths(directory: &Path) -> HidlinsPaths {
    HidlinsPaths::with_state_dir(directory.join("state"))
}

#[test]
#[ignore = "isolated fault helper; inert without explicit fixture environment"]
fn registry_worker() {
    let Some(directory) = std::env::var_os("HIDLINS_REGISTRY_COMMIT_FIXTURE") else {
        return;
    };
    let directory = Path::new(&directory);
    let mut registry = VaultRegistry::with_paths(paths(directory));
    let entry = RegisteredVault {
        name: "imported".to_string(),
        path: directory.join("imported.kdbx"),
        created_at: "2026-11-04T00:00:00Z".to_string(),
        keyfile_path: None,
        extra: toml::Table::new(),
    };
    let result = registry.register_and_save(entry);
    assert!(
        matches!(result, Err(VaultError::Io { .. })),
        "fixture must reach the actual atomic fault"
    );
    let installed = std::env::var("HIDLINS_REGISTRY_COMMIT_PHASE").unwrap() == "rename_done";
    let on_disk = VaultRegistry::load(paths(directory)).unwrap();
    assert_eq!(on_disk.get("imported").is_some(), installed);
    assert_eq!(
        registry.get("imported").is_some(),
        installed,
        "live registry must reflect whether its own rename installed the registration"
    );
    // A later save may repeat the injected I/O error, but cannot incorrectly
    // reject the caller's own committed generation as an unrelated writer.
    assert!(!matches!(registry.save(), Err(VaultError::RegistryChanged)));
}

fn run_phase(phase: &str) {
    let directory = tempfile::tempdir().unwrap();
    let signals = directory.path().join("signals");
    std::fs::create_dir(&signals).unwrap();
    std::fs::create_dir(signals.join(phase)).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "registry_worker", "--ignored", "--nocapture"])
        .env("HIDLINS_REGISTRY_COMMIT_FIXTURE", directory.path())
        .env("HIDLINS_REGISTRY_COMMIT_PHASE", phase)
        .env(
            "HIDLINS_ATOMIC_FAULT_TARGET",
            paths(directory.path()).vaults_toml(),
        )
        .env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", signals)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated registry assertion failed: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn post_rename_error_keeps_live_registration_and_baseline_current() {
    run_phase("rename_done");
}

#[test]
fn pre_rename_error_leaves_live_registration_unchanged() {
    run_phase("fsync_done");
}

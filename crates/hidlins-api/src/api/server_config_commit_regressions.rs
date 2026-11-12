//! Installed registry identity owns authority even when durability reports an error.
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;

use hidlins_core::{HidlinsPaths, VaultRegistry};

use super::*;
use crate::fixtures::create_test_vault;

const WORKER: &str = "api::sync::server_config_commit_regressions::registry_fault_worker";

fn paths(directory: &Path) -> HidlinsPaths {
    HidlinsPaths::with_state_dir(directory.join("state"))
}

fn identity(registry: &VaultRegistry) -> [u8; 32] {
    LocalSyncConfig::from_vault_entry(registry.get("vault").unwrap())
        .unwrap()
        .identity()
        .public_key()
        .into_bytes()
}

#[test]
#[ignore = "isolated fault worker; outer cases execute it with explicit fixture environment"]
fn registry_fault_worker() {
    let Some(directory) = std::env::var_os("HIDLINS_CONFIG_COMMIT_FIXTURE") else {
        return;
    };
    let directory = Path::new(&directory);
    let paths = paths(directory);
    let phase = std::env::var("HIDLINS_CONFIG_COMMIT_PHASE").unwrap();
    let vault_path = create_test_vault(&paths, "vault", "fixture password");
    let session = AppSession::for_test(paths.clone()).unwrap();
    session
        .unlock("vault".into(), "fixture password".into(), None)
        .unwrap();
    session
        .configure_local_sync(LocalSyncRoleDto::Server)
        .unwrap();
    let endpoint = (0..8)
        .find_map(|_| {
            let probe = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = probe.local_addr().unwrap().port();
            drop(probe);
            session
                .start_sync_server(LocalEndpointDto {
                    address: "127.0.0.1".into(),
                    port,
                    scope_id: 0,
                })
                .ok()
        })
        .expect("actual loopback API server started")
        .checked()
        .unwrap();
    session.open_pairing_window().unwrap();
    assert!(TcpStream::connect(endpoint.socket_addr()).is_ok());
    let original = identity(&session.inner.lock().unwrap().registry);
    let encrypted_vault = std::fs::read(&vault_path).unwrap();

    // Setup writes completed normally. Turn only the selected signal file
    // into a directory so the next actual atomic transaction faults there.
    let signal = directory.join("signals").join(&phase);
    std::fs::remove_file(&signal).unwrap();
    std::fs::create_dir(&signal).unwrap();
    let result = session.configure_local_sync(LocalSyncRoleDto::Server);
    assert!(matches!(
        result,
        Err(HidlinsApiError::LocalSyncConfiguration)
    ));
    let disk = identity(&VaultRegistry::load(paths).unwrap());
    let installed = phase == "rename_done";
    assert_eq!(
        disk != original,
        installed,
        "actual rename boundary reached"
    );
    assert_eq!(
        identity(&session.inner.lock().unwrap().registry),
        disk,
        "live registry must follow the identity actually installed by rename"
    );
    let status = session.local_sync_status().unwrap();
    assert_eq!(status.server_running, !installed);
    assert_eq!(status.server_enabled, !installed);
    assert_eq!(status.pairing_open, !installed);
    assert_eq!(
        TcpStream::connect(endpoint.socket_addr()).is_ok(),
        !installed
    );
    assert!(session.has_vault() && session.has_credentials());
    assert_eq!(std::fs::read(vault_path).unwrap(), encrypted_vault);
}

fn run_phase(phase: &str) {
    let directory = tempfile::tempdir().unwrap();
    let signals = directory.path().join("signals");
    std::fs::create_dir(&signals).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", WORKER, "--ignored", "--nocapture"])
        .env("HIDLINS_CONFIG_COMMIT_FIXTURE", directory.path())
        .env("HIDLINS_CONFIG_COMMIT_PHASE", phase)
        .env(
            "HIDLINS_ATOMIC_FAULT_TARGET",
            paths(directory.path()).vaults_toml(),
        )
        .env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", signals)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated API configuration assertion failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn post_rename_failure_retires_installed_old_authority() {
    run_phase("rename_done");
}

#[test]
fn pre_fsync_failure_preserves_unchanged_authority() {
    run_phase("temp_written");
}

#[test]
fn pre_rename_failure_preserves_unchanged_authority() {
    run_phase("fsync_done");
}

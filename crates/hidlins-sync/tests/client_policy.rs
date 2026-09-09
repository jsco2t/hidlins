use std::{
    net::{IpAddr, Ipv4Addr, TcpListener},
    thread,
    time::{Duration, Instant},
};

use hidlins_core::{
    HidlinsPaths, KdfParams, MasterPassword, NoRecoveryConfirmed, Vault, VaultRegistry,
};
use hidlins_sync::{
    address::LocalEndpoint,
    client::{
        import_paired_vault, ImportError, LanCancellation, LanError, LanTransport,
        StartupSyncTracker, SyncTrigger,
    },
    config::local::LocalSyncConfig,
    identity::SyncRole,
    pairing::PairingWindow,
    SyncTransport,
};
use tempfile::TempDir;

fn password() -> MasterPassword {
    MasterPassword::new("import password".to_string())
}

// LNS-CLIENT-001
#[test]
fn client_cancellation_preempts_candidate_connection() {
    let cancellation = LanCancellation::default();
    cancellation.cancel();
    let identity = hidlins_sync::noise::NoiseKeypair::generate().unwrap();
    let endpoint: LocalEndpoint = "127.0.0.1:9".parse().unwrap();
    let mut transport = LanTransport::new(identity, [7; 32], Some(endpoint), [])
        .unwrap()
        .with_cancellation(cancellation);
    assert_eq!(transport.head().unwrap_err(), LanError::Cancelled);
}

// LNS-SEC-007
// LNS-REVIEW-006
#[test]
fn client_noise_read_is_bounded_by_the_absolute_handshake_deadline() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let endpoint = LocalEndpoint::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port, 0).unwrap();
    let stalled = thread::spawn(move || {
        let (_stream, _) = listener.accept().unwrap();
        thread::sleep(Duration::from_secs(7));
    });
    let identity = hidlins_sync::noise::NoiseKeypair::generate().unwrap();
    let mut transport = LanTransport::new(identity, [7; 32], Some(endpoint), []).unwrap();
    let started = Instant::now();
    assert!(matches!(
        transport.head(),
        Err(LanError::Authentication | LanError::Unreachable | LanError::TimedOut)
    ));
    assert!(started.elapsed() < Duration::from_millis(6_500));
    stalled.join().unwrap();
}

fn paired_client() -> LocalSyncConfig {
    let master = password();
    let mut server = LocalSyncConfig::create("source", SyncRole::Server, &master).unwrap();
    let mut client = LocalSyncConfig::create("imported", SyncRole::Client, &master).unwrap();
    let now = Instant::now();
    let hash = [9; 32];
    let mut server_window = PairingWindow::open_at(now);
    let mut client_window = PairingWindow::open_at(now);
    let server_sas = server_window
        .begin_candidate(client.identity().public_key(), hash, now)
        .unwrap();
    let client_sas = client_window
        .begin_candidate(server.identity().public_key(), hash, now)
        .unwrap();
    let server_confirmation = server_window.confirm(server_sas, now).unwrap();
    let client_confirmation = client_window.confirm(client_sas, now).unwrap();
    let transaction = client_confirmation
        .transaction(
            server.identity().public_key(),
            client.identity().public_key(),
        )
        .unwrap();
    server
        .prepare_client(
            &transaction,
            &server_confirmation,
            "Client".to_string(),
            1_000,
            1_180,
        )
        .unwrap();
    client
        .prepare_server(
            &transaction,
            &client_confirmation,
            "Server".to_string(),
            1_000,
            1_180,
        )
        .unwrap();
    server.activate_client(&transaction, 1_001).unwrap();
    client.activate_server(&transaction, 1_001).unwrap();
    client
}

// LNS-CLIENT-002
#[test]
fn client_startup_tracker_attempts_only_first_configured_unlock_per_process() {
    let mut tracker = StartupSyncTracker::default();
    assert_eq!(hidlins_sync::SyncOptions::default().max_commit_attempts, 2);
    assert!(!tracker.should_attempt("work", false, SyncTrigger::StartupUnlock));
    assert!(tracker.should_attempt("work", true, SyncTrigger::StartupUnlock));
    assert!(!tracker.should_attempt("work", true, SyncTrigger::StartupUnlock));
    assert!(!tracker.should_attempt("work", true, SyncTrigger::PostMutation));
    assert!(tracker.should_attempt("work", true, SyncTrigger::Manual));
    assert!(tracker.should_attempt("work", true, SyncTrigger::Manual));
    assert!(tracker.should_attempt("personal", true, SyncTrigger::StartupUnlock));
}

// LNS-CLIENT-003
#[test]
fn pair_and_import_installs_only_valid_complete_registered_kdbx() {
    let directory = TempDir::new().unwrap();
    let source_path = directory.path().join("source.kdbx");
    let source = Vault::create(
        &source_path,
        &password(),
        None,
        KdfParams {
            memory_kib: 1_024,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let bytes = std::fs::read(source.path()).unwrap();
    drop(source);
    let paths = HidlinsPaths::with_state_dir(directory.path().join("state"));
    let mut registry = VaultRegistry::with_paths(paths.clone());
    let target = directory.path().join("imported.kdbx");
    let config = paired_client();

    import_paired_vault(
        &bytes,
        &target,
        "imported",
        &password(),
        None,
        &config,
        &mut registry,
    )
    .unwrap();
    assert!(target.exists());
    Vault::open(&target, &password(), None).unwrap();
    let reloaded = VaultRegistry::load(paths).unwrap();
    assert!(reloaded.get("imported").is_some());
}

// LNS-CLIENT-004
#[test]
fn pair_and_import_failure_leaves_no_file_or_registration() {
    let directory = TempDir::new().unwrap();
    let paths = HidlinsPaths::with_state_dir(directory.path().join("state"));
    let mut registry = VaultRegistry::with_paths(paths.clone());
    let target = directory.path().join("rejected.kdbx");
    let config = paired_client();
    assert!(matches!(
        import_paired_vault(
            b"not a vault",
            &target,
            "rejected",
            &password(),
            None,
            &config,
            &mut registry,
        ),
        Err(ImportError::InvalidVault)
    ));
    assert!(!target.exists());
    assert!(VaultRegistry::load(paths)
        .unwrap()
        .get("rejected")
        .is_none());
}

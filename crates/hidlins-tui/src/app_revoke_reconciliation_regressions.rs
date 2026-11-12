use super::*;
use hidlins_sync::client::{LanCancellation, LanTransport};
use hidlins_sync::pairing::PairingWindow;
use hidlins_sync::SyncTransport;
use std::path::Path;
use std::process::Command as ProcessCommand;

const WORKER: &str = "app::tests::revoke_reconciliation_regressions::fault_worker";
fn paths(directory: &Path) -> HidlinsPaths {
    HidlinsPaths::with_state_dir(directory.join("state"))
}

fn fixture(directory: &Path) -> (App, LocalSyncConfig, [u8; 32]) {
    let paths = paths(directory);
    let path = directory.join("personal.kdbx");
    drop(
        Vault::create(
            &path,
            &MasterPassword::new(PASSWORD.into()),
            None,
            fast_kdf(),
            NoRecoveryConfirmed::yes(),
        )
        .unwrap(),
    );
    let mut registry = VaultRegistry::with_paths(paths.clone());
    registry
        .register(RegisteredVault {
            name: "personal".into(),
            path,
            created_at: "2026-01-01T00:00:00Z".into(),
            keyfile_path: None,
            extra: toml::Table::new(),
        })
        .unwrap();
    registry.save().unwrap();
    let mut app = App::from_registry(registry, paths, AutoLockConfig::default()).unwrap();
    unlock(&mut app, PASSWORD);
    let master = MasterPassword::new(PASSWORD.into());
    let client = LocalSyncConfig::create("client", SyncRole::Client, &master).unwrap();
    let peer = client.identity().public_key();
    let mut server = LocalSyncConfig::configure(
        app.registry_mut().unwrap(),
        "personal",
        SyncRole::Server,
        &master,
    )
    .unwrap();
    let now = Instant::now();
    let mut window = PairingWindow::open_at(now);
    let sas = window.begin_candidate(peer, [73; 32], now).unwrap();
    let confirmation = window.confirm(sas, now).unwrap();
    let transaction = confirmation
        .transaction(server.identity().public_key(), peer)
        .unwrap();
    server
        .prepare_client(
            &transaction,
            &confirmation,
            "confirmed client".into(),
            1000,
            1180,
        )
        .unwrap();
    server.activate_client(&transaction, 1001).unwrap();
    server
        .persist(app.registry_mut().unwrap(), "personal")
        .unwrap();
    app.start_server(master).unwrap();
    (app, client, server.identity().public_key().into_bytes())
}

fn peer_can_head(app: &mut App, client: &LocalSyncConfig, server_key: [u8; 32]) -> bool {
    let identity = client
        .identity()
        .unlock(
            "client",
            SyncRole::Client,
            &MasterPassword::new(PASSWORD.into()),
        )
        .unwrap();
    let endpoint = app.server.as_ref().unwrap().endpoint().parse().unwrap();
    let cancellation = LanCancellation::default();
    let worker_cancel = cancellation.clone();
    let worker = std::thread::spawn(move || {
        LanTransport::new(identity, server_key, Some(endpoint), [])
            .unwrap()
            .with_cancellation(worker_cancel)
            .head()
            .is_ok()
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while (!worker.is_finished() || app.is_syncing()) && Instant::now() < deadline {
        app.tick(Instant::now());
        std::thread::yield_now();
    }
    let completed = worker.is_finished() && !app.is_syncing();
    if !completed {
        cancellation.cancel();
    }
    let result = worker.join().unwrap();
    assert!(
        completed,
        "actual TUI authenticated request completed; timeout is not behavioral red"
    );
    result
}

#[test]
#[ignore = "isolated fault worker; outer cases execute it explicitly"]
fn fault_worker() {
    let Some(directory) = std::env::var_os("HIDLINS_TUI_REVOKE_FIXTURE") else {
        return;
    };
    let directory = Path::new(&directory);
    let phase = std::env::var("HIDLINS_TUI_REVOKE_PHASE").unwrap();
    let (mut app, client, server_key) = fixture(directory);
    assert!(
        peer_can_head(&mut app, &client, server_key),
        "confirmed client succeeds before revocation"
    );
    let path = app.vault().unwrap().backing_path().to_path_buf();
    let before = std::fs::read(&path).unwrap();
    let signal = directory.join("signals").join(&phase);
    std::fs::remove_file(&signal).unwrap();
    std::fs::create_dir(&signal).unwrap();
    app.revoke_peer(0);
    assert_eq!(
        app.status_bar.current(),
        Some("Could not revoke peer."),
        "original durability error remains visible"
    );
    let installed = phase == "rename_done";
    let peer = client.identity().public_key();
    assert_eq!(
        !app.selected_local_config().unwrap().authorizes(peer),
        installed
    );
    let disk = VaultRegistry::load(app.paths.clone()).unwrap();
    assert_eq!(
        !LocalSyncConfig::from_vault_entry(disk.get("personal").unwrap())
            .unwrap()
            .authorizes(peer),
        installed
    );
    assert_eq!(
        peer_can_head(&mut app, &client, server_key),
        !installed,
        "running TUI authority must follow installed revocation despite durability error"
    );
    assert!(app.server.is_some() && app.vault().is_some());
    assert!(std::fs::read(path).unwrap() == before);
    app.lock_app();
}
fn run_phase(phase: &str) {
    let directory = tempfile::tempdir().unwrap();
    let signals = directory.path().join("signals");
    std::fs::create_dir(&signals).unwrap();
    let output = ProcessCommand::new(std::env::current_exe().unwrap())
        .args(["--exact", WORKER, "--ignored", "--nocapture"])
        .env("HIDLINS_TUI_REVOKE_FIXTURE", directory.path())
        .env("HIDLINS_TUI_REVOKE_PHASE", phase)
        .env(
            "HIDLINS_ATOMIC_FAULT_TARGET",
            paths(directory.path()).vaults_toml(),
        )
        .env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", signals)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("running 1 test"),
        "isolated worker actually selected"
    );
    assert!(
        output.status.success(),
        "isolated TUI revocation assertion failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn post_rename_error_revokes_running_peer() {
    run_phase("rename_done");
}
#[test]
fn pre_fsync_error_keeps_peer_usable() {
    run_phase("temp_written");
}
#[test]
fn pre_rename_error_keeps_peer_usable() {
    run_phase("fsync_done");
}

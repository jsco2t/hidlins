//! Installed revocation must reach the running authority on every exit path.
use std::cell::RefCell;
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::rc::Rc;
use std::time::Instant;

use hidlins_core::{HidlinsPaths, VaultRegistry};
use hidlins_sync::client::{LanCancellation, LanTransport};
use hidlins_sync::pairing::PairingWindow;
use hidlins_sync::SyncTransport;

use super::*;
use crate::fixtures::create_test_vault;

const MASTER: &str = "revocation fixture master";
const WORKER: &str = "api::sync::revoke_reconciliation_regressions::fault_worker";
thread_local! {
    static UPDATE_HOOK: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
    static CLAIM_HOOK: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}
pub(super) fn after_update() {
    if let Some(hook) = UPDATE_HOOK.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
}
pub(super) fn after_claim() {
    if let Some(hook) = CLAIM_HOOK.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
}
fn paths(directory: &Path) -> HidlinsPaths {
    HidlinsPaths::with_state_dir(directory.join("state"))
}

struct Fixture {
    session: Arc<AppSession>,
    client: LocalSyncConfig,
    server_key: [u8; 32],
    endpoint: LocalEndpoint,
    paths: HidlinsPaths,
    vault_path: PathBuf,
}
impl Fixture {
    fn new(directory: &Path) -> Self {
        let paths = paths(directory);
        let vault_path = create_test_vault(&paths, "vault", MASTER);
        let master = MasterPassword::new(MASTER.into());
        let client = LocalSyncConfig::create("client", SyncRole::Client, &master).unwrap();
        let mut registry = VaultRegistry::load(paths.clone()).unwrap();
        let mut server =
            LocalSyncConfig::configure(&mut registry, "vault", SyncRole::Server, &master).unwrap();
        let peer = client.identity().public_key();
        let now = Instant::now();
        let mut window = PairingWindow::open_at(now);
        let sas = window.begin_candidate(peer, [71; 32], now).unwrap();
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
        server.persist(&mut registry, "vault").unwrap();
        let server_key = server.identity().public_key().into_bytes();
        let session = Arc::new(AppSession::for_test(paths.clone()).unwrap());
        session.unlock("vault".into(), MASTER.into(), None).unwrap();
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
            .expect("actual local API server started")
            .checked()
            .unwrap();
        Self {
            session,
            client,
            server_key,
            endpoint,
            paths,
            vault_path,
        }
    }
    fn peer_can_head(&self) -> bool {
        let master = MasterPassword::new(MASTER.into());
        let identity = self
            .client
            .identity()
            .unlock("client", SyncRole::Client, &master)
            .unwrap();
        let endpoint = self.endpoint;
        let key = self.server_key;
        let cancellation = LanCancellation::default();
        let worker_cancel = cancellation.clone();
        let worker = thread::spawn(move || {
            LanTransport::new(identity, key, None, vec![endpoint])
                .unwrap()
                .with_cancellation(worker_cancel)
                .head()
                .is_ok()
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !worker.is_finished() && Instant::now() < deadline {
            self.session.pump_sync_server_for_test();
            thread::yield_now();
        }
        let completed = worker.is_finished();
        if !completed {
            cancellation.cancel();
        }
        let result = worker.join().unwrap();
        assert!(
            completed,
            "actual authenticated request completed; timeout is not a behavioral red"
        );
        result
    }
    fn assert_config_revoked(&self, revoked: bool) {
        let peer = self.client.identity().public_key();
        let disk = VaultRegistry::load(self.paths.clone()).unwrap();
        let state = self.session.inner.lock().unwrap();
        for registry in [&disk, &state.registry] {
            let config = LocalSyncConfig::from_vault_entry(registry.get("vault").unwrap()).unwrap();
            assert_eq!(!config.authorizes(peer), revoked);
        }
    }
}

#[test]
#[ignore = "isolated fault worker; executed explicitly by the outer scenarios"]
fn fault_worker() {
    let Some(directory) = std::env::var_os("HIDLINS_REVOKE_FIXTURE") else {
        return;
    };
    let directory = Path::new(&directory);
    let phase = std::env::var("HIDLINS_REVOKE_PHASE").unwrap();
    let fixture = Fixture::new(directory);
    assert!(
        fixture.peer_can_head(),
        "confirmed peer succeeds before revocation"
    );
    let before = std::fs::read(&fixture.vault_path).unwrap();
    let signal = directory.join("signals").join(&phase);
    std::fs::remove_file(&signal).unwrap();
    std::fs::create_dir(&signal).unwrap();
    assert!(matches!(
        fixture.session.revoke_sync_peer("peer-0".into()),
        Err(HidlinsApiError::SyncRevoked)
    ));
    let installed = phase == "rename_done";
    fixture.assert_config_revoked(installed);
    assert_eq!(fixture.peer_can_head(), !installed, "running authority must match installed peer revocation even when durability returns an error");
    assert!(fixture.session.has_vault() && fixture.session.has_credentials());
    assert!(fixture.session.local_sync_status().unwrap().server_running);
    assert!(std::fs::read(&fixture.vault_path).unwrap() == before);
}
fn run_phase(phase: &str) {
    let directory = tempfile::tempdir().unwrap();
    let signals = directory.path().join("signals");
    std::fs::create_dir(&signals).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", WORKER, "--ignored", "--nocapture"])
        .env("HIDLINS_REVOKE_FIXTURE", directory.path())
        .env("HIDLINS_REVOKE_PHASE", phase)
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
        "isolated revocation assertion failed:\n{}\n{}",
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

#[test]
fn host_claim_after_update_cannot_skip_successful_revocation() {
    let directory = tempfile::tempdir().unwrap();
    let fixture = Fixture::new(directory.path());
    assert!(fixture.peer_can_head());
    let session = Arc::clone(&fixture.session);
    let handoff = Rc::new(RefCell::new(None));
    let saved = Rc::clone(&handoff);
    UPDATE_HOOK.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(move || {
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let worker = thread::spawn(move || {
                CLAIM_HOOK.with(|hook| {
                    *hook.borrow_mut() = Some(Box::new(move || {
                        entered_tx.send(()).unwrap();
                        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    }));
                });
                session.pump_sync_server_for_test();
            });
            entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            *saved.borrow_mut() = Some((release_tx, worker));
        }));
    });
    let result = fixture.session.revoke_sync_peer("peer-0".into());
    let (release, worker) = handoff
        .borrow_mut()
        .take()
        .expect("actual postupdate hook and host claim reached");
    assert!(fixture.session.inner.lock().unwrap().syncing);
    release.send(()).unwrap();
    worker.join().unwrap();
    assert!(result.is_ok());
    fixture.assert_config_revoked(true);
    assert!(
        !fixture.peer_can_head(),
        "successful revocation must remain enforced across a host ownership claim"
    );
    assert!(fixture.session.has_vault() && fixture.session.has_credentials());
}

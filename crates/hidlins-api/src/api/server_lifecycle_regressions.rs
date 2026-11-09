//! Real API server authority and controls during exclusive host ownership.
use std::cell::{Cell, RefCell};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use hidlins_core::{EntryBuilder, Vault, VaultRegistry};
use hidlins_sync::client::{LanCancellation, LanTransport};
use hidlins_sync::SyncTransport;

use super::*;
use crate::dto::{LockEvent, SyncEvent};
use crate::event::{EventSink, MpscEventSink};
use crate::fixtures::{create_test_vault, TestEnv};

const MASTER: &str = "server lifecycle regression master";
const URI: &str = "otpauth://totp/Example:server?secret=JBSWY3DPEHPK3PXP&issuer=Example";
thread_local! {
    static CLAIM_HOOK: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

pub(super) fn after_claim() {
    if let Some(hook) = CLAIM_HOOK.with(|cell| cell.borrow_mut().take()) {
        hook();
    }
}

fn during_claim(session: &AppSession, action: impl FnOnce() + 'static) {
    let called = Rc::new(Cell::new(false));
    let observed = Rc::clone(&called);
    CLAIM_HOOK.with(|cell| {
        assert!(cell
            .borrow_mut()
            .replace(Box::new(move || {
                observed.set(true);
                action();
            }))
            .is_none());
    });
    session.pump_sync_server_for_test();
    assert!(
        called.get(),
        "actual host ownership loan must reach the hook"
    );
}

fn paired_configs() -> (LocalSyncConfig, LocalSyncConfig) {
    use hidlins_sync::config::local::LocalSyncConfig;
    use hidlins_sync::identity::SyncRole;
    use hidlins_sync::pairing::PairingWindow;
    let master = MasterPassword::new(MASTER.into());
    let mut client = LocalSyncConfig::create("vault", SyncRole::Client, &master).unwrap();
    let mut server = LocalSyncConfig::create("vault", SyncRole::Server, &master).unwrap();
    let now = Instant::now();
    let hash = [37; 32];
    let mut client_window = PairingWindow::open_at(now);
    let sas = client_window
        .begin_candidate(server.identity().public_key(), hash, now)
        .unwrap();
    let client_confirmation = client_window.confirm(sas, now).unwrap();
    let mut server_window = PairingWindow::open_at(now);
    let sas = server_window
        .begin_candidate(client.identity().public_key(), hash, now)
        .unwrap();
    let server_confirmation = server_window.confirm(sas, now).unwrap();
    let transaction = client_confirmation
        .transaction(
            server.identity().public_key(),
            client.identity().public_key(),
        )
        .unwrap();
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    server
        .prepare_client(
            &transaction,
            &server_confirmation,
            "client".into(),
            epoch,
            epoch + 180,
        )
        .unwrap();
    client
        .prepare_server(
            &transaction,
            &client_confirmation,
            "server".into(),
            epoch,
            epoch + 180,
        )
        .unwrap();
    server.activate_client(&transaction, epoch).unwrap();
    client.activate_server(&transaction, epoch).unwrap();
    (server, client)
}

struct Fixture {
    session: Arc<AppSession>,
    env: TestEnv,
    peer: LocalSyncConfig,
    endpoint: LocalEndpoint,
    path: PathBuf,
    otp: String,
    original_key: [u8; 32],
    events: mpsc::Receiver<SyncEvent>,
}

impl Fixture {
    fn new() -> Self {
        let env = TestEnv::new();
        let path = create_test_vault(&env.paths, "vault", MASTER);
        let master = MasterPassword::new(MASTER.into());
        let mut vault = Vault::open(&path, &master, None).unwrap();
        let otp = vault
            .add_entry(
                vault.root_group_uuid(),
                EntryBuilder::totp("OTP", URI).unwrap().build(),
            )
            .unwrap()
            .to_string();
        vault.save().unwrap();
        drop(vault);
        let (server, peer) = paired_configs();
        let original_key = server.identity().public_key().into_bytes();
        let mut registry = VaultRegistry::load(env.paths.clone()).unwrap();
        server.persist(&mut registry, "vault").unwrap();
        let session = Arc::new(AppSession::for_test(env.paths.clone()).unwrap());
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
            .expect("private test listener can be bound");
        let endpoint = endpoint.checked().unwrap();
        session.open_pairing_window().unwrap();
        session.entry_detail(otp.clone()).unwrap();
        assert!(session.totp_now(otp.clone()).is_ok());
        let (tx, events) = mpsc::channel();
        session.sync_events_for_test(Box::new(MpscEventSink::new(tx)));
        Self {
            session,
            env,
            peer,
            endpoint,
            path,
            otp,
            original_key,
            events,
        }
    }

    fn old_peer_can_head(&self) -> bool {
        let master = MasterPassword::new(MASTER.into());
        let identity = self
            .peer
            .identity()
            .unlock("vault", SyncRole::Client, &master)
            .unwrap();
        let endpoint = self.endpoint;
        let pinned = self.original_key;
        let cancellation = LanCancellation::default();
        let worker_cancel = cancellation.clone();
        let worker = thread::spawn(move || {
            let Ok(transport) = LanTransport::new(identity, pinned, None, vec![endpoint]) else {
                return false;
            };
            transport.with_cancellation(worker_cancel).head().is_ok()
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
            "real peer request must finish, never use a fixture timeout as red"
        );
        result
    }

    fn config(&self) -> LocalSyncConfig {
        let registry = VaultRegistry::load(self.env.paths.clone()).unwrap();
        LocalSyncConfig::from_vault_entry(registry.get("vault").unwrap()).unwrap()
    }
}

fn listening(endpoint: LocalEndpoint) -> bool {
    TcpStream::connect_timeout(&endpoint.socket_addr(), Duration::from_secs(1)).is_ok()
}

#[test]
fn fresh_client_configuration_retires_previous_server_authority() {
    let fixture = Fixture::new();
    assert!(
        fixture.old_peer_can_head(),
        "confirmed old peer is the admission control"
    );
    fixture
        .session
        .configure_local_sync(LocalSyncRoleDto::Client)
        .unwrap();
    assert!(
        !fixture.old_peer_can_head(),
        "previous peer must lose the retired server identity"
    );
    let status = fixture.session.local_sync_status().unwrap();
    assert!(!status.server_running && !status.server_enabled && !status.pairing_open);
    assert_eq!(fixture.config().role(), SyncRole::Client);
    assert!(fixture.session.has_vault() && fixture.session.has_credentials());
}

#[test]
fn fresh_server_configuration_cannot_reuse_a_stale_listener_on_same_endpoint() {
    let fixture = Fixture::new();
    assert!(fixture.old_peer_can_head());
    fixture
        .session
        .configure_local_sync(LocalSyncRoleDto::Server)
        .unwrap();
    assert_ne!(
        fixture.config().identity().public_key().into_bytes(),
        fixture.original_key
    );
    fixture
        .session
        .start_sync_server(endpoint_dto(fixture.endpoint))
        .unwrap();
    assert!(
        !fixture.old_peer_can_head(),
        "same endpoint must serve the fresh identity, not old trust"
    );
    assert_eq!(
        fixture
            .session
            .local_sync_status()
            .unwrap()
            .active_peer_count,
        0
    );
    assert!(fixture.session.has_vault());
}

#[test]
fn client_reconfiguration_clears_old_server_restart_intent() {
    let fixture = Fixture::new();
    fixture
        .session
        .configure_local_sync(LocalSyncRoleDto::Client)
        .unwrap();
    fixture.session.lock_now().unwrap();
    fixture
        .session
        .unlock("vault".into(), MASTER.into(), None)
        .unwrap();
    let status = fixture.session.local_sync_status().unwrap();
    assert!(!status.server_enabled && !status.server_running);
    assert!(
        fixture.session.totp_now(fixture.otp.clone()).is_err(),
        "lock still revokes cache"
    );
}

struct ReadOnlyDirectory {
    path: PathBuf,
    permissions: std::fs::Permissions,
}
impl ReadOnlyDirectory {
    fn new(path: &std::path::Path) -> Self {
        use std::os::unix::fs::PermissionsExt as _;
        let permissions = std::fs::metadata(path).unwrap().permissions();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o500)).unwrap();
        Self {
            path: path.to_path_buf(),
            permissions,
        }
    }
}
impl Drop for ReadOnlyDirectory {
    fn drop(&mut self) {
        std::fs::set_permissions(&self.path, self.permissions.clone()).unwrap();
    }
}

#[test]
fn precommit_configuration_failure_preserves_unchanged_running_authority() {
    let fixture = Fixture::new();
    let directory = ReadOnlyDirectory::new(fixture.env.paths.state_dir());
    let result = fixture
        .session
        .configure_local_sync(LocalSyncRoleDto::Client);
    drop(directory);
    assert!(matches!(
        result,
        Err(HidlinsApiError::LocalSyncConfiguration)
    ));
    assert_eq!(
        fixture.config().identity().public_key().into_bytes(),
        fixture.original_key
    );
    assert!(
        fixture.old_peer_can_head(),
        "unchanged authority survives a pre-commit registry failure"
    );
    assert!(fixture.session.has_vault() && fixture.session.has_credentials());
}

#[test]
fn stop_during_host_ownership_closes_admission_before_return_and_preserves_vault() {
    let fixture = Fixture::new();
    let session = Arc::clone(&fixture.session);
    let endpoint = fixture.endpoint;
    let remained_open = Rc::new(Cell::new(false));
    let observed = Rc::clone(&remained_open);
    during_claim(&fixture.session, move || {
        session.stop_sync_server();
        observed.set(listening(endpoint));
    });
    assert!(
        !remained_open.get(),
        "Stop must close actual admission while the host still owns its vault"
    );
    assert!(fixture
        .session
        .inner
        .lock()
        .unwrap()
        .server_runtime
        .is_none());
    assert!(fixture.session.has_vault() && fixture.session.has_credentials());
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_ok());
}

#[test]
fn close_pairing_during_host_ownership_is_not_undone_by_handback() {
    let fixture = Fixture::new();
    let session = Arc::clone(&fixture.session);
    during_claim(&fixture.session, move || session.close_pairing_window());
    let status = fixture.session.local_sync_status().unwrap();
    assert!(
        !status.pairing_open,
        "closed window must stay closed when host work returns"
    );
    assert!(status.server_running);
    assert!(
        fixture.old_peer_can_head(),
        "closing pairing keeps existing trusted service"
    );
}

#[test]
fn close_then_reopen_while_host_owns_vault_preserves_new_window() {
    let fixture = Fixture::new();
    let session = Arc::clone(&fixture.session);
    let reopened = Rc::new(Cell::new(false));
    let observed = Rc::clone(&reopened);
    during_claim(&fixture.session, move || {
        session.close_pairing_window();
        observed.set(session.open_pairing_window().is_ok());
    });
    assert!(
        reopened.get(),
        "controller remains reachable while vault work runs"
    );
    assert!(fixture.session.local_sync_status().unwrap().pairing_open);
}

#[test]
fn running_status_during_host_ownership_reflects_actual_live_listener() {
    let fixture = Fixture::new();
    let session = Arc::clone(&fixture.session);
    let running = Rc::new(Cell::new(false));
    let observed = Rc::clone(&running);
    during_claim(&fixture.session, move || {
        let status = session.local_sync_status().unwrap();
        observed.set(status.in_flight && status.server_running);
    });
    assert!(running.get());
    assert!(fixture.session.local_sync_status().unwrap().server_running);
}

#[test]
fn ordinary_host_poll_preserves_vault_cache_and_emits_no_false_change() {
    let fixture = Fixture::new();
    let before = std::fs::read(&fixture.path).unwrap();
    during_claim(&fixture.session, || {});
    assert_eq!(std::fs::read(&fixture.path).unwrap(), before);
    assert!(fixture.session.has_vault() && fixture.session.has_credentials());
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_ok());
    assert!(fixture
        .events
        .try_iter()
        .all(|event| !matches!(event, SyncEvent::Done(_))));
}

#[test]
fn idle_stop_never_locks_vault_or_discards_cached_local_data() {
    let fixture = Fixture::new();
    let (tx, events) = mpsc::channel();
    fixture
        .session
        .lock_events_for_test(Box::new(MpscEventSink::new(tx)));
    assert!(matches!(events.recv().unwrap(), LockEvent::Unlocked));
    fixture.session.stop_sync_server();
    assert!(!listening(fixture.endpoint));
    assert!(fixture.session.has_vault() && fixture.session.has_credentials());
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_ok());
    assert!(events
        .try_iter()
        .all(|event| !matches!(event, LockEvent::Locked)));
}

#[test]
fn pending_host_lock_closes_admission_before_locked_notification() {
    let fixture = Fixture::new();
    let session = Arc::clone(&fixture.session);
    let remained_open = Rc::new(Cell::new(false));
    let observed = Rc::clone(&remained_open);
    let endpoint = fixture.endpoint;
    let (tx, events) = mpsc::channel();
    fixture
        .session
        .lock_events_for_test(Box::new(LockAdmissionSink { endpoint, tx }));
    assert!(matches!(events.recv().unwrap().0, LockEvent::Unlocked));
    during_claim(&fixture.session, move || {
        session.lock_now().unwrap();
        observed.set(listening(endpoint));
    });
    assert!(!remained_open.get());
    assert!(!fixture.session.has_vault() && !fixture.session.has_credentials());
    let observed: Vec<_> = events.try_iter().collect();
    assert_eq!(observed.len(), 1);
    assert!(matches!(observed[0].0, LockEvent::Locked));
    assert!(
        !observed[0].1,
        "admission must be closed during Locked notification"
    );
}

struct LockAdmissionSink {
    endpoint: LocalEndpoint,
    tx: mpsc::Sender<(LockEvent, bool)>,
}

impl EventSink<LockEvent> for LockAdmissionSink {
    fn send(&self, event: LockEvent) {
        let open = matches!(event, LockEvent::Locked) && listening(self.endpoint);
        let _ = self.tx.send((event, open));
    }
}

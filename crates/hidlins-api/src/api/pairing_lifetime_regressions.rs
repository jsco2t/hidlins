//! Bilateral Noise pairing/import lifetime and durable-origin regressions.
use std::cell::RefCell;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool as Flag, Ordering as Order};
use std::thread::JoinHandle;
use std::time::Instant;

use super::*;
use crate::dto::{DiscoveryPermissionDto, LifecycleStateDto};
use crate::event::MpscEventSink;
use crate::fixtures::{create_test_vault, TestEnv};
use hidlins_core::{MasterPassword, VaultRegistry};
use hidlins_sync::client::PendingPairingStore;

const MASTER: &str = "pairing lifetime master";
const OTHER_MASTER: &str = "other pairing vault master";
const NEW_MASTER: &str = "rotated pairing vault master";
#[derive(Clone, Copy)]
enum Boundary {
    Begin = 0,
    Remove = 1,
    Persist = 2,
    Fetch = 3,
}
type Hook = Box<dyn FnOnce()>;
thread_local! { static HOOKS: RefCell<Vec<Option<Hook>>> = const { RefCell::new(Vec::new()) }; }
fn run(boundary: Boundary) {
    let hook = HOOKS.with(|slots| {
        slots
            .borrow_mut()
            .get_mut(boundary as usize)
            .and_then(Option::take)
    });
    if let Some(hook) = hook {
        hook();
    }
}
pub(super) fn after_begin() {
    run(Boundary::Begin);
}
pub(super) fn after_remove() {
    run(Boundary::Remove);
}
pub(super) fn before_persist() {
    run(Boundary::Persist);
}
pub(super) fn after_fetch() {
    run(Boundary::Fetch);
}
fn install(boundary: Boundary, action: impl FnOnce() + 'static) -> Arc<Flag> {
    let reached = Arc::new(Flag::new(false));
    let seen = Arc::clone(&reached);
    HOOKS.with(|slots| {
        let mut slots = slots.borrow_mut();
        slots.resize_with(4, || None);
        assert!(slots[boundary as usize]
            .replace(Box::new(move || {
                seen.store(true, Order::Release);
                action();
            }))
            .is_none());
    });
    reached
}
struct Server {
    session: Arc<AppSession>,
    endpoint: LocalEndpointDto,
    events: mpsc::Receiver<SyncEvent>,
    path: PathBuf,
    _env: TestEnv,
}
impl Server {
    fn new() -> Self {
        let env = TestEnv::new();
        let path = create_test_vault(&env.paths, "shared", MASTER);
        let session = Arc::new(AppSession::for_test(env.paths.clone()).unwrap());
        session
            .unlock("shared".into(), MASTER.into(), None)
            .unwrap();
        session
            .configure_local_sync(LocalSyncRoleDto::Server)
            .unwrap();
        let (sender, events) = mpsc::channel();
        session.sync_events_for_test(Box::new(MpscEventSink::new(sender)));
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
            .expect("actual private API server started");
        session.open_pairing_window().unwrap();
        Self {
            session,
            endpoint,
            events,
            path,
            _env: env,
        }
    }
    fn accept(&self, client: &PairingPromptDto) {
        let deadline = Instant::now() + Duration::from_secs(3);
        let server = loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .expect("actual server prompt deadline");
            if let SyncEvent::PairingRequested(prompt) = self
                .events
                .recv_timeout(remaining)
                .expect("actual server prompt")
            {
                break prompt;
            }
        };
        assert_eq!(
            server.sas, client.sas,
            "bilateral authenticated transcript SAS"
        );
        assert_eq!(client.transaction_handle.len(), 32);
        assert_eq!(server.transaction_handle.len(), 32);
        assert_ne!(client.transaction_handle, client.sas);
        self.session
            .confirm_pairing(server.transaction_handle, true, "confirmed client".into())
            .unwrap();
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.session.shutdown();
    }
}
struct Fixture {
    server: Server,
    client: Arc<AppSession>,
    env: TestEnv,
}
impl Fixture {
    fn new(existing: bool) -> Self {
        let server = Server::new();
        let env = TestEnv::new();
        create_test_vault(&env.paths, "other", OTHER_MASTER);
        let client = Arc::new(
            if existing {
                AppSession::for_test(env.paths.clone())
            } else {
                AppSession::for_test_mobile(env.paths.clone())
            }
            .unwrap(),
        );
        if existing {
            let target = env.paths.state_dir().join("shared.kdbx");
            std::fs::copy(&server.path, &target).unwrap();
            client
                .register_existing_vault("shared".into(), target.display().to_string(), None)
                .unwrap();
            client.unlock("shared".into(), MASTER.into(), None).unwrap();
            client
                .configure_local_sync(LocalSyncRoleDto::Client)
                .unwrap();
            client
                .set_discovery_candidates(
                    DiscoveryPermissionDto::Granted,
                    vec![server.endpoint.clone()],
                )
                .unwrap();
        }
        Self {
            server,
            client,
            env,
        }
    }
    fn begin_import(
        &self,
        name: &str,
        endpoint: LocalEndpointDto,
    ) -> Result<PairingPromptDto, HidlinsApiError> {
        self.client
            .begin_pair_import(name.into(), MASTER.into(), None, vec![endpoint])
    }
    fn begin(&self, existing: bool) -> PairingPromptDto {
        if existing {
            self.client.begin_pairing()
        } else {
            self.begin_import("imported", self.server.endpoint.clone())
        }
        .expect("actual XX pairing begins")
    }
    fn config(&self) -> LocalSyncConfig {
        let registry = VaultRegistry::load(self.env.paths.clone()).unwrap();
        LocalSyncConfig::from_vault_entry(registry.get("shared").unwrap()).unwrap()
    }
    fn assert_no_import(&self) {
        assert!(
            !self.env.paths.state_dir().join("imported.kdbx").exists(),
            "canceled import must not install ciphertext"
        );
        assert!(
            VaultRegistry::load(self.env.paths.clone())
                .unwrap()
                .get("imported")
                .is_none(),
            "canceled import must not register a vault"
        );
    }
    fn join<T: Send + 'static>(&self, worker: JoinHandle<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !worker.is_finished() && Instant::now() < deadline {
            self.server.session.pump_sync_server_for_test();
            thread::yield_now();
        }
        let completed = worker.is_finished();
        if !completed {
            self.client.cancel_sync();
            self.server.session.shutdown();
        }
        let result = worker.join().unwrap();
        assert!(
            completed,
            "actual bilateral work must finish; fixture deadline is not a behavioral red"
        );
        result
    }
    fn imported(&self, name: &str) {
        let registry = VaultRegistry::load(self.env.paths.clone()).unwrap();
        let entry = registry.get(name).expect("completed import registered");
        assert!(LocalSyncConfig::from_vault_entry(entry)
            .unwrap()
            .is_active_client());
        let source = std::fs::read(&self.server.path).unwrap();
        assert!(
            std::fs::read(&entry.path).unwrap() == source,
            "exact complete encrypted KDBX installed"
        );
        assert!(
            hidlins_core::Vault::open(&entry.path, &MasterPassword::new(MASTER.into()), None)
                .is_ok()
        );
        assert!(PendingPairingStore::new(self.env.paths.state_dir(), name)
            .load()
            .unwrap()
            .is_none());
    }
}
#[derive(Clone, Copy)]
enum Action {
    Lock,
    Cancel,
    Shutdown,
    SameVault,
    OtherVault,
    Reconfigure,
    ClearConfig,
    Rekey,
    FailedUnlock,
    OsLock,
    Lifecycle,
}
fn apply(client: &AppSession, action: Action) {
    match action {
        Action::Lock => client.lock_now().unwrap(),
        Action::Cancel => client.cancel_sync(),
        Action::Shutdown => client.shutdown(),
        Action::SameVault => {
            client.lock_now().unwrap();
            client.unlock("shared".into(), MASTER.into(), None).unwrap();
        }
        Action::OtherVault => {
            client.lock_now().unwrap();
            client
                .unlock("other".into(), OTHER_MASTER.into(), None)
                .unwrap();
        }
        Action::Reconfigure => client
            .configure_local_sync(LocalSyncRoleDto::Client)
            .unwrap(),
        Action::ClearConfig => client.clear_sync_config("shared".into()).unwrap(),
        Action::Rekey => client
            .change_master_password(MASTER.into(), NEW_MASTER.into())
            .unwrap(),
        Action::FailedUnlock => {
            assert!(matches!(
                client.unlock("other".into(), "wrong password".into(), None),
                Err(HidlinsApiError::AuthenticationFailed)
            ));
        }
        Action::OsLock => {
            struct Source(mpsc::Sender<()>);
            impl hidlins_security::OsLockEventSource for Source {
                fn run(
                    self: Box<Self>,
                    sender: mpsc::Sender<hidlins_security::SecurityEvent>,
                ) -> Result<(), hidlins_security::SecurityError> {
                    sender
                        .send(hidlins_security::SecurityEvent::OsLock {
                            reason: hidlins_security::OsLockReason::Screensaver,
                        })
                        .unwrap();
                    self.0.send(()).unwrap();
                    Ok(())
                }
                fn name(&self) -> &'static str {
                    "pairing-lifetime-one-shot"
                }
                fn shutdown_handle(&mut self) -> hidlins_security::ShutdownHandle {
                    hidlins_security::ShutdownHandle::new(|| {})
                }
            }
            let (sent, received) = mpsc::channel();
            client.attach_lock_source_for_test(Source(sent));
            received.recv_timeout(Duration::from_secs(2)).unwrap();
            client.drive_tick(Instant::now());
        }
        Action::Lifecycle => {
            client.report_lifecycle_state(LifecycleStateDto::Hidden);
            client.drive_tick(Instant::now() + Duration::from_secs(16));
        }
    }
}
#[test]
fn ordinary_existing_pairing_retains_bilateral_trust_and_opaque_handles() {
    let fixture = Fixture::new(true);
    let prompt = fixture.begin(true);
    fixture.server.accept(&prompt);
    assert!(fixture
        .client
        .confirm_pairing(prompt.transaction_handle, true, "authority".into())
        .unwrap()
        .is_none());
    assert!(fixture.config().is_active_client());
    assert_eq!(
        fixture
            .server
            .session
            .local_sync_status()
            .unwrap()
            .active_peer_count,
        1
    );
}
fn locked_import_control() {
    let fixture = Fixture::new(false);
    fixture.client.drive_tick(Instant::now());
    let prompt = fixture.begin(false);
    fixture.client.drive_tick(Instant::now());
    fixture.server.accept(&prompt);
    let client = Arc::clone(&fixture.client);
    let summary = fixture
        .join(thread::spawn(move || {
            client.confirm_pairing(prompt.transaction_handle, true, "authority".into())
        }))
        .unwrap()
        .unwrap();
    assert_eq!(summary.name, "imported");
    assert!(!fixture.client.has_vault());
    fixture.imported("imported");
}
#[cfg(feature = "desktop")]
#[test]
fn desktop_locked_pair_and_import_control() {
    locked_import_control();
}
#[cfg(not(feature = "desktop"))]
#[test]
fn native_locked_pair_and_import_control() {
    locked_import_control();
}

fn superseded_begin(existing: bool, action: Action) {
    let fixture = Fixture::new(existing);
    let client = Arc::clone(&fixture.client);
    let reached = install(Boundary::Begin, move || apply(&client, action));
    let result = if existing {
        fixture.client.begin_pairing()
    } else {
        fixture.begin_import("imported", fixture.server.endpoint.clone())
    };
    assert!(
        reached.load(Order::Acquire),
        "actual completed handshake reached publication boundary"
    );
    assert!(
        matches!(result, Err(HidlinsApiError::SyncCanceled)),
        "superseded begin must not publish a usable prompt"
    );
    if !existing {
        fixture.assert_no_import();
    }
}
#[test]
fn begin_after_same_vault_lock_reopen_cannot_publish() {
    superseded_begin(true, Action::SameVault);
}
#[test]
fn canceled_begin_cannot_publish() {
    superseded_begin(true, Action::Cancel);
}
#[test]
fn locked_import_begin_after_shutdown_cannot_publish() {
    superseded_begin(false, Action::Shutdown);
}

fn removed_confirmation(action: Action) {
    let fixture = Fixture::new(true);
    let prompt = fixture.begin(true);
    fixture.server.accept(&prompt);
    let client = Arc::clone(&fixture.client);
    let paths = fixture.env.paths.clone();
    let expected = Arc::new(Mutex::new(None));
    let saved = Arc::clone(&expected);
    let reached = install(Boundary::Remove, move || {
        apply(&client, action);
        let registry = VaultRegistry::load(paths).unwrap();
        *saved.lock().unwrap() = Some(
            registry
                .get("shared")
                .and_then(LocalSyncConfig::from_vault_entry),
        );
    });
    let result =
        fixture
            .client
            .confirm_pairing(prompt.transaction_handle, true, "authority".into());
    assert!(
        reached.load(Order::Acquire),
        "actual client prompt was removed before cancellation"
    );
    assert!(
        matches!(result, Err(HidlinsApiError::SyncCanceled)),
        "removed confirmation must remain cancellation/origin bound"
    );
    let registry = VaultRegistry::load(fixture.env.paths.clone()).unwrap();
    assert!(
        registry
            .get("shared")
            .and_then(LocalSyncConfig::from_vault_entry)
            == expected.lock().unwrap().take().unwrap(),
        "stale pairing must not overwrite the new origin configuration"
    );
}
#[test]
fn cancel_reaches_confirmation_after_prompt_was_removed() {
    removed_confirmation(Action::Cancel);
}
#[test]
fn same_vault_aba_rejects_removed_confirmation() {
    removed_confirmation(Action::SameVault);
}
#[test]
fn other_vault_rejects_removed_confirmation() {
    removed_confirmation(Action::OtherVault);
}
#[test]
fn reconfiguration_rejects_removed_confirmation() {
    removed_confirmation(Action::Reconfigure);
}
#[test]
fn cleared_configuration_rejects_removed_confirmation() {
    removed_confirmation(Action::ClearConfig);
}
#[test]
fn successful_rekey_rejects_removed_confirmation() {
    removed_confirmation(Action::Rekey);
}

fn locked_pending_import(action: Action) {
    let fixture = Fixture::new(false);
    let prompt = fixture.begin(false);
    apply(&fixture.client, action);
    assert!(
        matches!(
            fixture
                .client
                .confirm_pairing(prompt.transaction_handle, false, "unused".into()),
            Err(HidlinsApiError::SyncCanceled)
        ),
        "locked-session invalidation must remove an unconfirmed import prompt"
    );
    fixture.assert_no_import();
}
#[test]
fn explicit_lock_cancels_an_already_locked_import_prompt() {
    locked_pending_import(Action::Lock);
}
#[test]
fn shutdown_cancels_an_already_locked_import_prompt() {
    locked_pending_import(Action::Shutdown);
}

fn invalidate_fetched_import(action: Action) {
    let fixture = Fixture::new(false);
    let prompt = fixture.begin(false);
    fixture.server.accept(&prompt);
    let client = Arc::clone(&fixture.client);
    let worker = thread::spawn(move || {
        let actor = Arc::clone(&client);
        let reached = install(Boundary::Fetch, move || apply(&actor, action));
        let result = client.confirm_pairing(prompt.transaction_handle, true, "authority".into());
        (reached.load(Order::Acquire), result)
    });
    let (reached, result) = fixture.join(worker);
    assert!(
        reached,
        "actual authenticated KDBX fetch completed before invalidation"
    );
    assert!(
        matches!(result, Err(HidlinsApiError::SyncCanceled)),
        "superseded fetched import cannot install or register a vault"
    );
    fixture.assert_no_import();
    let store = PendingPairingStore::new(fixture.env.paths.state_dir(), "imported");
    assert!(
        store.load().unwrap().is_some(),
        "already committed encrypted recovery config survives cancellation"
    );
    let bytes = std::fs::read(store.path()).unwrap();
    assert!(
        !bytes
            .windows(MASTER.len())
            .any(|part| part == MASTER.as_bytes()),
        "recovery never persists the master password"
    );
}
#[test]
fn explicit_lock_after_fetch_cancels_locked_import_commit() {
    invalidate_fetched_import(Action::Lock);
}
#[test]
fn cancel_after_fetch_cancels_import_commit() {
    invalidate_fetched_import(Action::Cancel);
}
#[test]
fn shutdown_after_fetch_cancels_import_commit() {
    invalidate_fetched_import(Action::Shutdown);
}
#[test]
fn failed_unlock_claim_after_fetch_supersedes_import() {
    invalidate_fetched_import(Action::FailedUnlock);
}
#[test]
fn affirmative_os_lock_after_fetch_supersedes_locked_import() {
    invalidate_fetched_import(Action::OsLock);
}
#[test]
fn mobile_lifecycle_lock_after_fetch_supersedes_locked_import() {
    invalidate_fetched_import(Action::Lifecycle);
}

#[test]
fn changed_durable_config_with_same_identity_cannot_be_overwritten_by_pairing() {
    let fixture = Fixture::new(true);
    let prompt = fixture.begin(true);
    fixture.server.accept(&prompt);
    let paths = fixture.env.paths.clone();
    let expected = Arc::new(Mutex::new(None));
    let saved = Arc::clone(&expected);
    let reached = install(Boundary::Persist, move || {
        let mut registry = VaultRegistry::load(paths).unwrap();
        let mut changed =
            LocalSyncConfig::from_vault_entry(registry.get("shared").unwrap()).unwrap();
        let original_key = changed.identity().public_key();
        changed.set_sync_versions(
            Some(hidlins_sync::protocol::RemoteVersion::new([83; 32])),
            None,
        );
        changed.persist(&mut registry, "shared").unwrap();
        assert_eq!(changed.identity().public_key(), original_key);
        *saved.lock().unwrap() = Some(changed);
    });
    let result =
        fixture
            .client
            .confirm_pairing(prompt.transaction_handle, true, "authority".into());
    assert!(
        reached.load(Order::Acquire),
        "independent registry writer ran at actual persistence boundary"
    );
    assert!(
        matches!(result, Err(HidlinsApiError::SyncCanceled)),
        "same-identity changed durable origin rejects stale persistence"
    );
    assert!(
        fixture.config() == expected.lock().unwrap().take().unwrap(),
        "latest durable config survives the stale operation"
    );
}
#[test]
fn parallel_existing_prompts_do_not_overwrite_first_completed_pairing() {
    let fixture = Fixture::new(true);
    let other = Server::new();
    let first = fixture.begin(true);
    fixture
        .client
        .set_discovery_candidates(
            DiscoveryPermissionDto::Granted,
            vec![other.endpoint.clone()],
        )
        .unwrap();
    let second = fixture
        .client
        .begin_pairing()
        .expect("parallel existing begin remains supported");
    fixture.server.accept(&first);
    other.accept(&second);
    fixture
        .client
        .confirm_pairing(first.transaction_handle, true, "first authority".into())
        .unwrap();
    let completed = fixture.config();
    assert!(completed.is_active_client());
    assert!(
        matches!(
            fixture.client.confirm_pairing(
                second.transaction_handle,
                true,
                "second authority".into()
            ),
            Err(HidlinsApiError::SyncCanceled)
        ),
        "stale parallel confirmation cannot replace completed trust"
    );
    assert!(fixture.config() == completed);
    assert_eq!(
        fixture
            .server
            .session
            .local_sync_status()
            .unwrap()
            .active_peer_count,
        1
    );
    assert_eq!(
        other.session.local_sync_status().unwrap().active_peer_count,
        0
    );
}
#[test]
fn overlapping_imports_cannot_share_a_live_recovery_target() {
    let fixture = Fixture::new(false);
    let other = Server::new();
    let first = fixture.begin(false);
    let second = fixture.begin_import("imported", other.endpoint.clone());
    let rejected = matches!(second, Err(HidlinsApiError::SyncBusy));
    fixture
        .client
        .confirm_pairing(first.transaction_handle, false, "unused".into())
        .unwrap();
    if let Ok(prompt) = second {
        fixture
            .client
            .confirm_pairing(prompt.transaction_handle, false, "unused".into())
            .unwrap();
    }
    fixture.assert_no_import();
    assert!(
        rejected,
        "one live operation owns each deterministic import recovery target"
    );
}
#[test]
fn distinct_imports_keep_parallel_begin_and_independent_cancel() {
    let fixture = Fixture::new(false);
    let other = Server::new();
    let first = fixture
        .begin_import("first-import", fixture.server.endpoint.clone())
        .unwrap();
    let second = fixture
        .begin_import("second-import", other.endpoint.clone())
        .expect("distinct import remains parallel");
    assert_ne!(first.transaction_handle, second.transaction_handle);
    fixture
        .client
        .confirm_pairing(first.transaction_handle, false, "unused".into())
        .unwrap();
    fixture
        .client
        .confirm_pairing(second.transaction_handle, false, "unused".into())
        .unwrap();
    for name in ["first-import", "second-import"] {
        assert!(VaultRegistry::load(fixture.env.paths.clone())
            .unwrap()
            .get(name)
            .is_none());
        assert!(
            PendingPairingStore::new(fixture.env.paths.state_dir(), name)
                .load()
                .unwrap()
                .is_none()
        );
    }
}
#[test]
fn canceled_import_releases_live_target_reservation() {
    let fixture = Fixture::new(false);
    let other = Server::new();
    let _first = fixture.begin(false);
    fixture.client.cancel_sync();
    let second = fixture
        .begin_import("imported", other.endpoint.clone())
        .expect("cancel releases target reservation");
    fixture
        .client
        .confirm_pairing(second.transaction_handle, false, "unused".into())
        .unwrap();
    fixture.assert_no_import();
}
#[test]
fn cancellation_wakes_a_real_stalled_begin_without_waiting_for_peer() {
    let fixture = Fixture::new(true);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    fixture
        .client
        .set_discovery_candidates(
            DiscoveryPermissionDto::Granted,
            vec![LocalEndpointDto {
                address: "127.0.0.1".into(),
                port,
                scope_id: 0,
            }],
        )
        .unwrap();
    let (accepted, observed_accept) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let peer = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        accepted.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(5)).unwrap();
        drop(stream);
    });
    let client = Arc::clone(&fixture.client);
    let (completed, completion) = mpsc::channel();
    let worker = thread::spawn(move || {
        completed.send(client.begin_pairing()).unwrap();
    });
    observed_accept
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    fixture.client.cancel_sync();
    let prompt = completion.recv_timeout(Duration::from_secs(1));
    let woke = prompt.is_ok();
    // Cleanup releases the deliberately silent peer before any failure assertion.
    release.send(()).unwrap();
    peer.join().unwrap();
    worker.join().unwrap();
    let result =
        prompt.unwrap_or_else(|_| completion.recv_timeout(Duration::from_secs(2)).unwrap());
    assert!(
        woke,
        "cancellation must wake an accepted blocked handshake before peer release"
    );
    assert!(matches!(result, Err(HidlinsApiError::SyncCanceled)));
}
#[test]
fn cancellation_waits_for_guarded_persistence_then_forbids_later_writes() {
    let fixture = Fixture::new(true);
    let prompt = fixture.begin(true);
    fixture.server.accept(&prompt);
    let client = Arc::clone(&fixture.client);
    let (entered, observed_entry) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let worker = thread::spawn(move || {
        let reached = install(Boundary::Persist, move || {
            entered.send(()).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        let result = client.confirm_pairing(prompt.transaction_handle, true, "authority".into());
        (reached.load(Order::Acquire), result)
    });
    observed_entry.recv_timeout(Duration::from_secs(5)).unwrap();
    let authority = ApiPairingAuthority::new(
        Arc::downgrade(&fixture.client.inner),
        Arc::clone(&fixture.client.pairing_api),
    );
    let path = fixture.env.paths.vaults_toml();
    let after_path = path.clone();
    let (canceled, observed_cancel) = mpsc::channel();
    let canceller = thread::spawn(move || {
        // Actual authority cancellation route intentionally does not acquire State.
        hidlins_sync::server::PairingAuthority::cancel_pending(&authority);
        canceled.send(std::fs::read(path).unwrap()).unwrap();
    });
    let early = observed_cancel.recv_timeout(Duration::from_millis(400));
    let canceled_during_commit = early.is_ok();
    release.send(()).unwrap();
    let (reached, result) = worker.join().unwrap();
    canceller.join().unwrap();
    let at_cancel = early.unwrap_or_else(|_| {
        observed_cancel
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
    });
    assert!(reached);
    assert!(
        matches!(result, Ok(None) | Err(HidlinsApiError::SyncCanceled)),
        "completed trust may linearize before cancellation; stale later writes may not"
    );
    assert!(
        !canceled_during_commit,
        "cancel returned while the guarded persistence could still write"
    );
    assert!(
        std::fs::read(after_path).unwrap() == at_cancel,
        "no write may follow cancellation return"
    );
}
#[test]
fn cancellation_after_completed_existing_pairing_preserves_authorized_trust() {
    let fixture = Fixture::new(true);
    let prompt = fixture.begin(true);
    fixture.server.accept(&prompt);
    fixture
        .client
        .confirm_pairing(prompt.transaction_handle, true, "authority".into())
        .unwrap();
    let config = fixture.config();
    fixture.client.cancel_sync();
    assert!(fixture.config() == config && config.is_active_client());
}

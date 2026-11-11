//! Observable lock/cache interleavings through real vaults and API methods.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{atomic::AtomicUsize, mpsc};
use std::thread::{self, JoinHandle};

use hidlins_core::{EntryBuilder, Keyfile};
use hidlins_sync::{client::LanCancellation, SyncOptions, SyncOutcome};

use super::*;
use crate::dto::{EntryDetail, EntryEditDto, GroupDeleteBehaviorDto, RevealField};
use crate::event::MpscEventSink;
use crate::fixtures::{create_test_vault, TestEnv};
use crate::sync_port::{SucceedingSyncEngine, SyncEnginePort};

const MASTER: &str = "lock-cache regression master";
const OLD_URI: &str = "otpauth://totp/Example:old?secret=JBSWY3DPEHPK3PXP&issuer=Example";
const NEW_URI: &str = "otpauth://totp/Example:new?secret=KRSXG5DSNFXGOIDB&issuer=Example";
const WAIT: Duration = Duration::from_secs(5);

type PublishHook = Box<dyn FnOnce()>;
thread_local! {
    static PUBLISH_HOOK: RefCell<Option<PublishHook>> = const { RefCell::new(None) };
}

pub(crate) fn before_publish() {
    if let Some(hook) = PUBLISH_HOOK.with(|cell| cell.borrow_mut().take()) {
        hook();
    }
}

fn active_client_config() -> hidlins_sync::config::local::LocalSyncConfig {
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
    client
}

struct Fixture {
    session: Arc<AppSession>,
    otp: String,
    other_otp: String,
    metadata: String,
    group: String,
    vault_path: PathBuf,
    env: TestEnv,
}

impl Fixture {
    fn new(engine: Arc<dyn SyncEnginePort>, mobile: bool) -> Self {
        let env = TestEnv::new();
        let vault_path = create_test_vault(&env.paths, "vault", MASTER);
        let master = MasterPassword::new(MASTER.into());
        let mut vault = Vault::open(&vault_path, &master, None).unwrap();
        let root = vault.root_group_uuid();
        let group = vault.create_group(root, "parent").unwrap();
        let nested = vault.create_group(group, "nested").unwrap();
        let otp = vault
            .add_entry(
                nested,
                EntryBuilder::totp("affected OTP", OLD_URI).unwrap().build(),
            )
            .unwrap();
        let other_otp = vault
            .add_entry(
                root,
                EntryBuilder::totp("unrelated OTP", OLD_URI)
                    .unwrap()
                    .build(),
            )
            .unwrap();
        let metadata = vault
            .add_entry(root, EntryBuilder::credential("metadata").build())
            .unwrap();
        vault.save().unwrap();
        drop(vault);
        let mut registry = VaultRegistry::load(env.paths.clone()).unwrap();
        active_client_config()
            .persist(&mut registry, "vault")
            .unwrap();
        let mut session = if mobile {
            AppSession::for_test_mobile_with_timeout(env.paths.clone(), Duration::from_secs(5))
        } else {
            AppSession::for_test_with_timeout(env.paths.clone(), Duration::from_secs(5))
        }
        .unwrap();
        session.sync_engine = engine;
        session
            .set_discovery_candidates(crate::dto::DiscoveryPermissionDto::Granted, Vec::new())
            .unwrap();
        session.unlock("vault".into(), MASTER.into(), None).unwrap();
        Self {
            session: Arc::new(session),
            otp: otp.to_string(),
            other_otp: other_otp.to_string(),
            metadata: metadata.to_string(),
            group: group.to_string(),
            vault_path,
            env,
        }
    }

    fn ordinary() -> Self {
        Self::new(
            Arc::new(SucceedingSyncEngine(SyncOutcome::AlreadyInSync)),
            false,
        )
    }

    fn warm(&self, uuid: &str) {
        self.session.entry_detail(uuid.into()).unwrap();
        assert!(self.session.totp_now(uuid.into()).is_ok());
    }

    fn relock_and_unlock(&self) {
        self.session.lock_now().unwrap();
        self.session
            .unlock("vault".into(), MASTER.into(), None)
            .unwrap();
    }
}

fn edit(uri: Option<&str>) -> EntryEditDto {
    EntryEditDto {
        title: Some("changed metadata".into()),
        username: None,
        password: None,
        url: None,
        notes: None,
        tags: None,
        custom_fields: None,
        totp_uri: uri.map(str::to_owned),
    }
}

struct DetailGate {
    release: mpsc::Sender<()>,
    worker: Option<JoinHandle<Result<EntryDetail, HidlinsApiError>>>,
}

impl DetailGate {
    fn pause(session: Arc<AppSession>, uuid: String) -> Self {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let worker = thread::spawn(move || {
            PUBLISH_HOOK.with(|cell| {
                *cell.borrow_mut() = Some(Box::new(move || {
                    entered_tx.send(()).unwrap();
                    released
                        .recv_timeout(WAIT)
                        .expect("release paused publication");
                }));
            });
            session.entry_detail(uuid)
        });
        let gate = Self {
            release,
            worker: Some(worker),
        };
        entered_rx
            .recv_timeout(WAIT)
            .expect("actual detail extraction reached publication");
        gate
    }

    fn finish(mut self) {
        self.release.send(()).unwrap();
        self.worker.take().unwrap().join().unwrap().unwrap();
    }
}

impl Drop for DetailGate {
    fn drop(&mut self) {
        let _ = self.release.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct GateEngine {
    entered: mpsc::Sender<LanCancellation>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl SyncEnginePort for GateEngine {
    fn sync_now(
        &self,
        _vault: &mut Vault,
        _name: &str,
        _registry: &mut VaultRegistry,
        _master: &MasterPassword,
        _keyfile: Option<&Keyfile>,
        _candidates: Vec<hidlins_sync::address::LocalEndpoint>,
        _options: SyncOptions,
        cancellation: LanCancellation,
    ) -> Result<SyncOutcome, hidlins_sync::SyncError> {
        self.entered.send(cancellation).unwrap();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(WAIT)
            .expect("release owned sync");
        Ok(SyncOutcome::AlreadyInSync)
    }
}

struct SyncGate {
    release: mpsc::Sender<()>,
    worker: Option<JoinHandle<Result<crate::dto::SyncOutcomeDto, HidlinsApiError>>>,
}

impl SyncGate {
    fn finish(&mut self) -> Result<crate::dto::SyncOutcomeDto, HidlinsApiError> {
        self.release.send(()).unwrap();
        self.worker.take().unwrap().join().unwrap()
    }
}

impl Drop for SyncGate {
    fn drop(&mut self) {
        let _ = self.release.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct OwnedSync {
    // Release/join before dropping the fixture, including assertion unwinds.
    gate: SyncGate,
    fixture: Fixture,
    cancellation: LanCancellation,
    events: mpsc::Receiver<LockEvent>,
    now: Instant,
    #[cfg(not(feature = "desktop"))]
    ticket: crate::dto::ClipboardTransferTicket,
}

impl OwnedSync {
    fn new(mobile: bool) -> Self {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let fixture = Fixture::new(
            Arc::new(GateEngine {
                entered: entered_tx,
                release: Mutex::new(released),
            }),
            mobile,
        );
        fixture.warm(&fixture.otp);
        #[cfg(not(feature = "desktop"))]
        let ticket = fixture
            .session
            .prepare_clipboard_transfer(fixture.otp.clone(), crate::dto::CopyField::TotpCode)
            .unwrap();
        let (sink, events) = mpsc::channel();
        fixture
            .session
            .lock_events_for_test(Box::new(MpscEventSink::new(sink)));
        assert!(matches!(
            events.recv_timeout(WAIT).unwrap(),
            LockEvent::Unlocked
        ));
        let now = Instant::now();
        fixture.session.drive_tick(now);
        let session = Arc::clone(&fixture.session);
        let worker = thread::spawn(move || session.sync_now());
        let gate = SyncGate {
            release,
            worker: Some(worker),
        };
        let cancellation = entered_rx
            .recv_timeout(WAIT)
            .expect("actual sync engine entered");
        Self {
            gate,
            fixture,
            cancellation,
            events,
            now,
            #[cfg(not(feature = "desktop"))]
            ticket,
        }
    }

    fn expire(&self) {
        self.fixture
            .session
            .drive_tick(self.now + Duration::from_secs(5));
        self.fixture
            .session
            .drive_tick(self.now + Duration::from_secs(6));
    }
}

#[test]
fn idle_lock_during_owned_sync_revokes_before_handback_and_emits_once() {
    let mut owned = OwnedSync::new(false);
    owned.expire();
    let canceled = owned.cancellation.is_cancelled();
    let cache_revoked = owned
        .fixture
        .session
        .totp_now(owned.fixture.otp.clone())
        .is_err();
    let credentials_revoked = !owned.fixture.session.has_credentials();
    let before = owned
        .events
        .try_iter()
        .filter(|e| matches!(e, LockEvent::Locked))
        .count();
    let result = owned.gate.finish();
    let after = owned
        .events
        .try_iter()
        .filter(|e| matches!(e, LockEvent::Locked))
        .count();
    assert!(canceled && cache_revoked && credentials_revoked,
        "pending idle lock must cancel and revoke before worker release: {canceled}/{cache_revoked}/{credentials_revoked}");
    assert_eq!(
        (before, after),
        (1, 0),
        "one authoritative transition, no handback duplicate"
    );
    assert!(matches!(result, Err(HidlinsApiError::VaultLocked)));
    assert!(!owned.fixture.session.has_vault());
    assert!(!owned.fixture.session.has_credentials());
    assert!(Vault::open(
        &owned.fixture.vault_path,
        &MasterPassword::new(MASTER.into()),
        None
    )
    .is_ok());
}

#[test]
fn detached_lock_during_owned_sync_revokes_before_handback() {
    let mut owned = OwnedSync::new(true);
    let reply = owned
        .fixture
        .session
        .report_lifecycle_state(crate::dto::LifecycleStateDto::Detached);
    let revoked = owned
        .fixture
        .session
        .totp_now(owned.fixture.otp.clone())
        .is_err();
    let before = owned
        .events
        .try_iter()
        .filter(|e| matches!(e, LockEvent::Locked))
        .count();
    let result = owned.gate.finish();
    let after = owned
        .events
        .try_iter()
        .filter(|e| matches!(e, LockEvent::Locked))
        .count();
    assert!(matches!(reply, LockEvent::Locked));
    assert!(
        revoked,
        "lifecycle lock must revoke cached OTP while the worker is paused"
    );
    assert_eq!((before, after), (1, 0));
    assert!(matches!(result, Err(HidlinsApiError::VaultLocked)));
}

#[test]
fn ordinary_owned_sync_keeps_codes_and_returns_vault_without_locking() {
    let mut owned = OwnedSync::new(false);
    assert!(owned
        .fixture
        .session
        .totp_now(owned.fixture.otp.clone())
        .is_ok());
    assert!(!owned.cancellation.is_cancelled());
    owned.gate.finish().unwrap();
    assert!(owned.fixture.session.has_vault());
    assert!(owned.fixture.session.has_credentials());
    assert!(owned
        .events
        .try_iter()
        .all(|e| !matches!(e, LockEvent::Locked)));
    // Existing successful-sync invalidation is retained.
    assert!(owned
        .fixture
        .session
        .totp_now(owned.fixture.otp.clone())
        .is_err());
    owned.fixture.warm(&owned.fixture.otp);
}

#[test]
fn lock_subscription_during_owned_sync_reports_logically_unlocked() {
    let mut owned = OwnedSync::new(false);
    let (sink, events) = mpsc::channel();
    owned
        .fixture
        .session
        .lock_events_for_test(Box::new(MpscEventSink::new(sink)));
    let snapshot = events.recv_timeout(WAIT).unwrap();
    owned.gate.finish().unwrap();
    assert!(
        matches!(snapshot, LockEvent::Unlocked),
        "worker ownership is not a lock"
    );
}

#[test]
fn lifecycle_reply_during_ordinary_owned_sync_reports_unlocked() {
    let mut owned = OwnedSync::new(false);
    let reply = owned
        .fixture
        .session
        .report_lifecycle_state(crate::dto::LifecycleStateDto::Resumed);
    owned.gate.finish().unwrap();
    assert!(matches!(reply, LockEvent::Unlocked));
}

#[test]
fn subscription_after_pending_manual_lock_reports_locked() {
    let mut owned = OwnedSync::new(false);
    owned.fixture.session.lock_now().unwrap();
    let (sink, events) = mpsc::channel();
    owned
        .fixture
        .session
        .lock_events_for_test(Box::new(MpscEventSink::new(sink)));
    assert!(matches!(
        events.recv_timeout(WAIT).unwrap(),
        LockEvent::Locked
    ));
    assert!(matches!(
        owned.gate.finish(),
        Err(HidlinsApiError::VaultLocked)
    ));
}

#[test]
fn detail_publication_cannot_recreate_cache_after_lock() {
    let fixture = Fixture::ordinary();
    let detail = DetailGate::pause(Arc::clone(&fixture.session), fixture.otp.clone());
    fixture.session.lock_now().unwrap();
    detail.finish();
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_err());
}

#[test]
fn old_detail_cannot_replace_new_fill_after_lock_unlock_and_edit() {
    let fixture = Fixture::ordinary();
    let detail = DetailGate::pause(Arc::clone(&fixture.session), fixture.otp.clone());
    fixture.relock_and_unlock();
    fixture
        .session
        .update_entry(fixture.otp.clone(), edit(Some(NEW_URI)))
        .unwrap();
    fixture.warm(&fixture.otp);
    detail.finish();
    assert!(
        fixture
            .session
            .reveal_field(fixture.otp.clone(), RevealField::TotpUri)
            .unwrap()
            == NEW_URI,
        "the current fill must retain its current secret identity"
    );
}

#[test]
fn edit_revokes_an_older_inflight_fill() {
    let fixture = Fixture::ordinary();
    let detail = DetailGate::pause(Arc::clone(&fixture.session), fixture.otp.clone());
    fixture
        .session
        .update_entry(fixture.otp.clone(), edit(Some(NEW_URI)))
        .unwrap();
    detail.finish();
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_err());
    fixture.warm(&fixture.otp);
    assert!(
        fixture
            .session
            .reveal_field(fixture.otp.clone(), RevealField::TotpUri)
            .unwrap()
            == NEW_URI
    );
}

#[test]
fn entry_delete_revokes_an_older_inflight_fill() {
    let fixture = Fixture::ordinary();
    let detail = DetailGate::pause(Arc::clone(&fixture.session), fixture.otp.clone());
    fixture.session.delete_entry(fixture.otp.clone()).unwrap();
    detail.finish();
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_err());
}

#[test]
fn entry_purge_revokes_an_older_inflight_fill() {
    let fixture = Fixture::ordinary();
    fixture.session.delete_entry(fixture.otp.clone()).unwrap();
    let detail = DetailGate::pause(Arc::clone(&fixture.session), fixture.otp.clone());
    fixture.session.purge_entry(fixture.otp.clone()).unwrap();
    detail.finish();
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_err());
}

#[test]
fn recursive_group_delete_revokes_descendant_fill_but_preserves_other_cache() {
    let fixture = Fixture::ordinary();
    fixture.warm(&fixture.other_otp);
    let detail = DetailGate::pause(Arc::clone(&fixture.session), fixture.otp.clone());
    fixture
        .session
        .delete_group(fixture.group.clone(), GroupDeleteBehaviorDto::Recurse)
        .unwrap();
    detail.finish();
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_err());
    assert!(fixture.session.totp_now(fixture.other_otp.clone()).is_ok());
}

#[test]
fn unrelated_mutation_preserves_cached_and_pending_other_entry() {
    let fixture = Fixture::ordinary();
    fixture.warm(&fixture.otp);
    let other = DetailGate::pause(Arc::clone(&fixture.session), fixture.other_otp.clone());
    fixture
        .session
        .update_entry(fixture.metadata.clone(), edit(None))
        .unwrap();
    other.finish();
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_ok());
    assert!(fixture.session.totp_now(fixture.other_otp.clone()).is_ok());
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

#[derive(Clone, Copy)]
enum FailedMutation {
    Edit,
    Delete,
    Purge,
}

fn assert_failed_save_revokes_cache(mutation: FailedMutation) {
    let fixture = Fixture::ordinary();
    if matches!(mutation, FailedMutation::Purge) {
        fixture.session.delete_entry(fixture.otp.clone()).unwrap();
    }
    fixture.warm(&fixture.otp);
    let before = std::fs::read(&fixture.vault_path).unwrap();
    let directory = ReadOnlyDirectory::new(fixture.env.paths.state_dir());
    let result = match mutation {
        FailedMutation::Edit => fixture
            .session
            .update_entry(fixture.otp.clone(), edit(Some(NEW_URI))),
        FailedMutation::Delete => fixture.session.delete_entry(fixture.otp.clone()),
        FailedMutation::Purge => fixture.session.purge_entry(fixture.otp.clone()),
    };
    drop(directory);
    assert!(result.is_err(), "the actual atomic stage write must fail");
    assert!(
        fixture.session.totp_now(fixture.otp.clone()).is_err(),
        "in-memory mutation must revoke cached material even when persistence fails"
    );
    assert_eq!(std::fs::read(&fixture.vault_path).unwrap(), before);
}

#[test]
fn failed_edit_save_revokes_cache() {
    assert_failed_save_revokes_cache(FailedMutation::Edit);
}
#[test]
fn failed_delete_save_revokes_cache() {
    assert_failed_save_revokes_cache(FailedMutation::Delete);
}
#[test]
fn failed_purge_save_revokes_cache() {
    assert_failed_save_revokes_cache(FailedMutation::Purge);
}

#[test]
fn failed_rotation_revokes_cached_codes_before_returning() {
    let fixture = Fixture::ordinary();
    fixture.warm(&fixture.otp);
    let before = std::fs::read(&fixture.vault_path).unwrap();
    let directory = ReadOnlyDirectory::new(fixture.env.paths.state_dir());
    let result = fixture
        .session
        .change_master_password(MASTER.into(), "replacement master".into());
    drop(directory);
    assert!(result.is_err());
    assert!(!fixture.session.has_vault());
    assert!(!fixture.session.has_credentials());
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_err());
    assert_eq!(std::fs::read(&fixture.vault_path).unwrap(), before);
}

struct CountingEngine(Arc<AtomicUsize>);
impl SyncEnginePort for CountingEngine {
    fn sync_now(
        &self,
        _vault: &mut Vault,
        _name: &str,
        _registry: &mut VaultRegistry,
        _master: &MasterPassword,
        _keyfile: Option<&Keyfile>,
        _candidates: Vec<hidlins_sync::address::LocalEndpoint>,
        _options: SyncOptions,
        _cancellation: LanCancellation,
    ) -> Result<SyncOutcome, hidlins_sync::SyncError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(SyncOutcome::AlreadyInSync)
    }
}

fn poison(session: Arc<AppSession>) {
    assert!(thread::spawn(move || {
        let _guard = session.hold_mutex_for_test();
        panic!("deliberate session poison regression");
    })
    .join()
    .is_err());
}

#[test]
fn poisoned_sync_claim_never_enters_engine() {
    let calls = Arc::new(AtomicUsize::new(0));
    let fixture = Fixture::new(Arc::new(CountingEngine(Arc::clone(&calls))), false);
    fixture.warm(&fixture.otp);
    poison(Arc::clone(&fixture.session));
    let result = fixture.session.sync_now(); // First accessor must be the real claim.
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(matches!(result, Err(HidlinsApiError::VaultLocked)));
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_err());
    assert!(!fixture.session.has_vault());
    assert!(!fixture.session.has_credentials());
}

#[test]
fn poisoned_sync_handback_discards_owned_resources() {
    let mut owned = OwnedSync::new(false);
    poison(Arc::clone(&owned.fixture.session));
    let result = owned.gate.finish(); // No intervening recovering state accessor.
    assert!(matches!(result, Err(HidlinsApiError::VaultLocked)));
    assert!(owned
        .fixture
        .session
        .totp_now(owned.fixture.otp.clone())
        .is_err());
    assert!(!owned.fixture.session.has_vault());
    assert!(!owned.fixture.session.has_credentials());
}

#[test]
fn poisoned_host_pump_revokes_cached_codes() {
    let fixture = Fixture::ordinary();
    fixture.warm(&fixture.otp);
    poison(Arc::clone(&fixture.session));
    fixture.session.pump_sync_server_for_test();
    assert!(fixture.session.totp_now(fixture.otp.clone()).is_err());
    assert!(fixture.session.is_dead());
}

#[test]
fn plain_tick_during_ordinary_unlock_does_not_abort_it() {
    let fixture = Fixture::ordinary();
    fixture.session.lock_now().unwrap();
    let tree = fixture.session.unlock_with_hook("vault", MASTER, None, || {
        fixture.session.drive_tick(Instant::now());
    });
    assert!(tree.is_ok());
    assert!(fixture.session.has_vault());
}

#[test]
fn cached_totp_does_not_wait_for_session_ownership() {
    let fixture = Fixture::ordinary();
    fixture.warm(&fixture.otp);
    let guard = fixture.session.hold_mutex_for_test();
    let session = Arc::clone(&fixture.session);
    let uuid = fixture.otp.clone();
    let (sent, received) = mpsc::channel();
    let worker = thread::spawn(move || sent.send(session.totp_now(uuid).is_ok()).unwrap());
    let completed_while_owned = received.recv_timeout(Duration::from_secs(1));
    drop(guard);
    worker.join().unwrap();
    assert!(completed_while_owned.unwrap());
}

#[cfg(not(feature = "desktop"))]
#[test]
fn pending_idle_lock_revokes_native_clipboard_ticket_before_handback() {
    let mut owned = OwnedSync::new(true);
    owned.expire();
    let revoked = crate::api::secrets::consume_clipboard_transfer(&owned.ticket.id).is_err();
    let result = owned.gate.finish();
    assert!(
        revoked,
        "pending lock must revoke the actual native-only transfer"
    );
    assert!(matches!(result, Err(HidlinsApiError::VaultLocked)));
}

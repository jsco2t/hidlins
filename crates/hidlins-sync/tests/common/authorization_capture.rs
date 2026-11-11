//! Shared private-authority race fixture; no shipped source or token constructors.

use std::{
    any::Any,
    cell::RefCell,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use hidlins_core::{
    HidlinsPaths, KdfParams, MasterPassword, NoRecoveryConfirmed, RegisteredVault, Vault,
    VaultRegistry,
};
use hidlins_sync::{
    address::LocalBindEndpoint,
    client::ClientPairingSession,
    config::local::LocalSyncConfig,
    identity::{PublicIdentity, SyncRole},
    pairing::{ConfirmedPairing, PairingTransaction, SasCode},
    server::{
        HostQueue, PairingAuthority, PairingAuthorityError, PairingAuthorization, ServerController,
    },
};

thread_local! {
    static BEFORE_LOCK: RefCell<Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>> = const { RefCell::new(None) };
}

pub fn before_lock() {
    BEFORE_LOCK.with(|hook| {
        if let Some((entered, release)) = hook.borrow_mut().take() {
            entered.send(()).unwrap();
            release.recv_timeout(Duration::from_secs(10)).unwrap();
        }
    });
}

pub struct Fixture {
    pub paths: HidlinsPaths,
    pub master: MasterPassword,
    config: LocalSyncConfig,
    _directory: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let paths = HidlinsPaths::with_state_dir(directory.path().join("state"));
        let path = directory.path().join("vault.kdbx");
        let master = MasterPassword::new("authority regression master".into());
        drop(
            Vault::create(
                &path,
                &master,
                None,
                KdfParams {
                    memory_kib: 1_024,
                    iterations: 1,
                    parallelism: 1,
                },
                NoRecoveryConfirmed::yes(),
            )
            .unwrap(),
        );
        let mut registry = VaultRegistry::load(paths.clone()).unwrap();
        registry
            .register_and_save(RegisteredVault {
                name: "vault".into(),
                path,
                created_at: "2026-11-04T00:00:00Z".into(),
                keyfile_path: None,
                extra: toml::Table::new(),
            })
            .unwrap();
        let config =
            LocalSyncConfig::configure(&mut registry, "vault", SyncRole::Server, &master).unwrap();
        Self {
            paths,
            master,
            config,
            _directory: directory,
        }
    }

    fn reload(&self) -> LocalSyncConfig {
        let registry = VaultRegistry::load(self.paths.clone()).unwrap();
        LocalSyncConfig::from_vault_entry(registry.get("vault").unwrap()).unwrap()
    }
}

pub struct AuthorityOwner {
    pub authority: Arc<dyn PairingAuthority>,
    pub keepalive: Box<dyn Any>,
}

struct CaptureAuthority {
    target: Arc<dyn PairingAuthority>,
    captured: Mutex<Option<(PairingTransaction, PairingAuthorization)>>,
}

impl PairingAuthority for CaptureAuthority {
    fn confirm(
        &self,
        _peer: PublicIdentity,
        _sas: SasCode,
        _timeout: Duration,
    ) -> Result<String, PairingAuthorityError> {
        Ok("confirmed client".into())
    }

    fn prepare(
        &self,
        tx: &PairingTransaction,
        proof: &ConfirmedPairing,
        name: String,
    ) -> Result<(), PairingAuthorityError> {
        self.target.prepare(tx, proof, name)
    }

    fn activate(&self, _tx: &PairingTransaction) -> Result<(), PairingAuthorityError> {
        unreachable!("runtime must use the original-window authorization boundary")
    }

    fn activate_authorized(
        &self,
        tx: &PairingTransaction,
        permission: &PairingAuthorization,
    ) -> Result<(), PairingAuthorityError> {
        *self.captured.lock().unwrap() = Some((tx.clone(), permission.clone()));
        Err(PairingAuthorityError::Persistence)
    }

    fn recover(&self, peer: PublicIdentity) -> Result<PairingTransaction, PairingAuthorityError> {
        self.target.recover(peer)
    }

    fn shutdown(&self) {}
}

#[allow(clippy::too_many_lines)]
pub fn run_case(factory: impl FnOnce(&Fixture) -> AuthorityOwner, replace_window: Option<bool>) {
    let fixture = Fixture::new();
    let owner = factory(&fixture);
    let capture = Arc::new(CaptureAuthority {
        target: owner.authority.clone(),
        captured: Mutex::new(None),
    });
    let identity = fixture
        .config
        .identity()
        .unlock("vault", SyncRole::Server, &fixture.master)
        .unwrap();
    let (queue, _processor) = HostQueue::new();
    let mut server = ServerController::start_on_bind_endpoint_with_pairing_authority(
        LocalBindEndpoint::ephemeral("127.0.0.1".parse().unwrap(), 0).unwrap(),
        Arc::new(identity),
        [],
        queue,
        Some(capture.clone()),
    )
    .unwrap();
    server.open_pairing().unwrap();
    let mut client = LocalSyncConfig::create("client", SyncRole::Client, &fixture.master).unwrap();
    let client_identity = client
        .identity()
        .unlock("client", SyncRole::Client, &fixture.master)
        .unwrap();
    let session = ClientPairingSession::begin(&client_identity, [server.endpoint()]).unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(session
        .confirm(&mut client, "server".into(), now, |_| Ok(()))
        .is_err());
    let (transaction, permission) = capture.captured.lock().unwrap().take().unwrap();
    let peer = transaction.client_key();
    assert!(!fixture.reload().authorizes(peer));
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let target = owner.authority.clone();
    let activation = thread::spawn(move || {
        BEFORE_LOCK.with(|hook| *hook.borrow_mut() = Some((entered_tx, release_rx)));
        target.activate_authorized(&transaction, &permission)
    });
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    match replace_window {
        Some(true) => server.open_pairing().unwrap(),
        Some(false) => server.close_pairing(),
        None => {}
    }
    release_tx.send(()).unwrap();
    let result = activation.join().unwrap();
    if replace_window.is_some() {
        assert!(
            result.is_err(),
            "permission canceled while activation waited before its lock"
        );
        assert!(
            !fixture.reload().authorizes(peer),
            "canceled activation must not change durable trust"
        );
    } else {
        result.expect("original window remains authorized");
        assert!(fixture.reload().authorizes(peer));
        server.close_pairing();
        assert!(
            fixture.reload().authorizes(peer),
            "close preserves trust activated before cancellation"
        );
    }
    server.stop();
    // Keep private app/session owners alive through every callback and readback.
    drop(owner.keepalive);
}

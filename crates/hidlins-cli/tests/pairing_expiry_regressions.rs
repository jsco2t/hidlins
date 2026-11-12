//! A restarted CLI must leave expired recovery behind and confirm fresh XX.

mod common;

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use common::{run_with_stdin, seed_vault, VaultsToml};
use hidlins_core::{
    HidlinsPaths, KdfParams, MasterPassword, NoRecoveryConfirmed, Vault, VaultRegistry,
};
use hidlins_sync::{
    address::LocalBindEndpoint,
    client::PendingPairingStore,
    config::local::LocalSyncConfig,
    identity::{PublicIdentity, SyncRole},
    pairing::{ConfirmedPairing, PairingTransaction, PairingWindow, SasCode},
    server::{
        AuthoritativeVault, HostQueue, PairingAuthority, PairingAuthorityError, ServerController,
    },
};

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

struct Authority(Mutex<LocalSyncConfig>);

impl PairingAuthority for Authority {
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
        confirmation: &ConfirmedPairing,
        display_name: String,
    ) -> Result<(), PairingAuthorityError> {
        let now = epoch_seconds();
        self.0
            .lock()
            .unwrap()
            .prepare_client(tx, confirmation, display_name, now, now + 180)
            .map_err(|_| PairingAuthorityError::Persistence)
    }

    fn activate(&self, tx: &PairingTransaction) -> Result<(), PairingAuthorityError> {
        self.0
            .lock()
            .unwrap()
            .activate_client(tx, epoch_seconds())
            .map_err(|_| PairingAuthorityError::Persistence)
    }

    fn recover(&self, peer: PublicIdentity) -> Result<PairingTransaction, PairingAuthorityError> {
        self.0
            .lock()
            .unwrap()
            .recover_pairing_transaction(peer, epoch_seconds())
            .map_err(|_| PairingAuthorityError::Persistence)
    }

    fn shutdown(&self) {}
}

struct HostWorker {
    running: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for HostWorker {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        self.thread.take().unwrap().join().unwrap();
    }
}

fn expired_client(master: &MasterPassword) -> LocalSyncConfig {
    let mut client = LocalSyncConfig::create("client", SyncRole::Client, master).unwrap();
    let old_server = LocalSyncConfig::create("client", SyncRole::Server, master).unwrap();
    let now = Instant::now();
    let mut window = PairingWindow::open_at(now);
    let sas = window
        .begin_candidate(old_server.identity().public_key(), [11; 32], now)
        .unwrap();
    let confirmation = window.confirm(sas, now).unwrap();
    let tx = confirmation
        .transaction(
            old_server.identity().public_key(),
            client.identity().public_key(),
        )
        .unwrap();
    client
        .prepare_server(&tx, &confirmation, "expired server".into(), 1_000, 1_180)
        .unwrap();
    client
}

#[allow(clippy::too_many_lines)]
fn retry_via_cli(import: bool) {
    let fixture = VaultsToml::new();
    let paths = HidlinsPaths::with_registry_file(fixture.vaults_toml.clone());
    let master = MasterPassword::new("test-master".into());
    let config = expired_client(&master);
    let original_identity = config.identity().clone();
    let pending = PendingPairingStore::new(paths.state_dir(), "client");
    if import {
        pending.save(&config).unwrap();
    } else {
        seed_vault(&fixture, "client", "test-master");
        let mut registry = VaultRegistry::load(paths.clone()).unwrap();
        config.persist(&mut registry, "client").unwrap();
    }
    let server_config = LocalSyncConfig::create("server", SyncRole::Server, &master).unwrap();
    let identity = server_config
        .identity()
        .unlock("server", SyncRole::Server, &master)
        .unwrap();
    let authority = Arc::new(Authority(Mutex::new(server_config)));
    let (queue, mut processor) = HostQueue::new();
    let mut server = ServerController::start_on_bind_endpoint_with_pairing_authority(
        LocalBindEndpoint::ephemeral("127.0.0.1".parse().unwrap(), 0).unwrap(),
        Arc::new(identity),
        [],
        queue,
        Some(authority.clone()),
    )
    .unwrap();
    server.open_pairing().unwrap();
    let source = Vault::create(
        &fixture.tempdir.path().join("source.kdbx"),
        &master,
        None,
        KdfParams {
            memory_kib: 1_024,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let thread_running = Arc::clone(&running);
    let worker = HostWorker {
        running,
        thread: Some(thread::spawn(move || {
            let mut source = source;
            let master = MasterPassword::new("test-master".into());
            let mut host = AuthoritativeVault::new(&mut source, &master, None);
            while thread_running.load(Ordering::Acquire) {
                if !processor.process_one(&mut host).unwrap_or(false) {
                    thread::yield_now();
                }
            }
        })),
    };
    let port = server.endpoint().port().to_string();
    let target = fixture.tempdir.path().join("imported.kdbx");
    let mut args = vec!["--format", "json", "sync"];
    if import {
        args.extend([
            "import",
            "--id",
            "client",
            "--path",
            target.to_str().unwrap(),
        ]);
    } else {
        args.extend(["pair", "--vault", "client"]);
    }
    args.extend(["--address", "127.0.0.1", "--port", &port]);
    let result = run_with_stdin(&fixture, &args, "test-master\nyes\n");
    server.stop();
    drop(worker);
    assert_eq!(
        result.0, 0,
        "fresh CLI pairing failed: {} {}",
        result.1, result.2
    );
    assert!(
        result.2.contains("Pairing SAS:"),
        "fresh XX requires local SAS confirmation"
    );
    let registry = VaultRegistry::load(paths).unwrap();
    let active = LocalSyncConfig::from_vault_entry(registry.get("client").unwrap()).unwrap();
    assert!(active.is_active_client());
    assert_eq!(
        active.identity(),
        &original_identity,
        "expiry must not rotate identity"
    );
    assert!(authority
        .0
        .lock()
        .unwrap()
        .authorizes(active.identity().public_key()));
    if import {
        assert!(target.is_file());
        assert!(pending.load().unwrap().is_none());
        Vault::open(&target, &MasterPassword::new("test-master".into()), None).unwrap();
    }
}

#[test]
fn registered_pair_retries_expired_prepare_with_fresh_confirmed_xx() {
    retry_via_cli(false);
}

#[test]
fn pending_import_retries_expired_prepare_with_fresh_confirmed_xx() {
    retry_via_cli(true);
}

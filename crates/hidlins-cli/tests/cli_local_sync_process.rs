//! Real spawned-process CLI journey for local serving, pairing, import, and cleanup.

mod common;

use std::{
    io::{BufRead as _, BufReader, Write as _},
    net::{Ipv4Addr, TcpListener},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use common::{hidlins_cmd, run_with_stdin, seed_vault, VaultsToml};
use hidlins_core::{
    HidlinsPaths, KdfParams, MasterPassword, NoRecoveryConfirmed, RegisteredVault, Vault,
    VaultRegistry,
};
use hidlins_sync::{
    address::LocalEndpoint,
    client::{ClientPairingSession, PendingPairingStore},
    config::local::LocalSyncConfig,
    identity::{PublicIdentity, SyncRole},
    pairing::{ConfirmedPairing, PairingTransaction, SasCode},
    protocol::PAIRING_WINDOW,
    server::{
        AuthoritativeVault, HostProcessor, HostQueue, PairingAuthority, PairingAuthorityError,
        ServerController,
    },
};

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_secs()
}

fn unused_loopback() -> LocalEndpoint {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind ephemeral port");
    let port = listener.local_addr().expect("ephemeral address").port();
    drop(listener);
    LocalEndpoint::new(Ipv4Addr::LOCALHOST.into(), port, 0).expect("loopback endpoint")
}

struct LoseFirstActivationAcknowledgement {
    config: Mutex<LocalSyncConfig>,
    lose_first: AtomicBool,
}

impl PairingAuthority for LoseFirstActivationAcknowledgement {
    fn confirm(
        &self,
        _peer: PublicIdentity,
        _sas: SasCode,
        _timeout: Duration,
    ) -> Result<String, PairingAuthorityError> {
        Ok("CLI recovery client".to_string())
    }

    fn prepare(
        &self,
        transaction: &PairingTransaction,
        confirmation: &ConfirmedPairing,
        display_name: String,
    ) -> Result<(), PairingAuthorityError> {
        let now = epoch_seconds();
        self.config
            .lock()
            .expect("authority config")
            .prepare_client(
                transaction,
                confirmation,
                display_name,
                now,
                now.saturating_add(PAIRING_WINDOW.as_secs()),
            )
            .map_err(|_| PairingAuthorityError::Persistence)
    }

    fn activate(&self, transaction: &PairingTransaction) -> Result<(), PairingAuthorityError> {
        self.config
            .lock()
            .expect("authority config")
            .activate_client(transaction, epoch_seconds())
            .map_err(|_| PairingAuthorityError::Persistence)?;
        if self.lose_first.swap(false, Ordering::AcqRel) {
            return Err(PairingAuthorityError::Persistence);
        }
        Ok(())
    }

    fn recover(&self, peer: PublicIdentity) -> Result<PairingTransaction, PairingAuthorityError> {
        self.config
            .lock()
            .expect("authority config")
            .recover_pairing_transaction(peer, epoch_seconds())
            .map_err(|_| PairingAuthorityError::Persistence)
    }

    fn shutdown(&self) {}
}

fn start_server_that_loses_first_activation(
    vault_name: &str,
    master: &MasterPassword,
) -> (ServerController, HostProcessor) {
    let server_config =
        LocalSyncConfig::create(vault_name, SyncRole::Server, master).expect("server config");
    let server_identity = Arc::new(
        server_config
            .identity()
            .unlock(vault_name, SyncRole::Server, master)
            .expect("server identity"),
    );
    let authority = Arc::new(LoseFirstActivationAcknowledgement {
        config: Mutex::new(server_config),
        lose_first: AtomicBool::new(true),
    });
    let (queue, processor) = HostQueue::new();
    let server = ServerController::start_with_pairing_authority(
        unused_loopback(),
        server_identity,
        [],
        queue,
        Some(authority),
    )
    .expect("start pairing server");
    server.open_pairing().expect("open pairing window");
    (server, processor)
}

fn spawn_host_processor(
    mut processor: HostProcessor,
    source: Vault,
) -> (Arc<AtomicBool>, thread::JoinHandle<()>) {
    let running = Arc::new(AtomicBool::new(true));
    let worker_running = Arc::clone(&running);
    let handle = thread::spawn(move || {
        let mut source = source;
        let host_master = MasterPassword::new("test-master".to_string());
        let mut host = AuthoritativeVault::new(&mut source, &host_master, None);
        while worker_running.load(Ordering::Acquire) {
            if !processor.process_one(&mut host).unwrap_or(false) {
                thread::yield_now();
            }
        }
    });
    (running, handle)
}

fn sync_address() -> String {
    std::env::var("HIDLINS_TEST_SYNC_ADDRESS").unwrap_or_else(|_| "127.0.0.1".to_string())
}

fn register_copy(registry: &VaultsToml, source: &std::path::Path, name: &str) {
    let path = registry.tempdir.path().join(format!("{name}.kdbx"));
    std::fs::copy(source, &path).expect("copy vault");
    let mut records = VaultRegistry::load(HidlinsPaths::with_registry_file(
        registry.vaults_toml.clone(),
    ))
    .expect("load client registry");
    records
        .register(RegisteredVault {
            name: name.into(),
            path,
            created_at: "2026-01-01T00:00:00Z".into(),
            keyfile_path: None,
            extra: toml::Table::new(),
        })
        .expect("register copy");
    records.save().expect("save client registry");
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one ordered multi-process lifecycle journey"
)]
fn spawned_cli_local_sync_journey_and_signal_cleanup() {
    // LNS-PROCESS-001: this test is replayed in Linux network namespaces on
    // private IPv4 and IPv6 ULA addresses by tools/local-sync-tests/.
    let address = sync_address();
    let server = VaultsToml::new();
    let client = VaultsToml::new();
    let imported = VaultsToml::new();
    let server_vault = seed_vault(&server, "authority", "test-master");
    register_copy(&client, &server_vault, "client");

    let port = 40_000 + u16::try_from(std::process::id() % 20_000).expect("bounded process id");
    let mut command = hidlins_cmd();
    command
        .args([
            "--registry",
            &server.registry_arg(),
            "--format",
            "json",
            "sync",
            "serve",
            "--vault",
            "authority",
            "--address",
            &address,
            "--port",
            &port.to_string(),
            "--pairing-window",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut service = command.spawn().expect("spawn foreground service");
    service
        .stdin
        .as_mut()
        .expect("server stdin")
        .write_all(b"test-master\nyes\nyes\n")
        .expect("preload secure prompt and bilateral confirmations");
    let mut readiness = String::new();
    BufReader::new(service.stdout.take().expect("server stdout"))
        .read_line(&mut readiness)
        .expect("read readiness");
    let ready: serde_json::Value = serde_json::from_str(&readiness).expect("JSON readiness");
    assert_eq!(
        ready["status"], "serving",
        "unexpected readiness: {readiness}"
    );
    let expected_endpoint = if address.contains(':') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    };
    assert_eq!(ready["endpoint"], expected_endpoint);

    let port_text = port.to_string();
    let pair = run_with_stdin(
        &client,
        &[
            "--format",
            "json",
            "sync",
            "pair",
            "--vault",
            "client",
            "--address",
            &address,
            "--port",
            &port_text,
        ],
        "test-master\nyes\n",
    );
    assert_eq!(pair.0, 0, "pair failed: {} {}", pair.1, pair.2);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&pair.1).unwrap()["status"],
        "paired"
    );
    assert!(
        service.try_wait().expect("server status").is_none(),
        "server exited after pair"
    );

    // LNS-PROCESS-004: active server-side trust must survive an authority
    // process restart before the first authenticated IK sync.
    let signal = std::process::Command::new("kill")
        .args(["-INT", &service.id().to_string()])
        .status()
        .expect("send SIGINT before authority restart");
    assert!(signal.success());
    let status = service.wait().expect("wait for authority restart");
    assert!(status.success(), "authority did not exit cleanly: {status}");

    let mut command = hidlins_cmd();
    command
        .args([
            "--registry",
            &server.registry_arg(),
            "--format",
            "json",
            "sync",
            "serve",
            "--vault",
            "authority",
            "--address",
            &address,
            "--port",
            &port.to_string(),
            "--pairing-window",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    service = command.spawn().expect("restart foreground service");
    service
        .stdin
        .as_mut()
        .expect("restarted server stdin")
        .write_all(b"test-master\nyes\n")
        .expect("preload unlock and import confirmation");
    readiness.clear();
    BufReader::new(service.stdout.take().expect("restarted server stdout"))
        .read_line(&mut readiness)
        .expect("read restarted readiness");
    let ready: serde_json::Value =
        serde_json::from_str(&readiness).expect("restarted JSON readiness");
    assert_eq!(
        ready["status"], "serving",
        "unexpected restart: {readiness}"
    );

    let first_sync = run_with_stdin(
        &client,
        &[
            "sync",
            "now",
            "--vault",
            "client",
            "--address",
            &address,
            "--port",
            &port_text,
        ],
        "test-master\n",
    );
    assert_eq!(
        first_sync.0, 0,
        "first paired sync failed: {} {}",
        first_sync.1, first_sync.2
    );

    let peers = run_with_stdin(
        &server,
        &[
            "--format",
            "json",
            "sync",
            "peers",
            "list",
            "--vault",
            "authority",
        ],
        "",
    );
    assert_eq!(peers.0, 0, "peer list failed: {}", peers.2);
    assert!(peers.1.contains("peer-0"));
    assert!(!peers.1.contains("public_key"));

    let import_path = imported.tempdir.path().join("imported.kdbx");
    let import = run_with_stdin(
        &imported,
        &[
            "--format",
            "json",
            "sync",
            "import",
            "--id",
            "imported",
            "--path",
            import_path.to_str().unwrap(),
            "--address",
            &address,
            "--port",
            &port_text,
        ],
        "test-master\nyes\n",
    );
    assert!(
        service.try_wait().expect("server status").is_none(),
        "server exited during import: {} {}",
        import.1,
        import.2
    );
    assert_eq!(import.0, 0, "import failed: {} {}", import.1, import.2);
    assert!(import_path.exists());

    let now = run_with_stdin(
        &imported,
        &[
            "--format",
            "json",
            "sync",
            "now",
            "--vault",
            "imported",
            "--address",
            &address,
            "--port",
            &port_text,
        ],
        "test-master\n",
    );
    assert_eq!(now.0, 0, "manual sync failed: {} {}", now.1, now.2);

    let rename = run_with_stdin(
        &server,
        &[
            "sync",
            "peers",
            "rename",
            "--vault",
            "authority",
            "--peer",
            "peer-0",
            "--name",
            "laptop",
        ],
        "",
    );
    assert_eq!(rename.0, 0, "rename failed: {}", rename.2);
    let revoke = run_with_stdin(
        &server,
        &[
            "sync",
            "peers",
            "revoke",
            "--vault",
            "authority",
            "--peer",
            "peer-0",
        ],
        "",
    );
    assert_eq!(revoke.0, 0, "revoke failed: {}", revoke.2);

    let signal = std::process::Command::new("kill")
        .args(["-INT", &service.id().to_string()])
        .status()
        .expect("send SIGINT to foreground service");
    assert!(signal.success());
    let status = service.wait().expect("wait for foreground service");
    assert!(status.success(), "service did not exit cleanly: {status}");
    assert!(std::net::TcpStream::connect((address.as_str(), port)).is_err());

    let offline_mutation = run_with_stdin(
        &client,
        &[
            "entry",
            "add",
            "--vault",
            "client",
            "--title",
            "offline-local",
        ],
        "test-master\n",
    );
    assert_eq!(
        offline_mutation.0, 0,
        "offline local mutation failed: {}",
        offline_mutation.2
    );
    assert_eq!(
        offline_mutation
            .2
            .matches("warning: pre-operation local sync failed")
            .count(),
        1,
        "startup must run once and never after save: {}",
        offline_mutation.2
    );
    let offline_read = run_with_stdin(
        &client,
        &["entry", "list", "--vault", "client"],
        "test-master\n",
    );
    assert_eq!(
        offline_read.0, 0,
        "offline local read failed: {}",
        offline_read.2
    );
    assert_eq!(
        offline_read
            .2
            .matches("warning: pre-operation local sync failed")
            .count(),
        1
    );
}

#[test]
fn spawned_cli_recovers_durable_pairing_after_lost_activation_acknowledgement() {
    // LNS-PROCESS-002: the first production protocol attempt leaves the
    // registered client durably provisional after the authority activates but
    // deliberately withholds its acknowledgement. A new CLI process must load
    // that state and choose authenticated IK recovery without a new SAS.
    let client = VaultsToml::new();
    seed_vault(&client, "client", "test-master");
    let paths = HidlinsPaths::with_registry_file(client.vaults_toml.clone());
    let master = MasterPassword::new("test-master".to_string());
    let mut registry = VaultRegistry::load(paths.clone()).expect("client registry");
    let mut config = LocalSyncConfig::configure(&mut registry, "client", SyncRole::Client, &master)
        .expect("configure client");
    let identity = config
        .identity()
        .unlock("client", SyncRole::Client, &master)
        .expect("client identity");
    let (mut server, _processor) = start_server_that_loses_first_activation("client", &master);
    let endpoint = server.endpoint();

    let session = ClientPairingSession::begin(&identity, [endpoint]).expect("begin pairing");
    assert!(
        session
            .confirm(
                &mut config,
                "authority".to_string(),
                epoch_seconds(),
                |prepared| prepared.persist(&mut registry, "client"),
            )
            .is_err(),
        "the injected lost acknowledgement must interrupt the first attempt"
    );
    let interrupted_registry = VaultRegistry::load(paths.clone()).expect("reload interrupted");
    let interrupted = LocalSyncConfig::from_vault_entry(
        interrupted_registry
            .get("client")
            .expect("registered client"),
    )
    .expect("interrupted sync config");
    assert!(interrupted.provisional().is_some());
    assert!(!interrupted.is_active_client());

    let port = endpoint.port().to_string();
    let recovered = run_with_stdin(
        &client,
        &[
            "--format",
            "json",
            "sync",
            "pair",
            "--vault",
            "client",
            "--address",
            "127.0.0.1",
            "--port",
            &port,
        ],
        "test-master\n",
    );
    assert_eq!(
        recovered.0, 0,
        "production recovery failed: {} {}",
        recovered.1, recovered.2
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&recovered.1).unwrap()["status"],
        "paired"
    );
    let recovered_registry = VaultRegistry::load(paths).expect("reload recovered");
    let recovered_config = LocalSyncConfig::from_vault_entry(
        recovered_registry.get("client").expect("registered client"),
    )
    .expect("recovered sync config");
    assert!(recovered_config.is_active_client());
    assert!(recovered_config.provisional().is_none());
    server.stop();
}

#[test]
fn spawned_cli_recovers_pending_import_without_early_file_or_registration() {
    // LNS-PROCESS-003: pending imports have a separate durable store and CLI
    // branch from registered vaults. Exercise that branch through a restarted
    // process while the provisional state remains non-authorizing locally.
    let client = VaultsToml::new();
    let paths = HidlinsPaths::with_registry_file(client.vaults_toml.clone());
    let master = MasterPassword::new("test-master".to_string());
    let mut config =
        LocalSyncConfig::create("imported", SyncRole::Client, &master).expect("client config");
    let identity = config
        .identity()
        .unlock("imported", SyncRole::Client, &master)
        .expect("client identity");
    let pending = PendingPairingStore::new(paths.state_dir(), "imported");
    let (mut server, processor) = start_server_that_loses_first_activation("imported", &master);
    let endpoint = server.endpoint();

    let session = ClientPairingSession::begin(&identity, [endpoint]).expect("begin pairing");
    assert!(
        session
            .confirm(
                &mut config,
                "authority".to_string(),
                epoch_seconds(),
                |prepared| pending.save(prepared),
            )
            .is_err(),
        "the injected lost acknowledgement must interrupt the first attempt"
    );
    let interrupted = pending
        .load()
        .expect("load pending state")
        .expect("durable pending state");
    assert!(interrupted.provisional().is_some());
    assert!(!interrupted.is_active_client());
    let target = client.tempdir.path().join("imported.kdbx");
    assert!(!target.exists());
    assert!(
        VaultRegistry::load(paths.clone())
            .expect("empty client registry")
            .get("imported")
            .is_none(),
        "provisional import must not create a usable registration"
    );

    let source_path = client.tempdir.path().join("authority.kdbx");
    let source = Vault::create(
        &source_path,
        &master,
        None,
        KdfParams {
            memory_kib: 1_024,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
    .expect("authority vault");
    let (running, host_thread) = spawn_host_processor(processor, source);
    let port = endpoint.port().to_string();
    let recovered = run_with_stdin(
        &client,
        &[
            "--format",
            "json",
            "sync",
            "import",
            "--id",
            "imported",
            "--path",
            target.to_str().expect("UTF-8 target"),
            "--address",
            "127.0.0.1",
            "--port",
            &port,
        ],
        "test-master\n",
    );
    running.store(false, Ordering::Release);
    host_thread.join().expect("host worker");
    server.stop();

    assert_eq!(
        recovered.0, 0,
        "production import recovery failed: {} {}",
        recovered.1, recovered.2
    );
    assert!(target.is_file());
    let recovered_registry = VaultRegistry::load(paths).expect("recovered registry");
    let recovered_config = LocalSyncConfig::from_vault_entry(
        recovered_registry
            .get("imported")
            .expect("import registration"),
    )
    .expect("active import config");
    assert!(recovered_config.is_active_client());
    assert!(pending
        .load()
        .expect("load cleared pending state")
        .is_none());
}

#[test]
fn public_literal_is_rejected_before_network_use() {
    let reg = VaultsToml::new();
    seed_vault(&reg, "authority", "test-master");
    let result = run_with_stdin(
        &reg,
        &[
            "sync",
            "serve",
            "--vault",
            "authority",
            "--address",
            "8.8.8.8",
            "--port",
            "37371",
        ],
        "test-master\n",
    );
    assert_eq!(result.0, 1, "public address accepted: {}", result.2);
    assert!(result.2.contains("non-public IP literal"));
}

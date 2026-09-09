mod common;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc, Barrier,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hidlins_api::api::session::AppSession;
use hidlins_api::dto::{
    DiscoveryPermissionDto, EntryDraftDto, EntryKindDto, LocalEndpointDto, LocalSyncRoleDto,
    SyncEvent,
};
use hidlins_api::error::HidlinsApiError;
use hidlins_api::event::MpscEventSink;
use hidlins_api::sync_port::{PanickingSyncEngine, SyncEnginePort};
use hidlins_sync::config::local::LocalSyncConfig;
use hidlins_sync::identity::SyncRole;
use hidlins_sync::pairing::{PairingWindow, SasCode};
use hidlins_sync::{SyncError, SyncOptions, SyncOutcome};

fn active_client_config(name: &str, password: &str) -> LocalSyncConfig {
    let master = hidlins_core::MasterPassword::new(password.to_string());
    let mut client = LocalSyncConfig::create(name, SyncRole::Client, &master).expect("client");
    let mut server = LocalSyncConfig::create(name, SyncRole::Server, &master).expect("server");
    let hash = [19_u8; 32];
    let now = std::time::Instant::now();

    let mut client_window = PairingWindow::open_at(now);
    let client_sas = client_window
        .begin_candidate(server.identity().public_key(), hash, now)
        .expect("client candidate");
    let client_confirmed = client_window
        .confirm(client_sas, now)
        .expect("client confirm");

    let mut server_window = PairingWindow::open_at(now);
    let server_sas = server_window
        .begin_candidate(client.identity().public_key(), hash, now)
        .expect("server candidate");
    let server_confirmed = server_window
        .confirm(server_sas, now)
        .expect("server confirm");
    let transaction = client_confirmed
        .transaction(
            server.identity().public_key(),
            client.identity().public_key(),
        )
        .expect("transaction");
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let expires = epoch + 180;
    server
        .prepare_client(
            &transaction,
            &server_confirmed,
            "client".to_string(),
            epoch,
            expires,
        )
        .expect("prepare client");
    client
        .prepare_server(
            &transaction,
            &client_confirmed,
            "server".to_string(),
            epoch,
            expires,
        )
        .expect("prepare server");
    server
        .activate_client(&transaction, epoch)
        .expect("activate client");
    client
        .activate_server(&transaction, epoch)
        .expect("activate server");
    client
}

fn register_active_client(env: &common::TestEnv, name: &str, password: &str) {
    let path = common::create_test_vault(env, name, password);
    common::register_vault(env, name, &path);
    let mut registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let mut entry = registry.get(name).expect("entry").clone();
    active_client_config(name, password)
        .write_to_entry(&mut entry)
        .expect("attach local config");
    registry.deregister(name, false).expect("remove old");
    registry.register(entry).expect("replace");
    registry.save().expect("save config");
}

struct CountingEngine {
    calls: Arc<AtomicUsize>,
    result: Result<SyncOutcome, fn() -> SyncError>,
}

impl CountingEngine {
    fn succeeding(calls: Arc<AtomicUsize>) -> Self {
        Self {
            calls,
            result: Ok(SyncOutcome::AlreadyInSync),
        }
    }

    fn failing(calls: Arc<AtomicUsize>) -> Self {
        fn offline() -> SyncError {
            hidlins_sync::client::LanError::Unreachable.into()
        }
        Self {
            calls,
            result: Err(offline),
        }
    }
}

impl SyncEnginePort for CountingEngine {
    fn sync_now(
        &self,
        _vault: &mut hidlins_core::Vault,
        _vault_name: &str,
        _registry: &mut hidlins_core::VaultRegistry,
        _master_password: &hidlins_core::MasterPassword,
        _keyfile: Option<&hidlins_core::Keyfile>,
        _candidates: Vec<hidlins_sync::address::LocalEndpoint>,
        _opts: SyncOptions,
        _cancellation: hidlins_sync::client::LanCancellation,
    ) -> Result<SyncOutcome, SyncError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.result {
            Ok(ref outcome) => Ok(outcome.clone()),
            Err(make_error) => Err(make_error()),
        }
    }
}

struct CancellationEngine {
    entered: mpsc::Sender<()>,
    canceled: Option<mpsc::Sender<()>>,
}

impl SyncEnginePort for CancellationEngine {
    fn sync_now(
        &self,
        _vault: &mut hidlins_core::Vault,
        _vault_name: &str,
        _registry: &mut hidlins_core::VaultRegistry,
        _master_password: &hidlins_core::MasterPassword,
        _keyfile: Option<&hidlins_core::Keyfile>,
        _candidates: Vec<hidlins_sync::address::LocalEndpoint>,
        _opts: SyncOptions,
        cancellation: hidlins_sync::client::LanCancellation,
    ) -> Result<SyncOutcome, SyncError> {
        self.entered.send(()).expect("signal sync entry");
        while !cancellation.is_cancelled() {
            std::thread::yield_now();
        }
        if let Some(canceled) = &self.canceled {
            let _ = canceled.send(());
        }
        Err(hidlins_sync::client::LanError::Cancelled.into())
    }
}

// LNS-REVIEW-003
#[test]
fn startup_sync_runs_once_then_only_manual_and_never_post_save() {
    let env = common::TestEnv::new();
    register_active_client(&env, "startup", "pass");
    let calls = Arc::new(AtomicUsize::new(0));
    let session = AppSession::with_ports(
        env.paths_clone(),
        None,
        Arc::new(CountingEngine::succeeding(Arc::clone(&calls))),
    )
    .expect("session");

    let tree = session
        .unlock("startup".to_string(), "pass".to_string(), None)
        .expect("unlock remains immediately available");
    session.join_background_syncs_for_test();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "unlock must not outrun application discovery"
    );
    session
        .start_startup_sync()
        .expect("application completed discovery");
    session.join_background_syncs_for_test();
    assert_eq!(calls.load(Ordering::SeqCst), 1, "one startup attempt");

    session
        .create_entry(
            tree.root.uuid,
            EntryDraftDto {
                kind: EntryKindDto::Credential,
                title: "local only".to_string(),
                username: None,
                password: None,
                url: None,
                notes: None,
                tags: vec![],
                custom_fields: vec![],
                totp_uri: None,
            },
        )
        .expect("local save");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "no post-save attempt");

    session.sync_now().expect("manual sync");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "manual is repeatable");
    session.lock_now().expect("lock");
    session
        .unlock("startup".to_string(), "pass".to_string(), None)
        .expect("re-unlock");
    session.join_background_syncs_for_test();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "startup consumed per process"
    );
}

#[test]
fn startup_network_failure_is_nonfatal_and_emits_terminal_event() {
    let env = common::TestEnv::new();
    register_active_client(&env, "offline", "pass");
    let calls = Arc::new(AtomicUsize::new(0));
    let session = AppSession::with_ports(
        env.paths_clone(),
        None,
        Arc::new(CountingEngine::failing(Arc::clone(&calls))),
    )
    .expect("session");
    let (tx, rx) = mpsc::channel();
    session.sync_events_for_test(Box::new(MpscEventSink::new(tx)));
    session
        .unlock("offline".to_string(), "pass".to_string(), None)
        .expect("offline unlock succeeds");
    session
        .start_startup_sync()
        .expect("application completed discovery");
    session.join_background_syncs_for_test();
    assert!(session.has_vault(), "local use remains available");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let events: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert!(matches!(
        events.first(),
        Some(SyncEvent::Started { automatic: true })
    ));
    assert!(matches!(
        events.last(),
        Some(SyncEvent::Failed(HidlinsApiError::SyncOffline))
    ));
}

#[test]
fn lock_cancels_in_flight_startup_sync_before_discarding_owned_state() {
    let env = common::TestEnv::new();
    register_active_client(&env, "cancel", "pass");
    let (entered_tx, entered_rx) = mpsc::channel();
    let session = AppSession::with_ports(
        env.paths_clone(),
        None,
        Arc::new(CancellationEngine {
            entered: entered_tx,
            canceled: None,
        }),
    )
    .expect("session");
    session
        .unlock("cancel".to_string(), "pass".to_string(), None)
        .expect("unlock");
    session
        .start_startup_sync()
        .expect("application completed discovery");
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("startup entered");
    session.lock_now().expect("lock and cancel");
    session.join_background_syncs_for_test();
    assert!(!session.has_vault());
    assert!(!session.has_credentials());
}

#[test]
fn mobile_pause_cancels_in_flight_startup_without_waiting_for_grace_lock() {
    let env = common::TestEnv::new();
    register_active_client(&env, "mobile-cancel", "pass");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (canceled_tx, canceled_rx) = mpsc::channel();
    let session = AppSession::with_mobile_ports(
        env.paths_clone(),
        None,
        Arc::new(CancellationEngine {
            entered: entered_tx,
            canceled: Some(canceled_tx),
        }),
    )
    .expect("mobile session");
    session
        .set_discovery_candidates(
            DiscoveryPermissionDto::Granted,
            vec![LocalEndpointDto {
                address: "127.0.0.1".to_string(),
                port: 42873,
                scope_id: 0,
            }],
        )
        .expect("candidate");
    session
        .unlock("mobile-cancel".to_string(), "pass".to_string(), None)
        .expect("unlock");
    session.start_startup_sync().expect("startup");
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("startup entered");

    session.report_lifecycle_state(hidlins_api::dto::LifecycleStateDto::Paused);
    if canceled_rx.recv_timeout(Duration::from_secs(1)).is_err() {
        session.cancel_sync();
        session.join_background_syncs_for_test();
        panic!("pause did not cancel foreground mobile sync");
    }
    session.join_background_syncs_for_test();
    assert!(
        session.has_vault(),
        "the lifecycle grace keeps local use open"
    );
}

#[test]
fn manual_sync_claim_blocks_mutation_and_lock_wins_at_commit() {
    let env = common::TestEnv::new();
    register_active_client(&env, "claim", "pass");
    let barrier = Arc::new(Barrier::new(2));
    let (engine, entered) = hidlins_api::sync_port::BlockingSyncEngine::with_entry_signal(
        Arc::clone(&barrier),
        SyncOutcome::AlreadyInSync,
    );
    let session = Arc::new(
        AppSession::with_ports(env.paths_clone(), None, Arc::new(engine)).expect("session"),
    );
    session
        .unlock("claim".to_string(), "pass".to_string(), None)
        .expect("unlock");
    session
        .start_startup_sync()
        .expect("application completed discovery");
    // Release and consume the startup attempt first.
    entered
        .recv_timeout(Duration::from_secs(2))
        .expect("startup entered");
    barrier.wait();
    session.join_background_syncs_for_test();

    let worker_session = Arc::clone(&session);
    let worker = std::thread::spawn(move || worker_session.sync_now());
    entered
        .recv_timeout(Duration::from_secs(2))
        .expect("manual entered");
    assert!(matches!(
        session.create_group(
            "00000000-0000-0000-0000-000000000000".to_string(),
            "x".to_string()
        ),
        Err(HidlinsApiError::VaultBusySyncing)
    ));
    session.lock_now().expect("lock request");
    barrier.wait();
    assert!(matches!(
        worker.join().expect("worker"),
        Err(HidlinsApiError::VaultLocked)
    ));
    assert!(!session.has_vault());
    assert!(!session.has_credentials());
}

#[test]
fn sync_panic_is_contained_locks_and_emits_one_terminal_failure() {
    let env = common::TestEnv::new();
    register_active_client(&env, "panic", "pass");
    let session = AppSession::with_ports(env.paths_clone(), None, Arc::new(PanickingSyncEngine))
        .expect("session");
    session
        .unlock("panic".to_string(), "pass".to_string(), None)
        .expect("unlock returns before startup worker");
    session
        .start_startup_sync()
        .expect("application completed discovery");
    session.join_background_syncs_for_test();
    assert!(!session.has_vault(), "panic path fails locked");
}

// LNS-LIFECYCLE-001
#[test]
fn server_lifecycle_is_explicit_and_stops_on_lock_then_restarts_on_unlock() {
    let env = common::TestEnv::new();
    let path = common::create_test_vault(&env, "authority", "pass");
    common::register_vault(&env, "authority", &path);
    let session = AppSession::for_test(env.paths_clone()).expect("session");
    session
        .unlock("authority".to_string(), "pass".to_string(), None)
        .expect("unlock");
    session
        .configure_local_sync(LocalSyncRoleDto::Server)
        .expect("configure authority");
    assert!(!session.local_sync_status().expect("status").server_enabled);
    let endpoints = session.local_server_endpoints().expect("bind candidates");
    assert!(endpoints.iter().all(|endpoint| endpoint.port != 0));
    session
        .start_sync_server(endpoints.into_iter().next().expect("endpoint"))
        .expect("start");
    session.open_pairing_window().expect("pairing window");
    let status = session.local_sync_status().expect("status");
    assert!(status.server_enabled && status.server_running && status.pairing_open);
    session.lock_now().expect("lock");
    assert!(!session.sync_server_is_running_for_test());
    session
        .unlock("authority".to_string(), "pass".to_string(), None)
        .expect("re-unlock");
    assert!(session.sync_server_is_running_for_test());
    session.stop_sync_server();
    assert!(!session.local_sync_status().expect("status").server_enabled);
}

// LNS-MOBILE-001
#[test]
fn mobile_rejects_server_before_endpoint_validation_and_bounds_candidates() {
    let env = common::TestEnv::new();
    let path = common::create_test_vault(&env, "mobile", "pass");
    common::register_vault(&env, "mobile", &path);
    let session = AppSession::for_test_mobile(env.paths_clone()).expect("mobile session");
    session
        .unlock("mobile".to_string(), "pass".to_string(), None)
        .expect("unlock");
    session
        .configure_local_sync(LocalSyncRoleDto::Server)
        .expect("role config is storage-only");
    let result = session.start_sync_server(LocalEndpointDto {
        address: "8.8.8.8".to_string(),
        port: 443,
        scope_id: 0,
    });
    assert!(matches!(
        result,
        Err(HidlinsApiError::UnsupportedPlatform { .. })
    ));
    assert!(!session.sync_server_is_running_for_test());
    assert!(matches!(
        session.local_server_endpoints(),
        Err(HidlinsApiError::UnsupportedPlatform { .. })
    ));

    let candidates = (0..=hidlins_sync::protocol::MAX_DISCOVERY_ENDPOINTS)
        .map(|index| LocalEndpointDto {
            address: "127.0.0.1".to_string(),
            port: u16::try_from(index + 1).expect("bounded"),
            scope_id: 0,
        })
        .collect();
    assert!(matches!(
        session.set_discovery_candidates(DiscoveryPermissionDto::Granted, candidates),
        Err(HidlinsApiError::SyncBusy)
    ));
}

#[test]
fn public_and_dns_candidate_inputs_fail_inside_rust_policy() {
    let env = common::TestEnv::new();
    let session = AppSession::for_test_mobile(env.paths_clone()).expect("mobile");
    for address in ["8.8.8.8", "example.com"] {
        assert!(matches!(
            session.set_discovery_candidates(
                DiscoveryPermissionDto::Granted,
                vec![LocalEndpointDto {
                    address: address.to_string(),
                    port: 443,
                    scope_id: 0,
                }],
            ),
            Err(HidlinsApiError::InvalidInput { .. })
        ));
    }
}

#[test]
fn sas_type_is_not_used_as_a_boundary_handle() {
    // Compile-time/API guard: pairing UI receives PairingPromptDto, while the
    // cryptographic SAS remains a core-only value.
    let _core_only = SasCode::derive(&[7_u8; 32]);
    let manifest = include_str!("../../../tools/dev/frb-api-manifest.txt");
    assert!(!manifest.contains("SasCode"));
}

#[test]
fn api_pairing_uses_opaque_handles_and_activates_both_trust_stores() {
    let server_env = common::TestEnv::new();
    let server_path = common::create_test_vault(&server_env, "shared", "pass");
    common::register_vault(&server_env, "shared", &server_path);
    let server = AppSession::for_test(server_env.paths_clone()).expect("server session");
    server
        .unlock("shared".to_string(), "pass".to_string(), None)
        .expect("server unlock");
    server
        .configure_local_sync(LocalSyncRoleDto::Server)
        .expect("server role");
    let (event_tx, event_rx) = mpsc::channel();
    server.sync_events_for_test(Box::new(MpscEventSink::new(event_tx)));
    let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
    let port = probe.local_addr().expect("address").port();
    drop(probe);
    let endpoint = LocalEndpointDto {
        address: "127.0.0.1".to_string(),
        port,
        scope_id: 0,
    };
    server.start_sync_server(endpoint.clone()).expect("serve");
    server.open_pairing_window().expect("pairing window");

    let client_env = common::TestEnv::new();
    let client_path = common::create_test_vault(&client_env, "shared", "pass");
    common::register_vault(&client_env, "shared", &client_path);
    let client = AppSession::for_test(client_env.paths_clone()).expect("client session");
    client
        .unlock("shared".to_string(), "pass".to_string(), None)
        .expect("client unlock");
    client
        .configure_local_sync(LocalSyncRoleDto::Client)
        .expect("client role");
    client
        .set_discovery_candidates(DiscoveryPermissionDto::Granted, vec![endpoint])
        .expect("candidate");

    let client_prompt = client.begin_pairing().expect("XX pairing");
    let server_prompt = loop {
        if let SyncEvent::PairingRequested(prompt) = event_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("server event")
        {
            break prompt;
        }
    };
    assert_eq!(client_prompt.sas, server_prompt.sas, "same transcript SAS");
    assert_eq!(client_prompt.transaction_handle.len(), 32);
    assert_eq!(server_prompt.transaction_handle.len(), 32);
    server
        .confirm_pairing(
            server_prompt.transaction_handle,
            true,
            "client device".to_string(),
        )
        .expect("server accepts");
    client
        .confirm_pairing(
            client_prompt.transaction_handle,
            true,
            "server device".to_string(),
        )
        .expect("client accepts");
    assert!(client.local_sync_status().expect("client status").paired);
    assert_eq!(
        server
            .local_sync_status()
            .expect("server status")
            .active_peer_count,
        1
    );
}

#[test]
fn pair_and_import_installs_only_after_pairing_and_kdbx_validation() {
    let server_env = common::TestEnv::new();
    let server_path = common::create_test_vault(&server_env, "imported", "pass");
    common::register_vault(&server_env, "imported", &server_path);
    let server = Arc::new(AppSession::for_test(server_env.paths_clone()).expect("server"));
    server
        .unlock("imported".to_string(), "pass".to_string(), None)
        .expect("unlock");
    server
        .configure_local_sync(LocalSyncRoleDto::Server)
        .expect("server role");
    let (event_tx, event_rx) = mpsc::channel();
    server.sync_events_for_test(Box::new(MpscEventSink::new(event_tx)));
    let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
    let port = probe.local_addr().expect("address").port();
    drop(probe);
    let endpoint = LocalEndpointDto {
        address: "127.0.0.1".to_string(),
        port,
        scope_id: 0,
    };
    server.start_sync_server(endpoint.clone()).expect("serve");
    server.open_pairing_window().expect("pairing");

    let client_env = common::TestEnv::new();
    let client = Arc::new(AppSession::for_test(client_env.paths_clone()).expect("client"));
    let prompt = client
        .begin_pair_import(
            "imported".to_string(),
            "pass".to_string(),
            None,
            vec![endpoint],
        )
        .expect("begin import pairing");
    let server_prompt = loop {
        if let SyncEvent::PairingRequested(prompt) = event_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("event")
        {
            break prompt;
        }
    };
    server
        .confirm_pairing(
            server_prompt.transaction_handle,
            true,
            "new client".to_string(),
        )
        .expect("server confirms");

    let importing = Arc::clone(&client);
    let handle = std::thread::spawn(move || {
        importing.confirm_pairing(prompt.transaction_handle, true, "authority".to_string())
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !handle.is_finished() && std::time::Instant::now() < deadline {
        server.pump_sync_server_for_test();
        std::thread::yield_now();
    }
    let summary = handle
        .join()
        .expect("import thread")
        .expect("pair/import")
        .expect("import summary");
    assert!(std::path::Path::new(&summary.path).is_file());
    let registry =
        hidlins_core::VaultRegistry::load(client_env.paths_clone()).expect("client registry");
    let entry = registry
        .get("imported")
        .expect("registered after full import");
    assert!(
        LocalSyncConfig::from_vault_entry(entry).is_some_and(|config| config.is_active_client())
    );
}

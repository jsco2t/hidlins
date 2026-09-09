//! LNS identity, pairing, durable trust, and crash-boundary conformance tests.

use std::time::{Duration, Instant};

use hidlins_core::{HidlinsPaths, MasterPassword, RegisteredVault, VaultRegistry};
use hidlins_sync::{
    config::local::LocalSyncConfig,
    identity::{PublicIdentity, SyncRole},
    noise::{HandshakeError, HandshakeSession},
    pairing::{
        ConfirmedPairing, PairingError, PairingTransaction, PairingWindow, SasCode, TransactionId,
        TranscriptDigest,
    },
    trust::{PeerStatus, TrustError},
};
use tempfile::TempDir;

fn password(value: &str) -> MasterPassword {
    MasterPassword::new(value.to_string())
}

fn entry_with_config(config: &LocalSyncConfig) -> RegisteredVault {
    let mut extra = toml::Table::new();
    extra.insert(
        "sync".to_string(),
        toml::Value::try_from(config).expect("serialize local sync config"),
    );
    RegisteredVault {
        name: "personal".to_string(),
        path: "/tmp/personal.kdbx".into(),
        created_at: "2026-09-06T00:00:00Z".to_string(),
        keyfile_path: None,
        extra,
    }
}

fn registry_with_config(paths: &HidlinsPaths, config: &LocalSyncConfig) -> VaultRegistry {
    let mut registry = VaultRegistry::with_paths(paths.clone());
    registry
        .register_and_save(entry_with_config(config))
        .expect("persist initial local config");
    registry
}

fn reload_config(paths: &HidlinsPaths) -> LocalSyncConfig {
    let registry = VaultRegistry::load(paths.clone()).expect("reload registry");
    LocalSyncConfig::from_vault_entry(registry.get("personal").expect("registered vault"))
        .expect("local sync config")
}

fn apply_concurrent_peer_renames(
    paths: &HidlinsPaths,
    first_key: PublicIdentity,
    second_key: PublicIdentity,
) {
    let mut first_writer = VaultRegistry::load(paths.clone()).expect("first stale writer");
    let mut second_writer = VaultRegistry::load(paths.clone()).expect("second stale writer");
    LocalSyncConfig::transactional_update(&mut first_writer, "personal", |config| {
        config.rename_peer(first_key, "Retired laptop".to_string())
    })
    .expect("first peer update");
    LocalSyncConfig::transactional_update(&mut second_writer, "personal", |config| {
        config.rename_peer(second_key, "Current tablet".to_string())
    })
    .expect("second peer update rebases on first");
}

#[cfg(unix)]
fn assert_registry_mode_is_private(paths: &HidlinsPaths) {
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(paths.vaults_toml())
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

fn confirmed_pairing(
    handshake_hash: &[u8; 32],
    server: &LocalSyncConfig,
    client: &LocalSyncConfig,
) -> (PairingTransaction, ConfirmedPairing, ConfirmedPairing) {
    let now = Instant::now();
    let mut server_window = PairingWindow::open_at(now);
    let mut client_window = PairingWindow::open_at(now);
    let server_sas = server_window
        .begin_candidate(client.identity().public_key(), *handshake_hash, now)
        .expect("server candidate");
    let client_sas = client_window
        .begin_candidate(server.identity().public_key(), *handshake_hash, now)
        .expect("client candidate");
    assert_eq!(server_sas, client_sas);
    let server_confirmation = server_window
        .confirm(server_sas, now)
        .expect("server confirmation");
    let client_confirmation = client_window
        .confirm(client_sas, now)
        .expect("client confirmation");
    let transaction = client_confirmation
        .transaction(
            server.identity().public_key(),
            client.identity().public_key(),
        )
        .expect("transaction");
    (transaction, server_confirmation, client_confirmation)
}

fn prepare_both(
    server: &mut LocalSyncConfig,
    client: &mut LocalSyncConfig,
    transaction: &PairingTransaction,
    server_confirmation: &ConfirmedPairing,
    client_confirmation: &ConfirmedPairing,
) {
    server
        .prepare_client(
            transaction,
            server_confirmation,
            "Laptop".to_string(),
            1_000,
            1_180,
        )
        .expect("server prepare");
    client
        .prepare_server(
            transaction,
            client_confirmation,
            "Home server".to_string(),
            1_000,
            1_180,
        )
        .expect("client prepare");
}

fn fully_pair(server: &mut LocalSyncConfig, client: &mut LocalSyncConfig, hash: &[u8; 32]) {
    let (transaction, server_confirmation, client_confirmation) =
        confirmed_pairing(hash, server, client);
    prepare_both(
        server,
        client,
        &transaction,
        &server_confirmation,
        &client_confirmation,
    );
    server
        .activate_client(&transaction, 1_001)
        .expect("activate client");
    client
        .activate_server(&transaction, 1_001)
        .expect("activate server pin");
}

fn mutually_authorized(server: &LocalSyncConfig, client: &LocalSyncConfig) -> bool {
    server.authorizes(client.identity().public_key())
        && client.authorizes(server.identity().public_key())
}

fn complete_ik(
    initiator: &mut HandshakeSession,
    responder: &mut HandshakeSession,
) -> Result<(), HandshakeError> {
    let mut wire = vec![0_u8; 65_535];
    let mut plain = vec![0_u8; 65_535];
    let written = initiator.write_handshake(b"", &mut wire)?;
    responder.read_handshake(&wire[..written], &mut plain)?;
    let written = responder.write_handshake(b"", &mut wire)?;
    initiator.read_handshake(&wire[..written], &mut plain)?;
    Ok(())
}

// LNS-IDENTITY-001
#[test]
fn identity_is_distinct_sealed_context_bound_and_rewrapped_without_rotation() {
    let old = password("old master password");
    let new = password("new master password");
    let wrong = password("wrong master password");
    let mut first =
        LocalSyncConfig::create("personal", SyncRole::Server, &old).expect("first identity");
    let second =
        LocalSyncConfig::create("personal", SyncRole::Server, &old).expect("second identity");

    assert_ne!(
        first.identity().public_key(),
        second.identity().public_key()
    );
    assert_ne!(
        first.identity().installation_id(),
        second.identity().installation_id()
    );
    assert!(first
        .identity()
        .unlock("personal", SyncRole::Server, &old)
        .is_ok());
    assert!(first
        .identity()
        .unlock("personal", SyncRole::Server, &wrong)
        .is_err());
    assert!(first
        .identity()
        .unlock("other-vault", SyncRole::Server, &old)
        .is_err());
    assert!(first
        .identity()
        .unlock("personal", SyncRole::Client, &old)
        .is_err());

    let public_before = first.identity().public_key();
    let sealed_before = first.identity().sealed_value().to_string();

    let mut failed_rewrap = first.clone();
    assert!(failed_rewrap
        .rewrap_identity("personal", &wrong, &new)
        .is_err());
    assert_eq!(failed_rewrap, first);

    first
        .rewrap_identity("personal", &old, &new)
        .expect("rewrap identity");
    assert_eq!(first.identity().public_key(), public_before);
    assert_ne!(first.identity().sealed_value(), sealed_before);
    assert!(first
        .identity()
        .unlock("personal", SyncRole::Server, &old)
        .is_err());
    assert!(first
        .identity()
        .unlock("personal", SyncRole::Server, &new)
        .is_ok());

    let encoded = toml::to_string(&first).expect("serialize");
    assert!(!encoded.contains("old master password"));
    assert!(!encoded.contains("new master password"));
    assert!(!format!("{first:?}").contains(first.identity().sealed_value()));
}

// LNS-IDENTITY-002
#[test]
fn identity_tampering_and_non_local_schema_fail_closed_without_mutation() {
    let master = password("master");
    let config = LocalSyncConfig::create("personal", SyncRole::Server, &master).expect("config");
    let mut entry = entry_with_config(&config);
    let serialized_before = toml::to_string(&entry.extra).expect("serialize original");

    let sync = entry
        .extra
        .get_mut("sync")
        .and_then(toml::Value::as_table_mut)
        .expect("sync table");
    let sealed = sync
        .get_mut("identity")
        .and_then(toml::Value::as_table_mut)
        .and_then(|identity| identity.get_mut("sealed_private_key"))
        .and_then(|value| value.as_str())
        .expect("sealed identity")
        .to_string();
    let mut tampered = sealed.into_bytes();
    let last = tampered.last_mut().expect("nonempty sealed value");
    *last = if *last == b'A' { b'B' } else { b'A' };
    sync.get_mut("identity")
        .and_then(toml::Value::as_table_mut)
        .expect("identity table")
        .insert(
            "sealed_private_key".to_string(),
            toml::Value::String(String::from_utf8(tampered).expect("base64 ascii")),
        );
    let parsed = LocalSyncConfig::from_vault_entry(&entry).expect("structurally valid");
    assert!(parsed
        .identity()
        .unlock("personal", SyncRole::Server, &master)
        .is_err());

    let mut unknown = entry_with_config(&config);
    unknown
        .extra
        .get_mut("sync")
        .and_then(toml::Value::as_table_mut)
        .expect("sync table")
        .insert(
            "kind".to_string(),
            toml::Value::String("unknown_transport".to_string()),
        );
    assert!(LocalSyncConfig::from_vault_entry(&unknown).is_none());
    assert_ne!(
        toml::to_string(&unknown.extra).expect("serialize unknown"),
        serialized_before
    );
}

// LNS-IDENTITY-003
#[test]
fn identity_registered_rewrap_is_atomic_and_preserves_the_public_key() {
    let temp = TempDir::new().expect("tempdir");
    let paths = HidlinsPaths::with_state_dir(temp.path().join("state"));
    let old = password("old password");
    let new = password("new password");
    let wrong = password("wrong password");
    let original = LocalSyncConfig::create("personal", SyncRole::Server, &old).expect("identity");
    let public_key = original.identity().public_key();
    let mut registry = registry_with_config(&paths, &original);

    let bytes_before = std::fs::read(paths.vaults_toml()).expect("registry before failure");
    assert!(
        LocalSyncConfig::rewrap_registered_identity(&mut registry, "personal", &wrong, &new,)
            .is_err()
    );
    assert_eq!(
        std::fs::read(paths.vaults_toml()).expect("registry after failure"),
        bytes_before
    );

    LocalSyncConfig::rewrap_registered_identity(&mut registry, "personal", &old, &new)
        .expect("atomic registered rewrap");
    let rewrapped = reload_config(&paths);
    assert_eq!(rewrapped.identity().public_key(), public_key);
    assert!(rewrapped
        .identity()
        .unlock("personal", SyncRole::Server, &old)
        .is_err());
    assert!(rewrapped
        .identity()
        .unlock("personal", SyncRole::Server, &new)
        .is_ok());
}

// LNS-PAIRING-001
#[test]
fn sas_vectors_and_pairing_window_are_bounded_and_bilateral() {
    assert_eq!(SasCode::derive(&[0_u8; 32]).to_string(), "000-000");
    assert_eq!(SasCode::derive(&[0xff_u8; 32]).to_string(), "ZZZ-ZZZ");
    assert_ne!(SasCode::derive(&[0_u8; 32]), SasCode::derive(&[1_u8; 32]));

    let now = Instant::now();
    let peer = PublicIdentity::new([3_u8; 32]);
    let hash = [9_u8; 32];
    let mut empty = PairingWindow::open_at(now);
    assert!(matches!(
        empty.confirm(SasCode::derive(&hash), now),
        Err(PairingError::NoCandidate)
    ));
    assert_eq!(empty.failures(), 0);

    let mut left = PairingWindow::open_at(now);
    let mut right = PairingWindow::open_at(now);
    let left_sas = left
        .begin_candidate(peer, hash, now)
        .expect("left candidate");
    let right_sas = right
        .begin_candidate(peer, hash, now)
        .expect("right candidate");
    assert_eq!(left_sas, right_sas);
    assert!(left.begin_candidate(peer, hash, now).is_err());
    assert_eq!(
        left.confirm(left_sas, now)
            .expect("left confirm")
            .peer_key(),
        peer
    );
    assert_eq!(
        right
            .confirm(right_sas, now)
            .expect("right confirm")
            .peer_key(),
        peer
    );

    let mut failed = PairingWindow::open_at(now);
    for expected_failures in 1_u8..=3 {
        let correct = failed
            .begin_candidate(peer, hash, now)
            .expect("candidate before failure");
        let wrong = SasCode::derive(&[expected_failures; 32]);
        assert_ne!(correct, wrong);
        assert!(failed.confirm(wrong, now).is_err());
        assert_eq!(failed.failures(), usize::from(expected_failures));
    }
    assert!(!failed.is_open(now));
    assert!(failed.begin_candidate(peer, hash, now).is_err());

    let mut handshake_failures = PairingWindow::open_at(now);
    for _ in 0..3 {
        handshake_failures
            .record_failed_attempt(now)
            .expect("count failed XX handshake");
    }
    assert!(!handshake_failures.is_open(now));

    let mut rejected = PairingWindow::open_at(now);
    rejected
        .begin_candidate(peer, hash, now)
        .expect("candidate to reject");
    rejected.reject(now).expect("explicit rejection");
    assert_eq!(rejected.failures(), 1);

    let mut expired = PairingWindow::open_at(now);
    assert!(expired
        .begin_candidate(peer, hash, now + Duration::from_secs(180))
        .is_err());
}

// LNS-PAIRING-002
#[test]
fn pairing_transaction_binds_transcript_roles_keys_and_identifier() {
    let hash = [7_u8; 32];
    let server = PublicIdentity::new([1_u8; 32]);
    let client = PublicIdentity::new([2_u8; 32]);
    let now = Instant::now();
    let mut window = PairingWindow::open_at(now);
    let sas = window
        .begin_candidate(server, hash, now)
        .expect("pairing candidate");
    let confirmation = window.confirm(sas, now).expect("local SAS confirmation");
    let valid = confirmation
        .transaction(server, client)
        .expect("transaction");
    assert!(valid.validates(&hash));
    assert!(!valid.validates(&[8_u8; 32]));

    for malformed in [
        PairingTransaction::from_authenticated(
            TransactionId::new([99_u8; 16]),
            valid.transcript_digest(),
            server,
            client,
        ),
        PairingTransaction::from_authenticated(
            valid.transaction_id(),
            TranscriptDigest::new([99_u8; 32]),
            server,
            client,
        ),
        PairingTransaction::from_authenticated(
            valid.transaction_id(),
            valid.transcript_digest(),
            client,
            server,
        ),
    ] {
        assert!(!malformed.validates(&hash));
    }
}

// LNS-TRUST-001
#[test]
fn trust_every_commit_ack_boundary_is_non_authorizing_until_both_sides_commit() {
    let temp = TempDir::new().expect("tempdir");
    let master = password("master");
    let hash = [5_u8; 32];
    let base_server =
        LocalSyncConfig::create("personal", SyncRole::Server, &master).expect("server");
    let base_client =
        LocalSyncConfig::create("personal", SyncRole::Client, &master).expect("client");
    let (tx, server_confirmation, client_confirmation) =
        confirmed_pairing(&hash, &base_server, &base_client);
    let server_paths = HidlinsPaths::with_state_dir(temp.path().join("server"));
    let client_paths = HidlinsPaths::with_state_dir(temp.path().join("client"));
    let mut server_registry = registry_with_config(&server_paths, &base_server);
    let mut client_registry = registry_with_config(&client_paths, &base_client);

    // Boundary 0: authenticated confirmation but no durable prepare.
    assert!(!mutually_authorized(&base_server, &base_client));

    // Boundary 1: server prepare persisted, then disconnect.
    LocalSyncConfig::transactional_update(&mut server_registry, "personal", |server| {
        server.prepare_client(
            &tx,
            &server_confirmation,
            "Laptop".to_string(),
            1_000,
            1_180,
        )
    })
    .expect("server prepare");
    let server = reload_config(&server_paths);
    let client = reload_config(&client_paths);
    assert!(server.provisional().is_some());
    assert!(!mutually_authorized(&server, &client));

    // Boundary 2: both prepare records persisted, then disconnect.
    LocalSyncConfig::transactional_update(&mut client_registry, "personal", |client| {
        client.prepare_server(
            &tx,
            &client_confirmation,
            "Server".to_string(),
            1_000,
            1_180,
        )
    })
    .expect("client prepare");
    let server = reload_config(&server_paths);
    let client = reload_config(&client_paths);
    assert!(!mutually_authorized(&server, &client));

    let malformed = PairingTransaction::from_authenticated(
        TransactionId::new([0_u8; 16]),
        tx.transcript_digest(),
        tx.server_key(),
        tx.client_key(),
    );
    let server_bytes_before =
        std::fs::read(server_paths.vaults_toml()).expect("server registry before mismatch");
    assert!(
        LocalSyncConfig::transactional_update(&mut server_registry, "personal", |server| server
            .activate_client(&malformed, 1_001),)
        .is_err()
    );
    assert_eq!(
        std::fs::read(server_paths.vaults_toml()).expect("server registry after mismatch"),
        server_bytes_before
    );

    // Boundary 3: server commit persisted but acknowledgement is lost.
    LocalSyncConfig::transactional_update(&mut server_registry, "personal", |server| {
        server.activate_client(&tx, 1_001)
    })
    .expect("server commit");
    let server = reload_config(&server_paths);
    let client = reload_config(&client_paths);
    assert!(server.authorizes(client.identity().public_key()));
    assert!(!client.authorizes(server.identity().public_key()));
    assert!(!mutually_authorized(&server, &client));
    assert_eq!(
        server
            .recover_pairing_transaction(client.identity().public_key(), 1_001)
            .expect("server recovery receipt"),
        tx
    );
    assert_eq!(
        client
            .recover_pairing_transaction(server.identity().public_key(), 1_001)
            .expect("client provisional transaction"),
        tx
    );
    assert!(server
        .recover_pairing_transaction(PublicIdentity::new([99_u8; 32]), 1_001)
        .is_err());
    LocalSyncConfig::transactional_update(&mut server_registry, "personal", |server| {
        server.activate_client(&tx, 1_001)
    })
    .expect("idempotent server commit replay");

    // Boundary 4: authenticated acknowledgement promotes the exact client pin.
    LocalSyncConfig::transactional_update(&mut client_registry, "personal", |client| {
        client.activate_server(&tx, 1_001)
    })
    .expect("client commit");
    let mut server = reload_config(&server_paths);
    let client = reload_config(&client_paths);
    assert!(mutually_authorized(&server, &client));
    assert!(server.expire_provisional(1_180));
    assert!(mutually_authorized(&server, &client));
}

// LNS-TRUST-002
#[test]
fn trust_mismatches_expiry_revocation_and_redesignation_fail_closed() {
    let master = password("master");
    let hash = [6_u8; 32];
    let mut server =
        LocalSyncConfig::create("personal", SyncRole::Server, &master).expect("server");
    let mut client =
        LocalSyncConfig::create("personal", SyncRole::Client, &master).expect("client");
    let (tx, server_confirmation, client_confirmation) = confirmed_pairing(&hash, &server, &client);
    prepare_both(
        &mut server,
        &mut client,
        &tx,
        &server_confirmation,
        &client_confirmation,
    );

    let malformed = PairingTransaction::from_authenticated(
        tx.transaction_id(),
        TranscriptDigest::new([0_u8; 32]),
        tx.server_key(),
        tx.client_key(),
    );
    assert_eq!(
        server.activate_client(&malformed, 1_001),
        Err(TrustError::TransactionMismatch)
    );
    assert!(server.provisional().is_some());
    let mut expired_server = server.clone();
    assert_eq!(
        expired_server.activate_client(&tx, 1_180),
        Err(TrustError::Expired)
    );
    assert!(expired_server.expire_provisional(1_180));
    assert!(expired_server.provisional().is_none());
    assert!(!expired_server.authorizes(client.identity().public_key()));

    server.activate_client(&tx, 1_001).expect("server activate");
    client.activate_server(&tx, 1_001).expect("client activate");
    let client_key = client.identity().public_key();
    let server_key = server.identity().public_key();
    assert!(mutually_authorized(&server, &client));

    server.revoke_peer(client_key).expect("server revoke");
    assert!(!server.authorizes(client_key));
    assert_eq!(server.trusted_peers()[0].status(), PeerStatus::Revoked);
    assert!(
        server.provisional().is_none(),
        "revocation removes the matching recovery receipt so the config remains valid"
    );
    assert!(server
        .prepare_client(
            &tx,
            &server_confirmation,
            "silent re-pair".to_string(),
            1_010,
            1_100,
        )
        .is_err());

    assert_eq!(
        client.prepare_server(
            &tx,
            &client_confirmation,
            "silent re-pair".to_string(),
            1_010,
            1_100,
        ),
        Err(TrustError::RedesignationRequired)
    );
    client.begin_server_redesignation().expect("explicit reset");
    assert!(!client.authorizes(server_key));
    assert!(client.pinned_server().is_none());
}

// LNS-TRUST-003
#[test]
fn trust_completed_ik_requires_exact_active_pins_on_both_endpoints() {
    let master = password("master");
    let hash = [17_u8; 32];
    let mut server =
        LocalSyncConfig::create("personal", SyncRole::Server, &master).expect("server");
    let mut client =
        LocalSyncConfig::create("personal", SyncRole::Client, &master).expect("client");
    let (tx, server_confirmation, client_confirmation) = confirmed_pairing(&hash, &server, &client);
    prepare_both(
        &mut server,
        &mut client,
        &tx,
        &server_confirmation,
        &client_confirmation,
    );
    server.activate_client(&tx, 1_001).expect("server commit");
    client.activate_server(&tx, 1_001).expect("client commit");

    let server_keys = server
        .identity()
        .unlock("personal", SyncRole::Server, &master)
        .expect("server identity");
    let client_keys = client
        .identity()
        .unlock("personal", SyncRole::Client, &master)
        .expect("client identity");
    let mut initiator = HandshakeSession::trusted_initiator(&client_keys, server_keys.public_key())
        .expect("IK initiator");
    let mut responder = HandshakeSession::trusted_responder(&server_keys, client_keys.public_key())
        .expect("IK responder");
    complete_ik(&mut initiator, &mut responder).expect("complete IK");
    let client_channel = client
        .authorize_transport(initiator.finish().expect("client transport"))
        .expect("client authorizes exact server pin");
    let server_channel = server
        .authorize_transport(responder.finish().expect("server transport"))
        .expect("server authorizes exact client pin");
    assert_eq!(client_channel.peer(), server.identity().public_key());
    assert_eq!(server_channel.peer(), client.identity().public_key());

    server
        .revoke_peer(client.identity().public_key())
        .expect("revoke client");
    let mut initiator = HandshakeSession::trusted_initiator(&client_keys, server_keys.public_key())
        .expect("second IK initiator");
    let mut responder = HandshakeSession::trusted_responder(&server_keys, client_keys.public_key())
        .expect("second IK responder");
    complete_ik(&mut initiator, &mut responder).expect("cryptographic IK still completes");
    assert!(server
        .authorize_transport(responder.finish().expect("server transport"))
        .is_err());
}

// LNS-TRUST-004
#[test]
fn trust_multi_client_registry_updates_are_isolated_atomic_and_preserve_other_fields() {
    let temp = TempDir::new().expect("tempdir");
    let paths = HidlinsPaths::with_state_dir(temp.path().join("state"));
    let mut registry = VaultRegistry::with_paths(paths.clone());
    let mut extra = toml::Table::new();
    extra.insert(
        "unrelated".to_string(),
        toml::Value::String("keep".to_string()),
    );
    registry
        .register_and_save(RegisteredVault {
            name: "personal".to_string(),
            path: temp.path().join("personal.kdbx"),
            created_at: "2026-09-06T00:00:00Z".to_string(),
            keyfile_path: None,
            extra,
        })
        .expect("register");

    let master = password("registry secret");
    let mut config =
        LocalSyncConfig::create("personal", SyncRole::Server, &master).expect("config");
    let mut first_client =
        LocalSyncConfig::create("personal", SyncRole::Client, &master).expect("first client");
    let mut second_client =
        LocalSyncConfig::create("personal", SyncRole::Client, &master).expect("second client");

    fully_pair(&mut config, &mut first_client, &[31_u8; 32]);
    assert!(config.expire_provisional(1_180));
    fully_pair(&mut config, &mut second_client, &[32_u8; 32]);

    let first_key = first_client.identity().public_key();
    let second_key = second_client.identity().public_key();
    assert_eq!(config.trusted_peers().len(), 2);
    config.revoke_peer(first_key).expect("revoke first only");
    config
        .rename_peer(second_key, "Tablet".to_string())
        .expect("rename second only");
    assert!(!config.authorizes(first_key));
    assert!(config.authorizes(second_key));
    assert_eq!(config.trusted_peers()[1].display_name(), "Tablet");

    let mut stale = VaultRegistry::load(paths.clone()).expect("stale registry");
    registry
        .update_registered_extra("personal", |fields| {
            fields.insert("concurrent".to_string(), toml::Value::Boolean(true));
        })
        .expect("concurrent edit");
    config
        .persist(&mut stale, "personal")
        .expect("rebased local persistence");
    apply_concurrent_peer_renames(&paths, first_key, second_key);

    let loaded = VaultRegistry::load(paths.clone()).expect("reload");
    let entry = loaded.get("personal").expect("vault");
    assert_eq!(
        entry.extra.get("unrelated").and_then(toml::Value::as_str),
        Some("keep")
    );
    assert_eq!(
        entry.extra.get("concurrent").and_then(toml::Value::as_bool),
        Some(true)
    );
    let persisted = LocalSyncConfig::from_vault_entry(entry).expect("local config");
    assert_eq!(
        persisted.trusted_peers()[0].display_name(),
        "Retired laptop"
    );
    assert_eq!(
        persisted.trusted_peers()[1].display_name(),
        "Current tablet"
    );

    let mut malformed_entry = entry.clone();
    let peers = malformed_entry
        .extra
        .get_mut("sync")
        .and_then(toml::Value::as_table_mut)
        .and_then(|sync| sync.get_mut("trusted_peers"))
        .and_then(toml::Value::as_array_mut)
        .expect("trusted peer array");
    peers
        .first_mut()
        .and_then(toml::Value::as_table_mut)
        .expect("peer table")
        .insert(
            "display_name".to_string(),
            toml::Value::String(String::new()),
        );
    assert!(LocalSyncConfig::from_vault_entry(&malformed_entry).is_none());

    let disk = std::fs::read_to_string(paths.vaults_toml()).expect("registry bytes");
    assert!(!disk.contains("registry secret"));
    assert!(disk.contains("sealed_private_key"));
    assert!(!disk
        .lines()
        .any(|line| line.trim_start().starts_with("private_key =")));

    #[cfg(unix)]
    assert_registry_mode_is_private(&paths);
}

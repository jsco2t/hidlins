//! Expired durable prepares must not strand a vault's next confirmed pairing.

use std::time::Instant;

use hidlins_core::{MasterPassword, RegisteredVault};
use hidlins_sync::{
    config::local::LocalSyncConfig,
    identity::SyncRole,
    pairing::{ConfirmedPairing, PairingTransaction, PairingWindow},
    trust::TrustError,
};

fn config(role: SyncRole) -> LocalSyncConfig {
    LocalSyncConfig::create("vault", role, &MasterPassword::new("test master".into())).unwrap()
}

fn confirmed(
    server: &LocalSyncConfig,
    client: &LocalSyncConfig,
    hash: u8,
) -> (PairingTransaction, ConfirmedPairing, ConfirmedPairing) {
    let now = Instant::now();
    let mut server_window = PairingWindow::open_at(now);
    let mut client_window = PairingWindow::open_at(now);
    let server_sas = server_window
        .begin_candidate(client.identity().public_key(), [hash; 32], now)
        .unwrap();
    let client_sas = client_window
        .begin_candidate(server.identity().public_key(), [hash; 32], now)
        .unwrap();
    let server_confirmation = server_window.confirm(server_sas, now).unwrap();
    let client_confirmation = client_window.confirm(client_sas, now).unwrap();
    let transaction = client_confirmation
        .transaction(
            server.identity().public_key(),
            client.identity().public_key(),
        )
        .unwrap();
    (transaction, server_confirmation, client_confirmation)
}

fn reload(config: &LocalSyncConfig) -> LocalSyncConfig {
    let mut entry = RegisteredVault {
        name: "vault".into(),
        path: "/unused/vault.kdbx".into(),
        created_at: "2026-11-04T00:00:00Z".into(),
        keyfile_path: None,
        extra: toml::Table::new(),
    };
    config.write_to_entry(&mut entry).unwrap();
    let serialized = toml::to_string(&entry).unwrap();
    let reloaded: RegisteredVault = toml::from_str(&serialized).unwrap();
    LocalSyncConfig::from_vault_entry(&reloaded).unwrap()
}

fn retry_expired(replace_peer: bool) {
    let mut server = config(SyncRole::Server);
    let mut client = config(SyncRole::Client);
    let (old, server_confirm, client_confirm) = confirmed(&server, &client, 1);
    server
        .prepare_client(&old, &server_confirm, "client".into(), 1_000, 1_180)
        .unwrap();
    client
        .prepare_server(&old, &client_confirm, "server".into(), 1_000, 1_180)
        .unwrap();
    let mut server = reload(&server);
    let mut client = reload(&client);
    assert_eq!(
        server.activate_client(&old, 1_180),
        Err(TrustError::Expired)
    );
    assert_eq!(
        client.activate_server(&old, 1_180),
        Err(TrustError::Expired)
    );
    let original_server = server.identity().clone();
    let original_client = client.identity().clone();
    if replace_peer {
        // Each endpoint independently retries with a different peer after expiry.
        let fresh_client = config(SyncRole::Client);
        let fresh_server = config(SyncRole::Server);
        let (server_tx, server_confirm, _) = confirmed(&server, &fresh_client, 2);
        let (client_tx, _, client_confirm) = confirmed(&fresh_server, &client, 3);
        server
            .prepare_client(
                &server_tx,
                &server_confirm,
                "new client".into(),
                1_180,
                1_360,
            )
            .expect("expired server prepare permits a freshly confirmed different peer");
        client
            .prepare_server(
                &client_tx,
                &client_confirm,
                "new server".into(),
                1_180,
                1_360,
            )
            .expect("expired client prepare permits a freshly confirmed different peer");
        server.activate_client(&server_tx, 1_181).unwrap();
        client.activate_server(&client_tx, 1_181).unwrap();
        assert!(server.authorizes(fresh_client.identity().public_key()));
        assert!(client.authorizes(fresh_server.identity().public_key()));
    } else {
        let (new, server_confirm, client_confirm) = confirmed(&server, &client, 4);
        server
            .prepare_client(&new, &server_confirm, "client".into(), 1_180, 1_360)
            .expect("expired prepare permits a fresh transaction for the same key");
        client
            .prepare_server(&new, &client_confirm, "server".into(), 1_180, 1_360)
            .unwrap();
        assert!(!server.authorizes(client.identity().public_key()));
        assert!(!client.authorizes(server.identity().public_key()));
        assert!(server.activate_client(&old, 1_181).is_err());
        assert!(client.activate_server(&old, 1_181).is_err());
        server.activate_client(&new, 1_181).unwrap();
        client.activate_server(&new, 1_181).unwrap();
        assert!(server.authorizes(client.identity().public_key()));
        assert!(client.authorizes(server.identity().public_key()));
    }
    assert_eq!(server.identity(), &original_server);
    assert_eq!(client.identity(), &original_client);
    assert!(reload(&client).is_active_client());
}

#[test]
fn expired_prepare_allows_fresh_confirmed_same_peer_transaction() {
    retry_expired(false);
}

#[test]
fn expired_prepare_allows_fresh_confirmed_different_peer_transaction() {
    retry_expired(true);
}

#[test]
fn unexpired_receipt_is_idempotent_and_cannot_be_replaced() {
    let mut server = config(SyncRole::Server);
    let client = config(SyncRole::Client);
    let (old, confirmation, _) = confirmed(&server, &client, 5);
    server
        .prepare_client(&old, &confirmation, "client".into(), 1_000, 1_180)
        .unwrap();
    let before = server.clone();
    server
        .prepare_client(&old, &confirmation, "ignored".into(), 1_179, 1_359)
        .unwrap();
    assert_eq!(
        server, before,
        "exact replay must not extend receipt lifetime"
    );
    let (new, confirmation, _) = confirmed(&server, &client, 6);
    assert_eq!(
        server.prepare_client(&new, &confirmation, "client".into(), 1_179, 1_359),
        Err(TrustError::TransactionMismatch)
    );
    assert_eq!(server, before);
    assert_eq!(
        server.activate_client(&old, 1_180),
        Err(TrustError::Expired)
    );
}

#[test]
fn expiry_never_overrides_active_pin_or_revocation() {
    let mut server = config(SyncRole::Server);
    let mut client = config(SyncRole::Client);
    let (old, server_confirm, client_confirm) = confirmed(&server, &client, 7);
    server
        .prepare_client(&old, &server_confirm, "client".into(), 1_000, 1_180)
        .unwrap();
    client
        .prepare_server(&old, &client_confirm, "server".into(), 1_000, 1_180)
        .unwrap();
    server.activate_client(&old, 1_001).unwrap();
    client.activate_server(&old, 1_001).unwrap();
    let (new, server_confirm, client_confirm) = confirmed(&server, &client, 8);
    assert!(server
        .prepare_client(&new, &server_confirm, "client".into(), 1_180, 1_360)
        .is_err());
    assert!(server.authorizes(client.identity().public_key()));
    assert_eq!(
        client.prepare_server(&new, &client_confirm, "server".into(), 1_180, 1_360),
        Err(TrustError::RedesignationRequired)
    );
    server.revoke_peer(client.identity().public_key()).unwrap();
    assert_eq!(
        server.prepare_client(&new, &server_confirm, "client".into(), 1_181, 1_361),
        Err(TrustError::DuplicatePeer)
    );
    assert!(!reload(&server).authorizes(client.identity().public_key()));
}

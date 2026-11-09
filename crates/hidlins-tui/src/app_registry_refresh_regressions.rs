use super::*;

fn add_peer_on_disk(paths: &HidlinsPaths) -> hidlins_sync::identity::PublicIdentity {
    use hidlins_sync::pairing::PairingWindow;

    let mut registry = VaultRegistry::load(paths.clone()).unwrap();
    let mut server = LocalSyncConfig::from_vault_entry(registry.get("personal").unwrap()).unwrap();
    let client = LocalSyncConfig::create(
        "fixture-client",
        SyncRole::Client,
        &MasterPassword::new(PASSWORD.to_string()),
    )
    .unwrap();
    let peer = client.identity().public_key();
    let now = Instant::now();
    let mut window = PairingWindow::open_at(now);
    let sas = window.begin_candidate(peer, [41; 32], now).unwrap();
    let confirmation = window.confirm(sas, now).unwrap();
    let transaction = confirmation
        .transaction(server.identity().public_key(), peer)
        .unwrap();
    server
        .prepare_client(
            &transaction,
            &confirmation,
            "new fixture peer".to_string(),
            1000,
            1180,
        )
        .unwrap();
    server.activate_client(&transaction, 1001).unwrap();
    server.persist(&mut registry, "personal").unwrap();
    peer
}

fn serving_app() -> (TempDir, App) {
    let (dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    app.start_server(MasterPassword::new(PASSWORD.to_string()))
        .unwrap();
    (dir, app)
}

#[test]
fn trust_reload_during_vault_work_survives_stale_registry_handback() {
    let (_dir, mut app) = serving_app();
    let (vault, stale_registry) = app.session.begin_sync().unwrap();
    let peer = add_peer_on_disk(&app.paths);
    app.server.as_ref().unwrap().notify_trust_reload_for_test();
    app.tick(Instant::now());
    assert!(
        app.is_syncing(),
        "the UI must not steal worker-owned resources"
    );
    app.session.finish_sync(vault, stale_registry);
    app.tick(Instant::now());
    assert!(app.selected_local_config().unwrap().authorizes(peer));
    app.open_peers();
    let Some(Overlay::Peers(state)) = app.overlay.as_ref() else {
        panic!("peer management overlay must open");
    };
    assert_eq!(state.names, ["new fixture peer"]);
    app.revoke_peer(0);
    let registry = VaultRegistry::load(app.paths.clone()).unwrap();
    assert!(
        !LocalSyncConfig::from_vault_entry(registry.get("personal").unwrap())
            .unwrap()
            .authorizes(peer)
    );
    app.lock_app();
}

#[test]
fn ready_session_applies_trust_reload_immediately() {
    let (_dir, mut app) = serving_app();
    let peer = add_peer_on_disk(&app.paths);
    app.server.as_ref().unwrap().notify_trust_reload_for_test();
    app.tick(Instant::now());
    assert!(app.selected_local_config().unwrap().authorizes(peer));
    app.lock_app();
}

#[test]
fn failed_registry_reload_is_retried_after_the_file_is_readable() {
    let (_dir, mut app) = serving_app();
    let peer = add_peer_on_disk(&app.paths);
    let path = app.paths.vaults_toml();
    let good = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"not = [valid toml").unwrap();
    app.server.as_ref().unwrap().notify_trust_reload_for_test();
    app.tick(Instant::now());
    assert!(!app.selected_local_config().unwrap().authorizes(peer));
    std::fs::write(&path, good).unwrap();
    app.tick(Instant::now());
    assert!(app.selected_local_config().unwrap().authorizes(peer));
    app.lock_app();
}

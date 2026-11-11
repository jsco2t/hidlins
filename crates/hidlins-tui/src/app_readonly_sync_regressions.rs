use super::*;

fn activate_fixture_peer(app: &mut App) -> hidlins_sync::identity::PublicIdentity {
    use hidlins_sync::pairing::PairingWindow;

    let master = MasterPassword::new(PASSWORD.to_string());
    let mut server = LocalSyncConfig::configure(
        app.registry_mut().unwrap(),
        "personal",
        SyncRole::Server,
        &master,
    )
    .unwrap();
    let client = LocalSyncConfig::create("fixture-client", SyncRole::Client, &master).unwrap();
    let peer = client.identity().public_key();
    let now = Instant::now();
    let mut window = PairingWindow::open_at(now);
    let sas = window.begin_candidate(peer, [37; 32], now).unwrap();
    let confirmation = window.confirm(sas, now).unwrap();
    let transaction = confirmation
        .transaction(server.identity().public_key(), peer)
        .unwrap();
    server
        .prepare_client(
            &transaction,
            &confirmation,
            "fixture peer".to_string(),
            1000,
            1180,
        )
        .unwrap();
    server.activate_client(&transaction, 1001).unwrap();
    server
        .persist(app.registry_mut().unwrap(), "personal")
        .unwrap();
    peer
}

#[test]
fn settings_enter_refuses_all_vault_affecting_rows_in_readonly_mode() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    app.read_only = true;
    let registry = std::fs::read(app.paths.vaults_toml()).unwrap();
    let path = app.vault().unwrap().backing_path().to_path_buf();
    let vault = std::fs::read(&path).unwrap();
    let mut opened_rows = Vec::new();
    for row in 2..=8 {
        app.overlay = None;
        app.settings_index = row;
        app.activate_settings_row();
        if app.overlay.is_some() || app.server.is_some() {
            opened_rows.push(row);
        }
        assert_eq!(std::fs::read(app.paths.vaults_toml()).unwrap(), registry);
        assert_eq!(std::fs::read(&path).unwrap(), vault);
        assert!(!app.is_syncing());
    }
    assert!(
        opened_rows.is_empty(),
        "read-only rows opened actions: {opened_rows:?}"
    );
}

#[test]
fn stale_server_prompt_cannot_start_work_after_readonly_is_enabled() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    app.open_local_sync(LocalSyncAction::StartServer);
    for character in PASSWORD.chars() {
        app.handle_event(&key(character));
    }
    let before = std::fs::read(app.paths.vaults_toml()).unwrap();
    app.read_only = true;
    app.handle_event(&key_code(KeyCode::Enter));
    assert!(
        app.server.is_none(),
        "a stale prompt must not create a listener"
    );
    assert_eq!(std::fs::read(app.paths.vaults_toml()).unwrap(), before);
}

#[test]
fn readonly_peer_revoke_preserves_active_trust_and_registry_bytes() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let peer = activate_fixture_peer(&mut app);
    let before = std::fs::read(app.paths.vaults_toml()).unwrap();
    app.read_only = true;
    app.revoke_peer(0);
    assert!(app.selected_local_config().unwrap().authorizes(peer));
    assert_eq!(std::fs::read(app.paths.vaults_toml()).unwrap(), before);
}

#[test]
fn writable_peer_revoke_still_persists_revocation() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let peer = activate_fixture_peer(&mut app);
    app.revoke_peer(0);
    assert!(!app.selected_local_config().unwrap().authorizes(peer));
    let registry = VaultRegistry::load(app.paths.clone()).unwrap();
    assert!(
        !LocalSyncConfig::from_vault_entry(registry.get("personal").unwrap())
            .unwrap()
            .authorizes(peer)
    );
}

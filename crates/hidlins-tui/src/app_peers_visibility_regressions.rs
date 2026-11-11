use super::*;
use crate::test_support;
use hidlins_sync::pairing::PairingWindow;

fn paired_app() -> (TempDir, App) {
    let (dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let master = MasterPassword::new(PASSWORD.to_string());
    let mut server = LocalSyncConfig::configure(
        app.registry_mut().unwrap(),
        "personal",
        SyncRole::Server,
        &master,
    )
    .unwrap();
    for index in 0..12 {
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
                format!("device-{index:02}"),
                1000,
                1180,
            )
            .unwrap();
        server.activate_client(&transaction, 1001).unwrap();
    }
    assert_eq!(server.trusted_peers().len(), 12);
    server
        .persist(app.registry_mut().unwrap(), "personal")
        .unwrap();
    (dir, app)
}

fn visible_revoke(width: u16, height: u16) {
    let (_dir, mut app) = paired_app();
    let config = app.selected_local_config().unwrap();
    let peers = config.trusted_peers();
    let target = peers[11].public_key();
    let label = peers[11].display_name().to_string();
    let others: Vec<_> = peers[..11]
        .iter()
        .map(hidlins_sync::trust::PeerRecord::public_key)
        .collect();
    app.open_peers();
    for _ in 0..11 {
        app.handle_event(&test_support::key('j'));
    }
    let Some(Overlay::Peers(state)) = app.overlay.as_ref() else {
        panic!("expected peers overlay");
    };
    assert_eq!(state.selected, 11);
    assert_eq!(state.names[11], label);
    let rendered = test_support::render_at(&app, width, height, Instant::now());
    assert!(
        rendered
            .transcript
            .lines()
            .any(|line| line.contains("> peer-11:") && line.contains(&label)),
        "the selected identity must be visible before it can be revoked"
    );
    rendered.assert_contains_all(&["Sync peers", "r: Revoke"]);
    app.handle_event(&test_support::key('r'));
    assert!(app.overlay.is_none());
    let config = app.selected_local_config().unwrap();
    assert!(!config.authorizes(target));
    assert!(others.iter().all(|peer| config.authorizes(*peer)));
    let registry = VaultRegistry::load(app.paths.clone()).unwrap();
    let persisted = LocalSyncConfig::from_vault_entry(registry.get("personal").unwrap()).unwrap();
    assert!(!persisted.authorizes(target));
    assert!(others.iter().all(|peer| persisted.authorizes(*peer)));
}

#[test]
fn selected_last_peer_is_visible_and_only_that_identity_is_revoked() {
    visible_revoke(90, 24);
}

#[test]
fn short_narrow_peer_modal_keeps_selection_and_action_visible() {
    visible_revoke(40, 8);
}

#[test]
fn empty_peer_list_and_cancel_preserve_registry() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let before = std::fs::read(app.paths.vaults_toml()).unwrap();
    app.open_peers();
    test_support::render_at(&app, 90, 24, Instant::now())
        .assert_contains_all(&["No paired peers."]);
    app.handle_event(&test_support::key_code(KeyCode::Esc));
    assert!(app.overlay.is_none());
    assert_eq!(std::fs::read(app.paths.vaults_toml()).unwrap(), before);
}

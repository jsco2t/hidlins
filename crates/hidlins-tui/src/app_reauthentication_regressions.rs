use super::*;

#[test]
fn correct_password_authenticates_while_app_owns_exclusive_vault() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let path = app.vault().unwrap().backing_path().to_path_buf();
    let before = std::fs::read(&path).unwrap();
    assert!(app
        .authenticate_sync_password(&MasterPassword::new(PASSWORD.to_string()))
        .is_ok());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(matches!(
        Vault::open(&path, &MasterPassword::new(PASSWORD.to_string()), None),
        Err(VaultError::Contended { .. })
    ));
}

#[test]
fn incorrect_password_is_rejected_without_touching_owned_vault() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    let path = app.vault().unwrap().backing_path().to_path_buf();
    let before = std::fs::read(&path).unwrap();
    assert!(app
        .authenticate_sync_password(&MasterPassword::new("wrong fixture password".to_string()))
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(app.vault().is_some());
}

#[test]
fn reauthentication_cannot_open_a_locked_session() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    app.lock_app();
    assert!(app
        .authenticate_sync_password(&MasterPassword::new(PASSWORD.to_string()))
        .is_err());
    assert!(app.vault().is_none());
}

#[test]
fn valid_server_overlay_submission_starts_and_stops_local_listener() {
    let (_dir, mut app) = populated_app();
    unlock(&mut app, PASSWORD);
    app.open_local_sync(LocalSyncAction::StartServer);
    for character in PASSWORD.chars() {
        app.handle_event(&key(character));
    }
    app.handle_event(&key_code(KeyCode::Enter));
    assert!(
        app.overlay.is_none(),
        "correct credentials finish the prompt"
    );
    let endpoint = app
        .server
        .as_ref()
        .expect("local server started")
        .endpoint();
    let endpoint: hidlins_sync::address::LocalEndpoint = endpoint.parse().unwrap();
    assert!(
        std::net::TcpStream::connect_timeout(&endpoint.socket_addr(), Duration::from_secs(1))
            .is_ok()
    );
    app.lock_app();
    assert!(app.server.is_none());
    assert!(std::net::TcpStream::connect_timeout(
        &endpoint.socket_addr(),
        Duration::from_millis(200)
    )
    .is_err());
}

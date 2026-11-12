use super::*;
use hidlins_core::Keyfile;

fn keyfile_app() -> (TempDir, App, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let paths = HidlinsPaths::with_state_dir(dir.path().join("state"));
    let keyfile = dir.path().join("fixture.key");
    std::fs::write(&keyfile, b"synthetic keyfile fixture bytes").unwrap();
    let path = dir.path().join("personal.kdbx");
    drop(
        Vault::create(
            &path,
            &MasterPassword::new(PASSWORD.to_string()),
            Some(&Keyfile::Path(keyfile.clone())),
            fast_kdf(),
            NoRecoveryConfirmed::yes(),
        )
        .unwrap(),
    );
    let mut registry = VaultRegistry::with_paths(paths.clone());
    registry
        .register(RegisteredVault {
            name: "personal".to_string(),
            path,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            keyfile_path: Some(keyfile.clone()),
            extra: toml::Table::new(),
        })
        .unwrap();
    registry.save().unwrap();
    let app = App::from_registry(registry, paths, AutoLockConfig::default()).unwrap();
    (dir, app, keyfile)
}

#[test]
fn registered_keyfile_unlocks_and_reauthenticates_without_writing_ciphertext() {
    let (_dir, mut app, _keyfile) = keyfile_app();
    let path = app.registry().get("personal").unwrap().path.clone();
    let before = std::fs::read(&path).unwrap();
    unlock(&mut app, PASSWORD);
    assert!(matches!(app.phase, Phase::Workspace));
    assert!(app.vault().is_some());
    assert!(app
        .authenticate_sync_password(&MasterPassword::new(PASSWORD.to_string()))
        .is_ok());
    assert!(app
        .authenticate_sync_password(&MasterPassword::new("wrong fixture password".to_string()))
        .is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn missing_registered_keyfile_does_not_unlock_or_rewrite_vault() {
    let (_dir, mut app, keyfile) = keyfile_app();
    let path = app.registry().get("personal").unwrap().path.clone();
    let before = std::fs::read(&path).unwrap();
    std::fs::remove_file(keyfile).unwrap();
    unlock(&mut app, PASSWORD);
    assert!(app.vault().is_none());
    assert!(!matches!(app.phase, Phase::Workspace));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn wrong_registered_keyfile_does_not_unlock_or_rewrite_vault() {
    let (_dir, mut app, keyfile) = keyfile_app();
    let path = app.registry().get("personal").unwrap().path.clone();
    let before = std::fs::read(&path).unwrap();
    std::fs::write(keyfile, b"different synthetic keyfile fixture").unwrap();
    unlock(&mut app, PASSWORD);
    assert!(app.vault().is_none());
    assert!(!matches!(app.phase, Phase::Workspace));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

fn configure_pinned_apps(server: &mut App, client: &mut App) {
    use hidlins_sync::pairing::PairingWindow;

    let master = MasterPassword::new(PASSWORD.to_string());
    let mut server_config = LocalSyncConfig::configure(
        server.registry_mut().unwrap(),
        "personal",
        SyncRole::Server,
        &master,
    )
    .unwrap();
    let mut client_config = LocalSyncConfig::configure(
        client.registry_mut().unwrap(),
        "personal",
        SyncRole::Client,
        &master,
    )
    .unwrap();
    let server_key = server_config.identity().public_key();
    let client_key = client_config.identity().public_key();
    let now = Instant::now();
    let mut server_window = PairingWindow::open_at(now);
    let mut client_window = PairingWindow::open_at(now);
    let server_sas = server_window
        .begin_candidate(client_key, [39; 32], now)
        .unwrap();
    let client_sas = client_window
        .begin_candidate(server_key, [39; 32], now)
        .unwrap();
    assert_eq!(server_sas, client_sas);
    let server_confirmation = server_window.confirm(server_sas, now).unwrap();
    let client_confirmation = client_window.confirm(client_sas, now).unwrap();
    let transaction = client_confirmation
        .transaction(server_key, client_key)
        .unwrap();
    server_config
        .prepare_client(
            &transaction,
            &server_confirmation,
            "fixture client".to_string(),
            1000,
            1180,
        )
        .unwrap();
    client_config
        .prepare_server(
            &transaction,
            &client_confirmation,
            "fixture server".to_string(),
            1000,
            1180,
        )
        .unwrap();
    server_config.activate_client(&transaction, 1001).unwrap();
    client_config.activate_server(&transaction, 1001).unwrap();
    server_config
        .persist(server.registry_mut().unwrap(), "personal")
        .unwrap();
    server
        .start_server(MasterPassword::new(PASSWORD.to_string()))
        .unwrap();
    client_config.set_routing_hint(Some(
        server.server.as_ref().unwrap().endpoint().parse().unwrap(),
    ));
    client_config
        .persist(client.registry_mut().unwrap(), "personal")
        .unwrap();
}

fn run_app_sync(server: &mut App, client: &mut App) {
    client.start_sync(
        SyncTrigger::Manual,
        MasterPassword::new(PASSWORD.to_string()),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while client.is_syncing() || server.is_syncing() {
        server.tick(Instant::now());
        client.tick(Instant::now());
        assert!(
            Instant::now() < deadline,
            "owned local App sync did not complete"
        );
        std::thread::yield_now();
    }
    assert!(
        matches!(client.phase, Phase::Workspace),
        "client result: {:?}",
        client.sync_status_line()
    );
    assert!(matches!(server.phase, Phase::Workspace));
    assert!(client.vault().is_some());
    assert!(server.vault().is_some());
}

// Isolate the client/server adapters from the independently tested unlock bug:
// establish the App session through the existing correct core keyfile API.
fn core_keyfile_unlock(app: &mut App, keyfile: PathBuf) {
    let path = app.registry().get("personal").unwrap().path.clone();
    let vault = Vault::open(
        &path,
        &MasterPassword::new(PASSWORD.to_string()),
        Some(&Keyfile::Path(keyfile)),
    )
    .unwrap();
    app.complete_unlock(
        vault,
        &UnlockOrigin::Direct {
            vault_name: "personal".to_string(),
        },
        PASSWORD,
    );
}

// Keep the host-upload case independent of RealSyncEngine's missing keyfile.
// This forwards to the established client with the correct registered keyfile;
// the actual App host adapter, encrypted transport and commit remain in use.
struct KeyfileClientControl;
impl SyncEngine for KeyfileClientControl {
    fn sync_now(
        &self,
        vault: &mut Vault,
        name: &str,
        registry: &mut VaultRegistry,
        master: &MasterPassword,
        opts: hidlins_sync::SyncOptions,
    ) -> Result<SyncOutcome, SyncError> {
        let keyfile = registry
            .get(name)
            .unwrap()
            .keyfile_path
            .clone()
            .map(Keyfile::Path);
        hidlins_sync::client::sync_vault(vault, name, registry, master, keyfile.as_ref(), [], opts)
    }
}

#[test]
fn registered_keyfile_real_client_decrypts_a_changed_remote_snapshot() {
    let (_server_dir, mut server, server_keyfile) = keyfile_app();
    let (_client_dir, mut client, client_keyfile) = keyfile_app();
    std::fs::copy(
        &server.registry().get("personal").unwrap().path,
        &client.registry().get("personal").unwrap().path,
    )
    .unwrap();
    core_keyfile_unlock(&mut server, server_keyfile);
    core_keyfile_unlock(&mut client, client_keyfile);
    configure_pinned_apps(&mut server, &mut client);
    let group = server.vault().unwrap().root_group_uuid();
    let entry = server
        .vault_mut()
        .unwrap()
        .add_entry(
            group,
            EntryBuilder::credential("remote keyfile fixture").build(),
        )
        .unwrap();
    server.vault_mut().unwrap().save().unwrap();
    run_app_sync(&mut server, &mut client);
    assert!(client.vault().unwrap().get_entry(entry).is_ok());
    server.lock_app();
    client.lock_app();
}

#[test]
fn registered_keyfile_real_apps_fetch_merge_and_validate_encrypted_upload() {
    let (_server_dir, mut server, server_keyfile) = keyfile_app();
    let (_client_dir, mut client, client_keyfile) = keyfile_app();
    let server_path = server.registry().get("personal").unwrap().path.clone();
    let client_path = client.registry().get("personal").unwrap().path.clone();
    std::fs::copy(&server_path, &client_path).unwrap();
    core_keyfile_unlock(&mut server, server_keyfile);
    core_keyfile_unlock(&mut client, client_keyfile);
    client.set_sync_engine(Arc::new(KeyfileClientControl));
    configure_pinned_apps(&mut server, &mut client);
    run_app_sync(&mut server, &mut client);
    let group = client.vault().unwrap().root_group_uuid();
    let entry = client
        .vault_mut()
        .unwrap()
        .add_entry(
            group,
            EntryBuilder::credential("keyfile upload fixture").build(),
        )
        .unwrap();
    client.vault_mut().unwrap().save().unwrap();
    run_app_sync(&mut server, &mut client);
    assert!(
        server.vault().unwrap().get_entry(entry).is_ok(),
        "authoritative keyfile validation must commit the upload"
    );
    server.lock_app();
    client.lock_app();
    let master = MasterPassword::new(PASSWORD.to_string());
    assert!(Vault::open(&server_path, &master, None).is_err());
    assert!(Vault::open(&client_path, &master, None).is_err());
}

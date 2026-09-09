mod common;

use hidlins_api::api::session::AppSession;
use hidlins_api::dto::LocalSyncRoleDto;
use hidlins_api::error::HidlinsApiError;
use hidlins_sync::config::local::LocalSyncConfig;

#[test]
fn password_change_rewraps_local_identity_without_changing_public_key() {
    let env = common::TestEnv::new();
    let path = common::create_test_vault(&env, "rewrap", "old-pass");
    common::register_vault(&env, "rewrap", &path);
    let session = AppSession::for_test(env.paths_clone()).expect("session");
    session
        .unlock("rewrap".to_string(), "old-pass".to_string(), None)
        .expect("unlock");
    session
        .configure_local_sync(LocalSyncRoleDto::Server)
        .expect("configure");

    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let before = LocalSyncConfig::from_vault_entry(registry.get("rewrap").expect("entry"))
        .expect("local config");
    let public_before = before.identity().public_key();
    let sealed_before = before.identity().sealed_value().to_string();

    session
        .change_master_password("old-pass".to_string(), "new-pass".to_string())
        .expect("password change");
    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let after = LocalSyncConfig::from_vault_entry(registry.get("rewrap").expect("entry"))
        .expect("local config");
    assert_eq!(after.identity().public_key(), public_before);
    assert_ne!(after.identity().sealed_value(), sealed_before);
    assert!(after
        .identity()
        .unlock(
            "rewrap",
            hidlins_sync::identity::SyncRole::Server,
            &hidlins_core::MasterPassword::new("new-pass".to_string()),
        )
        .is_ok());

    session.lock_now().expect("lock");
    assert!(session
        .unlock("rewrap".to_string(), "new-pass".to_string(), None)
        .is_ok());
    session.lock_now().expect("lock");
    assert!(matches!(
        session.unlock("rewrap".to_string(), "old-pass".to_string(), None),
        Err(HidlinsApiError::AuthenticationFailed)
    ));
}

#[test]
fn wrong_current_password_changes_neither_vault_nor_identity() {
    let env = common::TestEnv::new();
    let path = common::create_test_vault(&env, "wrong", "correct");
    common::register_vault(&env, "wrong", &path);
    let session = AppSession::for_test(env.paths_clone()).expect("session");
    session
        .unlock("wrong".to_string(), "correct".to_string(), None)
        .expect("unlock");
    session
        .configure_local_sync(LocalSyncRoleDto::Client)
        .expect("configure");
    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let before =
        LocalSyncConfig::from_vault_entry(registry.get("wrong").expect("entry")).expect("config");
    let sealed = before.identity().sealed_value().to_string();

    assert!(matches!(
        session.change_master_password("incorrect".to_string(), "new".to_string()),
        Err(HidlinsApiError::AuthenticationFailed)
    ));
    assert!(session.has_vault());
    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let after =
        LocalSyncConfig::from_vault_entry(registry.get("wrong").expect("entry")).expect("config");
    assert_eq!(after.identity().sealed_value(), sealed);
}

#[test]
fn locked_password_change_is_rejected() {
    let env = common::TestEnv::new();
    let session = AppSession::for_test(env.paths_clone()).expect("session");
    assert!(matches!(
        session.change_master_password("old".to_string(), "new".to_string()),
        Err(HidlinsApiError::VaultLocked)
    ));
}

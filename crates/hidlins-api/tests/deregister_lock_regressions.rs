mod common;

use common::{create_test_vault, master, register_vault, TestEnv};
use hidlins_api::api::session::AppSession;
use hidlins_api::error::HidlinsApiError;
use hidlins_core::{Vault, VaultRegistry};

const PASSWORD: &str = "api-deletion-lock-fixture";

#[test]
fn an_unlocked_session_cannot_delete_its_own_vault() {
    let env = TestEnv::new();
    let path = create_test_vault(&env, "fixture", PASSWORD);
    register_vault(&env, "fixture", &path);
    let session = AppSession::for_test(env.paths_clone()).unwrap();
    session
        .unlock("fixture".into(), PASSWORD.into(), None)
        .unwrap();
    let before = std::fs::read(&path).unwrap();

    assert!(matches!(
        session.deregister_vault("fixture".into(), true),
        Err(HidlinsApiError::VaultContended { .. })
    ));
    assert!(session.has_vault());
    assert_eq!(session.list_vaults().unwrap().len(), 1);
    assert!(VaultRegistry::load(env.paths_clone())
        .unwrap()
        .get("fixture")
        .is_some());
    assert_eq!(std::fs::read(&path).unwrap(), before);

    session.lock_now().unwrap();
    session.deregister_vault("fixture".into(), true).unwrap();
    assert!(!path.exists());
    assert!(session.list_vaults().unwrap().is_empty());
    assert!(VaultRegistry::load(env.paths_clone())
        .unwrap()
        .get("fixture")
        .is_none());
    session.shutdown();
}

#[test]
fn deletion_reports_an_external_owner_and_succeeds_after_release() {
    let env = TestEnv::new();
    let path = create_test_vault(&env, "fixture", PASSWORD);
    register_vault(&env, "fixture", &path);
    let session = AppSession::for_test(env.paths_clone()).unwrap();
    let owner = Vault::open(&path, &master(PASSWORD), None).unwrap();
    let before = std::fs::read(&path).unwrap();

    assert!(matches!(
        session.deregister_vault("fixture".into(), true),
        Err(HidlinsApiError::VaultContended { .. })
    ));
    assert_eq!(session.list_vaults().unwrap().len(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), before);

    drop(owner);
    session.deregister_vault("fixture".into(), true).unwrap();
    assert!(!path.exists());
    assert!(session.list_vaults().unwrap().is_empty());
    session.shutdown();
}

#[test]
fn unregister_only_still_preserves_an_unlocked_session() {
    let env = TestEnv::new();
    let path = create_test_vault(&env, "fixture", PASSWORD);
    register_vault(&env, "fixture", &path);
    let session = AppSession::for_test(env.paths_clone()).unwrap();
    session
        .unlock("fixture".into(), PASSWORD.into(), None)
        .unwrap();
    let before = std::fs::read(&path).unwrap();

    session.deregister_vault("fixture".into(), false).unwrap();
    assert!(session.has_vault());
    assert!(session.list_vaults().unwrap().is_empty());
    assert_eq!(std::fs::read(path).unwrap(), before);
    session.shutdown();
}

mod common;

use common::{create_test_vault, master, TestEnv};
use hidlins_api::api::session::AppSession;
use hidlins_api::error::HidlinsApiError;
use hidlins_core::{Vault, VaultError};

#[test]
fn registered_alias_keeps_its_logical_path_and_locks_the_backing_vault() {
    let env = TestEnv::new();
    let real = create_test_vault(&env, "real", "alias-api-fixture");
    let alias = env.tempdir().join("linked.kdbx");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let session = AppSession::for_test(env.paths_clone()).unwrap();
    let summary = session
        .register_existing_vault("linked".into(), alias.to_str().unwrap().into(), None)
        .unwrap();
    assert_eq!(summary.path, alias.to_str().unwrap());
    session
        .unlock("linked".into(), "alias-api-fixture".into(), None)
        .unwrap();

    assert!(matches!(
        Vault::open(&real, &master("alias-api-fixture"), None),
        Err(VaultError::Contended { .. })
    ));
    assert!(alias.is_symlink());
    session.lock_now().unwrap();
    Vault::open(&real, &master("alias-api-fixture"), None).unwrap();
    session.shutdown();
}

#[test]
fn separately_registered_aliases_cannot_unlock_two_writers() {
    let env = TestEnv::new();
    let real = create_test_vault(&env, "real", "alias-api-fixture");
    let alias = env.tempdir().join("linked.kdbx");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let first = AppSession::for_test(env.paths_clone()).unwrap();
    first
        .register_existing_vault("linked".into(), alias.to_str().unwrap().into(), None)
        .unwrap();
    first
        .register_existing_vault("real".into(), real.to_str().unwrap().into(), None)
        .unwrap();
    let second = AppSession::for_test(env.paths_clone()).unwrap();
    first
        .unlock("linked".into(), "alias-api-fixture".into(), None)
        .unwrap();

    assert!(matches!(
        second.unlock("real".into(), "alias-api-fixture".into(), None),
        Err(HidlinsApiError::VaultContended { .. })
    ));
    assert!(!second.has_vault());
    first.lock_now().unwrap();
    second
        .unlock("real".into(), "alias-api-fixture".into(), None)
        .unwrap();
    first.shutdown();
    second.shutdown();
}

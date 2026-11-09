mod common;

use hidlins_api::api::session::AppSession;
use hidlins_api::dto::LocalSyncRoleDto;
use hidlins_api::error::HidlinsApiError;
use hidlins_sync::config::local::LocalSyncConfig;

// Before the additive API exists, exercise the currently shipped identity-free
// operation. Once implemented, Rust resolves these calls to the inherent method.
#[allow(dead_code)]
trait SelectedVaultRotation {
    fn change_vault_master_password(
        &self,
        vault_name: String,
        current: String,
        new_password: String,
    ) -> Result<(), HidlinsApiError>;
}

impl SelectedVaultRotation for AppSession {
    fn change_vault_master_password(
        &self,
        _vault_name: String,
        current: String,
        new_password: String,
    ) -> Result<(), HidlinsApiError> {
        self.change_master_password(current, new_password)
    }
}

fn two_vaults() -> (
    common::TestEnv,
    AppSession,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let env = common::TestEnv::new();
    let a = common::create_test_vault(&env, "a", "shared-old");
    let b = common::create_test_vault(&env, "b", "shared-old");
    common::register_vault(&env, "a", &a);
    common::register_vault(&env, "b", &b);
    let session = AppSession::for_test(env.paths_clone()).expect("session");
    session
        .unlock("a".into(), "shared-old".into(), None)
        .expect("unlock a");
    (env, session, a, b)
}

#[test]
fn another_selected_vault_cannot_rotate_the_unlocked_vault() {
    let (_env, session, a, b) = two_vaults();
    let before_a = std::fs::read(&a).expect("encrypted a");
    let before_b = std::fs::read(&b).expect("encrypted b");
    let result =
        session.change_vault_master_password("b".into(), "shared-old".into(), "new".into());
    assert!(
        matches!(result, Err(HidlinsApiError::InvalidInput { ref field, .. }) if field == "vault"),
        "selected b must not rotate a: {result:?}"
    );
    assert_eq!(std::fs::read(&a).expect("a"), before_a);
    assert_eq!(std::fs::read(&b).expect("b"), before_b);
    session.lock_now().expect("lock");
    session
        .unlock("a".into(), "shared-old".into(), None)
        .expect("a unchanged");
    session.lock_now().expect("lock");
    session
        .unlock("b".into(), "shared-old".into(), None)
        .expect("b unchanged");
}

#[test]
fn nonexistent_selected_vault_cannot_rotate_the_unlocked_vault() {
    let (_env, session, a, b) = two_vaults();
    let before_a = std::fs::read(&a).expect("a");
    let before_b = std::fs::read(&b).expect("b");
    let result =
        session.change_vault_master_password("missing".into(), "shared-old".into(), "new".into());
    assert!(
        matches!(result, Err(HidlinsApiError::InvalidInput { ref field, .. }) if field == "vault"),
        "unregistered target must be rejected: {result:?}"
    );
    assert_eq!(std::fs::read(a).expect("a"), before_a);
    assert_eq!(std::fs::read(b).expect("b"), before_b);
}

#[test]
fn matching_target_and_legacy_rotation_keep_the_same_sync_identity() {
    let (env, session, _a, b) = two_vaults();
    let before_b = std::fs::read(&b).expect("b");
    session
        .configure_local_sync(LocalSyncRoleDto::Server)
        .expect("configure");
    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let before = LocalSyncConfig::from_vault_entry(registry.get("a").expect("a")).expect("config");
    session
        .change_vault_master_password("a".into(), "shared-old".into(), "named-new".into())
        .expect("named rotation");
    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let after = LocalSyncConfig::from_vault_entry(registry.get("a").expect("a")).expect("config");
    assert_eq!(
        after.identity().public_key(),
        before.identity().public_key()
    );
    assert_ne!(
        after.identity().sealed_value(),
        before.identity().sealed_value()
    );
    after
        .identity()
        .unlock(
            "a",
            hidlins_sync::identity::SyncRole::Server,
            &common::master("named-new"),
        )
        .expect("new wrapping");
    session.lock_now().expect("lock");
    session
        .unlock("a".into(), "named-new".into(), None)
        .expect("new credentials");
    session
        .change_master_password("named-new".into(), "legacy-new".into())
        .expect("legacy remains supported");
    session.lock_now().expect("lock");
    session
        .unlock("a".into(), "legacy-new".into(), None)
        .expect("legacy credentials");
    assert_eq!(std::fs::read(b).expect("b"), before_b);
}

#[test]
fn matching_target_still_requires_the_current_password() {
    let (_env, session, a, b) = two_vaults();
    let before_a = std::fs::read(&a).expect("a");
    let before_b = std::fs::read(&b).expect("b");
    assert!(matches!(
        session.change_vault_master_password("a".into(), "wrong".into(), "new".into()),
        Err(HidlinsApiError::AuthenticationFailed)
    ));
    assert_eq!(std::fs::read(a).expect("a"), before_a);
    assert_eq!(std::fs::read(b).expect("b"), before_b);
}

#[test]
fn named_rotation_rejects_a_locked_session() {
    let (_env, session, a, b) = two_vaults();
    session.lock_now().expect("lock");
    let before_a = std::fs::read(&a).expect("a");
    let before_b = std::fs::read(&b).expect("b");
    assert!(matches!(
        session.change_vault_master_password("a".into(), "shared-old".into(), "new".into()),
        Err(HidlinsApiError::VaultLocked)
    ));
    assert_eq!(std::fs::read(a).expect("a"), before_a);
    assert_eq!(std::fs::read(b).expect("b"), before_b);
}

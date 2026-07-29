//! T1.9: bootstrap-from-remote + change-master-password flows.
//!
//! Tests for the two cross-cutting flows added by the independent review.
//! Bootstrap network-dependent tests (wrong-password-before-install, happy
//! path, rollback matrix) require `MinIO` and are deferred to T5.4's
//! real-bridge integration tests. This file covers the guard checks and
//! the change-master-password flow end-to-end.

mod common;

use std::sync::Arc;

use hidlins_api::api::session::AppSession;
use hidlins_api::dto::S3ConfigDto;
use hidlins_api::error::HidlinsApiError;
use hidlins_api::sync_port::SucceedingSyncEngine;

use common::{create_test_vault, register_vault, TestEnv};

fn s3_cfg(marker_secret: &str) -> S3ConfigDto {
    S3ConfigDto {
        bucket: "test-bucket".to_string(),
        key: "test.kdbx".to_string(),
        region: "us-east-1".to_string(),
        endpoint: None,
        path_style: false,
        access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
        secret_access_key: marker_secret.to_string(),
    }
}

fn session_with_sync_engine(env: &TestEnv) -> AppSession {
    AppSession::with_ports(
        env.paths_clone(),
        None,
        Arc::new(SucceedingSyncEngine(
            hidlins_sync::SyncOutcome::AlreadyInSync,
        )),
    )
    .expect("create session with sync engine")
}

// -----------------------------------------------------------------------
// Bootstrap guard tests (no network needed)
// -----------------------------------------------------------------------

#[test]
fn bootstrap_duplicate_name_rejected() {
    let env = TestEnv::new();
    let vault_path = create_test_vault(&env, "existing", "pass");
    register_vault(&env, "existing", &vault_path);

    let session = session_with_sync_engine(&env);
    session
        .unlock("existing".to_string(), "pass".to_string(), None)
        .expect("unlock");

    let result = session.bootstrap_vault_from_remote(
        "existing".to_string(),
        s3_cfg("secret"),
        "remote-pass".to_string(),
        None,
    );

    assert!(
        matches!(result, Err(HidlinsApiError::PathExists { .. })),
        "bootstrap must reject a name already registered; got: {result:?}"
    );
}

#[test]
fn bootstrap_duplicate_target_rejected() {
    let env = TestEnv::new();
    let vault_path = create_test_vault(&env, "first", "pass");
    register_vault(&env, "first", &vault_path);

    let session = session_with_sync_engine(&env);
    session
        .unlock("first".to_string(), "pass".to_string(), None)
        .expect("unlock");

    // Configure sync on "first" to create the duplicate target.
    session
        .configure_sync(s3_cfg("secret"))
        .expect("configure sync");

    // Attempt to bootstrap a second vault pointing at the same bucket+key.
    let result = session.bootstrap_vault_from_remote(
        "second".to_string(),
        s3_cfg("other-secret"),
        "remote-pass".to_string(),
        None,
    );

    assert!(
        matches!(result, Err(HidlinsApiError::SyncDuplicateTarget { .. })),
        "bootstrap must reject a duplicate sync target; got: {result:?}"
    );
}

// -----------------------------------------------------------------------
// Change master password
// -----------------------------------------------------------------------

#[test]
fn change_master_password_succeeds_and_new_password_unlocks() {
    let env = TestEnv::new();
    let vault_path = create_test_vault(&env, "chpw", "old-pass");
    register_vault(&env, "chpw", &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("create session");
    session
        .unlock("chpw".to_string(), "old-pass".to_string(), None)
        .expect("unlock");

    session
        .change_master_password("old-pass".to_string(), "new-pass".to_string())
        .expect("change password");

    // Lock and re-unlock with the new password.
    session.lock_now().expect("lock");
    let result = session.unlock("chpw".to_string(), "new-pass".to_string(), None);
    assert!(result.is_ok(), "new password must unlock; got: {result:?}");

    // Old password should fail.
    session.lock_now().expect("lock");
    let old_result = session.unlock("chpw".to_string(), "old-pass".to_string(), None);
    assert!(
        matches!(old_result, Err(HidlinsApiError::AuthenticationFailed)),
        "old password must fail; got: {old_result:?}"
    );
}

#[test]
fn change_master_password_blocked_during_sync() {
    use hidlins_api::sync_port::BlockingSyncEngine;
    use std::sync::Barrier;

    let env = TestEnv::new();
    let vault_path = create_test_vault(&env, "chpw-sync", "pass");
    register_vault(&env, "chpw-sync", &vault_path);

    let barrier = Arc::new(Barrier::new(2));
    let (engine, entered_rx) = BlockingSyncEngine::with_entry_signal(
        Arc::clone(&barrier),
        hidlins_sync::SyncOutcome::AlreadyInSync,
    );

    let session = Arc::new(session_with_sync_engine_custom(&env, Arc::new(engine)));
    session
        .unlock("chpw-sync".to_string(), "pass".to_string(), None)
        .expect("unlock");
    session
        .configure_sync(s3_cfg("secret"))
        .expect("configure sync");

    // Start sync on another thread — blocks on barrier.
    let session_clone = Arc::clone(&session);
    let sync_thread = std::thread::spawn(move || session_clone.sync_now());

    entered_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("sync engine must signal entry");

    // Attempt to change password while syncing.
    let result = session.change_master_password("pass".to_string(), "new-pass".to_string());
    assert!(
        matches!(result, Err(HidlinsApiError::VaultBusySyncing)),
        "change_master_password must be blocked during sync; got: {result:?}"
    );

    // Release the barrier so sync completes.
    barrier.wait();
    let _ = sync_thread.join();
}

fn session_with_sync_engine_custom(
    env: &TestEnv,
    engine: Arc<dyn hidlins_api::sync_port::SyncEnginePort>,
) -> AppSession {
    AppSession::with_ports(env.paths_clone(), None, engine).expect("create session")
}

#[test]
fn change_master_password_reencrypts_sync_credentials() {
    let env = TestEnv::new();
    let vault_path = create_test_vault(&env, "reencrypt", "old-pass");
    register_vault(&env, "reencrypt", &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("create session");
    session
        .unlock("reencrypt".to_string(), "old-pass".to_string(), None)
        .expect("unlock");

    let marker_secret = "MARKER-s3cret-K3Y-DO-NOT-LEAK";
    session
        .configure_sync(s3_cfg(marker_secret))
        .expect("configure sync");

    // Read encrypted credential before password change.
    let before = std::fs::read_to_string(env.paths().vaults_toml()).expect("read vaults.toml");
    assert!(
        !before.contains(marker_secret),
        "plaintext secret must not appear before password change"
    );
    assert!(
        before.contains("AKIAIOSFODNN7EXAMPLE"),
        "access_key_id must be present"
    );

    // Extract the encrypted value before the change.
    let encrypted_before = before
        .lines()
        .find(|l| l.contains("secret_access_key_encrypted"))
        .expect("encrypted key must exist before change")
        .to_string();

    // Change password.
    session
        .change_master_password("old-pass".to_string(), "new-pass".to_string())
        .expect("change password");

    // Read again — the encrypted value should have changed (re-encrypted
    // with new master) and plaintext still absent.
    let after = std::fs::read_to_string(env.paths().vaults_toml()).expect("read vaults.toml after");
    assert!(
        !after.contains(marker_secret),
        "plaintext secret must not appear after password change"
    );

    let encrypted_after = after
        .lines()
        .find(|l| l.contains("secret_access_key_encrypted"))
        .expect("encrypted key must exist after change")
        .to_string();

    assert_ne!(
        encrypted_before, encrypted_after,
        "encrypted credential must change after password change (re-encrypted with new master)"
    );
}

#[test]
fn change_master_password_credentials_replaced_before_registry_write() {
    let env = TestEnv::new();
    let vault_path = create_test_vault(&env, "cred-replace", "old-pass");
    register_vault(&env, "cred-replace", &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("create session");
    session
        .unlock("cred-replace".to_string(), "old-pass".to_string(), None)
        .expect("unlock");

    assert!(
        session.has_credentials(),
        "credentials must be retained after unlock (D-11)"
    );

    session
        .change_master_password("old-pass".to_string(), "new-pass".to_string())
        .expect("change password");

    // Credentials are still present (replaced, not dropped).
    assert!(
        session.has_credentials(),
        "credentials must still be present after password change"
    );

    // Lock and verify new password works (proves credentials were replaced).
    session.lock_now().expect("lock");
    let result = session.unlock("cred-replace".to_string(), "new-pass".to_string(), None);
    assert!(
        result.is_ok(),
        "new password must work after credential replacement; got: {result:?}"
    );
}

#[test]
fn change_master_password_wrong_current_fails() {
    let env = TestEnv::new();
    let vault_path = create_test_vault(&env, "wrong-curr", "correct-pass");
    register_vault(&env, "wrong-curr", &vault_path);

    let session = AppSession::for_test(env.paths_clone()).expect("create session");
    session
        .unlock("wrong-curr".to_string(), "correct-pass".to_string(), None)
        .expect("unlock");

    let result = session.change_master_password("wrong-pass".to_string(), "new-pass".to_string());
    assert!(
        matches!(result, Err(HidlinsApiError::AuthenticationFailed)),
        "wrong current password must fail; got: {result:?}"
    );

    // Vault should still be unlocked (no state corruption).
    assert!(
        session.has_vault(),
        "vault must still be unlocked after failed password change"
    );
}

#[test]
fn change_master_password_on_locked_vault_fails() {
    let env = TestEnv::new();
    let _vault_path = create_test_vault(&env, "locked-chpw", "pass");

    let session = AppSession::for_test(env.paths_clone()).expect("create session");
    // Do NOT unlock.

    let result = session.change_master_password("pass".to_string(), "new-pass".to_string());
    assert!(
        matches!(result, Err(HidlinsApiError::VaultLocked)),
        "change password on locked vault must fail; got: {result:?}"
    );
}

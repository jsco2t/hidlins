mod common;

use hidlins_api::api::session::AppSession;
use hidlins_api::dto::{KeyfileRef, LocalSyncRoleDto};
use hidlins_api::error::HidlinsApiError;
use hidlins_sync::config::local::LocalSyncConfig;

#[cfg(unix)]
use std::io::Write as _;
#[cfg(unix)]
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::time::{Duration, Instant};

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

#[cfg(unix)]
fn interrupt_rotation(env: &common::TestEnv, name: &str, phase: &str) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_api-test-driver"))
        .arg("change-password")
        .arg(env.paths().state_dir())
        .arg(name)
        .env("HIDLINS_ROTATION_FAULT_SIGNAL_DIR", env.tempdir())
        .env("HIDLINS_ROTATION_FAULT_PAUSE_PHASE", phase)
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn password-change helper");
    child
        .stdin
        .take()
        .expect("helper stdin")
        .write_all(b"old-pass\nnew-pass\n")
        .expect("write helper credentials");

    let signal = env.tempdir().join(phase);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !signal.exists() {
        assert!(
            Instant::now() < deadline,
            "helper did not pause at password-rotation phase {phase}"
        );
        std::thread::yield_now();
    }
    child.kill().expect("kill paused helper");
    child.wait().expect("wait for helper");
}

#[cfg(unix)]
fn rotation_artifacts(env: &common::TestEnv) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(env.paths().state_dir())
        .expect("read state directory")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with(".hidlins-password-rotation-")
            })
        })
        .collect()
}

#[cfg(unix)]
fn configured_rotation_fixture(
    name: &str,
) -> (common::TestEnv, hidlins_sync::identity::PublicIdentity) {
    let env = common::TestEnv::new();
    let path = common::create_test_vault(&env, name, "old-pass");
    common::register_vault(&env, name, &path);
    let session = AppSession::for_test(env.paths_clone()).expect("session");
    session
        .unlock(name.to_string(), "old-pass".to_string(), None)
        .expect("unlock");
    session
        .configure_local_sync(LocalSyncRoleDto::Server)
        .expect("configure");
    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let public_key = LocalSyncConfig::from_vault_entry(registry.get(name).expect("entry"))
        .expect("local config")
        .identity()
        .public_key();
    drop(session);
    (env, public_key)
}

#[cfg(unix)]
// LNS-IDENTITY-004
#[test]
fn interrupted_rotation_after_vault_commit_recovers_matching_identity() {
    let (env, public_before) = configured_rotation_fixture("recover");
    interrupt_rotation(&env, "recover", "vault_committed");

    // Constructing the next application session is the restart recovery
    // boundary. Both durable authorities must then accept the new password.
    let recovered = AppSession::for_test(env.paths_clone()).expect("restart session");
    recovered
        .unlock("recover".to_string(), "new-pass".to_string(), None)
        .expect("new password opens recovered vault");
    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let config = LocalSyncConfig::from_vault_entry(registry.get("recover").expect("entry"))
        .expect("local config");
    assert_eq!(config.identity().public_key(), public_before);
    config
        .identity()
        .unlock(
            "recover",
            hidlins_sync::identity::SyncRole::Server,
            &hidlins_core::MasterPassword::new("new-pass".to_string()),
        )
        .expect("new password unwraps recovered sync identity");
    assert!(rotation_artifacts(&env).is_empty());
}

#[cfg(unix)]
// LNS-IDENTITY-005
#[test]
fn every_durable_rotation_boundary_recovers_idempotently() {
    let boundaries = [
        ("stages_durable", false),
        ("prepared", true),
        ("vault_commit_started", true),
        ("vault_renamed", true),
        ("vault_committed", true),
        ("registry_renamed", true),
        ("registry_committed", true),
    ];
    for (index, (phase, expects_new)) in boundaries.into_iter().enumerate() {
        let name = format!("boundary-{index}");
        let (env, public_before) = configured_rotation_fixture(&name);
        interrupt_rotation(&env, &name, phase);

        AppSession::for_test(env.paths_clone()).expect("first restart recovery");
        AppSession::for_test(env.paths_clone()).expect("repeated recovery is a no-op");
        let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
        let entry = registry.get(&name).expect("entry");
        let config = LocalSyncConfig::from_vault_entry(entry).expect("local config");
        assert_eq!(
            config.identity().public_key(),
            public_before,
            "phase {phase}"
        );
        let expected_password = if expects_new { "new-pass" } else { "old-pass" };
        hidlins_core::Vault::open(
            &entry.path,
            &hidlins_core::MasterPassword::new(expected_password.to_string()),
            None,
        )
        .unwrap_or_else(|error| panic!("phase {phase} did not recover vault: {error:?}"));
        config
            .identity()
            .unlock(
                &name,
                hidlins_sync::identity::SyncRole::Server,
                &hidlins_core::MasterPassword::new(expected_password.to_string()),
            )
            .unwrap_or_else(|error| panic!("phase {phase} did not recover identity: {error:?}"));
        assert!(rotation_artifacts(&env).is_empty(), "phase {phase}");
    }
}

#[cfg(unix)]
// LNS-IDENTITY-006
#[test]
fn recovery_merges_without_losing_unrelated_registry_changes() {
    let (env, _) = configured_rotation_fixture("merge");
    interrupt_rotation(&env, "merge", "vault_committed");
    let unrelated_path = common::create_test_vault(&env, "unrelated", "other-pass");
    common::register_vault(&env, "unrelated", &unrelated_path);

    AppSession::for_test(env.paths_clone()).expect("restart recovery");
    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    assert!(registry.get("unrelated").is_some());
    let config = LocalSyncConfig::from_vault_entry(registry.get("merge").expect("entry"))
        .expect("local config");
    config
        .identity()
        .unlock(
            "merge",
            hidlins_sync::identity::SyncRole::Server,
            &hidlins_core::MasterPassword::new("new-pass".to_string()),
        )
        .expect("merged registry retains rewrapped identity");
}

// LNS-IDENTITY-007
#[test]
fn password_change_with_keyfile_path_rewraps_matching_identity() {
    let env = common::TestEnv::new();
    let (path, keyfile_path, _) =
        common::create_test_vault_with_keyfile(&env, "keyfile", "old-pass");
    common::register_vault(&env, "keyfile", &path);
    let keyfile = || KeyfileRef::Path(keyfile_path.to_string_lossy().into_owned());
    let session = AppSession::for_test(env.paths_clone()).expect("session");
    session
        .unlock(
            "keyfile".to_string(),
            "old-pass".to_string(),
            Some(keyfile()),
        )
        .expect("unlock with keyfile");
    session
        .configure_local_sync(LocalSyncRoleDto::Server)
        .expect("configure");
    session
        .change_master_password("old-pass".to_string(), "new-pass".to_string())
        .expect("rotate with retained keyfile");
    session.lock_now().expect("lock");
    session
        .unlock(
            "keyfile".to_string(),
            "new-pass".to_string(),
            Some(keyfile()),
        )
        .expect("new password and keyfile open vault");

    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let config = LocalSyncConfig::from_vault_entry(registry.get("keyfile").expect("entry"))
        .expect("local config");
    config
        .identity()
        .unlock(
            "keyfile",
            hidlins_sync::identity::SyncRole::Server,
            &hidlins_core::MasterPassword::new("new-pass".to_string()),
        )
        .expect("new password unwraps identity");
}

#[cfg(unix)]
// LNS-IDENTITY-008
#[test]
fn recovery_fails_closed_for_corrupt_marker_without_touching_old_pair() {
    use std::os::unix::fs::PermissionsExt as _;

    let (env, _) = configured_rotation_fixture("corrupt");
    interrupt_rotation(&env, "corrupt", "prepared");
    let artifacts = rotation_artifacts(&env);
    for artifact in &artifacts {
        let bytes = std::fs::read(artifact).expect("read transaction artifact");
        assert_eq!(
            std::fs::metadata(artifact)
                .expect("artifact metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "transaction artifacts stay owner-only"
        );
        assert!(!bytes
            .windows(b"old-pass".len())
            .any(|part| part == b"old-pass"));
        assert!(!bytes
            .windows(b"new-pass".len())
            .any(|part| part == b"new-pass"));
    }
    let marker = artifacts
        .into_iter()
        .find(|path| path.extension().is_some_and(|extension| extension == "txn"))
        .expect("transaction marker");
    let canary = "marker-secret-canary-must-not-escape";
    std::fs::write(&marker, format!("invalid = '{canary}'")).expect("corrupt marker");

    let error = AppSession::for_test(env.paths_clone()).expect_err("corrupt marker must fail");
    let error = format!("{error:?}");
    assert!(error.contains("marker is malformed"));
    assert!(!error.contains(canary));
    hidlins_core::Vault::open(
        &env.paths().state_dir().join("corrupt.kdbx"),
        &hidlins_core::MasterPassword::new("old-pass".to_string()),
        None,
    )
    .expect("old live vault remains intact");
}

#[cfg(unix)]
// LNS-IDENTITY-009
#[test]
fn recovery_fails_closed_when_committed_vault_loses_registry_stage() {
    let (env, _) = configured_rotation_fixture("missing");
    interrupt_rotation(&env, "missing", "vault_committed");
    let stage = rotation_artifacts(&env)
        .into_iter()
        .find(|path| path.to_string_lossy().ends_with(".registry.stage"))
        .expect("registry stage");
    std::fs::remove_file(stage).expect("remove stage to simulate damage");

    let error = AppSession::for_test(env.paths_clone()).expect_err("missing stage must fail");
    assert!(format!("{error:?}").contains("required password rotation stage is missing"));
    hidlins_core::Vault::open(
        &env.paths().state_dir().join("missing.kdbx"),
        &hidlins_core::MasterPassword::new("new-pass".to_string()),
        None,
    )
    .expect("committed live vault remains intact");
}

#[cfg(unix)]
// LNS-IDENTITY-011
#[test]
fn recovery_fails_closed_when_registry_stage_hash_is_corrupt() {
    let (env, _) = configured_rotation_fixture("corrupt-stage");
    interrupt_rotation(&env, "corrupt-stage", "vault_committed");
    let stage = rotation_artifacts(&env)
        .into_iter()
        .find(|path| path.to_string_lossy().ends_with(".registry.stage"))
        .expect("registry stage");
    std::fs::write(stage, b"valid-looking = 'but unauthenticated'")
        .expect("corrupt staged registry");

    let error = AppSession::for_test(env.paths_clone()).expect_err("corrupt stage must fail");
    assert!(format!("{error:?}").contains("password rotation stage hash mismatch"));
    hidlins_core::Vault::open(
        &env.paths().state_dir().join("corrupt-stage.kdbx"),
        &hidlins_core::MasterPassword::new("new-pass".to_string()),
        None,
    )
    .expect("committed live vault remains intact");
}

#[cfg(unix)]
// LNS-IDENTITY-010
#[test]
fn stage_write_failure_changes_neither_live_authority() {
    use std::os::unix::fs::PermissionsExt as _;

    let (env, _) = configured_rotation_fixture("stage-failure");
    let original_mode = std::fs::metadata(env.paths().state_dir())
        .expect("state metadata")
        .permissions()
        .mode();
    std::fs::set_permissions(
        env.paths().state_dir(),
        std::fs::Permissions::from_mode(0o500),
    )
    .expect("make state directory read-only");
    let session = AppSession::for_test(env.paths_clone()).expect("session");
    session
        .unlock("stage-failure".to_string(), "old-pass".to_string(), None)
        .expect("unlock");
    let result = session.change_master_password("old-pass".to_string(), "new-pass".to_string());
    std::fs::set_permissions(
        env.paths().state_dir(),
        std::fs::Permissions::from_mode(original_mode),
    )
    .expect("restore state directory permissions");
    assert!(result.is_err(), "read-only stage creation must fail");
    drop(session);

    let registry = hidlins_core::VaultRegistry::load(env.paths_clone()).expect("registry");
    let entry = registry.get("stage-failure").expect("entry");
    hidlins_core::Vault::open(
        &entry.path,
        &hidlins_core::MasterPassword::new("old-pass".to_string()),
        None,
    )
    .expect("old password still opens vault");
    LocalSyncConfig::from_vault_entry(entry)
        .expect("local config")
        .identity()
        .unlock(
            "stage-failure",
            hidlins_sync::identity::SyncRole::Server,
            &hidlins_core::MasterPassword::new("old-pass".to_string()),
        )
        .expect("old password still unwraps identity");
    assert!(rotation_artifacts(&env).is_empty());
}

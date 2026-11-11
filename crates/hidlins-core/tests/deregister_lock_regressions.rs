mod common;

use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{
    NoRecoveryConfirmed, RegisteredVault, Vault, VaultError, VaultReadOnly, VaultRegistry,
};

fn register(env: &TestEnv, path: &Path) -> VaultRegistry {
    let mut registry = VaultRegistry::with_paths(env.paths().clone());
    registry
        .register(RegisteredVault {
            name: "fixture".into(),
            path: path.into(),
            created_at: "2026-11-02T00:00:00Z".into(),
            keyfile_path: None,
            extra: toml::Table::new(),
        })
        .unwrap();
    registry
}

fn create(path: &Path) -> Vault {
    Vault::create(
        path,
        &master("deletion-lock-fixture"),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap()
}

fn assert_delete_is_contended(registry: &mut VaultRegistry, path: &Path) {
    let before = std::fs::read(path).unwrap();
    assert!(matches!(
        registry.deregister("fixture", true),
        Err(VaultError::Contended { .. })
    ));
    assert!(registry.get("fixture").is_some());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

fn assert_delete_succeeds(registry: &mut VaultRegistry, path: &Path) {
    registry.deregister("fixture", true).unwrap();
    assert!(registry.get("fixture").is_none());
    assert!(!path.exists());
}

#[test]
fn live_writable_vault_prevents_deletion_until_released() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    let vault = create(&path);
    let mut registry = register(&env, &path);

    assert_delete_is_contended(&mut registry, &path);
    drop(vault);
    assert_delete_succeeds(&mut registry, &path);
}

#[test]
fn live_read_only_vault_prevents_deletion_until_released() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    drop(create(&path));
    let reader = VaultReadOnly::open(&path, &master("deletion-lock-fixture"), None).unwrap();
    let mut registry = register(&env, &path);

    assert_delete_is_contended(&mut registry, &path);
    drop(reader);
    assert_delete_succeeds(&mut registry, &path);
}

struct Holder(Child);

impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn other_process_prevents_deletion(mode: &str) {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    drop(create(&path));
    let ready = env.tempdir().join("held");
    let mut holder = Holder(
        Command::new(env!("CARGO_BIN_EXE_lock_holder"))
            .arg(mode)
            .arg(&path)
            .arg(&ready)
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.exists() {
        assert!(
            holder.0.try_wait().unwrap().is_none(),
            "holder exited early"
        );
        assert!(Instant::now() < deadline, "holder did not signal readiness");
        std::thread::sleep(Duration::from_millis(10));
    }

    let mut registry = register(&env, &path);
    assert_delete_is_contended(&mut registry, &path);
    drop(holder);
    assert_delete_succeeds(&mut registry, &path);
}

#[test]
fn another_process_exclusive_lock_prevents_deletion() {
    other_process_prevents_deletion("--exclusive");
}

#[test]
fn another_process_shared_lock_prevents_deletion() {
    other_process_prevents_deletion("--shared");
}

#[test]
fn unregister_only_preserves_a_live_vault() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    let _vault = create(&path);
    let before = std::fs::read(&path).unwrap();
    let mut registry = register(&env, &path);

    registry.deregister("fixture", false).unwrap();
    assert!(registry.get("fixture").is_none());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn already_missing_vault_with_a_missing_parent_is_still_deregistered() {
    let env = TestEnv::new();
    let path = env.tempdir().join("absent/vault.kdbx");
    let mut registry = register(&env, &path);

    registry.deregister("fixture", true).unwrap();
    assert!(registry.get("fixture").is_none());
    assert!(!path.parent().unwrap().exists());
}

#[test]
fn dangling_registered_symlink_is_removed() {
    let env = TestEnv::new();
    let path = env.tempdir().join("alias.kdbx");
    std::os::unix::fs::symlink(env.tempdir().join("absent.kdbx"), &path).unwrap();
    let mut registry = register(&env, &path);

    registry.deregister("fixture", true).unwrap();
    assert!(std::fs::symlink_metadata(path).is_err());
    assert!(registry.get("fixture").is_none());
}

#[test]
fn failed_unlink_keeps_the_registration() {
    let env = TestEnv::new();
    let path = env.tempdir().join("directory.kdbx");
    std::fs::create_dir(&path).unwrap();
    let mut registry = register(&env, &path);

    assert!(matches!(
        registry.deregister("fixture", true),
        Err(VaultError::Io { .. })
    ));
    assert!(registry.get("fixture").is_some());
    assert!(path.is_dir());
}

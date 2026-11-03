mod common;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{
    fields, HidlinsPaths, NoRecoveryConfirmed, RegisteredVault, Vault, VaultError, VaultReadOnly,
    VaultRegistry,
};
use sha2::{Digest as _, Sha256};

const OLD: &str = "alias-old-fixture";
const NEW: &str = "alias-new-fixture";

fn create(path: &Path) -> Vault {
    Vault::create(
        path,
        &master(OLD),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap()
}

fn alias(env: &TestEnv, target: &Path) -> PathBuf {
    let alias = env.tempdir().join("alias.kdbx");
    std::os::unix::fs::symlink(target, &alias).unwrap();
    alias
}

fn record(path: &Path) -> RegisteredVault {
    RegisteredVault {
        name: "fixture".into(),
        path: path.into(),
        created_at: "2026-11-02T00:00:00Z".into(),
        keyfile_path: None,
        extra: toml::Table::new(),
    }
}

fn extra(state: &str) -> toml::Value {
    toml::Table::from_iter([("state".into(), state.into())]).into()
}

#[test]
fn real_path_and_file_alias_share_exclusive_ownership() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    let owner = create(&path);
    let alias = alias(&env, &path);
    assert!(matches!(
        Vault::open(&alias, &master(OLD), None),
        Err(VaultError::Contended { .. })
    ));
    drop(owner);

    let owner = Vault::open(&alias, &master(OLD), None).unwrap();
    assert_eq!(
        owner.path(),
        alias,
        "public logical path remains compatible"
    );
    assert!(matches!(
        Vault::open(&path, &master(OLD), None),
        Err(VaultError::Contended { .. })
    ));
}

#[test]
fn shared_alias_readers_block_a_real_path_writer() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    drop(create(&path));
    let alias = alias(&env, &path);
    let reader = VaultReadOnly::open(&alias, &master(OLD), None).unwrap();
    assert_eq!(reader.path(), alias);
    let second_reader = VaultReadOnly::open(&path, &master(OLD), None).unwrap();
    drop(second_reader);
    assert!(matches!(
        Vault::open(&path, &master(OLD), None),
        Err(VaultError::Contended { .. })
    ));
    drop(reader);
    Vault::open(&path, &master(OLD), None).unwrap();
}

#[test]
fn alias_save_updates_the_backing_file_and_keeps_ownership_after_rename() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    drop(create(&path));
    let alias = alias(&env, &path);
    let mut vault = Vault::open(&alias, &master(OLD), None).unwrap();
    vault.database_mut().root_mut().add_entry().edit(|entry| {
        entry.set_unprotected(fields::TITLE, "alias mutation");
    });
    vault.save().unwrap();

    assert!(alias.is_symlink(), "save must not replace a vault alias");
    assert!(matches!(
        Vault::open(&path, &master(OLD), None),
        Err(VaultError::Contended { .. })
    ));
    drop(vault);
    let vault = Vault::open(&path, &master(OLD), None).unwrap();
    assert!(vault
        .database()
        .root()
        .entry_by_name("alias mutation")
        .is_some());
}

#[test]
fn an_open_handle_does_not_follow_a_retargeted_alias_on_save() {
    let env = TestEnv::new();
    let first = env.tempdir().join("first.kdbx");
    let second = env.tempdir().join("second.kdbx");
    drop(create(&first));
    drop(create(&second));
    let second_before = std::fs::read(&second).unwrap();
    let alias = alias(&env, &first);
    let mut vault = Vault::open(&alias, &master(OLD), None).unwrap();
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&second, &alias).unwrap();
    vault.database_mut().root_mut().add_entry().edit(|entry| {
        entry.set_unprotected(fields::TITLE, "bound to first");
    });
    vault.save().unwrap();

    assert!(alias.is_symlink());
    assert_eq!(std::fs::read(&second).unwrap(), second_before);
    drop(vault);
    let first = Vault::open(&first, &master(OLD), None).unwrap();
    assert!(first
        .database()
        .root()
        .entry_by_name("bound to first")
        .is_some());
}

#[test]
fn direct_password_change_through_an_alias_updates_the_backing_file() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    drop(create(&path));
    let alias = alias(&env, &path);
    let mut vault = Vault::open(&alias, &master(OLD), None).unwrap();
    vault
        .change_master_password(&master(OLD), &master(NEW))
        .unwrap();
    assert!(alias.is_symlink());
    drop(vault);
    assert!(matches!(
        Vault::open(&path, &master(OLD), None),
        Err(VaultError::AuthenticationFailed)
    ));
    Vault::open(&path, &master(NEW), None).unwrap();
}

#[test]
fn coordinated_password_change_preserves_a_registered_file_alias() {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    drop(create(&path));
    let alias = alias(&env, &path);
    let paths = env.paths().clone();
    let mut registry = VaultRegistry::load(paths.clone()).unwrap();
    let mut registration = record(&alias);
    registration.extra.insert("sync".into(), extra("old"));
    registry.register_and_save(registration).unwrap();
    let mut vault = Vault::open(&alias, &master(OLD), None).unwrap();

    registry
        .change_master_password_and_extra(
            &mut vault,
            "fixture",
            &master(OLD),
            &master(NEW),
            "sync",
            &extra("old"),
            &extra("new"),
        )
        .unwrap();
    assert!(alias.is_symlink());
    drop(vault);
    Vault::open(&path, &master(NEW), None).unwrap();
    let reloaded = VaultRegistry::load(paths).unwrap();
    assert_eq!(reloaded.get("fixture").unwrap().path, alias);
    assert_eq!(
        reloaded.get("fixture").unwrap().extra.get("sync"),
        Some(&extra("new"))
    );
}

#[test]
fn directory_aliases_preserve_logical_vault_and_registry_paths() {
    let env = TestEnv::new();
    let real_directory = env.tempdir().join("real");
    std::fs::create_dir(&real_directory).unwrap();
    let directory_alias = env.tempdir().join("linked");
    std::os::unix::fs::symlink(&real_directory, &directory_alias).unwrap();
    let logical = directory_alias.join("vault.kdbx");
    let vault = create(&logical);
    assert_eq!(vault.path(), logical);
    assert!(matches!(
        Vault::open(&real_directory.join("vault.kdbx"), &master(OLD), None),
        Err(VaultError::Contended { .. })
    ));
    drop(vault);
    let paths = HidlinsPaths::with_registry_file(directory_alias.join("registry.toml"));
    let mut registry = VaultRegistry::load(paths.clone()).unwrap();
    registry.register_and_save(record(&logical)).unwrap();
    assert_eq!(registry.paths().vaults_toml(), paths.vaults_toml());
    assert!(directory_alias.is_symlink());
}

#[test]
fn registry_file_alias_is_preserved_and_updates_its_backing_file() {
    let env = TestEnv::new();
    let real_file = env.tempdir().join("actual-registry.toml");
    std::fs::write(&real_file, "version = 1\n").unwrap();
    let link = env.tempdir().join("registry-link.toml");
    std::os::unix::fs::symlink(&real_file, &link).unwrap();
    let paths = HidlinsPaths::with_registry_file(link.clone());
    let mut registry = VaultRegistry::load(paths).unwrap();
    registry
        .register_and_save(record(&env.tempdir().join("vault.kdbx")))
        .unwrap();

    assert!(link.is_symlink());
    let real = VaultRegistry::load(HidlinsPaths::with_registry_file(real_file)).unwrap();
    assert!(real.get("fixture").is_some());
    assert_eq!(registry.paths().vaults_toml(), link);
}

struct Holder(Child);
impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn legacy_hash(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut value = String::new();
    for byte in Sha256::digest(bytes) {
        write!(&mut value, "{byte:02x}").unwrap();
    }
    value
}

fn recovery_case(case: &str, legacy: bool) {
    const CHILD: &str = "HIDLINS_ALIAS_ROTATION_CHILD";
    const ROOT: &str = "HIDLINS_ALIAS_ROTATION_ROOT";
    if std::env::var(CHILD).as_deref() == Ok(case) {
        let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
        let paths = HidlinsPaths::with_registry_file(root.join("linked-state/registry.toml"));
        let mut registry = VaultRegistry::load(paths).unwrap();
        let mut vault = Vault::open(&root.join("vault.kdbx"), &master(OLD), None).unwrap();
        registry
            .change_master_password_and_extra(
                &mut vault,
                "fixture",
                &master(OLD),
                &master(NEW),
                "sync",
                &extra("old"),
                &extra("new"),
            )
            .unwrap();
        panic!("rotation should pause before returning");
    }

    let env = TestEnv::new();
    let root = env.tempdir();
    let real_state = root.join("real-state");
    std::fs::create_dir(&real_state).unwrap();
    std::os::unix::fs::symlink(&real_state, root.join("linked-state")).unwrap();
    let real_paths = HidlinsPaths::with_registry_file(real_state.join("registry.toml"));
    let linked_registry = root.join("linked-state/registry.toml");
    let vault_path = root.join("vault.kdbx");
    drop(create(&vault_path));
    let mut registry = VaultRegistry::load(real_paths.clone()).unwrap();
    let mut registration = record(&vault_path);
    registration.extra.insert("sync".into(), extra("old"));
    registry.register_and_save(registration).unwrap();
    let signals = root.join("signals");
    std::fs::create_dir(&signals).unwrap();
    let mut holder = Holder(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", case, "--nocapture"])
            .env(CHILD, case)
            .env(ROOT, root)
            .env("HIDLINS_ROTATION_FAULT_SIGNAL_DIR", &signals)
            .env("HIDLINS_ROTATION_FAULT_PAUSE_PHASE", "vault_renamed")
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !signals.join("vault_renamed").exists() {
        assert!(
            holder.0.try_wait().unwrap().is_none(),
            "rotation child exited before its pause"
        );
        assert!(Instant::now() < deadline, "rotation child did not pause");
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(holder);

    let recovery_paths = if legacy {
        // Pin the prior on-disk transaction naming convention, independently
        // of whichever convention the repaired writer uses. Markers contain
        // hashes and metadata, not artifact paths, so only names change.
        let old_base = format!(
            ".hidlins-password-rotation-{}-{}",
            &legacy_hash(linked_registry.as_os_str().as_encoded_bytes())[..16],
            &legacy_hash(b"fixture")[..16]
        );
        for entry in std::fs::read_dir(&real_state).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy();
            let suffix = if name.ends_with(".txn") {
                Some(".txn")
            } else if name.ends_with(".registry.stage") {
                Some(".registry.stage")
            } else {
                None
            };
            if let Some(suffix) = suffix {
                std::fs::rename(&path, real_state.join(format!("{old_base}{suffix}"))).unwrap();
            }
        }
        HidlinsPaths::with_registry_file(linked_registry)
    } else {
        real_paths.clone()
    };
    VaultRegistry::recover_password_rotations(&recovery_paths).unwrap();
    Vault::open(&vault_path, &master(NEW), None).unwrap();
    let registry = VaultRegistry::load(real_paths).unwrap();
    assert_eq!(
        registry.get("fixture").unwrap().extra.get("sync"),
        Some(&extra("new"))
    );
    assert!(std::fs::read_dir(real_state).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".hidlins-password-rotation-")
    }));
}

#[test]
fn recovery_finds_a_transaction_written_through_another_directory_alias() {
    recovery_case(
        "recovery_finds_a_transaction_written_through_another_directory_alias",
        false,
    );
}

#[test]
fn recovery_retains_the_prior_raw_path_transaction_naming_convention() {
    recovery_case(
        "recovery_retains_the_prior_raw_path_transaction_naming_convention",
        true,
    );
}

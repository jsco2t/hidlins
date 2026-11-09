mod common;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{
    fields, HidlinsPaths, NoRecoveryConfirmed, RegisteredVault, Vault, VaultRegistry,
};

const OLD: &str = "rotation-preflight-old-fixture";
const NEW: &str = "rotation-preflight-new-fixture";
const CHILD: &str = "HIDLINS_ROTATION_PREFLIGHT_CHILD";
const ROOT: &str = "HIDLINS_ROTATION_PREFLIGHT_ROOT";

fn extra(value: &str) -> toml::Value {
    toml::Table::from_iter([("state".into(), value.into())]).into()
}

struct RotationChild(Child);

impl Drop for RotationChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn artifact(directory: &Path, suffix: &str) -> PathBuf {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with(".hidlins-password-rotation-") && name.ends_with(suffix)
        })
        .expect("fixture transaction artifact")
}

fn fixture() -> (TestEnv, hidlins_core::Uuid) {
    let env = TestEnv::new();
    let path = env.tempdir().join("vault.kdbx");
    let mut vault = Vault::create(
        &path,
        &master(OLD),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    vault.database_mut().root_mut().add_entry().edit(|entry| {
        entry.set_unprotected(fields::TITLE, "kept fixture");
        entry.set_protected(fields::PASSWORD, "synthetic-entry-fixture");
    });
    vault.save().unwrap();
    let entry_id = vault
        .database()
        .root()
        .entry_by_name("kept fixture")
        .unwrap()
        .id()
        .uuid();
    drop(vault);
    let mut registry = VaultRegistry::load(env.paths().clone()).unwrap();
    registry
        .register_and_save(RegisteredVault {
            name: "fixture".into(),
            path,
            created_at: "2026-11-04T00:00:00Z".into(),
            keyfile_path: None,
            extra: toml::Table::from_iter([("sync".into(), extra("old"))]),
        })
        .unwrap();
    (env, entry_id)
}

fn interrupt_rotation(env: &TestEnv, case: &str, phase: &str) {
    let signals = env.tempdir().join("signals");
    std::fs::create_dir(&signals).unwrap();
    let mut child = RotationChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", case, "--nocapture"])
            .env(CHILD, case)
            .env(ROOT, env.tempdir())
            .env("HIDLINS_ROTATION_FAULT_SIGNAL_DIR", &signals)
            .env("HIDLINS_ROTATION_FAULT_PAUSE_PHASE", phase)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !signals.join(phase).exists() {
        assert!(child.0.try_wait().unwrap().is_none(), "child exited early");
        assert!(Instant::now() < deadline, "{phase}: deadline exceeded");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn preflight_case(case: &str, damage: &str) {
    if std::env::var(CHILD).as_deref() == Ok(case) {
        let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
        let paths = HidlinsPaths::with_state_dir(root.join("state"));
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
        panic!("rotation child must pause before live vault installation");
    }

    for phase in ["prepared", "vault_commit_started"] {
        let (env, entry_id) = fixture();
        let path = env.tempdir().join("vault.kdbx");
        interrupt_rotation(&env, case, phase);

        let marker = artifact(env.paths().state_dir(), ".txn");
        let stage = artifact(env.paths().state_dir(), ".registry.stage");
        match damage {
            "missing" => std::fs::remove_file(stage).unwrap(),
            "corrupt" => std::fs::write(stage, b"damaged registry fixture").unwrap(),
            "conflict" => VaultRegistry::load(env.paths().clone())
                .unwrap()
                .update_registered_extra("fixture", |values| {
                    values.insert("sync".into(), extra("unexpected"));
                })
                .unwrap(),
            _ => unreachable!(),
        }
        let original_vault = std::fs::read(&path).unwrap();
        let original_registry = std::fs::read(env.paths().vaults_toml()).unwrap();
        let original_marker = std::fs::read(&marker).unwrap();
        for _ in 0..2 {
            assert!(VaultRegistry::recover_password_rotations(env.paths()).is_err());
            assert!(
                std::fs::read(&path).unwrap() == original_vault,
                "{phase}/{damage}: failed preflight must preserve the live vault"
            );
            assert!(
                std::fs::read(env.paths().vaults_toml()).unwrap() == original_registry,
                "{phase}/{damage}: failed preflight must preserve the live registry"
            );
            assert!(
                std::fs::read(&marker).unwrap() == original_marker,
                "{phase}/{damage}: failed preflight must preserve the marker"
            );
        }
        let vault = Vault::open(&path, &master(OLD), None).unwrap();
        let root = vault.database().root();
        let entry = root.entry_by_name("kept fixture").unwrap();
        assert_eq!(entry.id().uuid(), entry_id);
        assert_eq!(entry.get(fields::PASSWORD), Some("synthetic-entry-fixture"));
    }
}

#[test]
fn missing_registry_stage_preserves_the_old_pair_on_failed_recovery() {
    preflight_case(
        "missing_registry_stage_preserves_the_old_pair_on_failed_recovery",
        "missing",
    );
}

#[test]
fn corrupt_registry_stage_preserves_the_old_pair_on_failed_recovery() {
    preflight_case(
        "corrupt_registry_stage_preserves_the_old_pair_on_failed_recovery",
        "corrupt",
    );
}

#[test]
fn conflicting_registry_value_preserves_the_old_pair_on_failed_recovery() {
    preflight_case(
        "conflicting_registry_value_preserves_the_old_pair_on_failed_recovery",
        "conflict",
    );
}

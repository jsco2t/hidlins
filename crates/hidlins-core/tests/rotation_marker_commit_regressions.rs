mod common;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{
    fields, NoRecoveryConfirmed, RegisteredVault, Vault, VaultError, VaultRegistry,
};
use sha2::{Digest as _, Sha256};

const OLD: &str = "marker-commit-old-fixture";
const NEW: &str = "marker-commit-new-fixture";
const CHILD: &str = "HIDLINS_MARKER_COMMIT_CHILD";
const ROOT: &str = "HIDLINS_MARKER_COMMIT_ROOT";

fn extra(value: &str) -> toml::Value {
    toml::Table::from_iter([("state".into(), value.into())]).into()
}

// Select the protocol's derived target for the existing atomic fault hook.
fn marker_path(registry: &Path) -> PathBuf {
    let registry = std::fs::canonicalize(registry).unwrap();
    let encode_hash = |bytes: &[u8]| {
        let mut encoded = String::new();
        for byte in Sha256::digest(bytes) {
            write!(&mut encoded, "{byte:02x}").unwrap();
        }
        encoded
    };
    let registry_hash = encode_hash(registry.as_os_str().as_encoded_bytes());
    let name_hash = encode_hash(b"fixture");
    registry.parent().unwrap().join(format!(
        ".hidlins-password-rotation-{}-{}.txn",
        &registry_hash[..16],
        &name_hash[..16]
    ))
}

fn marker_error_case(case: &str, phase: &str, installed: bool) {
    if std::env::var(CHILD).as_deref() == Ok(case) {
        let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
        let paths = hidlins_core::HidlinsPaths::with_state_dir(root.join("state"));
        let path = root.join("vault.kdbx");
        let mut registry = VaultRegistry::load(paths.clone()).unwrap();
        let mut vault = Vault::open(&path, &master(OLD), None).unwrap();
        let entry_id = vault
            .database()
            .root()
            .entry_by_name("kept fixture")
            .unwrap()
            .id()
            .uuid();
        assert!(matches!(
            registry.change_master_password_and_extra(
                &mut vault,
                "fixture",
                &master(OLD),
                &master(NEW),
                "sync",
                &extra("old"),
                &extra("new"),
            ),
            Err(VaultError::Io { .. })
        ));
        drop(vault);
        drop(registry);
        std::fs::remove_dir(root.join("signals").join(phase)).unwrap();

        VaultRegistry::recover_password_rotations(&paths)
            .expect("reported marker-write failure must leave recoverable transaction state");
        VaultRegistry::recover_password_rotations(&paths).unwrap();
        let vault = Vault::open(&path, &master(if installed { NEW } else { OLD }), None)
            .expect("vault and registry must recover to the same credential");
        let database_root = vault.database().root();
        let entry = database_root.entry_by_name("kept fixture").unwrap();
        assert_eq!(entry.id().uuid(), entry_id);
        assert_eq!(entry.get(fields::PASSWORD), Some("synthetic-entry-fixture"));
        let registry = VaultRegistry::load(paths.clone()).unwrap();
        assert_eq!(
            registry.get("fixture").unwrap().extra.get("sync"),
            Some(&extra(if installed { "new" } else { "old" }))
        );
        assert!(!marker_path(&paths.vaults_toml()).exists());
        return;
    }

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
    drop(registry);
    let signals = env.tempdir().join("signals");
    std::fs::create_dir_all(signals.join(phase)).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", case, "--nocapture"])
        .env(CHILD, case)
        .env(ROOT, env.tempdir())
        .env(
            "HIDLINS_ATOMIC_FAULT_TARGET",
            marker_path(&env.paths().vaults_toml()),
        )
        .env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", signals)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{case} child failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn failure_after_prepared_marker_rename_retains_stages_for_recovery() {
    marker_error_case(
        "failure_after_prepared_marker_rename_retains_stages_for_recovery",
        "rename_done",
        true,
    );
}

#[test]
fn failure_before_prepared_marker_rename_keeps_the_old_pair() {
    marker_error_case(
        "failure_before_prepared_marker_rename_keeps_the_old_pair",
        "temp_written",
        false,
    );
}

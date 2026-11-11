mod common;

use std::io::Write as _;
use std::path::Path;
use std::process::Command;

use common::{fast_kdf, master};
use hidlins_core::{
    atomic::{write_atomic, write_atomic_with},
    fields, HidlinsPaths, NoRecoveryConfirmed, RegisteredVault, Vault, VaultError, VaultRegistry,
};

fn in_isolated_directory(case: &str, body: impl FnOnce()) {
    const CHILD_CASE: &str = "HIDLINS_RELATIVE_REGRESSION_CHILD";
    if std::env::var(CHILD_CASE).as_deref() == Ok(case) {
        body();
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", case, "--nocapture"])
        .env(CHILD_CASE, case)
        .current_dir(directory.path())
        .output()
        .expect("run regression in a child with its own working directory");
    assert!(
        output.status.success(),
        "{case}: child failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn registered(path: &Path) -> RegisteredVault {
    RegisteredVault {
        name: "relative-fixture".into(),
        path: path.into(),
        created_at: "2026-11-02T00:00:00Z".into(),
        keyfile_path: None,
        extra: toml::Table::new(),
    }
}

#[test]
fn bare_buffered_write_creates_and_replaces_a_file() {
    in_isolated_directory("bare_buffered_write_creates_and_replaces_a_file", || {
        let path = Path::new("buffered.bin");
        write_atomic(path, b"first fixture").expect("write a bare filename");
        write_atomic(path, b"replacement fixture").expect("replace a bare filename");
        assert_eq!(std::fs::read(path).unwrap(), b"replacement fixture");
    });
}

#[test]
fn bare_streaming_write_preserves_original_bytes_on_writer_error() {
    in_isolated_directory(
        "bare_streaming_write_preserves_original_bytes_on_writer_error",
        || {
            let path = Path::new("streamed.bin");
            write_atomic_with(path, |file| {
                file.write_all(b"original fixture")
                    .map_err(|source| VaultError::Io {
                        source,
                        path: path.into(),
                    })
            })
            .expect("stream to a bare filename");
            let error = write_atomic_with(path, |_| Err(VaultError::AuthenticationFailed));
            assert!(matches!(error, Err(VaultError::AuthenticationFailed)));
            assert_eq!(std::fs::read(path).unwrap(), b"original fixture");
        },
    );
}

#[test]
fn bare_registry_path_saves_and_reloads_registered_vaults() {
    in_isolated_directory(
        "bare_registry_path_saves_and_reloads_registered_vaults",
        || {
            let paths = HidlinsPaths::with_registry_file("registry.toml".into());
            assert_eq!(paths.state_dir(), Path::new("."));
            let mut registry = VaultRegistry::load(paths.clone()).unwrap();
            registry
                .register(registered(Path::new("vault.kdbx")))
                .unwrap();
            registry
                .save()
                .expect("persist registry using its bare configured filename");

            let reloaded = VaultRegistry::load(paths).unwrap();
            assert_eq!(reloaded.list().count(), 1);
            assert_eq!(
                reloaded.get("relative-fixture").unwrap().path,
                Path::new("vault.kdbx")
            );
        },
    );
}

#[test]
fn bare_vault_path_creates_saves_and_reopens() {
    in_isolated_directory("bare_vault_path_creates_saves_and_reopens", || {
        let path = Path::new("vault.kdbx");
        let password = master("relative-vault-fixture");
        let mut vault = Vault::create(
            path,
            &password,
            None,
            fast_kdf(),
            NoRecoveryConfirmed::yes(),
        )
        .expect("create a bare filename vault");
        vault.database_mut().root_mut().add_entry().edit(|entry| {
            entry.set_unprotected(fields::TITLE, "relative fixture");
        });
        vault.save().expect("save a bare filename vault");
        drop(vault);

        let reopened = Vault::open(path, &password, None).expect("reopen a bare filename vault");
        assert!(reopened
            .database()
            .root()
            .entry_by_name("relative fixture")
            .is_some());
    });
}

#[test]
fn bare_password_rotation_installs_and_removes_sibling_stages() {
    in_isolated_directory(
        "bare_password_rotation_installs_and_removes_sibling_stages",
        || {
            let path = Path::new("vault.kdbx");
            let paths = HidlinsPaths::with_registry_file("registry.toml".into());
            let old = master("relative-old-fixture");
            let new = master("relative-new-fixture");
            let mut vault = Vault::create(path, &old, None, fast_kdf(), NoRecoveryConfirmed::yes())
                .expect("create rotation fixture");
            let mut record = registered(path);
            let previous: toml::Value =
                toml::Table::from_iter([("state".into(), "old".into())]).into();
            let replacement: toml::Value =
                toml::Table::from_iter([("state".into(), "new".into())]).into();
            record.extra.insert("sync".into(), previous.clone());
            let mut registry = VaultRegistry::load(paths.clone()).unwrap();
            registry
                .register_and_save(record)
                .expect("register rotation fixture");

            registry
                .change_master_password_and_extra(
                    &mut vault,
                    "relative-fixture",
                    &old,
                    &new,
                    "sync",
                    &previous,
                    &replacement,
                )
                .expect("install stages when bare targets and ./stages have equivalent parents");
            drop(vault);

            Vault::open(path, &new, None).expect("new credential opens rotated vault");
            let registry = VaultRegistry::load(paths.clone()).unwrap();
            assert_eq!(
                registry.get("relative-fixture").unwrap().extra.get("sync"),
                Some(&replacement)
            );
            VaultRegistry::recover_password_rotations(&paths)
                .expect("no incomplete transaction remains");
            assert!(std::fs::read_dir(".").unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".hidlins-password-rotation-")
            }));
        },
    );
}

#[test]
fn invalid_or_missing_parent_paths_still_fail_without_creating_a_target() {
    in_isolated_directory(
        "invalid_or_missing_parent_paths_still_fail_without_creating_a_target",
        || {
            assert!(
                matches!(write_atomic(Path::new(""), b"fixture"), Err(VaultError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::InvalidInput)
            );
            let missing = Path::new("absent/fixture.bin");
            assert!(write_atomic(missing, b"fixture").is_err());
            assert!(!missing.exists());
        },
    );
}

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::{fast_kdf, master};
use hidlins_core::{fields, NoRecoveryConfirmed, Vault, VaultError};

const OLD: &str = "password-commit-old-fixture";
const NEW: &str = "password-commit-new-fixture";

fn error_case(case: &str, phase: &str, new_committed: bool) {
    const CHILD: &str = "HIDLINS_PASSWORD_COMMIT_CHILD";
    const ROOT: &str = "HIDLINS_PASSWORD_COMMIT_ROOT";
    if std::env::var(CHILD).as_deref() == Ok(case) {
        let directory = PathBuf::from(std::env::var_os(ROOT).unwrap());
        let path = directory.join("vault.kdbx");
        let mut vault = Vault::open(&path, &master(OLD), None).unwrap();
        let (root_id, entry_id) = {
            let root = vault.database().root();
            (
                root.id().uuid(),
                root.entry_by_name("kept fixture").unwrap().id().uuid(),
            )
        };
        let expected = master(if new_committed { NEW } else { OLD });
        let assert_disk = || {
            let bytes = std::fs::read(&path).unwrap();
            let database = Vault::open_from_bytes(&bytes, &expected, None)
                .expect("disk retains the credential committed at the rename boundary");
            let root = database.root();
            assert_eq!(root.id().uuid(), root_id);
            let entry = root.entry_by_name("kept fixture").unwrap();
            assert_eq!(entry.id().uuid(), entry_id);
            assert_eq!(entry.get(fields::PASSWORD), Some("synthetic-entry-fixture"));
        };

        assert!(matches!(
            vault.change_master_password(&master(OLD), &master(NEW)),
            Err(VaultError::Io { .. })
        ));
        assert_disk();
        // Turn the injected failing signal directory into a writable signal
        // path before the next save, without mutating process-global env.
        std::fs::remove_dir(directory.join("signals").join(phase)).unwrap();
        vault
            .save()
            .expect("ordinary save after a reported password-change error");
        assert_disk();
        return;
    }

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.kdbx");
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
    let signals = directory.path().join("signals");
    std::fs::create_dir_all(signals.join(phase)).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", case, "--nocapture"])
        .env(CHILD, case)
        .env(ROOT, directory.path())
        .env("HIDLINS_ATOMIC_FAULT_TARGET", &path)
        .env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", &signals)
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
fn error_before_temp_fsync_keeps_the_old_key_on_followup_save() {
    error_case(
        "error_before_temp_fsync_keeps_the_old_key_on_followup_save",
        "temp_written",
        false,
    );
}

#[test]
fn error_before_rename_keeps_the_old_key_on_followup_save() {
    error_case(
        "error_before_rename_keeps_the_old_key_on_followup_save",
        "fsync_done",
        false,
    );
}

#[test]
fn error_after_rename_keeps_the_new_key_on_followup_save() {
    error_case(
        "error_after_rename_keeps_the_new_key_on_followup_save",
        "rename_done",
        true,
    );
}

#[test]
fn wrong_current_password_preserves_disk_and_the_live_key() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("vault.kdbx");
    let mut vault = Vault::create(
        &path,
        &master(OLD),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        vault.change_master_password(&master("wrong-fixture"), &master(NEW)),
        Err(VaultError::AuthenticationFailed)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    vault.save().unwrap();
    Vault::open_from_bytes(&std::fs::read(path).unwrap(), &master(OLD), None).unwrap();
}

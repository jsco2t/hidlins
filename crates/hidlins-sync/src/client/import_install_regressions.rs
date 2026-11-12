//! Competing installations must never replace another vault.

use std::cell::RefCell;

thread_local! {
    static BEFORE_INSTALL: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
}

pub(super) fn before_install() {
    let action = BEFORE_INSTALL.with(|hook| hook.borrow_mut().take());
    if let Some(action) = action {
        action();
    }
}

use std::{path::Path, rc::Rc, time::Instant};

use hidlins_core::{HidlinsPaths, KdfParams, NoRecoveryConfirmed, VaultError};

use super::*;

fn password() -> MasterPassword {
    MasterPassword::new("synthetic import race password".to_string())
}

fn create(path: &Path) -> Result<Vault, VaultError> {
    Vault::create(
        path,
        &password(),
        None,
        KdfParams {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
}

fn active_config() -> LocalSyncConfig {
    let mut config =
        LocalSyncConfig::create("imported", crate::identity::SyncRole::Client, &password())
            .unwrap();
    let server =
        crate::identity::PublicIdentity::new(NoiseKeypair::generate().unwrap().public_key());
    let now = Instant::now();
    let mut window = crate::pairing::PairingWindow::open_at(now);
    let sas = window.begin_candidate(server, [9; 32], now).unwrap();
    let confirmation = window.confirm(sas, now).unwrap();
    let transaction = confirmation
        .transaction(server, config.identity().public_key())
        .unwrap();
    config
        .prepare_server(
            &transaction,
            &confirmation,
            "Server".to_string(),
            1000,
            1180,
        )
        .unwrap();
    config.activate_server(&transaction, 1001).unwrap();
    config
}

fn source(directory: &Path) -> Vec<u8> {
    let path = directory.join("source.kdbx");
    drop(create(&path).unwrap());
    std::fs::read(path).unwrap()
}

fn registry(directory: &Path) -> VaultRegistry {
    VaultRegistry::with_paths(HidlinsPaths::with_state_dir(directory.join("state")))
}

#[test]
fn normal_creator_cannot_enter_while_import_owns_target() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = source(directory.path());
    let target = directory.path().join("imported.kdbx");
    let created = Rc::new(RefCell::new(None));
    let captured = Rc::clone(&created);
    let competing = target.clone();
    BEFORE_INSTALL.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            *captured.borrow_mut() = Some(create(&competing));
        }));
    });
    let mut registry = registry(directory.path());
    let result = import_paired_vault(
        &bytes,
        &target,
        "imported",
        &password(),
        None,
        &active_config(),
        &mut registry,
    );
    assert!(
        matches!(
            created.borrow_mut().take().unwrap(),
            Err(VaultError::Contended { .. })
        ),
        "import must hold the same exclusive file identity as normal vault creation"
    );
    result.unwrap();
    assert_eq!(std::fs::read(target).unwrap(), bytes);
    assert!(registry.get("imported").is_some());
}

#[test]
fn external_install_between_check_and_commit_is_never_replaced() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = source(directory.path());
    let other = directory.path().join("other.kdbx");
    drop(create(&other).unwrap());
    let competing_bytes = std::fs::read(other).unwrap();
    let target = directory.path().join("imported.kdbx");
    let captured = competing_bytes.clone();
    let competing = target.clone();
    BEFORE_INSTALL.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            std::fs::write(competing, captured).unwrap();
        }));
    });
    let mut registry = registry(directory.path());
    assert!(matches!(
        import_paired_vault(
            &bytes,
            &target,
            "imported",
            &password(),
            None,
            &active_config(),
            &mut registry
        ),
        Err(ImportError::Install)
    ));
    assert_eq!(std::fs::read(target).unwrap(), competing_bytes);
    assert!(registry.get("imported").is_none());
}

#[cfg(unix)]
#[test]
fn dangling_existing_link_is_not_an_available_install_target() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = source(directory.path());
    let target = directory.path().join("imported.kdbx");
    let missing = directory.path().join("missing.kdbx");
    std::os::unix::fs::symlink(&missing, &target).unwrap();
    let mut registry = registry(directory.path());
    assert!(matches!(
        import_paired_vault(
            &bytes,
            &target,
            "imported",
            &password(),
            None,
            &active_config(),
            &mut registry
        ),
        Err(ImportError::Install)
    ));
    assert_eq!(std::fs::read_link(target).unwrap(), missing);
    assert!(registry.get("imported").is_none());
}

#[test]
fn preexisting_complete_vault_is_rejected_without_change() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = source(directory.path());
    let target = directory.path().join("imported.kdbx");
    drop(create(&target).unwrap());
    let before = std::fs::read(&target).unwrap();
    let mut registry = registry(directory.path());
    assert!(matches!(
        import_paired_vault(
            &bytes,
            &target,
            "imported",
            &password(),
            None,
            &active_config(),
            &mut registry
        ),
        Err(ImportError::Install)
    ));
    assert_eq!(std::fs::read(target).unwrap(), before);
    assert!(registry.get("imported").is_none());
}

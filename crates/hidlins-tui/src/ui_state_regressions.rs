//! Persist real state files from independent stale UI sessions.

use std::path::PathBuf;

use hidlins_core::locking::acquire_exclusive;
use hidlins_core::{HidlinsPaths, Uuid};
use tempfile::TempDir;

use crate::config::{config_path, TuiConfig};

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}

fn fixture() -> (TempDir, HidlinsPaths, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let paths = HidlinsPaths::with_state_dir(dir.path().join("state"));
    paths.ensure_exists().unwrap();
    let path = config_path(&paths);
    let mut config = TuiConfig::default();
    config.set_vault_state("alpha", vec![id(1)], vec![id(2)]);
    config.save(&paths, &path).unwrap();
    (dir, paths, path)
}

fn load(path: &std::path::Path) -> TuiConfig {
    let (config, warning) = TuiConfig::load(path);
    assert!(warning.is_none());
    config
}

#[test]
fn stale_disjoint_vault_sessions_preserve_both_updates() {
    let (_dir, paths, path) = fixture();
    let mut first = load(&path);
    let mut second = load(&path);
    first.set_vault_state("alpha", vec![id(1), id(3)], vec![id(4)]);
    first.save(&paths, &path).unwrap();
    second.set_vault_state("beta", vec![id(5)], vec![id(6)]);
    second.save(&paths, &path).unwrap();
    let latest = load(&path);
    assert_eq!(latest.vault_state("alpha").unwrap().pinned, [id(1), id(3)]);
    assert_eq!(latest.vault_state("alpha").unwrap().recents, [id(4)]);
    assert_eq!(latest.vault_state("beta").unwrap().pinned, [id(5)]);
    assert_eq!(latest.vault_state("beta").unwrap().recents, [id(6)]);
}

#[test]
fn same_vault_independent_pin_and_recent_edits_preserve_each_other() {
    let (_dir, paths, path) = fixture();
    let mut pins = load(&path);
    let mut recents = load(&path);
    pins.set_vault_state("alpha", vec![id(1), id(3)], vec![id(2)]);
    pins.save(&paths, &path).unwrap();
    recents.set_vault_state("alpha", vec![id(1)], vec![id(4), id(2)]);
    recents.save(&paths, &path).unwrap();
    let latest = load(&path);
    assert_eq!(latest.vault_state("alpha").unwrap().pinned, [id(1), id(3)]);
    assert_eq!(latest.vault_state("alpha").unwrap().recents, [id(4), id(2)]);
}

#[test]
fn successful_pending_updates_are_not_replayed_on_later_vault_save() {
    let (_dir, paths, path) = fixture();
    let mut first = load(&path);
    first.set_vault_state("alpha", vec![id(3)], vec![id(2)]);
    first.save(&paths, &path).unwrap();
    let mut other = load(&path);
    other.set_vault_state("alpha", vec![id(4)], vec![id(2)]);
    other.save(&paths, &path).unwrap();
    first.set_vault_state("beta", vec![id(5)], Vec::new());
    first.save(&paths, &path).unwrap();
    let latest = load(&path);
    assert_eq!(latest.vault_state("alpha").unwrap().pinned, [id(4)]);
    assert_eq!(latest.vault_state("beta").unwrap().pinned, [id(5)]);
}

#[test]
fn held_advisory_lock_prevents_write_and_failed_update_can_retry() {
    let (_dir, paths, path) = fixture();
    let mut config = load(&path);
    config.set_vault_state("alpha", vec![id(3)], vec![id(4)]);
    let before = std::fs::read(&path).unwrap();
    let guard = acquire_exclusive(&path).unwrap();
    assert!(config.save(&paths, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    drop(guard);
    config.save(&paths, &path).unwrap();
    let latest = load(&path);
    assert_eq!(latest.vault_state("alpha").unwrap().pinned, [id(3)]);
    assert_eq!(latest.vault_state("alpha").unwrap().recents, [id(4)]);
}

#[test]
fn latest_unknown_fields_and_foreign_vault_survive_stale_save() {
    let (_dir, paths, path) = fixture();
    let mut stale = load(&path);
    std::fs::write(
        &path,
        format!(
            "version = 1\nfuture_top = 'retain'\n[vaults.alpha]\npinned = []\nrecents = []\nfuture_local = 42\n[vaults.beta]\npinned = ['{}']\nrecents = []\nfuture_foreign = true\n",
            id(8)
        ),
    )
    .unwrap();
    stale.set_vault_state("alpha", vec![id(3)], vec![id(2)]);
    stale.save(&paths, &path).unwrap();
    let document: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(document["future_top"].as_str(), Some("retain"));
    assert_eq!(
        document["vaults"]["alpha"]["future_local"].as_integer(),
        Some(42)
    );
    assert_eq!(
        document["vaults"]["beta"]["future_foreign"].as_bool(),
        Some(true)
    );
    let latest = load(&path);
    assert_eq!(latest.vault_state("alpha").unwrap().pinned, [id(3)]);
    assert!(latest.vault_state("alpha").unwrap().recents.is_empty());
    assert_eq!(latest.vault_state("beta").unwrap().pinned, [id(8)]);
}

#[test]
fn corrupt_latest_file_is_preserved_with_content_safe_error() {
    let (_dir, paths, path) = fixture();
    let mut config = load(&path);
    config.set_vault_state("alpha", vec![id(3)], vec![id(4)]);
    let corrupt = b"version = [ synthetic-private-state-sentinel";
    std::fs::write(&path, corrupt).unwrap();
    let error = config.save(&paths, &path).expect_err("refuse overwrite");
    assert!(!error
        .to_string()
        .contains("synthetic-private-state-sentinel"));
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
}

#[test]
fn explicit_pin_and_recent_removals_still_persist() {
    let (_dir, paths, path) = fixture();
    let mut config = load(&path);
    config.set_vault_state("alpha", Vec::new(), Vec::new());
    config.save(&paths, &path).unwrap();
    let latest = load(&path);
    let alpha = latest.vault_state("alpha").unwrap();
    assert!(alpha.pinned.is_empty());
    assert!(alpha.recents.is_empty());
}

//! Real shared readers must coexist with read-only CLI operations.
mod common;

use std::time::Instant;

use common::{run_with_stdin, seed_vault, VaultsToml};
use hidlins_core::{
    EntryBuilder, HidlinsPaths, MasterPassword, Vault, VaultReadOnly, VaultRegistry,
};
use hidlins_sync::{
    config::local::LocalSyncConfig,
    identity::{PublicIdentity, SyncRole},
    noise::NoiseKeypair,
    pairing::PairingWindow,
};

const PASSWORD: &str = "synthetic shared-reader master";
const ENTRY_PASSWORD: &str = "synthetic entry must remain hidden";

fn fixture() -> (VaultsToml, std::path::PathBuf, uuid::Uuid) {
    let registry = VaultsToml::new();
    let path = seed_vault(&registry, "v", PASSWORD);
    let master = MasterPassword::new(PASSWORD.to_owned());
    let mut vault = Vault::open(&path, &master, None).unwrap();
    let id = vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::credential("synthetic title")
                .password(ENTRY_PASSWORD)
                .build(),
        )
        .unwrap();
    vault.save().unwrap();
    drop(vault);
    (registry, path, id)
}

fn configure_active_client(registry: &VaultsToml, master: &MasterPassword) {
    let mut config = LocalSyncConfig::create("v", SyncRole::Client, master).unwrap();
    let server = PublicIdentity::new(NoiseKeypair::generate().unwrap().public_key());
    let now = Instant::now();
    let mut window = PairingWindow::open_at(now);
    let sas = window.begin_candidate(server, [3; 32], now).unwrap();
    let confirmation = window.confirm(sas, now).unwrap();
    let transaction = confirmation
        .transaction(server, config.identity().public_key())
        .unwrap();
    config
        .prepare_server(
            &transaction,
            &confirmation,
            "Synthetic server".into(),
            1000,
            1180,
        )
        .unwrap();
    config.activate_server(&transaction, 1001).unwrap();
    let mut stored = VaultRegistry::load(HidlinsPaths::with_registry_file(
        registry.vaults_toml.clone(),
    ))
    .unwrap();
    config.persist(&mut stored, "v").unwrap();
}

fn check_shared_queries(configured: bool) {
    let (registry, path, id) = fixture();
    let master = MasterPassword::new(PASSWORD.to_owned());
    if configured {
        configure_active_client(&registry, &master);
    }
    let before = std::fs::read(&path).unwrap();
    let _reader = VaultReadOnly::open(&path, &master, None).unwrap();
    let id = id.to_string();
    let commands = [
        vec![
            "--format", "json", "entry", "get", "--vault", "v", "--uuid", &id,
        ],
        vec!["--format", "json", "entry", "list", "--vault", "v"],
        vec![
            "--format",
            "json",
            "entry",
            "search",
            "--vault",
            "v",
            "synthetic",
        ],
    ];
    let results: Vec<_> = commands
        .iter()
        .map(|args| run_with_stdin(&registry, args, &format!("{PASSWORD}\n")))
        .collect();
    assert!(
        results.iter().all(|result| result.0 == 0),
        "all shared queries must succeed: {results:?}"
    );
    for (_, stdout, stderr) in results {
        assert!(!stdout.contains(ENTRY_PASSWORD));
        assert!(!stderr.contains(ENTRY_PASSWORD));
        assert!(serde_json::from_str::<serde_json::Value>(&stdout).is_ok());
        if configured {
            assert!(stderr.contains("pre-operation local sync failed"));
        }
    }
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn unconfigured_get_list_search_coexist_with_a_shared_reader() {
    check_shared_queries(false);
}

#[test]
fn configured_get_list_search_continue_locally_when_sync_write_lock_is_contended() {
    check_shared_queries(true);
}

#[test]
fn register_authenticates_without_excluding_a_shared_reader() {
    let (registry, path, _) = fixture();
    let master = MasterPassword::new(PASSWORD.to_owned());
    let before = std::fs::read(&path).unwrap();
    let _reader = VaultReadOnly::open(&path, &master, None).unwrap();
    let (code, _, stderr) = run_with_stdin(
        &registry,
        &[
            "vault",
            "register",
            "--id",
            "alias",
            "--path",
            path.to_str().unwrap(),
        ],
        &format!("{PASSWORD}\n"),
    );
    assert_eq!(code, 0, "registration authenticates read-only: {stderr}");
    let stored = VaultRegistry::load(HidlinsPaths::with_registry_file(
        registry.vaults_toml.clone(),
    ))
    .unwrap();
    assert_eq!(
        stored.get("alias").unwrap().path,
        std::fs::canonicalize(&path).unwrap()
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn mutation_still_requires_exclusive_access() {
    let (registry, path, id) = fixture();
    let master = MasterPassword::new(PASSWORD.to_owned());
    let before = std::fs::read(&path).unwrap();
    let _reader = VaultReadOnly::open(&path, &master, None).unwrap();
    let (code, _, _) = run_with_stdin(
        &registry,
        &[
            "entry",
            "edit",
            "--vault",
            "v",
            "--uuid",
            &id.to_string(),
            "--title",
            "must not commit",
        ],
        &format!("{PASSWORD}\n"),
    );
    assert_eq!(code, 2);
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn shared_access_never_bypasses_password_authentication() {
    let (registry, path, _) = fixture();
    let master = MasterPassword::new(PASSWORD.to_owned());
    let before = std::fs::read(&path).unwrap();
    let _reader = VaultReadOnly::open(&path, &master, None).unwrap();
    let (code, _, _) = run_with_stdin(
        &registry,
        &["entry", "list", "--vault", "v"],
        "wrong synthetic password\n",
    );
    assert_eq!(code, 2);
    assert_eq!(std::fs::read(path).unwrap(), before);
}

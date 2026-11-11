//! Persisted paths must survive a later invocation from another directory.
mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use common::{hidlins_cmd, run_cmd_with_stdin, VaultsToml};
use hidlins_core::{
    HidlinsPaths, KdfParams, Keyfile, MasterPassword, NoRecoveryConfirmed, Vault, VaultRegistry,
};
use hidlins_sync::{
    client::import_paired_vault,
    config::local::LocalSyncConfig,
    identity::{PublicIdentity, SyncRole},
    noise::NoiseKeypair,
    pairing::PairingWindow,
};

const PASSWORD: &str = "synthetic registration master";

fn open_elsewhere(registry: &VaultsToml, directory: &Path, id: &str) {
    let mut command = hidlins_cmd();
    command.current_dir(directory).args([
        "--registry",
        &registry.registry_arg(),
        "vault",
        "open",
        "--id",
        id,
    ]);
    let (code, _, stderr) = run_cmd_with_stdin(&mut command, &format!("{PASSWORD}\n"));
    assert_eq!(
        code, 0,
        "registered vault must reopen from another directory: {stderr}"
    );
}

fn check_creation(relative_vault: bool, relative_keyfile: bool) {
    let registry = VaultsToml::new();
    let origin = registry.tempdir.path().join("origin");
    let elsewhere = registry.tempdir.path().join("elsewhere");
    std::fs::create_dir(&origin).unwrap();
    std::fs::create_dir(&elsewhere).unwrap();
    let vault_path = origin.join("created.kdbx");
    let keyfile_path = origin.join("factor.key");
    std::fs::write(&keyfile_path, b"synthetic registration keyfile").unwrap();
    let vault_argument = if relative_vault {
        Path::new("created.kdbx")
    } else {
        &vault_path
    };
    let keyfile_argument = if relative_keyfile {
        Path::new("factor.key")
    } else {
        &keyfile_path
    };
    let mut command = hidlins_cmd();
    command.current_dir(&origin).args([
        "--registry",
        &registry.registry_arg(),
        "--format",
        "json",
        "vault",
        "create",
        "--id",
        "created",
        "--path",
        vault_argument.to_str().unwrap(),
        "--keyfile",
        keyfile_argument.to_str().unwrap(),
        "--no-recovery-warning",
    ]);
    let (code, stdout, stderr) =
        run_cmd_with_stdin(&mut command, &format!("{PASSWORD}\n{PASSWORD}\n"));
    assert_eq!(code, 0, "creation must succeed: {stderr}");
    let view: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        view["path"],
        vault_argument.to_str().unwrap(),
        "preserve the existing create output contract"
    );
    assert_eq!(view["keyfile"], keyfile_argument.to_str().unwrap());
    open_elsewhere(&registry, &elsewhere, "created");
    let stored = VaultRegistry::load(HidlinsPaths::with_registry_file(
        registry.vaults_toml.clone(),
    ))
    .unwrap();
    let record = stored.get("created").unwrap();
    assert_eq!(record.path, std::fs::canonicalize(vault_path).unwrap());
    assert_eq!(
        record.keyfile_path.as_ref().unwrap(),
        &std::fs::canonicalize(keyfile_path).unwrap()
    );
}

#[test]
fn relative_created_vault_path_survives_another_working_directory() {
    check_creation(true, false);
}

#[test]
fn relative_created_keyfile_path_survives_another_working_directory() {
    check_creation(false, true);
}

#[test]
fn absolute_creation_paths_remain_compatible() {
    check_creation(false, false);
}

fn active_config(master: &MasterPassword) -> LocalSyncConfig {
    let mut config = LocalSyncConfig::create("imported", SyncRole::Client, master).unwrap();
    let server = PublicIdentity::new(NoiseKeypair::generate().unwrap().public_key());
    let now = Instant::now();
    let mut window = PairingWindow::open_at(now);
    let sas = window.begin_candidate(server, [9; 32], now).unwrap();
    let confirmation = window.confirm(sas, now).unwrap();
    let transaction = confirmation
        .transaction(server, config.identity().public_key())
        .unwrap();
    config
        .prepare_server(
            &transaction,
            &confirmation,
            "Synthetic server".to_owned(),
            1000,
            1180,
        )
        .unwrap();
    config.activate_server(&transaction, 1001).unwrap();
    config
}

#[test]
#[ignore = "owned child fixture; parent explicitly executes it with its isolated paths"]
fn relative_import_worker() {
    let Some(root) = std::env::var_os("HIDLINS_PATH_FIXTURE_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let master = MasterPassword::new(PASSWORD.to_owned());
    let keyfile = Keyfile::Path("factor.key".into());
    let source = root.join("source.kdbx");
    drop(
        Vault::create(
            &source,
            &master,
            Some(&keyfile),
            KdfParams {
                memory_kib: 1024,
                iterations: 1,
                parallelism: 1,
            },
            NoRecoveryConfirmed::yes(),
        )
        .unwrap(),
    );
    let mut registry =
        VaultRegistry::load(HidlinsPaths::with_registry_file(root.join("vaults.toml"))).unwrap();
    import_paired_vault(
        &std::fs::read(source).unwrap(),
        &root.join("imported.kdbx"),
        "imported",
        &master,
        Some(&keyfile),
        &active_config(&master),
        &mut registry,
    )
    .unwrap();
}

#[test]
fn relative_import_keyfile_survives_another_working_directory() {
    let registry = VaultsToml::new();
    let origin = registry.tempdir.path().join("origin");
    let elsewhere = registry.tempdir.path().join("elsewhere");
    std::fs::create_dir(&origin).unwrap();
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::write(origin.join("factor.key"), b"synthetic import keyfile").unwrap();
    let worker = Command::new(std::env::current_exe().unwrap())
        .current_dir(&origin)
        .args([
            "--exact",
            "relative_import_worker",
            "--ignored",
            "--nocapture",
        ])
        .env("HIDLINS_PATH_FIXTURE_ROOT", registry.tempdir.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        worker.status.success(),
        "import fixture failed: {}",
        String::from_utf8_lossy(&worker.stderr)
    );
    open_elsewhere(&registry, &elsewhere, "imported");
    let stored = VaultRegistry::load(HidlinsPaths::with_registry_file(
        registry.vaults_toml.clone(),
    ))
    .unwrap();
    assert_eq!(
        stored
            .get("imported")
            .unwrap()
            .keyfile_path
            .as_ref()
            .unwrap(),
        &std::fs::canonicalize(origin.join("factor.key")).unwrap()
    );
}

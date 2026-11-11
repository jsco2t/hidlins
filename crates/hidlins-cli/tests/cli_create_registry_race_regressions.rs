//! Independent registry commits while create waits for a password must survive.
mod common;

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use common::{hidlins_cmd, run_with_stdin, seed_vault, VaultsToml};
use hidlins_core::{HidlinsPaths, VaultRegistry};

const PASSWORD: &str = "synthetic creator master";

struct WaitingCreator {
    child: Child,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl WaitingCreator {
    fn start(registry: &VaultsToml, target: &Path) -> Self {
        let stdout = registry.tempdir.path().join("creator.stdout");
        let stderr = registry.tempdir.path().join("creator.stderr");
        let child = hidlins_cmd()
            .args([
                "--registry",
                &registry.registry_arg(),
                "--format",
                "json",
                "vault",
                "create",
                "--id",
                "created",
                "--path",
                target.to_str().unwrap(),
                "--no-recovery-warning",
            ])
            .stdin(Stdio::piped())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        let mut creator = Self {
            child,
            stdout,
            stderr,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let stderr = std::fs::read_to_string(&creator.stderr).unwrap();
            if stderr.contains("New master password: ") {
                return creator;
            }
            assert!(
                creator.child.try_wait().unwrap().is_none(),
                "creator exited before its prompt: {stderr}"
            );
            assert!(
                Instant::now() < deadline,
                "creator never reached its flushed prompt: {stderr}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn finish(&mut self) -> (i32, String, String) {
        let mut stdin = self.child.stdin.take().unwrap();
        stdin
            .write_all(format!("{PASSWORD}\n{PASSWORD}\n").as_bytes())
            .unwrap();
        drop(stdin);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return (
                    status.code().unwrap_or(-1),
                    std::fs::read_to_string(&self.stdout).unwrap(),
                    std::fs::read_to_string(&self.stderr).unwrap(),
                );
            }
            assert!(Instant::now() < deadline, "creator did not complete");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Drop for WaitingCreator {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn load(registry: &VaultsToml) -> VaultRegistry {
    VaultRegistry::load(HidlinsPaths::with_registry_file(
        registry.vaults_toml.clone(),
    ))
    .unwrap()
}

#[test]
fn concurrent_unrelated_registration_does_not_orphan_the_created_vault() {
    let registry = VaultsToml::new();
    let target = registry.tempdir.path().join("created.kdbx");
    let mut creator = WaitingCreator::start(&registry, &target);
    let other = seed_vault(&registry, "other", "synthetic other master");
    let other_before = std::fs::read(&other).unwrap();
    let (code, stdout, stderr) = creator.finish();
    assert_eq!(
        code, 0,
        "independent registration must not fail creation: {stderr}"
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stdout).unwrap()["id"],
        "created"
    );
    let stored = load(&registry);
    assert_eq!(stored.get("created").unwrap().path, target);
    assert_eq!(stored.get("other").unwrap().path, other);
    assert_eq!(std::fs::read(other).unwrap(), other_before);
    assert!(target.is_file());
    assert!(!stderr.contains(PASSWORD));
}

#[test]
fn concurrent_unrelated_settings_are_preserved_by_creation() {
    let registry = VaultsToml::new();
    let existing = seed_vault(&registry, "existing", "synthetic existing master");
    let before = std::fs::read(&existing).unwrap();
    let target = registry.tempdir.path().join("created.kdbx");
    let mut creator = WaitingCreator::start(&registry, &target);
    load(&registry)
        .update_registered_extra("existing", |extra| {
            extra.insert(
                "concurrent_marker".into(),
                toml::Value::String("retained".into()),
            );
        })
        .unwrap();
    let (code, _, stderr) = creator.finish();
    assert_eq!(
        code, 0,
        "independent metadata must not fail creation: {stderr}"
    );
    let stored = load(&registry);
    assert!(stored.get("created").is_some());
    assert_eq!(
        stored.get("existing").unwrap().extra["concurrent_marker"].as_str(),
        Some("retained")
    );
    assert_eq!(std::fs::read(existing).unwrap(), before);
}

#[test]
fn preexisting_duplicate_id_still_fails_without_creating_another_file() {
    let registry = VaultsToml::new();
    let existing = seed_vault(&registry, "created", PASSWORD);
    let before = std::fs::read(&existing).unwrap();
    let target = registry.tempdir.path().join("must-not-create.kdbx");
    let (code, _, stderr) = run_with_stdin(
        &registry,
        &[
            "vault",
            "create",
            "--id",
            "created",
            "--path",
            target.to_str().unwrap(),
            "--no-recovery-warning",
        ],
        "",
    );
    assert_eq!(code, 1);
    assert!(!stderr.contains("New master password:"));
    assert!(!target.exists());
    assert_eq!(std::fs::read(existing).unwrap(), before);
}

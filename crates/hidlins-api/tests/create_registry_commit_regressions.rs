mod common;

use std::path::PathBuf;
use std::process::Command;

use hidlins_api::api::session::AppSession;
use hidlins_api::dto::KeyfileRef;
use hidlins_api::error::HidlinsApiError;
use hidlins_core::{HidlinsPaths, Keyfile, Vault, VaultRegistry};

const CHILD: &str = "HIDLINS_CREATE_COMMIT_CHILD";
const ROOT: &str = "HIDLINS_CREATE_COMMIT_ROOT";
const MASTER: &str = "synthetic-create-commit-master";
const OTHER_MASTER: &str = "synthetic-unrelated-master";

fn exercise_child(phase: Option<&str>, installed: bool, keyed: bool) {
    let root = PathBuf::from(std::env::var_os(ROOT).expect("owned fixture root"));
    let paths = HidlinsPaths::with_state_dir(root.join("state"));
    let unrelated = paths.state_dir().join("unrelated.kdbx");
    let unrelated_bytes = std::fs::read(&unrelated).expect("original encrypted unrelated vault");
    let registry_bytes = std::fs::read(paths.vaults_toml()).expect("original registry");
    let candidate = paths.state_dir().join("candidate.kdbx");
    let keyfile = keyed.then(|| root.join("candidate.key"));
    let reference = || {
        keyfile
            .as_ref()
            .map(|path| KeyfileRef::Path(path.to_string_lossy().into_owned()))
    };
    let session = AppSession::for_test(paths.clone()).expect("isolated API session");
    let result = session.create_vault("candidate".into(), MASTER.into(), None, reference(), true);
    if let Some(phase) = phase {
        let signal = root.join("signals").join(phase);
        assert!(
            matches!(&result, Err(HidlinsApiError::Io { context })
                if context.contains(signal.to_string_lossy().as_ref())),
            "creation must reach the selected registry I/O fault, not an earlier fixture error"
        );
        assert!(
            signal.is_dir(),
            "the injected phase directory remains present"
        );
        if phase == "rename_done" {
            assert!(root.join("signals/temp_written").is_file());
            assert!(root.join("signals/fsync_done").is_file());
        }
    } else {
        assert!(result.is_ok(), "ordinary API creation must succeed");
    }

    let persisted = VaultRegistry::load(paths.clone()).expect("read installed registry");
    let current = session.list_vaults().expect("current API registry view");
    let current_candidate = current.iter().find(|entry| entry.name == "candidate");
    if installed {
        let record = persisted
            .get("candidate")
            .expect("committed registration remains installed");
        assert_eq!(record.path, candidate);
        assert!(
            candidate.is_file(),
            "an installed registration must retain its KDBX"
        );
        assert_eq!(record.keyfile_path, keyfile);
        let summary = current_candidate.expect("live API retains committed ownership");
        assert_eq!(summary.path, candidate.to_string_lossy());
        assert_eq!(summary.has_keyfile, keyed);
        {
            let core_keyfile = keyfile.as_ref().map(|path| Keyfile::Path(path.clone()));
            let _vault = Vault::open(&candidate, &common::master(MASTER), core_keyfile.as_ref())
                .expect("retained committed KDBX remains decryptable");
        }
        session
            .unlock("candidate".into(), MASTER.into(), reference())
            .expect("current session can use the retained committed vault");
        session.lock_now().expect("release fixture vault");
    } else {
        assert!(persisted.get("candidate").is_none());
        assert!(current_candidate.is_none());
        assert!(!candidate.exists(), "uncommitted new file is cleaned up");
        assert_eq!(
            std::fs::read(paths.vaults_toml()).expect("registry"),
            registry_bytes
        );
    }
    assert_eq!(
        persisted
            .get("unrelated")
            .expect("unrelated registration")
            .path,
        unrelated
    );
    assert_eq!(
        std::fs::read(&unrelated).expect("unrelated vault"),
        unrelated_bytes
    );
    let _other = Vault::open(&unrelated, &common::master(OTHER_MASTER), None)
        .expect("unrelated vault remains decryptable");
}

fn run_case(case: &str, phase: Option<&str>, installed: bool, keyed: bool) {
    if std::env::var(CHILD).as_deref() == Ok(case) {
        exercise_child(phase, installed, keyed);
        return;
    }

    let env = common::TestEnv::new();
    let unrelated = common::create_test_vault(&env, "unrelated", OTHER_MASTER);
    common::register_vault(&env, "unrelated", &unrelated);
    if keyed {
        std::fs::write(env.tempdir().join("candidate.key"), [0x42; 32])
            .expect("write synthetic keyfile fixture");
    }
    let signals = env.tempdir().join("signals");
    if let Some(phase) = phase {
        std::fs::create_dir_all(signals.join(phase)).expect("install exact atomic fault");
    }
    let mut child = Command::new(std::env::current_exe().expect("current test executable"));
    child
        .args(["--exact", case, "--nocapture"])
        .env(CHILD, case)
        .env(ROOT, env.tempdir())
        .env_remove("HIDLINS_ATOMIC_FAULT_TARGET")
        .env_remove("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR")
        .env_remove("HIDLINS_ATOMIC_FAULT_PAUSE_PHASE");
    if phase.is_some() {
        child
            .env("HIDLINS_ATOMIC_FAULT_TARGET", env.paths().vaults_toml())
            .env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", signals);
    }
    let output = child.output().expect("run isolated API/atomic boundary");
    assert!(
        output.status.success(),
        "{case} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn creation_keeps_the_vault_after_registry_rename_reports_error() {
    run_case(
        "creation_keeps_the_vault_after_registry_rename_reports_error",
        Some("rename_done"),
        true,
        false,
    );
}

#[test]
fn keyed_creation_keeps_the_vault_after_registry_rename_reports_error() {
    run_case(
        "keyed_creation_keeps_the_vault_after_registry_rename_reports_error",
        Some("rename_done"),
        true,
        true,
    );
}

#[test]
fn creation_cleans_only_uncommitted_file_on_pre_rename_error() {
    run_case(
        "creation_cleans_only_uncommitted_file_on_pre_rename_error",
        Some("temp_written"),
        false,
        false,
    );
}

#[test]
fn ordinary_creation_keeps_live_and_persisted_ownership_in_agreement() {
    run_case(
        "ordinary_creation_keeps_live_and_persisted_ownership_in_agreement",
        None,
        true,
        false,
    );
}

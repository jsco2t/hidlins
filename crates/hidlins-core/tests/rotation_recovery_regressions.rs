mod common;

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{
    fields, locking::acquire_exclusive, HidlinsPaths, NoRecoveryConfirmed, RegisteredVault, Vault,
    VaultError, VaultRegistry,
};

const OLD: &str = "recovery-old-fixture";
const NEW: &str = "recovery-new-fixture";
const CASE: &str = "HIDLINS_ROTATION_REGRESSION_CASE";
const ROLE: &str = "HIDLINS_ROTATION_REGRESSION_ROLE";
const ROOT: &str = "HIDLINS_ROTATION_REGRESSION_ROOT";

fn extra(value: &str) -> toml::Value {
    toml::Table::from_iter([("state".into(), value.into())]).into()
}

fn selected_child(case: &str) -> bool {
    if std::env::var(CASE).as_deref() != Ok(case) {
        return false;
    }
    let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
    let paths = HidlinsPaths::with_state_dir(root.join("state"));
    if std::env::var(ROLE).as_deref() == Ok("recover") {
        VaultRegistry::recover_password_rotations(&paths).unwrap();
        assert_pair(&root, &paths, true);
        return true;
    }
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
    panic!("rotation child must pause before it returns");
}

fn fixture() -> TestEnv {
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
        entry.set_unprotected(fields::TITLE, "recovery fixture");
    });
    vault.save().unwrap();
    drop(vault);
    let mut registry = VaultRegistry::load(env.paths().clone()).unwrap();
    let mut record = RegisteredVault {
        name: "fixture".into(),
        path,
        created_at: "2026-11-02T00:00:00Z".into(),
        keyfile_path: None,
        extra: toml::Table::new(),
    };
    record.extra.insert("sync".into(), extra("old"));
    registry.register_and_save(record).unwrap();
    std::fs::create_dir(env.tempdir().join("signals")).unwrap();
    env
}

struct Holder(Child);
impl Holder {
    fn wait_until(&mut self, label: &str, mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !condition() {
            assert!(
                self.0.try_wait().unwrap().is_none(),
                "{label}: child exited early"
            );
            assert!(Instant::now() < deadline, "{label}: deadline exceeded");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn assert_success(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success(), "recovery child failed: {status}");
                return;
            }
            assert!(
                Instant::now() < deadline,
                "recovery child deadline exceeded"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Holder {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn launch(env: &TestEnv, case: &str, phase: &str) -> Holder {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", case, "--nocapture"])
        .env(CASE, case)
        .env(ROLE, "writer")
        .env(ROOT, env.tempdir())
        .env(
            "HIDLINS_ROTATION_FAULT_SIGNAL_DIR",
            env.tempdir().join("signals"),
        )
        .env("HIDLINS_ROTATION_FAULT_PAUSE_PHASE", phase)
        .stdout(Stdio::null());
    Holder(command.spawn().unwrap())
}

fn pause(env: &TestEnv, case: &str, phase: &str) -> Holder {
    let mut owner = launch(env, case, phase);
    owner.wait_until("rotation phase", || {
        env.tempdir().join("signals").join(phase).exists()
    });
    owner
}

fn stages(env: &TestEnv) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files = Vec::new();
    for directory in [env.tempdir(), env.paths().state_dir()] {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy();
            if name.starts_with(".hidlins-password-rotation-") && name.ends_with(".stage") {
                let bytes = std::fs::read(&path).unwrap();
                files.push((path, bytes));
            }
        }
    }
    files.sort();
    files
}

fn assert_pair(root: &Path, paths: &HidlinsPaths, new: bool) {
    let vault = Vault::open(
        &root.join("vault.kdbx"),
        &master(if new { NEW } else { OLD }),
        None,
    )
    .expect("vault and sealed registry metadata use the same committed credential");
    assert!(vault
        .database()
        .root()
        .entry_by_name("recovery fixture")
        .is_some());
    let registry = VaultRegistry::load(paths.clone()).unwrap();
    assert_eq!(
        registry.get("fixture").unwrap().extra.get("sync"),
        Some(&extra(if new { "new" } else { "old" }))
    );
}

#[test]
fn live_unmarked_stages_survive_recovery_then_clean_after_crash() {
    let case = "live_unmarked_stages_survive_recovery_then_clean_after_crash";
    if selected_child(case) {
        return;
    }
    let env = fixture();
    let owner = pause(&env, case, "stages_durable");
    let before = stages(&env);
    assert_eq!(before.len(), 2);
    assert!(matches!(
        VaultRegistry::recover_password_rotations(env.paths()),
        Err(VaultError::Contended { .. })
    ));
    assert_eq!(
        stages(&env),
        before,
        "live stages must remain byte-identical"
    );
    drop(owner);
    VaultRegistry::recover_password_rotations(env.paths()).unwrap();
    assert!(stages(&env).is_empty());
    assert_pair(env.tempdir(), env.paths(), false);
}

#[test]
fn registry_contention_keeps_stages_and_releases_the_vault_lock() {
    let case = "registry_contention_keeps_stages_and_releases_the_vault_lock";
    if selected_child(case) {
        return;
    }
    let env = fixture();
    drop(pause(&env, case, "stages_durable"));
    let before = stages(&env);
    assert_eq!(before.len(), 2);
    let registry_lock = acquire_exclusive(&env.paths().vaults_toml()).unwrap();
    assert!(matches!(
        VaultRegistry::recover_password_rotations(env.paths()),
        Err(VaultError::Contended { .. })
    ));
    assert_eq!(stages(&env), before);
    // Acquiring the second lock failed; recovery must release its first lock.
    Vault::open(&env.tempdir().join("vault.kdbx"), &master(OLD), None).unwrap();
    drop(registry_lock);
    VaultRegistry::recover_password_rotations(env.paths()).unwrap();
    assert!(stages(&env).is_empty());
    assert_pair(env.tempdir(), env.paths(), false);
}

#[test]
fn durable_markers_preserve_live_stages_then_recover_idempotently() {
    let case = "durable_markers_preserve_live_stages_then_recover_idempotently";
    if selected_child(case) {
        return;
    }
    let env = fixture();
    let owner = pause(&env, case, "prepared");
    let before = stages(&env);
    assert_eq!(before.len(), 2);
    assert!(matches!(
        VaultRegistry::recover_password_rotations(env.paths()),
        Err(VaultError::Contended { .. })
    ));
    assert_eq!(stages(&env), before);
    drop(owner);
    VaultRegistry::recover_password_rotations(env.paths()).unwrap();
    assert!(stages(&env).is_empty());
    assert_pair(env.tempdir(), env.paths(), true);
    let before = std::fs::read(env.tempdir().join("vault.kdbx")).unwrap();
    VaultRegistry::recover_password_rotations(env.paths()).unwrap();
    assert_eq!(
        std::fs::read(env.tempdir().join("vault.kdbx")).unwrap(),
        before
    );
    assert_pair(env.tempdir(), env.paths(), true);
}

#[allow(unsafe_code)]
fn fifo(path: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: name is a live NUL-terminated path with no interior NUL; mkfifo
    // only borrows it for this call. This is isolated test-only fixture I/O.
    let status = unsafe { libc::mkfifo(name.as_ptr(), 0o600) };
    assert_eq!(status, 0, "mkfifo: {}", std::io::Error::last_os_error());
}

#[test]
fn marker_appearing_after_directory_scan_is_recovered_in_the_same_call() {
    let case = "marker_appearing_after_directory_scan_is_recovered_in_the_same_call";
    if selected_child(case) {
        return;
    }
    let env = fixture();
    let stage_signal = env.tempdir().join("signals/stages_durable");
    fifo(&stage_signal);
    // The writer blocks opening this signal FIFO before it can create a
    // marker; observing two stages alone is enough to continue fixture setup.
    let mut owner = launch(&env, case, "prepared");
    owner.wait_until("both stages", || stages(&env).len() == 2);
    let registry_path = env.paths().vaults_toml();
    let original_registry = std::fs::read(&registry_path).unwrap();
    let saved_registry = env.tempdir().join("saved-registry.toml");
    std::fs::rename(&registry_path, &saved_registry).unwrap();
    fifo(&registry_path);

    let mut recovery = Holder(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", case, "--nocapture"])
            .env(CASE, case)
            .env(ROLE, "recover")
            .env(ROOT, env.tempdir())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut feed = None;
    // A nonblocking FIFO writer succeeds only once recovery has completed
    // its directory scan and reached the registry read. Hold back its bytes.
    recovery.wait_until("recovery registry read", || {
        match OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&registry_path)
        {
            Ok(file) => {
                feed = Some(file);
                true
            }
            Err(error) if error.raw_os_error() == Some(libc::ENXIO) => false,
            Err(error) => panic!("registry FIFO: {error}"),
        }
    });
    // Now let the live writer make its marker durable, then terminate it.
    let _signal_reader = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(stage_signal)
        .unwrap();
    owner.wait_until("prepared marker", || {
        env.tempdir().join("signals/prepared").exists()
    });
    drop(owner);
    // Restore the real registry pathname before releasing the blocked read.
    // Its FIFO descriptor still receives the exact old registry bytes.
    std::fs::remove_file(&registry_path).unwrap();
    std::fs::rename(saved_registry, &registry_path).unwrap();
    let mut feed: File = feed.unwrap();
    feed.write_all(&original_registry).unwrap();
    drop(feed);
    recovery.assert_success();
    assert!(stages(&env).is_empty());
    assert_pair(env.tempdir(), env.paths(), true);
}

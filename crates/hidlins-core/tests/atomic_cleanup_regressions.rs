//! Ownership-safe cleanup and real cross-process atomic-write recovery.

mod common;

use std::fs::{self, File};
use std::io::{self, Write as _};
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::{symlink, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{atomic, NoRecoveryConfirmed, Vault, VaultError};

const PREFIX: &str = ".hidlins-tmp-";
const MASTER: &str = "atomic-recovery-fixture";
const BEFORE: &[u8] = b"opaque original fixture bytes";
const AFTER: &[u8] = b"opaque replacement fixture bytes";

#[test]
fn unrelated_prefixed_regular_files_survive() {
    let env = TestEnv::new();
    let user_file = env.tempdir().join(".hidlins-tmp-user-keyfile");
    fs::write(&user_file, BEFORE).unwrap();
    atomic::write_atomic(&env.tempdir().join("target"), AFTER).unwrap();
    assert_eq!(fs::read(user_file).unwrap(), BEFORE);
}

#[test]
fn unrelated_prefixed_directories_survive() {
    let env = TestEnv::new();
    let user_dir = env.tempdir().join(".hidlins-tmp-user-directory");
    fs::create_dir(&user_dir).unwrap();
    fs::write(user_dir.join("important"), BEFORE).unwrap();
    atomic::write_atomic(&env.tempdir().join("target"), AFTER).unwrap();
    assert_eq!(fs::read(user_dir.join("important")).unwrap(), BEFORE);
}

#[test]
fn prefixed_symlinks_are_never_followed_or_removed() {
    let env = TestEnv::new();
    let victim = env.tempdir().join("victim");
    fs::create_dir(&victim).unwrap();
    fs::write(victim.join("data"), BEFORE).unwrap();
    for (name, destination) in [
        ("file", victim.join("data")),
        ("directory", victim.clone()),
        ("dangling", victim.join("absent")),
    ] {
        symlink(destination, env.tempdir().join(format!("{PREFIX}{name}"))).unwrap();
    }
    atomic::write_atomic(&env.tempdir().join("target"), AFTER).unwrap();
    for name in ["file", "directory", "dangling"] {
        assert!(
            fs::symlink_metadata(env.tempdir().join(format!("{PREFIX}{name}")))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }
    assert_eq!(fs::read(victim.join("data")).unwrap(), BEFORE);
}

#[test]
fn an_unrecognized_owner_marker_does_not_authorize_cleanup() {
    let env = TestEnv::new();
    let stage = env.tempdir().join(".hidlins-tmp-lookalike");
    fs::create_dir(&stage).unwrap();
    fs::set_permissions(&stage, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(stage.join("owner"), b"unrecognized version").unwrap();
    fs::write(stage.join("payload"), BEFORE).unwrap();
    atomic::write_atomic(&env.tempdir().join("target"), AFTER).unwrap();
    assert_eq!(
        fs::read(stage.join("owner")).unwrap(),
        b"unrecognized version"
    );
    assert_eq!(fs::read(stage.join("payload")).unwrap(), BEFORE);
}

#[test]
fn failing_streaming_write_preserves_a_prefixed_destination() {
    let env = TestEnv::new();
    let target = env.tempdir().join(".hidlins-tmp-legitimate-target");
    fs::write(&target, BEFORE).unwrap();
    let result = atomic::write_atomic_with(&target, |_| {
        Err(VaultError::Io {
            source: io::Error::other("deliberate fixture writer failure"),
            path: target.clone(),
        })
    });
    assert!(result.is_err());
    assert_eq!(fs::read(target).unwrap(), BEFORE);
}

#[test]
fn live_prefixed_vault_and_keyfile_survive_an_unrelated_save() {
    let env = TestEnv::new();
    let path = env.tempdir().join(".hidlins-tmp-personal.kdbx");
    let mut vault = Vault::create(
        &path,
        &master(MASTER),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let uuid = vault.root_group_uuid();
    vault.save().unwrap();
    let original = fs::read(&path).unwrap();
    let keyfile = env.tempdir().join(".hidlins-tmp-keyfile");
    fs::write(&keyfile, BEFORE).unwrap();
    atomic::write_atomic(&env.tempdir().join("registry-fixture"), AFTER).unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::read(&keyfile).unwrap(), BEFORE);
    drop(vault);
    assert_eq!(
        Vault::open(&path, &master(MASTER), None)
            .unwrap()
            .root_group_uuid(),
        uuid
    );
}

#[test]
fn a_live_writer_is_preserved_while_an_independent_writer_finishes() {
    let env = TestEnv::new();
    let target = env.tempdir().join("first");
    fs::write(&target, BEFORE).unwrap();
    let mut writer = spawn_writer(env.tempdir(), &target, "writer", Some("temp_written"));
    wait_for_writer(&env.tempdir().join("temp_written"), &mut writer);
    let original_stage = stage_bytes(env.tempdir());
    assert!(!original_stage.is_empty());
    atomic::write_atomic(&env.tempdir().join("second"), AFTER).unwrap();
    assert_eq!(stage_bytes(env.tempdir()), original_stage);
    assert_eq!(fs::read(&target).unwrap(), BEFORE);
    writer.kill();
    atomic::write_atomic(&target, AFTER).unwrap();
    assert!(stages(env.tempdir()).is_empty());
}

#[test]
fn a_live_payload_is_inside_a_private_owned_directory() {
    let env = TestEnv::new();
    let target = env.tempdir().join("target");
    let mut writer = spawn_writer(env.tempdir(), &target, "writer", Some("temp_written"));
    wait_for_writer(&env.tempdir().join("temp_written"), &mut writer);
    let entries = stages(env.tempdir());
    assert_eq!(entries.len(), 1);
    let stage = &entries[0];
    assert!(fs::symlink_metadata(stage).unwrap().is_dir());
    assert_eq!(
        stage.metadata().unwrap().permissions().mode() & 0o777,
        0o700
    );
    let children: Vec<_> = fs::read_dir(stage).unwrap().map(Result::unwrap).collect();
    assert_eq!(
        children.len(),
        2,
        "ownership marker and encrypted payload only"
    );
    for child in children {
        assert!(child.file_type().unwrap().is_file());
        assert_eq!(
            child.metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn killed_writers_recover_at_every_existing_fault_boundary() {
    for phase in ["temp_written", "fsync_done", "rename_done"] {
        let env = TestEnv::new();
        let target = env.tempdir().join("target");
        fs::write(&target, BEFORE).unwrap();
        let mut writer = spawn_writer(env.tempdir(), &target, "writer", Some(phase));
        wait_for_writer(&env.tempdir().join(phase), &mut writer);
        writer.kill();
        let expected = if phase == "rename_done" {
            AFTER
        } else {
            BEFORE
        };
        assert_eq!(fs::read(&target).unwrap(), expected, "phase {phase}");
        atomic::write_atomic(&target, AFTER).unwrap();
        assert_eq!(fs::read(&target).unwrap(), AFTER);
        assert!(
            stages(env.tempdir()).is_empty(),
            "recover owned orphan at {phase}"
        );
    }
}

#[test]
fn independent_writers_can_all_enter_their_streaming_closures() {
    let env = TestEnv::new();
    let mut writers = Vec::new();
    for index in 0..4 {
        let target = env.tempdir().join(format!("writer-{index}"));
        writers.push(spawn_writer(
            env.tempdir(),
            &target,
            "blocking-writer",
            None,
        ));
    }
    for (index, writer) in writers.iter_mut().enumerate() {
        wait_for_writer(&env.tempdir().join(format!("writer-{index}.ready")), writer);
    }
    fs::write(env.tempdir().join("release"), b"release").unwrap();
    for mut writer in writers {
        writer.wait_success();
    }
    for index in 0..4 {
        assert_eq!(
            fs::read(env.tempdir().join(format!("writer-{index}"))).unwrap(),
            AFTER
        );
    }
    assert!(stages(env.tempdir()).is_empty());
}

#[test]
fn parent_coordination_precedes_stage_creation() {
    let env = TestEnv::new();
    let parent_lock = ParentLock::new(env.tempdir());
    let target = env.tempdir().join("target");
    let mut writer = spawn_writer(env.tempdir(), &target, "blocking-writer", None);
    wait_for_writer(&env.tempdir().join("target.started"), &mut writer);
    std::thread::sleep(Duration::from_millis(250));
    assert!(!env.tempdir().join("target.ready").exists());
    assert!(
        stages(env.tempdir()).is_empty(),
        "no create-before-lock window"
    );
    drop(parent_lock);
    wait_for_writer(&env.tempdir().join("target.ready"), &mut writer);
    fs::write(env.tempdir().join("release"), b"release").unwrap();
    writer.wait_success();
}

#[test]
fn replacing_an_abandoned_stage_with_a_symlink_preserves_user_data() {
    let env = TestEnv::new();
    let target = env.tempdir().join("target");
    let mut writer = spawn_writer(env.tempdir(), &target, "writer", Some("temp_written"));
    wait_for_writer(&env.tempdir().join("temp_written"), &mut writer);
    writer.kill();
    let entries = stages(env.tempdir());
    assert_eq!(entries.len(), 1);
    assert!(entries[0].is_dir());
    fs::rename(&entries[0], env.tempdir().join("moved-owned-stage")).unwrap();
    let victim = env.tempdir().join("victim");
    fs::create_dir(&victim).unwrap();
    fs::write(victim.join("payload"), BEFORE).unwrap();
    fs::write(victim.join("owner"), b"unrecognized owner").unwrap();
    symlink(&victim, &entries[0]).unwrap();
    atomic::write_atomic(&target, AFTER).unwrap();
    assert!(fs::symlink_metadata(&entries[0])
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read(victim.join("payload")).unwrap(), BEFORE);
    assert_eq!(
        fs::read(victim.join("owner")).unwrap(),
        b"unrecognized owner"
    );
}

#[test]
fn subprocess_writer() {
    let Some(action) = std::env::var_os("HIDLINS_RECOVERY_TEST_ACTION") else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("HIDLINS_RECOVERY_TEST_ROOT").unwrap());
    let target = PathBuf::from(std::env::var_os("HIDLINS_RECOVERY_TEST_TARGET").unwrap());
    let name = target.file_name().unwrap().to_string_lossy();
    fs::write(root.join(format!("{name}.started")), b"started").unwrap();
    atomic::write_atomic_with(&target, |file| {
        if action == "blocking-writer" {
            fs::write(root.join(format!("{name}.ready")), b"ready").unwrap();
            wait_for(&root.join("release"));
        }
        file.write_all(AFTER).map_err(|source| VaultError::Io {
            source,
            path: target.clone(),
        })
    })
    .unwrap();
}

struct Writer(Child);

impl Writer {
    fn kill(&mut self) {
        self.0.kill().unwrap();
        self.0.wait().unwrap();
    }

    fn wait_success(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                assert!(status.success(), "writer failed: {status}");
                return;
            }
            assert!(Instant::now() < deadline, "writer did not finish");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_writer(root: &Path, target: &Path, action: &str, phase: Option<&str>) -> Writer {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "subprocess_writer", "--nocapture"])
        .env("HIDLINS_RECOVERY_TEST_ACTION", action)
        .env("HIDLINS_RECOVERY_TEST_ROOT", root)
        .env("HIDLINS_RECOVERY_TEST_TARGET", target)
        .stdout(Stdio::null());
    if let Some(phase) = phase {
        command
            .env("HIDLINS_ATOMIC_FAULT_TARGET", target)
            .env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", root)
            .env("HIDLINS_ATOMIC_FAULT_PAUSE_PHASE", phase);
    }
    Writer(command.spawn().unwrap())
}

fn wait_for_writer(path: &Path, writer: &mut Writer) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while !path.exists() {
        assert!(
            writer.0.try_wait().unwrap().is_none(),
            "writer exited before signaling {}",
            path.display()
        );
        assert!(
            Instant::now() < deadline,
            "writer did not signal {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "writer did not signal {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn stages(root: &Path) -> Vec<PathBuf> {
    let mut stages: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry
                .file_name()
                .as_encoded_bytes()
                .starts_with(PREFIX.as_bytes())
        })
        .map(|entry| entry.path())
        .collect();
    stages.sort();
    stages
}

fn stage_bytes(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut result = Vec::new();
    for path in stages(root) {
        if path.is_dir() {
            for child in fs::read_dir(&path).unwrap().map(Result::unwrap) {
                assert!(child.file_type().unwrap().is_file());
                result.push((child.path(), fs::read(child.path()).unwrap()));
            }
        } else {
            result.push((path.clone(), fs::read(path).unwrap()));
        }
    }
    result.sort();
    result
}

struct ParentLock(File);

impl ParentLock {
    fn new(path: &Path) -> Self {
        let file = File::open(path).unwrap();
        flock(&file, libc::LOCK_EX);
        Self(file)
    }
}

impl Drop for ParentLock {
    fn drop(&mut self) {
        flock(&self.0, libc::LOCK_UN);
    }
}

#[allow(unsafe_code)] // test-only borrowed directory fd; same native primitive as production
fn flock(file: &File, operation: libc::c_int) {
    // SAFETY: the borrowed descriptor remains live; flock retains no pointer
    // or ownership. This test must exercise the real Bionic syscall too.
    assert_eq!(unsafe { libc::flock(file.as_raw_fd(), operation) }, 0);
}

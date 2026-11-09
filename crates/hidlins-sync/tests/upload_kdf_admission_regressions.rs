//! Reject incompatible uploaded KDF metadata before evaluating the KDF.

use std::{
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use hidlins_core::{KdfParams, MasterPassword, NoRecoveryConfirmed, Vault};
use hidlins_sync::{
    backup::backup_path_for,
    protocol::RemoteVersion,
    server::{AuthoritativeVault, ServerError, StagedUpload},
};
use sha2::{Digest, Sha256};

fn password() -> MasterPassword {
    MasterPassword::new("synthetic KDF admission fixture".to_string())
}

fn change_iterations(bytes: &mut [u8], iterations: u64) {
    // Mutate the generated fixture's exact UInt64 variant-dictionary field.
    // This is an independent attack fixture, not a production KDBX parser.
    let field = [5, 1, 0, 0, 0, b'I', 8, 0, 0, 0];
    let matches: Vec<_> = bytes
        .windows(field.len())
        .enumerate()
        .filter_map(|(at, candidate)| (candidate == field).then_some(at))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "fixture must identify one iteration field"
    );
    let start = matches[0] + field.len();
    assert_eq!(&bytes[start..start + 8], &1_u64.to_le_bytes());
    bytes[start..start + 8].copy_from_slice(&iterations.to_le_bytes());
    let mut end = 12; // The generated fixture starts with the fixed version header.
    loop {
        let kind = bytes[end];
        let length = u32::from_le_bytes(bytes[end + 1..end + 5].try_into().unwrap());
        end += 5 + usize::try_from(length).unwrap();
        assert!(
            end <= bytes.len(),
            "generated outer-header field is complete"
        );
        if kind == 0 {
            break;
        }
    }
    assert!(start + 8 < end);
    let header_hash = Sha256::digest(&bytes[..end]);
    bytes[end..end + 32].copy_from_slice(&header_hash);
    // Header HMAC remains untouched: even without credentials, this public
    // metadata and its unkeyed digest can be changed by an admitted peer.
}

#[test]
#[ignore = "owned child-only resource-order probe; inert without fixture environment"]
fn kdf_worker() {
    let Some(directory) = std::env::var_os("HIDLINS_KDF_ADMISSION_FIXTURE") else {
        return;
    };
    let directory = Path::new(&directory);
    let case = std::env::var("HIDLINS_KDF_ADMISSION_CASE").unwrap();
    let path = directory.join("vault.kdbx");
    let master = password();
    let mut vault = Vault::create(
        &path,
        &master,
        None,
        KdfParams {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut bytes = before.clone();
    match case.as_str() {
        "huge-iterations" => change_iterations(&mut bytes, u64::from(u32::MAX)),
        "small-incompatible" => change_iterations(&mut bytes, 2),
        "malformed" => bytes[0] = 0,
        "compatible" => {}
        _ => panic!("unknown fixture case"),
    }
    let mut stage = StagedUpload::new(
        &path,
        Some(RemoteVersion::new(Sha256::digest(&before).into())),
        bytes.len() as u64,
        Sha256::digest(&bytes).into(),
    )
    .unwrap();
    stage.write_chunk(0, &bytes).unwrap();
    let stage = stage.finish(1).unwrap();
    let mut host = AuthoritativeVault::new(&mut vault, &master, None);
    std::fs::write(directory.join("ready"), b"validated encrypted stage ready").unwrap();
    let result = host.commit(&stage);
    if case == "compatible" {
        let committed = result.unwrap();
        assert_eq!(
            committed,
            RemoteVersion::new(Sha256::digest(std::fs::read(&path).unwrap()).into())
        );
        assert_eq!(std::fs::read(backup_path_for(&path)).unwrap(), before);
    } else {
        assert_eq!(result, Err(ServerError::InvalidUpload));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!backup_path_for(&path).exists());
    }
}

fn run_case(case: &str) {
    let directory = tempfile::tempdir().unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "kdf_worker", "--ignored", "--nocapture"])
        .env("HIDLINS_KDF_ADMISSION_FIXTURE", directory.path())
        .env("HIDLINS_KDF_ADMISSION_CASE", case)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let setup_deadline = Instant::now() + Duration::from_secs(10);
    let mut operation_deadline = None;
    let finished = loop {
        if operation_deadline.is_none() && directory.path().join("ready").exists() {
            operation_deadline = Some(Instant::now() + Duration::from_secs(2));
        }
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= operation_deadline.unwrap_or(setup_deadline) {
            break None;
        }
        thread::sleep(Duration::from_millis(5));
    };
    if finished.is_none() {
        child.kill().unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        directory.path().join("ready").exists(),
        "fixture failed before validation: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        finished.is_some(),
        "incompatible uploaded KDF was evaluated instead of rejected before derivation"
    );
    assert!(
        output.status.success(),
        "owned probe failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn incompatible_cost_is_rejected_before_expensive_derivation() {
    run_case("huge-iterations");
}

#[test]
fn incompatible_small_cost_is_rejected_without_mutation() {
    run_case("small-incompatible");
}

#[test]
fn malformed_upload_is_rejected_without_mutation() {
    run_case("malformed");
}

#[test]
fn compatible_authenticated_upload_remains_supported() {
    run_case("compatible");
}

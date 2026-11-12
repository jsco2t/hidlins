//! Observe actual inbound allocations before deallocation on early API exits.
mod common;

#[path = "../../hidlins-core/tests/common/allocation_observer.rs"]
mod allocation_observer;

use allocation_observer::DeallocationWatch;
use hidlins_api::api::session::AppSession;
use hidlins_api::dto::{KeyfileRef, LocalEndpointDto};
use hidlins_api::error::HidlinsApiError;

fn session() -> (common::TestEnv, AppSession) {
    let env = common::TestEnv::new();
    env.paths().ensure_exists().unwrap();
    let session = AppSession::for_test(env.paths_clone()).unwrap();
    (env, session)
}

fn key_bytes() -> Vec<u8> {
    b"inbound keyfile regression secret: initialized owned bytes".to_vec()
}

#[test]
fn create_invalid_name_erases_keyfile() {
    let (_env, session) = session();
    let bytes = key_bytes();
    let watch = DeallocationWatch::new(&bytes);
    assert!(matches!(
        session.create_vault("../invalid".into(), "master".into(), None, Some(KeyfileRef::Bytes(bytes)), true),
        Err(HidlinsApiError::InvalidInput { field, .. }) if field == "name"
    ));
    watch.assert_erased();
}

#[test]
fn create_unconfirmed_recovery_erases_keyfile() {
    let (_env, session) = session();
    let bytes = key_bytes();
    let watch = DeallocationWatch::new(&bytes);
    assert!(matches!(
        session.create_vault("valid".into(), "master".into(), None, Some(KeyfileRef::Bytes(bytes)), false),
        Err(HidlinsApiError::InvalidInput { field, .. }) if field == "confirmed_no_recovery"
    ));
    watch.assert_erased();
}

#[test]
fn create_invalid_name_preserves_existing_master_erasure() {
    let (_env, session) = session();
    let master = String::from("existing protected master regression control");
    let watch = DeallocationWatch::new(master.as_bytes());
    assert!(matches!(
        session.create_vault("../invalid".into(), master, None, None, true),
        Err(HidlinsApiError::InvalidInput { field, .. }) if field == "name"
    ));
    watch.assert_erased();
}

#[test]
fn register_missing_file_erases_keyfile() {
    let (env, session) = session();
    let bytes = key_bytes();
    let watch = DeallocationWatch::new(&bytes);
    assert!(matches!(
        session.register_existing_vault(
            "valid".into(),
            env.tempdir().join("missing.kdbx").display().to_string(),
            Some(KeyfileRef::Bytes(bytes))
        ),
        Err(HidlinsApiError::FileNotFound { .. })
    ));
    watch.assert_erased();
}

#[test]
fn successful_registration_erases_unused_keyfile_and_preserves_vault() {
    let (env, session) = session();
    let path = common::create_test_vault(&env, "source", "master");
    let before = std::fs::read(&path).unwrap();
    let bytes = key_bytes();
    let watch = DeallocationWatch::new(&bytes);
    let summary = session
        .register_existing_vault(
            "imported".into(),
            path.display().to_string(),
            Some(KeyfileRef::Bytes(bytes)),
        )
        .unwrap();
    assert_eq!(summary.name, "imported");
    assert!(
        !summary.has_keyfile,
        "byte keyfile is supplied again at unlock, never persisted"
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
    watch.assert_erased();
}

enum InvalidImport {
    Name,
    PublicEndpoint,
    NoCandidates,
}

fn import_rejection(case: &InvalidImport, watch_master: bool) {
    let (_env, session) = session();
    let master = String::from("inbound import master regression secret");
    let bytes = key_bytes();
    let watch = DeallocationWatch::new(if watch_master {
        master.as_bytes()
    } else {
        &bytes
    });
    let (name, candidates) = match case {
        InvalidImport::Name => ("../invalid", Vec::new()),
        InvalidImport::PublicEndpoint => (
            "valid",
            vec![LocalEndpointDto {
                address: "8.8.8.8".into(),
                port: 12345,
                scope_id: 0,
            }],
        ),
        InvalidImport::NoCandidates => ("valid", Vec::new()),
    };
    let result = session.begin_pair_import(
        name.into(),
        master,
        Some(KeyfileRef::Bytes(bytes)),
        candidates,
    );
    match case {
        InvalidImport::Name => assert!(
            matches!(result, Err(HidlinsApiError::InvalidInput { field, .. }) if field == "name")
        ),
        InvalidImport::PublicEndpoint => assert!(matches!(
            result,
            Err(HidlinsApiError::InvalidInput { field, .. }) if field == "address"
        )),
        InvalidImport::NoCandidates => {
            assert!(matches!(result, Err(HidlinsApiError::SyncNotFound)));
        }
    }
    watch.assert_erased();
}

#[test]
fn import_invalid_name_erases_master() {
    import_rejection(&InvalidImport::Name, true);
}
#[test]
fn import_invalid_name_erases_keyfile() {
    import_rejection(&InvalidImport::Name, false);
}
#[test]
fn import_public_endpoint_erases_master() {
    import_rejection(&InvalidImport::PublicEndpoint, true);
}
#[test]
fn import_public_endpoint_erases_keyfile() {
    import_rejection(&InvalidImport::PublicEndpoint, false);
}
#[test]
fn import_empty_candidates_erases_master() {
    import_rejection(&InvalidImport::NoCandidates, true);
}
#[test]
fn import_empty_candidates_erases_keyfile() {
    import_rejection(&InvalidImport::NoCandidates, false);
}

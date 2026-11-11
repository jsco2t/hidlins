mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use hidlins_api::api::session::AppSession;
use hidlins_api::dto::{CustomFieldInputDto, EntryDraftDto, EntryEditDto, EntryKindDto};
use hidlins_api::error::HidlinsApiError;
use hidlins_core::{fields, HidlinsPaths, Vault};

const OLD_EXPIRY: i64 = 1_700_000_123;
const NEW_EXPIRY: i64 = 1_800_000_456;
const PASSWORD: &str = "expiration-save-fixture";

// Exercise the existing split-save alternative before the additive atomic
// methods exist. Inherent methods take precedence once implemented.
#[allow(dead_code)]
trait ExpirationSave {
    fn create_entry_with_expiration(
        &self,
        group: String,
        draft: EntryDraftDto,
        expiry_time: Option<i64>,
    ) -> Result<String, HidlinsApiError>;
    fn update_entry_with_expiration(
        &self,
        uuid: String,
        edit: EntryEditDto,
        expiry_time: Option<i64>,
    ) -> Result<(), HidlinsApiError>;
}

impl ExpirationSave for AppSession {
    fn create_entry_with_expiration(
        &self,
        group: String,
        draft: EntryDraftDto,
        expiry_time: Option<i64>,
    ) -> Result<String, HidlinsApiError> {
        let uuid = self.create_entry(group, draft)?;
        if let Some(epoch) = expiry_time {
            self.set_expiration(uuid.clone(), epoch)?;
        }
        Ok(uuid)
    }

    fn update_entry_with_expiration(
        &self,
        uuid: String,
        edit: EntryEditDto,
        expiry_time: Option<i64>,
    ) -> Result<(), HidlinsApiError> {
        self.update_entry(uuid.clone(), edit)?;
        match expiry_time {
            Some(epoch) => self.set_expiration(uuid, epoch),
            None => self.clear_expiration(uuid),
        }
    }
}

fn draft(title: &str) -> EntryDraftDto {
    EntryDraftDto {
        kind: EntryKindDto::Credential,
        title: title.into(),
        username: Some("kept user".into()),
        password: Some("synthetic-entry-secret".into()),
        url: Some("https://example.test".into()),
        notes: Some("kept notes".into()),
        tags: vec!["kept-tag".into()],
        custom_fields: vec![CustomFieldInputDto {
            name: "kept field".into(),
            value: "synthetic-field-secret".into(),
            protected: true,
        }],
        totp_uri: None,
    }
}

fn edit() -> EntryEditDto {
    EntryEditDto {
        title: Some("after".into()),
        username: None,
        password: None,
        url: None,
        notes: Some("changed notes".into()),
        tags: None,
        custom_fields: None,
        totp_uri: None,
    }
}

fn fixture() -> (common::TestEnv, AppSession, PathBuf, String) {
    let env = common::TestEnv::new();
    let path = common::create_test_vault(&env, "test", PASSWORD);
    common::register_vault(&env, "test", &path);
    let session = AppSession::for_test(env.paths_clone()).unwrap();
    session
        .unlock("test".into(), PASSWORD.into(), None)
        .unwrap();
    let root = session.vault_tree().unwrap().root.uuid;
    let uuid = session.create_entry(root, draft("before")).unwrap();
    session.set_expiration(uuid.clone(), OLD_EXPIRY).unwrap();
    (env, session, path, uuid)
}

fn assert_unchanged(session: &AppSession, path: &Path, uuid: &str, bytes: &[u8]) {
    let detail = session.entry_detail(uuid.into()).unwrap();
    assert_eq!(detail.title, "before");
    assert_eq!(detail.notes, "kept notes");
    assert_eq!(detail.expiry_time, Some(OLD_EXPIRY));
    assert!(session.entry_history(uuid.into()).unwrap().is_empty());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn invalid_expiration_cannot_apply_any_part_of_an_edit() {
    let (_env, session, path, uuid) = fixture();
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        session.update_entry_with_expiration(uuid.clone(), edit(), Some(i64::MAX)),
        Err(HidlinsApiError::InvalidInput { ref field, .. }) if field == "epoch_secs"
    ));
    assert_unchanged(&session, &path, &uuid, &before);
}

#[test]
fn invalid_expiration_cannot_create_an_entry() {
    let (_env, session, path, _uuid) = fixture();
    let before = std::fs::read(&path).unwrap();
    let root = session.vault_tree().unwrap().root.uuid;
    assert!(matches!(
        session.create_entry_with_expiration(root, draft("after"), Some(i64::MAX)),
        Err(HidlinsApiError::InvalidInput { ref field, .. }) if field == "epoch_secs"
    ));
    assert_eq!(session.vault_tree().unwrap().entries.len(), 1);
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn invalid_tags_or_totp_cannot_change_fields_or_expiration() {
    let (_env, session, path, uuid) = fixture();
    let before = std::fs::read(&path).unwrap();
    let mut invalid_tags = edit();
    invalid_tags.tags = Some(vec!["invalid;tag".into()]);
    assert!(session
        .update_entry_with_expiration(uuid.clone(), invalid_tags, Some(NEW_EXPIRY))
        .is_err());
    assert_unchanged(&session, &path, &uuid, &before);
    let mut invalid_totp = edit();
    invalid_totp.totp_uri = Some("invalid-fixture-uri".into());
    assert!(session
        .update_entry_with_expiration(uuid.clone(), invalid_totp, Some(NEW_EXPIRY))
        .is_err());
    assert_unchanged(&session, &path, &uuid, &before);
}

#[test]
fn combined_edit_retains_identity_secrets_and_one_previous_snapshot() {
    let (_env, session, path, uuid) = fixture();
    session
        .update_entry_with_expiration(uuid.clone(), edit(), Some(NEW_EXPIRY))
        .unwrap();
    session.lock_now().unwrap();
    session
        .unlock("test".into(), PASSWORD.into(), None)
        .unwrap();
    let detail = session.entry_detail(uuid.clone()).unwrap();
    assert_eq!(detail.uuid, uuid);
    assert_eq!(detail.title, "after");
    assert_eq!(detail.notes, "changed notes");
    assert_eq!(detail.expiry_time, Some(NEW_EXPIRY));
    let db = Vault::open_from_bytes(
        &std::fs::read(path).unwrap(),
        &common::master(PASSWORD),
        None,
    )
    .unwrap();
    let root = db.root();
    let entry = root.entry_by_name("after").unwrap();
    assert_eq!(entry.id().uuid().to_string(), uuid);
    assert_eq!(entry.get(fields::PASSWORD), Some("synthetic-entry-secret"));
    assert_eq!(entry.get("kept field"), Some("synthetic-field-secret"));
    let history = entry.history.as_ref().unwrap();
    assert_eq!(history.get_entries().len(), 1);
    let prior = &history.get_entries()[0];
    assert_eq!(prior.get(fields::TITLE), Some("before"));
    assert_eq!(prior.get(fields::NOTES), Some("kept notes"));
    assert_eq!(
        prior.times.expiry.unwrap().and_utc().timestamp(),
        OLD_EXPIRY
    );
    assert_eq!(prior.times.expires, Some(true));
}

#[test]
fn clearing_expiration_preserves_raw_cutoff_and_legacy_edit_keeps_expiration() {
    let (_env, session, path, uuid) = fixture();
    session.update_entry(uuid.clone(), edit()).unwrap();
    assert_eq!(
        session.entry_detail(uuid.clone()).unwrap().expiry_time,
        Some(OLD_EXPIRY)
    );
    session
        .update_entry_with_expiration(uuid.clone(), edit(), None)
        .unwrap();
    assert_eq!(session.entry_detail(uuid).unwrap().expiry_time, None);
    session.lock_now().unwrap();
    let db = Vault::open_from_bytes(
        &std::fs::read(path).unwrap(),
        &common::master(PASSWORD),
        None,
    )
    .unwrap();
    let root = db.root();
    let entry = root.entry_by_name("after").unwrap();
    assert_eq!(entry.times.expires, Some(false));
    assert_eq!(
        entry.times.expiry.unwrap().and_utc().timestamp(),
        OLD_EXPIRY
    );
}

#[test]
fn combined_creation_round_trips_and_locked_calls_still_reject() {
    let (_env, session, path, uuid) = fixture();
    let root = session.vault_tree().unwrap().root.uuid;
    let created = session
        .create_entry_with_expiration(root.clone(), draft("after"), Some(NEW_EXPIRY))
        .unwrap();
    assert_eq!(
        session.entry_detail(created.clone()).unwrap().expiry_time,
        Some(NEW_EXPIRY)
    );
    assert!(session.entry_history(created.clone()).unwrap().is_empty());
    session.lock_now().unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(
        session.update_entry_with_expiration(uuid, edit(), None),
        Err(HidlinsApiError::VaultLocked)
    ));
    assert!(matches!(
        session.create_entry_with_expiration(root, draft("locked"), Some(NEW_EXPIRY)),
        Err(HidlinsApiError::VaultLocked)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
    session
        .unlock("test".into(), PASSWORD.into(), None)
        .unwrap();
    assert_eq!(
        session.entry_detail(created).unwrap().expiry_time,
        Some(NEW_EXPIRY)
    );
}

fn fault_case(case: &str, phase: &str, committed: bool, create: bool) {
    const CHILD: &str = "HIDLINS_EXPIRATION_SAVE_CHILD";
    const ROOT: &str = "HIDLINS_EXPIRATION_SAVE_STATE";
    if std::env::var(CHILD).as_deref() == Ok(case) {
        let state = PathBuf::from(std::env::var_os(ROOT).unwrap());
        let path = state.join("test.kdbx");
        let session = AppSession::for_test(HidlinsPaths::with_state_dir(state)).unwrap();
        session
            .unlock("test".into(), PASSWORD.into(), None)
            .unwrap();
        let tree = session.vault_tree().unwrap();
        let result = if create {
            session
                .create_entry_with_expiration(tree.root.uuid, draft("after"), Some(NEW_EXPIRY))
                .map(|_| ())
        } else {
            session.update_entry_with_expiration(
                tree.entries[0].uuid.clone(),
                edit(),
                Some(NEW_EXPIRY),
            )
        };
        assert!(
            result.is_err(),
            "signal directory injects actual save I/O failure"
        );
        let db = Vault::open_from_bytes(
            &std::fs::read(path).unwrap(),
            &common::master(PASSWORD),
            None,
        )
        .unwrap();
        let root = db.root();
        if committed {
            let entry = root
                .entry_by_name("after")
                .expect("new complete state committed");
            assert_eq!(entry.times.expires, Some(true));
            assert_eq!(
                entry.times.expiry.unwrap().and_utc().timestamp(),
                NEW_EXPIRY,
                "fields and expiration must commit together"
            );
            assert_eq!(entry.get(fields::PASSWORD), Some("synthetic-entry-secret"));
            assert_eq!(entry.get("kept field"), Some("synthetic-field-secret"));
            if create {
                assert!(root.entry_by_name("before").is_some());
            } else {
                assert!(root.entry_by_name("before").is_none());
                assert_eq!(entry.history.as_ref().unwrap().get_entries().len(), 1);
            }
        } else {
            assert!(root.entry_by_name("after").is_none());
            let before = root.entry_by_name("before").unwrap();
            assert_eq!(
                before.times.expiry.unwrap().and_utc().timestamp(),
                OLD_EXPIRY
            );
            assert_eq!(before.get(fields::NOTES), Some("kept notes"));
        }
        return;
    }
    let (env, session, path, _uuid) = fixture();
    session.lock_now().unwrap();
    drop(session);
    let signals = env.tempdir().join("signals");
    std::fs::create_dir_all(signals.join(phase)).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", case, "--nocapture"])
        .env(CHILD, case)
        .env(ROOT, env.paths().state_dir())
        .env("HIDLINS_ATOMIC_FAULT_TARGET", path)
        .env("HIDLINS_ATOMIC_FAULT_SIGNAL_DIR", signals)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child failed\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn edit_error_before_rename_keeps_complete_old_state() {
    fault_case(
        "edit_error_before_rename_keeps_complete_old_state",
        "fsync_done",
        false,
        false,
    );
}

#[test]
fn edit_error_after_rename_has_complete_new_state() {
    fault_case(
        "edit_error_after_rename_has_complete_new_state",
        "rename_done",
        true,
        false,
    );
}

#[test]
fn create_error_before_rename_keeps_complete_old_state() {
    fault_case(
        "create_error_before_rename_keeps_complete_old_state",
        "fsync_done",
        false,
        true,
    );
}

#[test]
fn create_error_after_rename_has_complete_new_state() {
    fault_case(
        "create_error_after_rename_has_complete_new_state",
        "rename_done",
        true,
        true,
    );
}

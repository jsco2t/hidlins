mod common;

use hidlins_api::api::session::AppSession;
use hidlins_api::dto::{CustomFieldInputDto, EntryDraftDto, EntryEditDto, EntryKindDto};
use hidlins_api::error::HidlinsApiError;
use hidlins_core::{fields, EntryBuilder, Uuid, Vault};

const MASTER: &str = "synthetic-api-master";
const URI: &str = "otpauth://totp/Fixture?secret=JBSWY3DPEHPK3PXP";
const STANDARD: [&str; 5] = ["Title", "UserName", "Password", "URL", "Notes"];

fn custom(name: &str, value: &str, protected: bool) -> CustomFieldInputDto {
    CustomFieldInputDto {
        name: name.into(),
        value: value.into(),
        protected,
    }
}

fn draft(custom_fields: Vec<CustomFieldInputDto>) -> EntryDraftDto {
    EntryDraftDto {
        kind: EntryKindDto::Credential,
        title: "dedicated-title".into(),
        username: Some("dedicated-user".into()),
        password: Some("dedicated-password".into()),
        url: Some("https://fixture.invalid".into()),
        notes: Some("dedicated-notes".into()),
        tags: Vec::new(),
        custom_fields,
        totp_uri: None,
    }
}

fn edit(custom_fields: Vec<CustomFieldInputDto>) -> EntryEditDto {
    EntryEditDto {
        title: Some("must-roll-back".into()),
        username: None,
        password: None,
        url: None,
        notes: None,
        tags: None,
        custom_fields: Some(custom_fields),
        totp_uri: None,
    }
}

fn fixture() -> (common::TestEnv, AppSession, String, Uuid) {
    let env = common::TestEnv::new();
    let path = common::create_test_vault(&env, "fixture", MASTER);
    common::register_vault(&env, "fixture", &path);
    let id = {
        let mut vault = Vault::open(&path, &common::master(MASTER), None).expect("vault");
        let id = vault
            .add_entry(
                vault.root_group_uuid(),
                EntryBuilder::credential("original")
                    .username("original-user")
                    .password("original-password")
                    .notes("original-notes")
                    .custom_field("ordinary", "original-custom", true)
                    .build(),
            )
            .expect("seed entry");
        vault.set_max_history_per_entry(1).expect("history cap");
        vault
            .update_entry(id, |view| {
                view.set_title("current");
                Ok(())
            })
            .expect("seed history");
        vault.save().expect("save fixture");
        id
    };
    let session = AppSession::for_test(env.paths_clone()).expect("API session");
    let root = session
        .unlock("fixture".into(), MASTER.into(), None)
        .expect("unlock")
        .root
        .uuid;
    (env, session, root, id)
}

fn assert_reserved_input<T>(result: Result<T, HidlinsApiError>) {
    match result {
        Err(error @ HidlinsApiError::InvalidInput { .. }) => {
            assert!(matches!(&error, HidlinsApiError::InvalidInput { field, .. }
                if field == "custom_fields"));
            assert!(!format!("{error:?} {error}").contains("override-marker"));
        }
        _ => panic!("reserved custom input must return InvalidInput(custom_fields)"),
    }
}

#[test]
fn api_create_rejects_all_standard_custom_names_without_saving() {
    let (env, session, root, id) = fixture();
    let path = env.paths().state_dir().join("fixture.kdbx");
    let before = std::fs::read(&path).expect("original ciphertext");
    for name in STANDARD {
        assert_reserved_input(session.create_entry(
            root.clone(),
            draft(vec![custom(name, "override-marker", false)]),
        ));
        let tree = session.vault_tree().expect("tree");
        assert_eq!(tree.entries.len(), 1);
        assert_eq!(tree.entries[0].uuid, id.to_string());
        assert_eq!(std::fs::read(&path).expect("ciphertext"), before);
    }
}

#[test]
fn api_update_rejects_reserved_names_without_changing_fields_history_or_disk() {
    let (env, session, _root, id) = fixture();
    let path = env.paths().state_dir().join("fixture.kdbx");
    let before = std::fs::read(&path).expect("original ciphertext");
    for name in STANDARD {
        assert_reserved_input(session.update_entry(
            id.to_string(),
            edit(vec![
                custom("ordinary", "must-roll-back", false),
                custom(name, "override-marker", false),
            ]),
        ));
        let detail = session.entry_detail(id.to_string()).expect("live detail");
        assert_eq!(detail.uuid, id.to_string());
        assert_eq!(detail.title, "current");
        assert_eq!(detail.username, "original-user");
        assert_eq!(detail.notes, "original-notes");
        assert!(detail.has_password);
        assert_eq!(detail.custom_fields.len(), 1);
        assert_eq!(detail.custom_fields[0].name, "ordinary");
        assert!(detail.custom_fields[0].is_protected);
        let history = session.entry_history(id.to_string()).expect("history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].title, "original");
        assert_eq!(std::fs::read(&path).expect("ciphertext"), before);
    }
    session.lock_now().expect("release vault");
    let vault = Vault::open(&path, &common::master(MASTER), None).expect("reopen KDBX");
    let entry = vault.get_entry(id).expect("same entry");
    assert_eq!(entry.password(), "original-password");
    assert_eq!(
        entry.custom_field_is_protected(fields::PASSWORD),
        Some(true)
    );
}

#[test]
fn api_preserves_application_totp_custom_field_and_case_distinct_custom_names() {
    let (env, session, root, _) = fixture();
    let mut input = draft(vec![
        custom("password", "case-distinct", false),
        custom(fields::OTP, URI, true),
    ]);
    input.kind = EntryKindDto::Totp;
    input.totp_uri = Some(URI.into());
    let id = session.create_entry(root, input).expect("valid create");
    let mut update = edit(vec![
        custom("password", "edited-custom", false),
        custom(fields::OTP, URI, true),
    ]);
    update.totp_uri = Some(URI.into());
    session
        .update_entry(id.clone(), update)
        .expect("valid update");
    session.lock_now().expect("release");
    let vault = Vault::open(
        &env.paths().state_dir().join("fixture.kdbx"),
        &common::master(MASTER),
        None,
    )
    .expect("reopen KDBX");
    let id = id.parse().expect("UUID");
    let view = vault.get_entry(id).expect("entry");
    assert_eq!(view.password(), "dedicated-password");
    assert_eq!(view.custom_field_is_protected(fields::PASSWORD), Some(true));
    assert_eq!(view.custom_field("password"), Some("edited-custom"));
    assert_eq!(view.custom_field_is_protected("password"), Some(false));
    assert_eq!(view.custom_field(fields::OTP), Some(URI));
    assert_eq!(view.custom_field_is_protected(fields::OTP), Some(true));
    assert!(vault.totp(id).is_ok());
}

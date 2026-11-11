mod common;

use hidlins_core::{fields, EntryBuilder, NoRecoveryConfirmed, Vault};
use keepass::db::{Entry, EntryId};

const STANDARD: [&str; 5] = ["Title", "UserName", "Password", "URL", "Notes"];
const URI: &str = "otpauth://totp/Fixture?secret=JBSWY3DPEHPK3PXP";

fn fixture() -> (common::TestEnv, Vault, uuid::Uuid) {
    let env = common::TestEnv::new();
    let mut vault = Vault::create(
        &env.tempdir().join("fields.kdbx"),
        &common::master("synthetic-master"),
        None,
        common::fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .expect("create fixture");
    let id = vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::credential("original")
                .username("original-user")
                .password("original-password")
                .url("https://fixture.invalid")
                .notes("original-notes")
                .custom_field("ordinary", "original-custom", true)
                .build(),
        )
        .expect("ordinary entry");
    vault.set_max_history_per_entry(1).expect("history cap");
    vault
        .update_entry(id, |view| {
            view.set_title("current");
            Ok(())
        })
        .expect("seed history at cap");
    vault.save().expect("save fixture");
    (env, vault, id)
}

fn snapshot(vault: &Vault, id: uuid::Uuid) -> Entry {
    vault
        .database()
        .entry(EntryId::from_uuid(id))
        .expect("entry")
        .clone()
}

#[test]
fn drafts_cannot_override_any_dedicated_field() {
    let (_env, mut vault, id) = fixture();
    let original = snapshot(&vault, id);
    for name in STANDARD {
        let result = vault.add_entry(
            vault.root_group_uuid(),
            EntryBuilder::credential("dedicated-title")
                .password("dedicated-password")
                .custom_field(name, "override-marker", false)
                .build(),
        );
        assert!(
            result.is_err(),
            "reserved custom field must be rejected: {name}"
        );
        assert_eq!(vault.database().iter_all_entries().count(), 1);
        assert_eq!(snapshot(&vault, id), original);
        let error = result.expect_err("rejected draft");
        assert!(!format!("{error:?} {error}").contains("override-marker"));
    }
}

#[test]
fn reserved_custom_updates_rollback_all_fields_and_capped_history() {
    let (_env, mut vault, id) = fixture();
    let original = snapshot(&vault, id);
    for name in STANDARD {
        let result = vault.update_entry(id, |view| {
            view.set_notes("must roll back");
            view.set_custom_field("ordinary", "must also roll back", false);
            view.set_custom_field(name, "override-marker", false);
            Ok(())
        });
        assert!(result.is_err(), "reserved update must reject: {name}");
        assert_eq!(snapshot(&vault, id), original);
        assert_eq!(vault.get_entry(id).expect("entry").history().len(), 1);
    }
}

#[test]
fn reserved_custom_removals_rollback_the_whole_edit() {
    let (_env, mut vault, id) = fixture();
    let original = snapshot(&vault, id);
    for name in STANDARD {
        let result = vault.update_entry(id, |view| {
            view.set_username("must roll back");
            view.remove_custom_field(name);
            Ok(())
        });
        assert!(result.is_err(), "reserved removal must reject: {name}");
        assert_eq!(snapshot(&vault, id), original);
    }
}

#[test]
fn direct_mutable_views_cannot_replace_or_remove_standard_fields() {
    let (_env, mut vault, id) = fixture();
    let original = snapshot(&vault, id);
    for name in STANDARD {
        vault
            .get_entry_mut(id)
            .expect("mutable entry")
            .set_custom_field(name, "override-marker", false);
        assert_eq!(snapshot(&vault, id), original);
        vault
            .get_entry_mut(id)
            .expect("mutable entry")
            .remove_custom_field(name);
        assert_eq!(snapshot(&vault, id), original);
    }
}

#[test]
fn case_distinct_custom_fields_and_managed_otp_round_trip() {
    let (env, mut vault, _) = fixture();
    let id = vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::totp("TOTP", URI)
                .expect("valid TOTP")
                .password("dedicated-password")
                .custom_field("password", "lowercase-custom", false)
                .custom_field("ordinary", "protected-custom", true)
                .build(),
        )
        .expect("valid custom fields");
    vault
        .update_entry(id, |view| {
            view.set_custom_field("password", "edited-custom", false);
            view.set_custom_field(fields::OTP, URI, true);
            view.remove_custom_field("ordinary");
            Ok(())
        })
        .expect("valid update");
    vault.save().expect("encrypted save");
    drop(vault);
    let vault = Vault::open(
        &env.tempdir().join("fields.kdbx"),
        &common::master("synthetic-master"),
        None,
    )
    .expect("reopen encrypted KDBX");
    let view = vault.get_entry(id).expect("same UUID");
    assert_eq!(view.password(), "dedicated-password");
    assert_eq!(view.custom_field_is_protected(fields::PASSWORD), Some(true));
    assert_eq!(view.custom_field("password"), Some("edited-custom"));
    assert_eq!(view.custom_field_is_protected("password"), Some(false));
    assert_eq!(view.custom_field("ordinary"), None);
    assert_eq!(view.custom_field(fields::OTP), Some(URI));
    assert_eq!(view.custom_field_is_protected(fields::OTP), Some(true));
    assert_eq!(view.history().len(), 1);
    assert!(vault.totp(id).is_ok());
}

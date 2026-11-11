mod common;

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{EntryBuilder, NoRecoveryConfirmed, Uuid, Vault, VaultError};

const NAME: &str = "fixture.bin";
const ORIGINAL: &[u8] = b"original attachment fixture";
const REPLACEMENT: &[u8] = b"replacement attachment fixture";

fn fixture(env: &TestEnv) -> (Vault, Uuid) {
    let mut vault = Vault::create(
        &env.tempdir().join("vault.kdbx"),
        &master("attachment-history-fixture"),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let id = vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::credential("fixture").build(),
        )
        .unwrap();
    vault.add_attachment(id, NAME, ORIGINAL).unwrap();
    (vault, id)
}

fn edit(vault: &mut Vault, id: Uuid, title: &str) {
    vault
        .update_entry(id, |entry| {
            entry.set_title(title);
            Ok(())
        })
        .unwrap();
}

fn assert_historical_bytes(vault: &Vault, id: Uuid, index: usize, expected: &[u8]) {
    let entry = vault
        .database()
        .iter_all_entries()
        .find(|entry| entry.id().uuid() == id)
        .unwrap();
    let historical = entry.historical(index).expect("retained history snapshot");
    let attachment = historical
        .attachment_by_name(NAME)
        .expect("historical attachment must retain its binary");
    assert!(
        attachment.data.as_slice() == expected,
        "historical attachment bytes must remain intact"
    );
}

fn reopen(mut vault: Vault, env: &TestEnv) -> Vault {
    vault.save().unwrap();
    drop(vault);
    Vault::open(
        &env.tempdir().join("vault.kdbx"),
        &master("attachment-history-fixture"),
        None,
    )
    .unwrap()
}

#[test]
fn removing_current_attachment_preserves_edited_entry_history() {
    let env = TestEnv::new();
    let (mut vault, id) = fixture(&env);
    edit(&mut vault, id, "edited");
    vault.remove_attachment(id, NAME).unwrap();
    assert_historical_bytes(&vault, id, 0, ORIGINAL);
    let vault = reopen(vault, &env);
    assert_historical_bytes(&vault, id, 0, ORIGINAL);
    assert!(matches!(
        vault.get_attachment(id, NAME),
        Err(VaultError::AttachmentNotFound { .. })
    ));
}

#[test]
fn replacing_current_attachment_preserves_the_historical_binary() {
    let env = TestEnv::new();
    let (mut vault, id) = fixture(&env);
    edit(&mut vault, id, "edited");
    vault.add_attachment(id, NAME, REPLACEMENT).unwrap();
    assert_historical_bytes(&vault, id, 0, ORIGINAL);
    let vault = reopen(vault, &env);
    assert_historical_bytes(&vault, id, 0, ORIGINAL);
    assert!(vault.get_attachment(id, NAME).unwrap().as_slice() == REPLACEMENT);
}

#[test]
fn prepending_history_keeps_shifted_attachment_references_valid() {
    let env = TestEnv::new();
    let (mut vault, id) = fixture(&env);
    edit(&mut vault, id, "first edit");
    edit(&mut vault, id, "second edit");
    let entry_id = vault
        .database()
        .iter_all_entries()
        .find(|entry| entry.id().uuid() == id)
        .unwrap()
        .id();
    vault
        .database_mut()
        .entry_mut(entry_id)
        .unwrap()
        .historical(0)
        .unwrap()
        .remove_attachment_by_name(NAME);
    vault.remove_attachment(id, NAME).unwrap();
    assert_historical_bytes(&vault, id, 1, ORIGINAL);
    let vault = reopen(vault, &env);
    assert_historical_bytes(&vault, id, 1, ORIGINAL);
}

#[test]
fn pruning_and_failed_edit_keep_the_retained_attachment_history() {
    let env = TestEnv::new();
    let (mut vault, id) = fixture(&env);
    vault.set_max_history_per_entry(1).unwrap();
    edit(&mut vault, id, "first edit");
    vault.add_attachment(id, NAME, REPLACEMENT).unwrap();
    edit(&mut vault, id, "second edit");
    let result = vault.update_entry(id, |entry| {
        entry.set_title("must roll back");
        Err(VaultError::InvalidGroupTarget {
            reason: "intentional fixture failure",
        })
    });
    assert!(matches!(result, Err(VaultError::InvalidGroupTarget { .. })));
    assert_eq!(vault.get_entry(id).unwrap().title(), "second edit");
    assert_eq!(vault.get_entry(id).unwrap().history().len(), 1);
    vault.remove_attachment(id, NAME).unwrap();
    assert_historical_bytes(&vault, id, 0, REPLACEMENT);
    let vault = reopen(vault, &env);
    assert_historical_bytes(&vault, id, 0, REPLACEMENT);
}

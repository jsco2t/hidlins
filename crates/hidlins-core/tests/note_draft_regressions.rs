#[path = "common/allocation_observer.rs"]
mod allocation_observer;
mod common;

use allocation_observer::DeallocationWatch;
use common::{fast_kdf, master, TestEnv};
use hidlins_core::{EntryBuilder, NoRecoveryConfirmed, Uuid, Vault, VaultError};

const SENTINEL: &str = "note-draft-secret-sentinel";
const BODY: &str = "note-draft-secret-sentinel\nGàidhlig 🔒 recovery codes";

fn builder(kind: u8) -> EntryBuilder {
    match kind {
        0 => EntryBuilder::credential("fixture"),
        1 => EntryBuilder::secure_note("fixture"),
        _ => {
            EntryBuilder::totp("fixture", "otpauth://totp/fixture?secret=JBSWY3DPEHPK3PXP").unwrap()
        }
    }
}

#[test]
fn notes_never_appear_in_builder_or_draft_debug_output() {
    for kind in 0..3 {
        let builder = builder(kind).notes(BODY);
        let draft = builder.clone().build();
        for output in [format!("{builder:?}"), format!("{draft:?}")] {
            assert!(
                !output.contains(SENTINEL),
                "Debug must redact note contents"
            );
        }
        assert_eq!(draft.notes(), Some(BODY));
    }
}

#[test]
fn abandoned_replaced_and_cloned_note_buffers_are_erased() {
    for kind in 0..3 {
        let notes = BODY.to_owned();
        let watch = DeallocationWatch::new(notes.as_bytes());
        drop(builder(kind).notes(notes));
        watch.assert_erased();

        let notes = BODY.to_owned();
        let watch = DeallocationWatch::new(notes.as_bytes());
        let draft = builder(kind).notes(notes).notes("replacement").build();
        watch.assert_erased();
        assert_eq!(draft.notes(), Some("replacement"));

        let notes = BODY.to_owned();
        let watch = DeallocationWatch::new(notes.as_bytes());
        drop(builder(kind).notes(notes).build());
        watch.assert_erased();

        let draft = builder(kind).notes(BODY).build();
        let cloned = draft.clone();
        let watch = DeallocationWatch::new(cloned.notes().unwrap().as_bytes());
        drop(draft);
        assert_eq!(cloned.notes(), Some(BODY));
        drop(cloned);
        watch.assert_erased();
    }
}

#[test]
fn failed_entry_insertion_erases_the_consumed_note_draft() {
    let env = TestEnv::new();
    let path = env.tempdir().join("note-draft.kdbx");
    let mut vault = Vault::create(
        &path,
        &master("fixture"),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let draft = EntryBuilder::secure_note("fixture").notes(BODY).build();
    let watch = DeallocationWatch::new(draft.notes().unwrap().as_bytes());
    assert!(matches!(
        vault.add_entry(Uuid::nil(), draft),
        Err(VaultError::GroupNotFound { .. })
    ));
    watch.assert_erased();
}

#[test]
fn note_text_and_protection_flags_survive_insertion_and_reopen() {
    let env = TestEnv::new();
    let path = env.tempdir().join("note-roundtrip.kdbx");
    let password = master("fixture");
    let mut vault = Vault::create(
        &path,
        &password,
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let root = vault.root_group_uuid();
    let ids: Vec<_> = ["", BODY, "  padded\nbody\t "]
        .into_iter()
        .map(|body| {
            vault
                .add_entry(
                    root,
                    EntryBuilder::secure_note("fixture").notes(body).build(),
                )
                .unwrap()
        })
        .collect();
    vault.save().unwrap();
    drop(vault);
    let vault = Vault::open(&path, &password, None).unwrap();
    let root = vault.database().root();
    for (id, expected) in ids.into_iter().zip(["", BODY, "  padded\nbody\t "]) {
        let entry = vault.get_entry(id).unwrap();
        assert_eq!(entry.notes(), expected);
        let stored = root
            .entries()
            .find(|entry| entry.id().uuid() == id)
            .unwrap();
        assert!(!stored
            .fields
            .get(hidlins_core::fields::NOTES)
            .unwrap()
            .is_protected());
    }
}

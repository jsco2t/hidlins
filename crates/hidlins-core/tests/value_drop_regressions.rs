#[path = "common/allocation_observer.rs"]
mod allocation_observer;
mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use allocation_observer::DeallocationWatch;
use common::{fast_kdf, master, TestEnv};
use hidlins_core::{EntryBuilder, NoRecoveryConfirmed, Value, Vault};
use zeroize::Zeroize;

struct Payload {
    bytes: Vec<u8>,
    observed_erasure: Arc<AtomicBool>,
}

impl Zeroize for Payload {
    fn zeroize(&mut self) {
        self.bytes.zeroize();
    }
}

impl Drop for Payload {
    fn drop(&mut self) {
        self.observed_erasure
            .store(self.bytes.is_empty(), Ordering::Release);
    }
}

#[test]
fn value_erases_its_payload_before_the_payload_destructor_runs() {
    for protected in [false, true] {
        let observed = Arc::new(AtomicBool::new(false));
        let payload = Payload {
            bytes: b"owned-payload-fixture".to_vec(),
            observed_erasure: Arc::clone(&observed),
        };
        let value: Value<Payload> = if protected {
            Value::protected(payload)
        } else {
            Value::unprotected(payload)
        };
        assert_eq!(value.is_protected(), protected);
        drop(value);
        assert!(
            observed.load(Ordering::Acquire),
            "the wrapper must erase owned payloads before dropping them"
        );
    }
}

#[test]
fn unprotected_string_and_binary_values_are_erased_before_free() {
    let text = "owned-note-value-fixture".to_owned();
    let watch = DeallocationWatch::new(text.as_bytes());
    drop(Value::<String>::unprotected(text));
    watch.assert_erased();

    let bytes = b"owned-attachment-value-fixture".to_vec();
    let watch = DeallocationWatch::new(&bytes);
    drop(Value::<Vec<u8>>::unprotected(bytes));
    watch.assert_erased();
}

#[test]
fn replacing_notes_erases_the_previous_database_owned_buffer() {
    let env = TestEnv::new();
    let mut vault = Vault::create(
        &env.tempdir().join("replace-note.kdbx"),
        &master("fixture"),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let id = vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::secure_note("fixture")
                .notes("owned-note-fixture")
                .build(),
        )
        .unwrap();
    let watch = DeallocationWatch::new(vault.get_entry(id).unwrap().notes().as_bytes());
    vault
        .update_entry(id, |entry| {
            entry.set_notes("replacement fixture");
            Ok(())
        })
        .unwrap();
    watch.assert_erased();
    assert_eq!(vault.get_entry(id).unwrap().notes(), "replacement fixture");
    assert_eq!(
        vault.get_entry(id).unwrap().history()[0].notes(),
        "owned-note-fixture"
    );
}

#[test]
fn dropping_a_vault_erases_database_owned_attachment_bytes() {
    let env = TestEnv::new();
    let mut vault = Vault::create(
        &env.tempdir().join("drop-attachment.kdbx"),
        &master("fixture"),
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
    vault
        .add_attachment(id, "fixture.bin", b"owned-attachment-fixture")
        .unwrap();
    let watch = {
        let root = vault.database().root();
        let entry = root
            .entries()
            .find(|entry| entry.id().uuid() == id)
            .unwrap();
        let attachment = entry.attachment_by_name("fixture.bin").unwrap();
        assert!(!attachment.data.is_protected());
        DeallocationWatch::new(attachment.data.as_slice())
    };
    drop(vault);
    watch.assert_erased();
}

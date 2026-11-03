mod common;
#[path = "common/search_allocation_observer.rs"]
mod search_allocation_observer;

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{
    fuzzy_match, EntryBuilder, NoRecoveryConfirmed, SearchMode, SearchOptions, Vault, Zeroizing,
};
use search_allocation_observer::assert_copy_erased;

const COPY_BYTES: usize = 17 * 4;

fn note_copy(mode: SearchMode, ordinal: usize) {
    let env = TestEnv::new();
    let body = Zeroizing::new("🔒".repeat(17));
    assert_eq!(body.len(), COPY_BYTES);
    let mut vault = Vault::create(
        &env.tempdir().join("note-search.kdbx"),
        &master("fixture"),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::secure_note("").notes(body.as_str()).build(),
        )
        .unwrap();
    let options = SearchOptions::new("z").with_mode(mode);
    assert_copy_erased(COPY_BYTES, ordinal, || {
        assert!(vault.search(options).unwrap().is_empty());
    });
}

#[test]
fn substring_note_normalization_is_erased() {
    note_copy(SearchMode::Substring, 1);
}

#[test]
fn wildcard_note_normalization_is_erased() {
    note_copy(SearchMode::Wildcard, 1);
}

#[test]
fn wildcard_note_character_copy_is_erased() {
    note_copy(SearchMode::Wildcard, 2);
}

#[test]
fn fuzzy_note_character_copy_is_erased() {
    note_copy(SearchMode::Fuzzy, 1);
}

#[test]
fn fuzzy_haystack_copy_is_erased_on_early_return() {
    let body = Zeroizing::new("🔒".repeat(17));
    assert_copy_erased(COPY_BYTES, 1, || {
        assert!(fuzzy_match("z", body.as_str()).is_none());
    });
}

#[test]
fn fuzzy_needle_copy_is_erased_on_early_return() {
    let body = Zeroizing::new("🔒".repeat(17));
    assert_copy_erased(COPY_BYTES, 1, || {
        assert!(fuzzy_match(body.as_str(), "z").is_none());
    });
}

#[test]
fn fuzzy_haystack_copy_is_erased_after_a_match() {
    let body = Zeroizing::new("🔒".repeat(17));
    assert_copy_erased(COPY_BYTES, 1, || {
        assert!(fuzzy_match("🔒", body.as_str()).is_some());
    });
}

mod common;

use std::collections::BTreeSet;

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{
    EntryBuilder, MatchedField, NoRecoveryConfirmed, SearchOptions, Uuid, Vault, VaultReadOnly,
};

const TITLES: [&str; 10] = [
    "İ", "i\u{307}", "xİ", "İx", "aİz", "ABc", "λ", "🔒", "", "İİ",
];

fn fixture(env: &TestEnv) -> (Vault, Vec<Uuid>) {
    let mut vault = Vault::create(
        &env.tempdir().join("wildcard-unicode.kdbx"),
        &master("fixture"),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let root = vault.root_group_uuid();
    let ids = TITLES
        .iter()
        .map(|title| {
            vault
                .add_entry(root, EntryBuilder::credential(*title).build())
                .unwrap()
        })
        .collect();
    (vault, ids)
}

fn assert_character_queries(search: impl Fn(&str) -> Vec<Uuid>, ids: &[Uuid]) {
    for (pattern, expected) in [
        ("?", vec!["İ", "λ", "🔒"]),
        ("??", vec!["i\u{307}", "xİ", "İx", "İİ"]),
        ("a?z", vec!["aİz"]),
        ("a??z", vec![]),
        ("i?", vec!["i\u{307}"]),
        ("*i?", vec!["i\u{307}"]),
        ("*?x", vec!["İx"]),
        ("?İ", vec!["xİ", "İİ"]),
        ("İ?", vec!["İx", "İİ"]),
        (
            "*?",
            TITLES
                .iter()
                .copied()
                .filter(|title| !title.is_empty())
                .collect(),
        ),
        (
            "?*?",
            TITLES
                .iter()
                .copied()
                .filter(|title| title.chars().count() >= 2)
                .collect(),
        ),
    ] {
        let expected: BTreeSet<_> = TITLES
            .iter()
            .zip(ids)
            .filter_map(|(title, id)| expected.contains(title).then_some(*id))
            .collect();
        let actual: BTreeSet<_> = search(pattern).into_iter().collect();
        assert_eq!(
            actual, expected,
            "wildcard {pattern:?} must count original characters"
        );
    }
}

#[test]
fn wildcard_units_are_original_characters_on_both_vault_surfaces() {
    let env = TestEnv::new();
    let (mut vault, ids) = fixture(&env);
    assert_character_queries(
        |pattern| {
            vault
                .search(SearchOptions::new(pattern).wildcard(true))
                .unwrap()
                .into_iter()
                .map(|hit| hit.uuid)
                .collect()
        },
        &ids,
    );
    vault.save().unwrap();
    drop(vault);
    let vault = VaultReadOnly::open(
        &env.tempdir().join("wildcard-unicode.kdbx"),
        &master("fixture"),
        None,
    )
    .unwrap();
    assert_character_queries(
        |pattern| {
            vault
                .search(SearchOptions::new(pattern).wildcard(true))
                .unwrap()
                .into_iter()
                .map(|hit| hit.uuid)
                .collect()
        },
        &ids,
    );
}

#[test]
fn literal_lowercase_equivalence_and_ascii_matching_are_preserved() {
    let env = TestEnv::new();
    let (mut vault, ids) = fixture(&env);
    let greek = vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::credential("ΟΣ").build(),
        )
        .unwrap();
    for pattern in ["ΟΣ", "ος"] {
        assert!(vault
            .search(SearchOptions::new(pattern).wildcard(true))
            .unwrap()
            .iter()
            .any(|hit| hit.uuid == greek));
    }
    for pattern in ["İ", "i\u{307}"] {
        let wildcard: BTreeSet<_> = vault
            .search(SearchOptions::new(pattern).wildcard(true))
            .unwrap()
            .into_iter()
            .map(|hit| hit.uuid)
            .collect();
        assert_eq!(wildcard, BTreeSet::from([ids[0], ids[1]]));
        let substring = vault.search(SearchOptions::new(pattern)).unwrap();
        assert!(substring.iter().any(|hit| hit.uuid == ids[0]));
        assert!(substring.iter().any(|hit| hit.uuid == ids[1]));
    }
    let ascii = vault
        .search(SearchOptions::new("a?C").wildcard(true))
        .unwrap();
    assert_eq!(ascii.len(), 1);
    assert_eq!(ascii[0].uuid, ids[5]);
}

#[test]
fn unicode_wildcard_keeps_existing_title_score_and_match_metadata() {
    let env = TestEnv::new();
    let (vault, ids) = fixture(&env);
    let hits = vault
        .search(SearchOptions::new("a?z").wildcard(true))
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].uuid, ids[4]);
    assert_eq!(hits[0].score, 8);
    assert_eq!(hits[0].matched_fields, vec![MatchedField::Title]);
    assert!(hits[0].match_indices.is_empty());
}

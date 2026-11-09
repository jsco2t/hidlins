mod common;

use std::collections::BTreeSet;

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{EntryBuilder, NoRecoveryConfirmed, SearchOptions, Uuid, Vault, VaultReadOnly};

const TITLES: [&str; 10] = [
    "İ",
    "i\u{307}",
    "xİ",
    "xi\u{307}",
    "İx",
    "i\u{307}x",
    "İİ",
    "i",
    "ix",
    "ABC",
];

fn assert_queries(search: impl Fn(&str) -> Vec<Uuid>, ids: &[Uuid]) {
    let cases: &[(&str, &[&str])] = &[
        ("*\u{307}", &["i\u{307}", "xi\u{307}"]),
        ("i*", &["i\u{307}", "i\u{307}x", "i", "ix"]),
        ("I*", &["i\u{307}", "i\u{307}x", "i", "ix"]),
        ("*i*", &["i\u{307}", "xi\u{307}", "i\u{307}x", "i", "ix"]),
        ("*\u{307}*", &["i\u{307}", "xi\u{307}", "i\u{307}x"]),
        ("x*\u{307}", &["xi\u{307}"]),
        ("*\u{307}?", &["i\u{307}x"]),
        ("İ*", &["İ", "i\u{307}", "İx", "i\u{307}x", "İİ"]),
        ("i\u{307}*", &["İ", "i\u{307}", "İx", "i\u{307}x", "İİ"]),
        ("*İ", &["İ", "i\u{307}", "xİ", "xi\u{307}", "İİ"]),
        ("?", &["İ", "i"]),
        ("?*", &TITLES),
        ("ab*", &["ABC"]),
    ];
    for (pattern, titles) in cases {
        let expected: BTreeSet<_> = TITLES
            .iter()
            .zip(ids)
            .filter_map(|(title, id)| titles.contains(title).then_some(*id))
            .collect();
        let actual: BTreeSet<_> = search(pattern).into_iter().collect();
        assert_eq!(
            actual, expected,
            "wildcard {pattern:?} must not split an original character"
        );
    }
}

#[test]
fn star_consumes_original_characters_on_writable_and_read_only_vaults() {
    let env = TestEnv::new();
    let path = env.tempdir().join("wildcard-star.kdbx");
    let mut vault = Vault::create(
        &path,
        &master("wildcard-star-fixture"),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let root = vault.root_group_uuid();
    let ids: Vec<_> = TITLES
        .iter()
        .map(|title| {
            vault
                .add_entry(root, EntryBuilder::credential(*title).build())
                .unwrap()
        })
        .collect();
    assert_queries(
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
    let vault = VaultReadOnly::open(&path, &master("wildcard-star-fixture"), None).unwrap();
    assert_queries(
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

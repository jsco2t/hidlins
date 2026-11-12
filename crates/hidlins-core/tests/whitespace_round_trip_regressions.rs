//! Literal KDBX entry text must survive both Hidlins and `KeePassXC` rewrites.

mod common;

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use common::{fast_kdf, master, TestEnv};
use hidlins_core::{fields, NoRecoveryConfirmed, Vault};
use keepass::db::{Entry, EntryId, Value};

const PASSWORD: &str = "whitespace-fixture-master";
const CASES: &[&str] = &[
    "",
    " ",
    "   ",
    "\t",
    "\n",
    " \t\n ",
    "\u{a0}\u{2003}",
    " \t猫é\n ",
];
const FIELDS: &[&str] = &[
    fields::TITLE,
    fields::USERNAME,
    fields::PASSWORD,
    fields::URL,
    fields::NOTES,
    "Literal custom field",
];

fn populate(path: &Path, values: &[&str], protected: bool) -> Vec<EntryId> {
    let mut vault = Vault::create(
        path,
        &master(PASSWORD),
        None,
        fast_kdf(),
        NoRecoveryConfirmed::yes(),
    )
    .expect("create fixture");
    let mut ids = Vec::new();
    for value in values {
        let mut root = vault.database_mut().root_mut();
        let mut entry = root.add_entry();
        for field in FIELDS {
            entry.set(*field, field_value(value, protected));
        }
        let id = entry.id();
        // A real tracked edit pins the original text in KDBX history too.
        entry
            .track_changes()
            .set_unprotected("Later edit", "current revision");
        ids.push(id);
    }
    vault.save().expect("save fixture");
    ids
}

fn field_value(value: &str, protected: bool) -> Value<String> {
    if protected {
        Value::protected(value.to_owned())
    } else {
        Value::unprotected(value.to_owned())
    }
}

fn assert_fields(entry: &Entry, expected: &str, protected: Option<bool>) {
    for field in FIELDS {
        assert_eq!(entry.get(field), Some(expected), "field {field}");
        if let Some(protected) = protected {
            assert_eq!(
                entry.fields.get(*field),
                Some(&field_value(expected, protected))
            );
        }
    }
}

fn assert_fixture(path: &Path, ids: &[EntryId], values: &[&str], protected: Option<bool>) {
    let mut vault = Vault::open(path, &master(PASSWORD), None).expect("reopen fixture");
    for (id, value) in ids.iter().zip(values) {
        let entry = vault.database().entry(*id).expect("same entry UUID");
        assert_fields(&entry, value, protected);
        let history = entry.history.as_ref().expect("history retained");
        assert_eq!(history.get_entries().len(), 1);
        assert_fields(&history.get_entries()[0], value, protected);
    }
    // Exercise the ordinary default optional metadata (including empty colors)
    // through another write, not just a single parser invocation.
    vault.save().expect("second save");
}

#[test]
fn unprotected_empty_and_mixed_unicode_are_preserved() {
    let env = TestEnv::new();
    let path = env.tempdir().join("controls.kdbx");
    let values = [CASES[0], CASES[7]];
    let ids = populate(&path, &values, false);
    assert_fixture(&path, &ids, &values, Some(false));
    assert_fixture(&path, &ids, &values, Some(false));
}

#[test]
fn unprotected_whitespace_is_preserved_in_fields_and_history() {
    let env = TestEnv::new();
    let path = env.tempdir().join("unprotected.kdbx");
    let ids = populate(&path, CASES, false);
    assert_fixture(&path, &ids, CASES, Some(false));
    assert_fixture(&path, &ids, CASES, Some(false));
}

#[test]
fn protected_whitespace_is_preserved_in_fields_and_history() {
    let env = TestEnv::new();
    let path = env.tempdir().join("protected.kdbx");
    let ids = populate(&path, CASES, true);
    assert_fixture(&path, &ids, CASES, Some(true));
    assert_fixture(&path, &ids, CASES, Some(true));
}

#[test]
#[ignore = "requires keepassxc-cli >= 2.7 on PATH"]
fn keepassxc_reads_and_rewrites_literal_fields_and_history() {
    let env = TestEnv::new();
    for protected in [false, true] {
        let path = env.tempdir().join(format!("keepassxc-{protected}.kdbx"));
        let ids = populate(&path, CASES, protected);
        let mut child = Command::new("keepassxc-cli")
            .args(["add", "-q"])
            .arg(&path)
            .arg("KeePassXC rewrite control")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn KeePassXC");
        writeln!(child.stdin.take().expect("secure stdin"), "{PASSWORD}")
            .expect("provide test master password over stdin");
        let result = child.wait_with_output().expect("wait for KeePassXC");
        assert!(
            result.status.success(),
            "KeePassXC add failed: {:?}",
            result.status
        );
        // KeePassXC applies its memory-protection preferences to standard
        // fields. Assert literal values/history here; Hidlins-only round
        // trips above separately assert every field's protection flag.
        assert_fixture(&path, &ids, CASES, None);
        let vault = Vault::open(&path, &master(PASSWORD), None).expect("reopen rewrite");
        for (id, value) in ids.iter().zip(CASES) {
            let entry = vault.database().entry(*id).expect("entry UUID retained");
            let history = entry.history.as_ref().expect("history retained");
            for revision in std::iter::once(&*entry).chain(history.get_entries()) {
                assert_eq!(
                    revision.fields.get("Literal custom field"),
                    Some(&field_value(value, protected)),
                    "custom protection retained across KeePassXC rewrite"
                );
            }
        }
        assert!(vault
            .database()
            .iter_all_entries()
            .any(|entry| { entry.get(fields::TITLE) == Some("KeePassXC rewrite control") }));
    }
}

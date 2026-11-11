//! Preserve the conflict/content masks before eliminating comparison clones.
use hidlins_core::{fields, Database, Entry, Value};
use hidlins_sync::reconcile;

fn at(seconds: i64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(1_700_000_000 + seconds, 0)
        .unwrap()
        .naive_utc()
}

fn fixture() -> Database {
    let mut db = Database::new();
    {
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected(fields::TITLE, "comparison fixture");
        entry.set_protected(fields::PASSWORD, "synthetic comparison secret");
        entry.times.last_modification = Some(at(10));
        entry.times.expires = Some(false);
        entry.times.expiry = Some(at(100));
    }
    db
}

type Change = fn(&mut Entry);

#[test]
fn equal_time_content_and_protection_changes_remain_conflicts() {
    let cases: &[(&str, Change)] = &[
        ("password", |entry| {
            entry.fields.insert(
                fields::PASSWORD.into(),
                Value::protected("changed synthetic secret".to_owned()),
            );
        }),
        ("protection flag", |entry| {
            entry.fields.insert(
                fields::PASSWORD.into(),
                Value::unprotected("synthetic comparison secret".to_owned()),
            );
        }),
        ("custom field", |entry| {
            entry
                .fields
                .insert("custom".into(), Value::unprotected("fixture".to_owned()));
        }),
        ("tags", |entry| {
            entry.tags.push("fixture-tag".to_owned());
        }),
        ("autotype", |entry| {
            entry.autotype.get_or_insert_default().enabled = true;
        }),
        ("custom data", |entry| {
            entry.custom_data.entry("fixture-data".into()).or_default();
        }),
        ("foreground", |entry| {
            entry.foreground_color = Some("#010203".parse().unwrap());
        }),
        ("background", |entry| {
            entry.background_color = Some("#AABBCC".parse().unwrap());
        }),
        ("override URL", |entry| {
            entry.override_url = Some("https://fixture.invalid".to_owned());
        }),
        ("quality flag", |entry| {
            entry.quality_check = false;
        }),
        ("expiration flag", |entry| {
            entry.times.expires = Some(true);
        }),
        ("expiration cutoff", |entry| {
            entry.times.expiry = Some(at(101));
        }),
    ];
    for (name, change) in cases {
        let mut local = fixture();
        let before = local.clone();
        let mut remote = local.clone();
        let id = remote.root().entries().next().unwrap().id();
        change(&mut remote.entry_mut(id).unwrap());
        assert!(
            reconcile(&mut local, &remote).is_err(),
            "equal-time {name} must conflict"
        );
        assert!(
            local == before,
            "single-entry conflict must leave local unchanged"
        );
    }
}

#[test]
fn bookkeeping_clocks_do_not_become_equal_time_content_conflicts() {
    let cases: &[(&str, Change)] = &[
        ("creation", |entry| {
            entry.times.creation = Some(at(2));
        }),
        ("access", |entry| {
            entry.times.last_access = Some(at(2));
        }),
        ("usage", |entry| {
            entry.times.usage_count = Some(77);
        }),
        ("location clock", |entry| {
            entry.times.location_changed = Some(at(2));
        }),
    ];
    for (name, change) in cases {
        let mut local = fixture();
        let before = local.clone();
        let mut remote = local.clone();
        let id = remote.root().entries().next().unwrap().id();
        change(&mut remote.entry_mut(id).unwrap());
        reconcile(&mut local, &remote)
            .unwrap_or_else(|error| panic!("bookkeeping {name} must not conflict: {error}"));
        assert!(local == before);
    }
}

#[test]
fn icon_changes_remain_content() {
    let mut local = fixture();
    let mut remote = local.clone();
    let id = remote.root().entries().next().unwrap().id();
    remote.entry_mut(id).unwrap().set_icon_builtin(3);
    assert!(reconcile(&mut local, &remote).is_err());
}

#[test]
fn history_is_processed_separately_from_current_content_comparison() {
    let mut local = fixture();
    let mut remote = local.clone();
    let id = remote.root().entries().next().unwrap().id();
    let mut historical = (*remote.entry(id).unwrap()).clone();
    historical.fields.insert(
        fields::PASSWORD.into(),
        Value::protected("synthetic historical secret".to_owned()),
    );
    historical.times.last_modification = Some(at(5));
    remote
        .entry_mut(id)
        .unwrap()
        .history
        .get_or_insert_default()
        .add_entry(historical);
    for _ in 0..3 {
        reconcile(&mut local, &remote).unwrap();
        let entry = local.entry(id).unwrap();
        assert_eq!(
            entry.get(fields::PASSWORD),
            Some("synthetic comparison secret")
        );
        let history = entry.history.as_ref().unwrap();
        assert_eq!(history.get_entries().len(), 1);
        assert_eq!(
            history.get_entries()[0].get(fields::PASSWORD),
            Some("synthetic historical secret")
        );
    }
}

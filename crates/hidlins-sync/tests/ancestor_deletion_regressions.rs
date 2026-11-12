//! An old ancestor purge must not erase a newer descendant's local work.
use hidlins_core::{fields, KdfParams, MasterPassword, NoRecoveryConfirmed, Vault};
use hidlins_sync::reconcile;

fn at(seconds: i64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(1_700_000_000 + seconds, 0)
        .unwrap()
        .naive_utc()
}

#[derive(Clone, Copy)]
enum Change {
    Entry,
    Group,
    None,
}

fn ancestor_case(depth: usize, change: Change) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nested.kdbx");
    let master = MasterPassword::new("nested merge synthetic master".to_owned());
    let mut vault = Vault::create(
        &path,
        &master,
        None,
        KdfParams {
            memory_kib: 64,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    let mut parent = vault.database().root().id();
    let mut ancestry = Vec::new();
    for index in 0..depth {
        let child = {
            let mut parent_group = vault.database_mut().group_mut(parent).unwrap();
            let mut group = parent_group.add_group();
            group.name = format!("nested fixture {index}");
            group.times.last_modification = Some(at(1));
            group.times.location_changed = Some(at(1));
            group.id()
        };
        ancestry.push(child);
        parent = child;
    }
    let entry_id = {
        let mut group = vault.database_mut().group_mut(parent).unwrap();
        let mut entry = group.add_entry();
        entry.set_protected(fields::PASSWORD, "older synthetic secret");
        entry.times.last_modification = Some(at(1));
        entry.times.location_changed = Some(at(1));
        entry.id()
    };
    let mut remote = vault.database().clone();
    remote
        .group_mut(ancestry[0])
        .unwrap()
        .track_changes()
        .remove()
        .unwrap();
    for deleted_at in remote.deleted_objects.values_mut() {
        *deleted_at = Some(at(10));
    }
    match change {
        Change::Entry => {
            let mut entry = vault.database_mut().entry_mut(entry_id).unwrap();
            entry.set_protected(fields::PASSWORD, "newer synthetic secret");
            entry.times.last_modification = Some(at(20));
        }
        Change::Group => {
            let mut group = vault.database_mut().group_mut(parent).unwrap();
            "newer group name".clone_into(&mut group.name);
            group.times.last_modification = Some(at(20));
        }
        Change::None => {}
    }
    let keep_ancestors = !matches!(change, Change::None);
    let keep_entry = matches!(change, Change::Entry);
    for _ in 0..3 {
        reconcile(vault.database_mut(), &remote).unwrap();
        assert_eq!(
            vault.database().entry(entry_id).is_some(),
            keep_entry,
            "an older ancestor deletion must preserve newer descendant secret data"
        );
        for id in &ancestry {
            assert_eq!(
                vault.database().group(*id).is_some(),
                keep_ancestors,
                "newer descendants require their complete ancestor path"
            );
        }
    }
    vault.save().unwrap();
    drop(vault);
    let reopened = Vault::open(&path, &master, None).unwrap();
    assert_eq!(reopened.database().entry(entry_id).is_some(), keep_entry);
    if keep_entry {
        assert_eq!(
            reopened
                .database()
                .entry(entry_id)
                .unwrap()
                .get(fields::PASSWORD),
            Some("newer synthetic secret")
        );
    }
    if matches!(change, Change::Group) {
        assert_eq!(
            reopened.database().group(parent).unwrap().name,
            "newer group name"
        );
    }
}

#[test]
fn ancestor_purge_preserves_newer_entry_in_a_child_group() {
    ancestor_case(2, Change::Entry);
}

#[test]
fn ancestor_purge_preserves_newer_entry_several_levels_down() {
    ancestor_case(4, Change::Entry);
}

#[test]
fn ancestor_purge_preserves_newer_deep_group_metadata() {
    ancestor_case(4, Change::Group);
}

#[test]
fn newer_purge_still_removes_an_older_subtree() {
    ancestor_case(4, Change::None);
}

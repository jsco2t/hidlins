//! A newer tracked move counts even when the entry's content date is older.
use hidlins_core::{fields, Database};
use hidlins_sync::reconcile;

fn at(seconds: i64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(1_700_000_000 + seconds, 0)
        .unwrap()
        .naive_utc()
}

fn entry_case(move_at: i64, moved_is_source: bool) {
    let mut moved = Database::new();
    let original = {
        let mut root = moved.root_mut();
        let mut group = root.add_group();
        "original group".clone_into(&mut group.name);
        group.times.last_modification = Some(at(1));
        group.times.location_changed = Some(at(1));
        group.id()
    };
    let target = {
        let mut root = moved.root_mut();
        let mut group = root.add_group();
        "target group".clone_into(&mut group.name);
        group.times.last_modification = Some(at(1));
        group.times.location_changed = Some(at(1));
        group.id()
    };
    let id = {
        let mut group = moved.group_mut(original).unwrap();
        let mut entry = group.add_entry();
        entry.set_protected(fields::PASSWORD, "moved synthetic secret");
        entry.times.last_modification = Some(at(1));
        entry.times.location_changed = Some(at(1));
        entry.id()
    };
    let mut deleted = moved.clone();
    deleted.entry_mut(id).unwrap().track_changes().remove();
    deleted.deleted_objects.insert(id.uuid(), Some(at(10)));
    {
        let mut entry = moved.entry_mut(id).unwrap();
        entry.track_changes().move_to(target).unwrap();
        entry.times.location_changed = Some(at(move_at));
        assert_eq!(
            entry.times.last_modification,
            Some(at(1)),
            "a move changes location, not the content clock"
        );
    }
    let (mut local, remote) = if moved_is_source {
        (deleted, moved)
    } else {
        (moved, deleted)
    };
    let survives = move_at > 10;
    for _ in 0..3 {
        reconcile(&mut local, &remote).unwrap();
        assert_eq!(
            local.entry(id).is_some(),
            survives,
            "the later of content and location clocks must order deletion"
        );
        if survives {
            let entry = local.entry(id).unwrap();
            assert_eq!(entry.parent().id(), target);
            assert_eq!(entry.times.location_changed, Some(at(move_at)));
            assert_eq!(entry.get(fields::PASSWORD), Some("moved synthetic secret"));
        }
    }
}

fn group_case(moved_is_source: bool) {
    let mut moved = Database::new();
    let id = {
        let mut root = moved.root_mut();
        let mut group = root.add_group();
        "moved group".clone_into(&mut group.name);
        group.times.last_modification = Some(at(1));
        group.times.location_changed = Some(at(1));
        group.id()
    };
    let target = {
        let mut root = moved.root_mut();
        let mut group = root.add_group();
        "target group".clone_into(&mut group.name);
        group.times.last_modification = Some(at(1));
        group.times.location_changed = Some(at(1));
        group.id()
    };
    let mut deleted = moved.clone();
    deleted
        .group_mut(id)
        .unwrap()
        .track_changes()
        .remove()
        .unwrap();
    deleted.deleted_objects.insert(id.uuid(), Some(at(10)));
    {
        let mut group = moved.group_mut(id).unwrap();
        group.track_changes().move_to(target).unwrap();
        group.times.location_changed = Some(at(20));
        assert_eq!(group.times.last_modification, Some(at(1)));
    }
    let (mut local, remote) = if moved_is_source {
        (deleted, moved)
    } else {
        (moved, deleted)
    };
    for _ in 0..3 {
        reconcile(&mut local, &remote).unwrap();
        let group = local
            .group(id)
            .expect("newer moved group must survive the older purge");
        assert_eq!(group.parent().unwrap().id(), target);
        assert_eq!(group.times.location_changed, Some(at(20)));
    }
}

#[test]
fn newer_local_entry_move_survives_older_remote_purge() {
    entry_case(20, false);
}
#[test]
fn newer_remote_entry_move_restores_an_older_local_purge() {
    entry_case(20, true);
}
#[test]
fn older_local_entry_move_still_loses_to_newer_purge() {
    entry_case(5, false);
}
#[test]
fn older_remote_entry_move_still_loses_to_newer_purge() {
    entry_case(5, true);
}
#[test]
fn newer_local_group_move_survives_older_remote_purge() {
    group_case(false);
}
#[test]
fn newer_remote_group_move_restores_an_older_local_purge() {
    group_case(true);
}

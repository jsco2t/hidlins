//! Explicit deletions must survive clients that never held the deleted object.
use hidlins_core::{fields, Database, KdfParams, MasterPassword, NoRecoveryConfirmed, Uuid, Vault};
use hidlins_sync::reconcile;

fn at(seconds: i64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(1_700_000_000 + seconds, 0)
        .unwrap()
        .naive_utc()
}

fn three_device_case(stale_is_source: bool) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("never-held-secret.kdbx");
    let master = MasterPassword::new("tombstone synthetic master".to_owned());
    let mut client = Vault::create(
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
    let mut server = client.database().clone();
    let purged = {
        let mut root = server.root_mut();
        let mut entry = root.add_entry();
        entry.set_protected(fields::PASSWORD, "purged synthetic secret");
        entry.times.last_modification = Some(at(1));
        entry.times.location_changed = Some(at(1));
        entry.id()
    };
    let mut stale = server.clone();
    server.entry_mut(purged).unwrap().track_changes().remove();
    server.deleted_objects.insert(purged.uuid(), Some(at(10)));
    let unrelated = {
        let mut root = client.database_mut().root_mut();
        let mut entry = root.add_entry();
        entry.set_unprotected(fields::TITLE, "unrelated offline work");
        entry.times.last_modification = Some(at(20));
        entry.id()
    };
    // This client never saw the secret; its unrelated merged upload must
    // nevertheless retain the authority's deletion marker.
    reconcile(client.database_mut(), &server).unwrap();
    client.save().unwrap();
    drop(client);
    let reopened = Vault::open(&path, &master, None).unwrap();
    let mut new_authority = reopened.database().clone();
    if stale_is_source {
        reconcile(&mut new_authority, &stale).unwrap();
        assert!(
            new_authority.entry(purged).is_none(),
            "stale source must not resurrect a purged secret"
        );
        assert!(new_authority.entry(unrelated).is_some());
    } else {
        reconcile(&mut stale, &new_authority).unwrap();
        assert!(
            stale.entry(purged).is_none(),
            "stale destination must receive the retained purge"
        );
        assert!(stale.entry(unrelated).is_some());
    }
    assert_eq!(
        reopened.database().deleted_objects.get(&purged.uuid()),
        Some(&Some(at(10)))
    );
}

#[test]
fn retained_purge_prevents_resurrection_from_a_stale_third_source() {
    three_device_case(true);
}

#[test]
fn retained_purge_removes_the_secret_from_a_stale_third_destination() {
    three_device_case(false);
}

#[test]
fn tombstone_union_preserves_presence_and_the_newest_known_deletion_time() {
    let uuid = Uuid::from_bytes([17; 16]);
    for (left_marker, right_marker, expected) in [
        (None, Some(None), Some(None)),
        (Some(None), Some(Some(at(10))), Some(Some(at(10)))),
        (Some(Some(at(10))), Some(Some(at(30))), Some(Some(at(30)))),
        (Some(Some(at(30))), Some(Some(at(10))), Some(Some(at(30)))),
        (Some(Some(at(20))), None, Some(Some(at(20)))),
    ] {
        let mut left = Database::new();
        let mut right = left.clone();
        if let Some(marker) = left_marker {
            left.deleted_objects.insert(uuid, marker);
        }
        if let Some(marker) = right_marker {
            right.deleted_objects.insert(uuid, marker);
        }
        for _ in 0..3 {
            reconcile(&mut left, &right).unwrap();
            assert_eq!(left.deleted_objects.get(&uuid).copied(), expected);
        }
    }
}

//! Equal bytes may legitimately have distinct KDBX binary-pool identifiers.
use hidlins_core::{fields, Database, Value};
use hidlins_sync::reconcile;

fn at(seconds: i64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(1_700_000_000 + seconds, 0)
        .unwrap()
        .naive_utc()
}

fn repeated_union(same_second: bool, subset: bool) {
    let mut local = Database::new();
    let id = {
        let mut root = local.root_mut();
        let mut entry = root.add_entry();
        entry.set_protected(fields::PASSWORD, "first prior synthetic secret");
        entry.add_attachment(
            "same.bin",
            Value::protected(b"equal historical bytes".to_vec()),
        );
        entry.times.last_modification = Some(at(10));
        entry.id()
    };
    {
        let mut entry = local.entry_mut(id).unwrap();
        entry.edit_tracking(|entry| {
            entry.set_protected(fields::PASSWORD, "second prior synthetic secret");
        });
        // A normal attachment replacement allocates a fresh pool ID even
        // when the bytes are equal; history still owns the first allocation.
        entry.add_attachment(
            "same.bin",
            Value::protected(b"equal historical bytes".to_vec()),
        );
        entry.times.last_modification = Some(at(if same_second { 10 } else { 20 }));
        entry.edit_tracking(|entry| {
            entry.set_protected(fields::PASSWORD, "current synthetic secret");
        });
        entry.remove_attachment_by_name("same.bin");
        entry.times.last_modification = Some(at(30));
    }
    let mut remote = local.clone();
    if subset {
        let second = remote
            .entry(id)
            .unwrap()
            .history
            .as_ref()
            .unwrap()
            .get_entries()[0]
            .clone();
        {
            let mut entry = remote.entry_mut(id).unwrap();
            entry.history = None;
            entry.history.get_or_insert_default().add_entry(second);
        }
        remote.rebuild_attachment_references();
    }
    let before = local.clone();
    for _ in 0..5 {
        reconcile(&mut local, &remote).unwrap();
        let entry = local.entry(id).unwrap();
        let history = entry.history.as_ref().unwrap();
        assert_eq!(
            history.get_entries().len(),
            2,
            "repeated unions must preserve the exact version set without duplicate growth"
        );
        for password in [
            "first prior synthetic secret",
            "second prior synthetic secret",
        ] {
            assert_eq!(
                history
                    .get_entries()
                    .iter()
                    .filter(|past| past.get(fields::PASSWORD) == Some(password))
                    .count(),
                1
            );
        }
        for index in 0..2 {
            let past = entry.historical(index).unwrap();
            let attachment = past.attachment_by_name("same.bin").unwrap();
            assert!(attachment.data.is_protected());
            assert_eq!(attachment.data.as_slice(), b"equal historical bytes");
        }
        if !subset {
            assert!(
                local == before,
                "identical replicas must remain an exact no-op"
            );
        }
    }
}

#[test]
fn identical_histories_with_equal_bytes_at_distinct_pool_ids_are_a_noop() {
    repeated_union(false, false);
}

#[test]
fn same_second_distinct_history_versions_do_not_multiply() {
    repeated_union(true, false);
}

#[test]
fn same_second_subset_history_union_remains_idempotent() {
    repeated_union(true, true);
}

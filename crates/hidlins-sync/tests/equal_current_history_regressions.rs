//! Equal current values do not imply that both replicas have the same history.
use hidlins_core::{fields, KdfParams, MasterPassword, NoRecoveryConfirmed, Uuid, Vault};
use hidlins_sync::reconcile;

fn at(seconds: i64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(1_700_000_000 + seconds, 0)
        .unwrap()
        .naive_utc()
}

fn check_versions(vault: &Vault, uuid: Uuid, local_prior: bool) {
    let id = vault
        .database()
        .root()
        .entries()
        .find(|entry| entry.id().uuid() == uuid)
        .unwrap()
        .id();
    let entry = vault.database().entry(id).unwrap();
    assert_eq!(
        entry.get(fields::PASSWORD),
        Some("current synthetic secret")
    );
    let history = entry
        .history
        .as_ref()
        .expect("remote-only history must survive equal-current reconciliation");
    assert_eq!(history.get_entries().len(), if local_prior { 2 } else { 1 });
    for (password, name, bytes) in [
        (
            "remote prior synthetic secret",
            "remote.bin",
            b"remote historical bytes".as_slice(),
        ),
        (
            "local prior synthetic secret",
            "local.bin",
            b"local historical bytes".as_slice(),
        ),
    ]
    .into_iter()
    .take(if local_prior { 2 } else { 1 })
    {
        let index = history
            .get_entries()
            .iter()
            .position(|past| past.get(fields::PASSWORD) == Some(password))
            .expect("every prior secret version must be retained");
        let past = entry.historical(index).unwrap();
        assert!(past.fields.get(fields::PASSWORD).unwrap().is_protected());
        assert_eq!(
            past.attachment_by_name(name).unwrap().data.as_slice(),
            bytes
        );
    }
}

fn equal_current_case(local_prior: bool) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history.kdbx");
    let master = MasterPassword::new("equal-current synthetic master".to_owned());
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
    let id = {
        let mut root = vault.database_mut().root_mut();
        let mut entry = root.add_entry();
        entry.set_protected(fields::PASSWORD, "remote prior synthetic secret");
        entry.add_attachment(
            "remote.bin",
            hidlins_core::Value::protected(b"remote historical bytes".to_vec()),
        );
        entry.times.last_modification = Some(at(10));
        entry.id()
    };
    let uuid = id.uuid();
    {
        let mut entry = vault.database_mut().entry_mut(id).unwrap();
        entry.edit_tracking(|entry| {
            entry.set_protected(fields::PASSWORD, "current synthetic secret");
        });
        entry.remove_attachment_by_name("remote.bin");
        entry.times.last_modification = Some(at(30));
    }
    let remote = vault.database().clone();
    {
        let mut entry = vault.database_mut().entry_mut(id).unwrap();
        if local_prior {
            let mut past = entry.historical(0).unwrap();
            past.set_protected(fields::PASSWORD, "local prior synthetic secret");
            past.times.last_modification = Some(at(20));
            past.remove_attachment_by_name("remote.bin");
            past.add_attachment(
                "local.bin",
                hidlins_core::Value::protected(b"local historical bytes".to_vec()),
            );
        } else {
            entry.history = None;
        }
    }
    vault.database_mut().rebuild_attachment_references();
    vault.save().unwrap();
    for _ in 0..3 {
        reconcile(vault.database_mut(), &remote).unwrap();
        check_versions(&vault, uuid, local_prior);
    }
    vault.save().unwrap();
    drop(vault);
    let reopened = Vault::open(&path, &master, None).unwrap();
    check_versions(&reopened, uuid, local_prior);
}

#[test]
fn equal_current_recovers_remote_only_protected_history_and_attachment() {
    equal_current_case(false);
}

#[test]
fn equal_current_unions_both_histories_and_reindexes_foreign_attachment_bytes() {
    equal_current_case(true);
}

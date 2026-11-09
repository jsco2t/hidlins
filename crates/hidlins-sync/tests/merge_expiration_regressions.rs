//! Expiration flags and cutoffs are entry content, not disposable clock data.
use hidlins_core::{fields, KdfParams, MasterPassword, NoRecoveryConfirmed, Vault};
use hidlins_sync::reconcile;

fn at(seconds: i64) -> chrono::NaiveDateTime {
    chrono::DateTime::from_timestamp(1_700_000_000 + seconds, 0)
        .unwrap()
        .naive_utc()
}

#[derive(Clone, Copy)]
enum Case {
    Enable,
    Clear,
    LocalWins,
}

fn expiration_case(case: Case) {
    let (local_enabled, remote_enabled, local_time, remote_time) = match case {
        Case::Enable => (false, true, 10, 20),
        Case::Clear => (true, false, 10, 20),
        Case::LocalWins => (false, true, 20, 10),
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("expiration.kdbx");
    let master = MasterPassword::new("expiration merge synthetic master".to_owned());
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
        entry.set_protected(fields::PASSWORD, "unchanged synthetic secret");
        entry.times.last_modification = Some(at(local_time));
        entry.times.expires = Some(local_enabled);
        entry.times.expiry = Some(at(100));
        entry.id()
    };
    let mut remote = vault.database().clone();
    {
        let mut entry = remote.entry_mut(id).unwrap();
        entry.times.last_modification = Some(at(remote_time));
        entry.times.expires = Some(remote_enabled);
    }
    let winner = if remote_time > local_time {
        remote_enabled
    } else {
        local_enabled
    };
    let loser = if remote_time > local_time {
        local_enabled
    } else {
        remote_enabled
    };
    for _ in 0..3 {
        reconcile(vault.database_mut(), &remote).unwrap();
        let entry = vault.database().entry(id).unwrap();
        assert_eq!(
            entry.times.expires,
            Some(winner),
            "newer expiration content must win"
        );
        assert_eq!(
            entry.times.expiry,
            Some(at(100)),
            "disabled expiration retains its stored cutoff"
        );
        let history = entry
            .history
            .as_ref()
            .expect("expiration-only losing version must survive");
        assert_eq!(history.get_entries().len(), 1);
        assert_eq!(history.get_entries()[0].times.expires, Some(loser));
        assert_eq!(history.get_entries()[0].times.expiry, Some(at(100)));
        assert_eq!(
            history.get_entries()[0].get(fields::PASSWORD),
            Some("unchanged synthetic secret")
        );
    }
    vault.save().unwrap();
    drop(vault);
    let reopened = Vault::open(&path, &master, None).unwrap();
    let entry = reopened.database().entry(id).unwrap();
    assert_eq!(entry.times.expires, Some(winner));
    assert_eq!(entry.times.expiry, Some(at(100)));
    assert_eq!(
        entry.history.as_ref().unwrap().get_entries()[0]
            .times
            .expires,
        Some(loser)
    );
    assert_eq!(
        reopened
            .is_expired(
                id.uuid(),
                chrono::DateTime::from_naive_utc_and_offset(at(100), chrono::Utc)
            )
            .unwrap(),
        winner
    );
}

#[test]
fn newer_remote_enabling_expiration_survives_merge_and_reopen() {
    expiration_case(Case::Enable);
}

#[test]
fn newer_remote_clearing_expiration_preserves_disabled_cutoff() {
    expiration_case(Case::Clear);
}

#[test]
fn expiration_only_losing_remote_version_survives_in_history() {
    expiration_case(Case::LocalWins);
}

#[test]
fn equal_time_contradictory_expiration_fails_closed() {
    let mut local = hidlins_core::Database::new();
    let id = {
        let mut root = local.root_mut();
        let mut entry = root.add_entry();
        entry.times.last_modification = Some(at(10));
        entry.times.expires = Some(false);
        entry.times.expiry = Some(at(100));
        entry.id()
    };
    let mut remote = local.clone();
    remote.entry_mut(id).unwrap().times.expires = Some(true);
    let before = local.clone();
    assert!(
        reconcile(&mut local, &remote).is_err(),
        "same-second expiration conflict cannot choose an arbitrary winner"
    );
    assert!(local == before);
}

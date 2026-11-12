//! Real mobile ticket consumption must remain revocable through publication.

use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use hidlins_core::{EntryBuilder, MasterPassword, Vault};

use super::secrets::{
    consume_clipboard_transfer, purge_clipboard_transfers, purge_expired_clipboard_transfers,
};
use super::session::AppSession;
use crate::dto::{ClipboardTransferTicket, CopyField};
use crate::fixtures::{create_test_vault, TestEnv};

#[path = "../../../hidlins-core/tests/common/allocation_observer.rs"]
mod allocation_observer;

const MASTER: &str = "native-publication-fixture-master";
const SECRET: &str = "native-publication-synthetic-secret";
const WAIT: Duration = Duration::from_secs(5);
const BLOCKED: Duration = Duration::from_millis(400);

struct Fixture {
    session: Arc<AppSession>,
    entry: String,
    owner: u64,
    _env: TestEnv,
}

impl Fixture {
    fn new() -> Self {
        let env = TestEnv::new();
        let path = create_test_vault(&env.paths, "native", MASTER);
        let entry = {
            let mut vault = Vault::open(&path, &MasterPassword::new(MASTER.into()), None)
                .expect("fixture vault");
            let id = vault
                .add_entry(
                    vault.root_group_uuid(),
                    EntryBuilder::credential("native").password(SECRET).build(),
                )
                .expect("fixture entry");
            vault.save().expect("save fixture");
            id.to_string()
        };
        let session = Arc::new(AppSession::for_test(env.paths.clone()).expect("session"));
        session
            .unlock("native".into(), MASTER.into(), None)
            .expect("unlock");
        let owner = session.lock_state().clipboard_owner;
        Self {
            session,
            entry,
            owner,
            _env: env,
        }
    }

    fn ticket(&self) -> ClipboardTransferTicket {
        self.session
            .prepare_clipboard_transfer(self.entry.clone(), CopyField::Password)
            .expect("actual native ticket")
    }
}

fn check_lock_publication_order(sweep_pending: bool) {
    let fixture = Fixture::new();
    let ticket = fixture.ticket();
    let id = ticket.id.clone();
    let (entered, received) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let consuming = thread::spawn(move || {
        // Same lifetime as JNI/C: local Rust value spans the synchronous
        // receiver call and is released only after it finishes publishing.
        let value = consume_clipboard_transfer(&id).expect("consume");
        entered.send(()).unwrap();
        released.recv_timeout(WAIT).unwrap();
        assert_eq!(value.as_str(), SECRET);
    });
    received.recv_timeout(WAIT).unwrap();
    assert!(consume_clipboard_transfer(&ticket.id).is_err());
    let other = sweep_pending.then(Fixture::new);
    if let Some(other) = &other {
        purge_expired_clipboard_transfers();
        let pending = other.ticket(); // insertion also sweeps expired pending tickets
        drop(consume_clipboard_transfer(&pending.id).unwrap());
    }
    let (starting, started) = mpsc::channel();
    let (locked, observed) = mpsc::channel();
    let session = Arc::clone(&fixture.session);
    let locking = thread::spawn(move || {
        starting.send(()).unwrap();
        session.lock_now().unwrap();
        locked.send(()).unwrap();
    });
    started.recv_timeout(WAIT).unwrap();
    let returned_before_receiver = observed.recv_timeout(BLOCKED).is_ok();
    release.send(()).unwrap();
    consuming.join().unwrap();
    locking.join().unwrap();
    assert!(!fixture.session.has_vault());
    assert!(consume_clipboard_transfer(&ticket.id).is_err());
    assert!(
        !returned_before_receiver,
        "lock returned while a consumed native value could still publish"
    );
}

#[test]
fn lock_waits_for_the_started_native_receiver() {
    check_lock_publication_order(false);
}

#[test]
fn pending_cleanup_and_insertion_cannot_forget_an_active_receiver() {
    check_lock_publication_order(true);
}

#[test]
fn simultaneous_owner_purges_both_wait_for_the_same_consumption() {
    let fixture = Fixture::new();
    let ticket = fixture.ticket();
    let value = consume_clipboard_transfer(&ticket.id).unwrap();
    let (starting, started) = mpsc::channel();
    let (finished, observed) = mpsc::channel();
    let mut workers = Vec::new();
    for _ in 0..2 {
        let starting = starting.clone();
        let finished = finished.clone();
        let owner = fixture.owner;
        workers.push(thread::spawn(move || {
            starting.send(()).unwrap();
            purge_clipboard_transfers(owner);
            finished.send(()).unwrap();
        }));
    }
    started.recv_timeout(WAIT).unwrap();
    started.recv_timeout(WAIT).unwrap();
    let returned_before_drop = observed.recv_timeout(BLOCKED).is_ok();
    drop(value);
    for worker in workers {
        worker.join().unwrap();
    }
    assert!(
        !returned_before_drop,
        "each purge must retain the same active fence until native consumption ends"
    );
}

#[test]
fn unrelated_owner_can_stage_consume_and_lock_while_first_owner_waits() {
    let fixture = Fixture::new();
    let other = Fixture::new();
    let ticket = fixture.ticket();
    let value = consume_clipboard_transfer(&ticket.id).unwrap();
    let (starting, started) = mpsc::channel();
    let (locked, observed) = mpsc::channel();
    let session = Arc::clone(&fixture.session);
    let locking = thread::spawn(move || {
        starting.send(()).unwrap();
        session.lock_now().unwrap();
        locked.send(()).unwrap();
    });
    started.recv_timeout(WAIT).unwrap();
    let first_returned_early = observed.recv_timeout(BLOCKED).is_ok();
    let (progress, progressed) = mpsc::channel();
    let session = Arc::clone(&other.session);
    let entry = other.entry.clone();
    let independent = thread::spawn(move || {
        let ticket = session
            .prepare_clipboard_transfer(entry, CopyField::Password)
            .unwrap();
        let secret = consume_clipboard_transfer(&ticket.id).unwrap();
        assert_eq!(secret.as_str(), SECRET);
        drop(secret);
        session.lock_now().unwrap();
        progress.send(()).unwrap();
    });
    let other_progressed = progressed.recv_timeout(Duration::from_secs(2)).is_ok();
    drop(value); // release before joining even if a global-lock regression failed
    locking.join().unwrap();
    independent.join().unwrap();
    assert!(
        !first_returned_early,
        "first owner returned during native use"
    );
    assert!(
        other_progressed,
        "one owner's publication blocked another owner"
    );
}

#[test]
fn consumption_remains_single_use_while_active_and_after_drop() {
    let fixture = Fixture::new();
    let ticket = fixture.ticket();
    let value = consume_clipboard_transfer(&ticket.id).unwrap();
    assert_eq!(value.as_str(), SECRET);
    assert!(consume_clipboard_transfer(&ticket.id).is_err());
    drop(value);
    assert!(consume_clipboard_transfer(&ticket.id).is_err());
    fixture.session.lock_now().unwrap();
}

#[test]
fn lock_before_consumption_still_revokes_the_pending_ticket() {
    let fixture = Fixture::new();
    let ticket = fixture.ticket();
    fixture.session.lock_now().unwrap();
    assert!(consume_clipboard_transfer(&ticket.id).is_err());
}

#[test]
fn success_rejection_and_contained_rust_unwind_erase_actual_consumed_allocation() {
    for behavior in ["success", "reject", "unwind"] {
        let fixture = Fixture::new();
        let ticket = fixture.ticket();
        let value = consume_clipboard_transfer(&ticket.id).unwrap();
        let watch = allocation_observer::DeallocationWatch::new(value.as_bytes());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let value = value;
            assert_eq!(value.as_str(), SECRET);
            match behavior {
                "reject" => Err(()),
                "unwind" => panic!("synthetic contained Rust receiver panic"),
                _ => Ok(()),
            }
        }));
        match behavior {
            "reject" => assert!(matches!(result, Ok(Err(())))),
            "unwind" => assert!(result.is_err()),
            _ => assert!(matches!(result, Ok(Ok(())))),
        }
        watch.assert_erased();
        assert!(consume_clipboard_transfer(&ticket.id).is_err());
        fixture.session.lock_now().unwrap();
    }
}

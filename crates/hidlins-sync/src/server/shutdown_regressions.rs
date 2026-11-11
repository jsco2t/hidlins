//! Shutdown remains authoritative while host work is loaned to another thread.
use std::cell::RefCell;
use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::thread;
use std::time::Instant;

use hidlins_core::{KdfParams, NoRecoveryConfirmed};
use tempfile::TempDir;

use super::*;
use crate::address::LocalEndpoint;
use crate::client::{LanCancellation, LanTransport};
use crate::noise::NoiseKeypair;
use crate::SyncTransport;

thread_local! {
    static OPERATION_HOOK: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

pub(super) fn before_operation() {
    if let Some(hook) = OPERATION_HOOK.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
}

fn vault() -> (TempDir, Vault, MasterPassword) {
    let directory = tempfile::tempdir().unwrap();
    let master = MasterPassword::new("shutdown fixture password".into());
    let vault = Vault::create(
        &directory.path().join("server.kdbx"),
        &master,
        None,
        KdfParams {
            memory_kib: 1024,
            iterations: 1,
            parallelism: 1,
        },
        NoRecoveryConfirmed::yes(),
    )
    .unwrap();
    (directory, vault, master)
}

fn stage(vault: &Vault) -> StagedUpload {
    let bytes = std::fs::read(vault.path()).unwrap();
    let digest = Sha256::digest(&bytes).into();
    let mut stage = StagedUpload::new(
        vault.path(),
        Some(RemoteVersion::new(digest)),
        bytes.len() as u64,
        digest,
    )
    .unwrap();
    stage.write_chunk(0, &bytes).unwrap();
    stage.finish(1).unwrap()
}

fn controller(queue: &HostQueue, peer: Option<[u8; 32]>) -> (ServerController, [u8; 32]) {
    let identity = Arc::new(NoiseKeypair::generate().unwrap());
    let key = identity.public_key();
    let server = (0..8)
        .find_map(|_| {
            let probe = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = probe.local_addr().unwrap().port();
            drop(probe);
            ServerController::start(
                LocalEndpoint::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port, 0).unwrap(),
                Arc::clone(&identity),
                peer,
                queue.clone(),
            )
            .ok()
        })
        .expect("actual local server started");
    (server, key)
}

#[test]
fn controller_stop_rejects_future_submissions_without_processor_ownership() {
    let (queue, _loaned_processor) = HostQueue::new();
    let (mut server, _) = controller(&queue, None);
    server.stop();
    assert!(matches!(
        queue.submit(HostOperation::Head),
        Err(ServerError::Stopped)
    ));
}

#[test]
fn dequeued_head_cannot_execute_after_processor_shutdown() {
    let (_directory, mut vault, master) = vault();
    let (queue, mut processor) = HostQueue::new();
    let ticket = queue.submit(HostOperation::Head).unwrap();
    let pending = processor.take_one().unwrap().unwrap();
    processor.shutdown();
    pending
        .execute(&mut AuthoritativeVault::new(&mut vault, &master, None))
        .unwrap();
    assert!(matches!(
        ticket.wait_timeout(Duration::from_secs(1)),
        Err(ServerError::Stopped)
    ));
}

#[test]
fn dequeued_commit_cannot_change_ciphertext_after_shutdown() {
    let (_directory, mut vault, master) = vault();
    let before = std::fs::read(vault.path()).unwrap();
    let (queue, mut processor) = HostQueue::new();
    let ticket = queue.submit(HostOperation::Commit(stage(&vault))).unwrap();
    let pending = processor.take_one().unwrap().unwrap();
    processor.shutdown();
    pending
        .execute(&mut AuthoritativeVault::new(&mut vault, &master, None))
        .unwrap();
    assert!(
        std::fs::read(vault.path()).unwrap() == before,
        "canonical ciphertext unchanged"
    );
    assert!(matches!(
        ticket.wait_timeout(Duration::from_secs(1)),
        Err(ServerError::Stopped)
    ));
}

#[test]
fn controller_stop_promptly_releases_authenticated_host_waiter_during_loan() {
    let (queue, mut loaned_processor) = HostQueue::new();
    let client = NoiseKeypair::generate().unwrap();
    let (mut server, server_key) = controller(&queue, Some(client.public_key()));
    let endpoint = server.endpoint();
    let cancellation = LanCancellation::default();
    let worker_cancellation = cancellation.clone();
    let worker = thread::spawn(move || {
        LanTransport::new(client, server_key, None, vec![endpoint])
            .unwrap()
            .with_cancellation(worker_cancellation)
            .head()
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let pending = loop {
        if let Some(pending) = loaned_processor.take_one().unwrap() {
            break Some(pending);
        }
        if worker.is_finished() || Instant::now() >= deadline {
            break None;
        }
        thread::yield_now();
    };
    if pending.is_none() {
        cancellation.cancel();
    }
    let started = Instant::now();
    server.stop();
    let stopped_in = started.elapsed();
    let result = worker.join().unwrap();
    assert!(
        pending.is_some(),
        "actual authenticated Head reached the host queue"
    );
    drop(pending);
    assert!(result.is_err());
    assert!(
        stopped_in < Duration::from_secs(2),
        "Stop must release its host waiter promptly; took {stopped_in:?}"
    );
}

#[test]
fn controller_stop_waits_for_already_executing_commit_before_returning() {
    let (_directory, mut vault, master) = vault();
    let before = std::fs::read(vault.path()).unwrap();
    let (queue, mut processor) = HostQueue::new();
    let ticket = queue.submit(HostOperation::Commit(stage(&vault))).unwrap();
    let pending = processor.take_one().unwrap().unwrap();
    let (mut server, _) = controller(&queue, None);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let executor = thread::spawn(move || {
        OPERATION_HOOK.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || {
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }));
        });
        pending
            .execute(&mut AuthoritativeVault::new(&mut vault, &master, None))
            .unwrap();
        std::fs::read(vault.path()).unwrap()
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let (stopped_tx, stopped_rx) = mpsc::channel();
    let stopper = thread::spawn(move || {
        server.stop();
        stopped_tx.send(()).unwrap();
    });
    let stopped_before_release = stopped_rx.recv_timeout(Duration::from_millis(400)).is_ok();
    release_tx.send(()).unwrap();
    let committed_bytes = executor.join().unwrap();
    stopper.join().unwrap();
    if !stopped_before_release {
        stopped_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    }
    drop(ticket);
    assert_ne!(
        committed_bytes, before,
        "already executing atomic commit completed"
    );
    assert!(
        !stopped_before_release,
        "Stop returned before the active commit could finish"
    );
}

#[test]
fn ordinary_queued_commit_still_succeeds() {
    let (_directory, mut vault, master) = vault();
    let before = std::fs::read(vault.path()).unwrap();
    let (queue, mut processor) = HostQueue::new();
    let ticket = queue.submit(HostOperation::Commit(stage(&vault))).unwrap();
    processor
        .process_one(&mut AuthoritativeVault::new(&mut vault, &master, None))
        .unwrap();
    assert!(matches!(
        ticket.wait_timeout(Duration::from_secs(1)),
        Ok(HostResponse::Committed(_))
    ));
    assert_ne!(std::fs::read(vault.path()).unwrap(), before);
}

#[test]
fn cancelled_pending_commit_preserves_ciphertext() {
    let (_directory, mut vault, master) = vault();
    let before = std::fs::read(vault.path()).unwrap();
    let (queue, mut processor) = HostQueue::new();
    let ticket = queue.submit(HostOperation::Commit(stage(&vault))).unwrap();
    let pending = processor.take_one().unwrap().unwrap();
    ticket.cancel();
    pending
        .execute(&mut AuthoritativeVault::new(&mut vault, &master, None))
        .unwrap();
    assert!(matches!(
        ticket.wait_timeout(Duration::from_secs(1)),
        Err(ServerError::Cancelled)
    ));
    assert!(
        std::fs::read(vault.path()).unwrap() == before,
        "canonical ciphertext unchanged"
    );
}

#[test]
fn live_ticket_retains_original_timeout_budget() {
    let (queue, _processor) = HostQueue::new();
    let ticket = queue.submit(HostOperation::Head).unwrap();
    let started = Instant::now();
    assert!(matches!(
        ticket.wait_timeout(Duration::from_millis(40)),
        Err(ServerError::TimedOut)
    ));
    assert!(started.elapsed() >= Duration::from_millis(40));
    assert!(started.elapsed() < Duration::from_secs(1));
}

//! Ownership, concurrency, restart and backend cleanup controls.
use super::*;
use crate::fixtures::{create_test_vault, TestEnv};
use hidlins_sync::discovery::{
    DiscoveryBatch, DiscoveryError, DiscoveryEvent, DiscoveryPort, RawDiscoveryRecord, RawEndpoint,
};
use std::cell::RefCell;
use std::sync::atomic::AtomicUsize;

type Hook = Box<dyn FnOnce()>;
thread_local! {
    static HOOKS: RefCell<[Option<Hook>; 3]> = const { RefCell::new([None, None, None]) };
}
fn run(index: usize) {
    let hook = HOOKS.with(|slots| slots.borrow_mut()[index].take());
    if let Some(hook) = hook {
        hook();
    }
}
fn install(index: usize, hook: impl FnOnce() + 'static) {
    HOOKS.with(|slots| assert!(slots.borrow_mut()[index].replace(Box::new(hook)).is_none()));
}
pub(super) fn after_claim() {
    run(0);
}
pub(super) fn before_publish() {
    run(1);
}
pub(super) fn after_stop_request() {
    run(2);
}

struct Release(Option<mpsc::Sender<()>>);
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}
fn gate() -> (
    mpsc::Sender<()>,
    mpsc::Receiver<()>,
    Release,
    mpsc::Receiver<()>,
) {
    let (entered, ready) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    (entered, ready, Release(Some(release)), wait)
}
fn wait(receiver: &mpsc::Receiver<()>) {
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("controlled fixture readiness; not a behavioral red");
}

// Record delivery, injected faults and shutdown are independent test axes.
#[allow(clippy::struct_excessive_bools)]
struct Port {
    owner: Weak<AppSession>,
    polls: Arc<AtomicUsize>,
    shutdowns: Arc<AtomicUsize>,
    first: bool,
    second: Arc<AtomicBool>,
    first_route: u8,
    gate: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
    poll_error: bool,
    shutdown_error: bool,
    stopped: bool,
}
impl DiscoveryPort for Port {
    fn poll(&mut self) -> Result<DiscoveryBatch, DiscoveryError> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        if let Some((entered, release)) = self.gate.take() {
            entered.send(()).unwrap();
            wait(&release);
        }
        if self.poll_error {
            return Err(DiscoveryError::Backend);
        }
        let last = if std::mem::take(&mut self.first) {
            Some(self.first_route)
        } else if self.second.swap(false, Ordering::AcqRel) {
            Some(self.first_route + 1)
        } else {
            None
        };
        let events = last.map_or_else(Vec::new, |last| {
            vec![DiscoveryEvent::Resolved(
                RawDiscoveryRecord::try_from_untrusted(
                    ServiceKind::Trusted.service_type().into(),
                    format!(
                        "000000000000000000000000{last:02}.{}",
                        ServiceKind::Trusted.service_type()
                    ),
                    48_103,
                    vec![("v".into(), b"1".to_vec())],
                    vec![RawEndpoint::new(
                        std::net::Ipv4Addr::new(192, 168, 1, last).into(),
                        0,
                    )],
                    Duration::from_secs(120),
                )
                .unwrap(),
            )]
        });
        DiscoveryBatch::new(DiscoveryPermission::Granted, events)
    }
    fn shutdown(&mut self) -> Result<(), DiscoveryError> {
        if !self.stopped {
            self.stopped = true;
            if let Some(owner) = self.owner.upgrade() {
                assert!(
                    owner.inner.try_lock().is_ok(),
                    "backend shutdown must not own State"
                );
                assert!(
                    owner.discovery_port.try_lock().is_ok(),
                    "backend shutdown must not own browser guard"
                );
            }
            self.shutdowns.fetch_add(1, Ordering::SeqCst);
        }
        if self.shutdown_error {
            Err(DiscoveryError::Backend)
        } else {
            Ok(())
        }
    }
}
impl Drop for Port {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
fn port(owner: &Arc<AppSession>, first_route: u8) -> Port {
    Port {
        owner: Arc::downgrade(owner),
        polls: Arc::new(AtomicUsize::new(0)),
        shutdowns: Arc::new(AtomicUsize::new(0)),
        first: true,
        second: Arc::new(AtomicBool::new(false)),
        first_route,
        gate: None,
        poll_error: false,
        shutdown_error: false,
        stopped: false,
    }
}
fn session() -> (TestEnv, Arc<AppSession>) {
    let env = TestEnv::new();
    create_test_vault(&env.paths, "discovery", "discovery stop control");
    let session = Arc::new(AppSession::for_test(env.paths.clone()).unwrap());
    session
        .unlock("discovery".into(), "discovery stop control".into(), None)
        .unwrap();
    (env, session)
}

#[test]
fn valid_queued_polls_preserve_records_published_by_preceding_poll() {
    let (_env, session) = session();
    let backend = port(&session, 10);
    let second = Arc::clone(&backend.second);
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    let (entered, ready, release, waiting) = gate();
    let caller = Arc::clone(&session);
    let first = thread::spawn(move || {
        install(1, move || {
            entered.send(()).unwrap();
            wait(&waiting);
        });
        caller.poll_local_discovery()
    });
    wait(&ready);
    let (claimed, claim_ready) = mpsc::channel();
    let caller = Arc::clone(&session);
    let queued = thread::spawn(move || {
        install(0, move || claimed.send(()).unwrap());
        caller.poll_local_discovery()
    });
    wait(&claim_ready);
    second.store(true, Ordering::Release);
    drop(release);
    assert_eq!(first.join().unwrap().unwrap().candidates.len(), 1);
    let result = queued.join().unwrap().unwrap();
    assert_eq!(
        result
            .candidates
            .iter()
            .map(|r| r.address.as_str())
            .collect::<Vec<_>>(),
        ["192.168.1.10", "192.168.1.11"]
    );
    session.stop_local_discovery().unwrap();
}

#[test]
fn stop_cancels_active_and_already_queued_polls_without_publication() {
    let (_env, session) = session();
    let mut backend = port(&session, 10);
    let polls = Arc::clone(&backend.polls);
    let shutdowns = Arc::clone(&backend.shutdowns);
    let (entered, ready, release, waiting) = gate();
    backend.gate = Some((entered, waiting));
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    let caller = Arc::clone(&session);
    let active = thread::spawn(move || caller.poll_local_discovery());
    wait(&ready);
    let (claimed, claim_ready) = mpsc::channel();
    let caller = Arc::clone(&session);
    let queued = thread::spawn(move || {
        install(0, move || claimed.send(()).unwrap());
        caller.poll_local_discovery()
    });
    wait(&claim_ready);
    let (canceled, cancel_ready) = mpsc::channel();
    let caller = Arc::clone(&session);
    let stopping = thread::spawn(move || {
        install(2, move || canceled.send(()).unwrap());
        caller.stop_local_discovery()
    });
    wait(&cancel_ready);
    drop(release);
    assert!(matches!(
        active.join().unwrap(),
        Err(HidlinsApiError::SyncCanceled)
    ));
    assert!(matches!(
        queued.join().unwrap(),
        Err(HidlinsApiError::SyncCanceled)
    ));
    stopping.join().unwrap().unwrap();
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
    assert!(session.local_discovery_status().candidates.is_empty());
    assert!(session.has_vault());
}

#[test]
fn older_concurrent_stop_never_takes_a_restarted_browser() {
    let (_env, session) = session();
    let backend = port(&session, 10);
    let old_shutdowns = Arc::clone(&backend.shutdowns);
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    session.poll_local_discovery().unwrap();
    let (entered, ready, release, waiting) = gate();
    let caller = Arc::clone(&session);
    let first = thread::spawn(move || {
        install(2, move || {
            entered.send(()).unwrap();
            wait(&waiting);
        });
        caller.stop_local_discovery()
    });
    wait(&ready);
    let (entered, ready, old_release, waiting) = gate();
    let caller = Arc::clone(&session);
    let old = thread::spawn(move || {
        install(2, move || {
            entered.send(()).unwrap();
            wait(&waiting);
        });
        caller.stop_local_discovery()
    });
    wait(&ready);
    drop(release);
    first.join().unwrap().unwrap();
    assert_eq!(old_shutdowns.load(Ordering::SeqCst), 1);
    let replacement = port(&session, 20);
    let new_shutdowns = Arc::clone(&replacement.shutdowns);
    *session.discovery_port.lock().unwrap() = Some(Box::new(replacement));
    assert_eq!(session.poll_local_discovery().unwrap().candidates.len(), 2);
    drop(old_release);
    old.join().unwrap().unwrap();
    assert_eq!(new_shutdowns.load(Ordering::SeqCst), 0);
    assert!(session.discovery_port.lock().unwrap().is_some());
    session.stop_local_discovery().unwrap();
    assert_eq!(new_shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn repeated_stop_preserves_cache_manual_routes_and_restart() {
    let (_env, session) = session();
    let backend = port(&session, 10);
    let shutdowns = Arc::clone(&backend.shutdowns);
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    session.poll_local_discovery().unwrap();
    let manual = LocalEndpointDto {
        address: "192.168.1.30".into(),
        port: 48_105,
        scope_id: 0,
    };
    session
        .set_discovery_candidates(DiscoveryPermissionDto::Granted, vec![manual.clone()])
        .unwrap();
    let before = session.local_discovery_status();
    session.stop_local_discovery().unwrap();
    session.stop_local_discovery().unwrap();
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
    assert_eq!(session.local_discovery_status(), before);
    *session.discovery_port.lock().unwrap() = Some(Box::new(port(&session, 20)));
    let restarted = session.poll_local_discovery().unwrap();
    assert_eq!(restarted.candidates.first(), Some(&manual));
    assert_eq!(restarted.candidates.len(), 3);
    assert!(session.has_vault());
    session.stop_local_discovery().unwrap();
}

#[test]
fn backend_errors_release_locks_and_allow_a_later_restart() {
    let (_env, session) = session();
    let mut backend = port(&session, 10);
    backend.poll_error = true;
    backend.shutdown_error = true;
    let shutdowns = Arc::clone(&backend.shutdowns);
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    assert!(session.poll_local_discovery().is_err());
    assert!(session.stop_local_discovery().is_err());
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
    *session.discovery_port.lock().unwrap() = Some(Box::new(port(&session, 20)));
    assert_eq!(session.poll_local_discovery().unwrap().candidates.len(), 1);
    session.stop_local_discovery().unwrap();
}

#[test]
fn stop_recovers_poisoned_metadata_mutexes_without_reusing_the_old_browser() {
    let (_env, session) = session();
    let backend = port(&session, 10);
    let shutdowns = Arc::clone(&backend.shutdowns);
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _guard = session.discovery_operation.lock().unwrap();
        panic!("controlled discovery operation poison");
    }));
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _guard = session.discovery_port.lock().unwrap();
        panic!("controlled browser poison");
    }));
    session.stop_local_discovery().unwrap();
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
    assert!(!session.discovery_operation.is_poisoned());
    assert!(!session.discovery_port.is_poisoned());
    *session.discovery_port.lock().unwrap() = Some(Box::new(port(&session, 20)));
    assert_eq!(session.poll_local_discovery().unwrap().candidates.len(), 1);
    session.stop_local_discovery().unwrap();
}

#[test]
fn stopping_discovery_preserves_the_independent_server_listener() {
    let (_env, session) = session();
    session
        .configure_local_sync(LocalSyncRoleDto::Server)
        .unwrap();
    let endpoint = (0..8)
        .find_map(|_| {
            let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = probe.local_addr().unwrap().port();
            drop(probe);
            session
                .start_sync_server(LocalEndpointDto {
                    address: "127.0.0.1".into(),
                    port,
                    scope_id: 0,
                })
                .ok()
        })
        .expect("actual private server starts");
    *session.discovery_port.lock().unwrap() = Some(Box::new(port(&session, 10)));
    session.poll_local_discovery().unwrap();
    session.stop_local_discovery().unwrap();
    assert!(session.local_sync_status().unwrap().server_running);
    assert!(std::net::TcpListener::bind(("127.0.0.1", endpoint.port)).is_err());
    session.open_pairing_window().unwrap();
    session.close_pairing_window();
    assert!(session.has_vault());
    session.stop_sync_server();
}

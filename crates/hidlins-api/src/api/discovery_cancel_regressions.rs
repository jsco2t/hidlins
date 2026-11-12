//! Discovery shutdown boundary and cancellation regressions.
use std::cell::RefCell;
use std::sync::atomic::AtomicUsize;

use super::*;
use crate::fixtures::{create_test_vault, TestEnv};
use hidlins_sync::discovery::{
    DiscoveryBatch, DiscoveryError, DiscoveryEvent, DiscoveryPort, RawDiscoveryRecord, RawEndpoint,
};

thread_local! {
    static BEFORE_SHUTDOWN_WAIT: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

pub(crate) fn before_shutdown_discovery_wait() {
    let hook = BEFORE_SHUTDOWN_WAIT.with(|slot| slot.borrow_mut().take());
    if let Some(hook) = hook {
        hook();
    }
}

struct Port {
    entered: Option<mpsc::Sender<()>>,
    release: Option<mpsc::Receiver<()>>,
    polls: Arc<AtomicUsize>,
    shutdowns: Arc<AtomicUsize>,
    record: bool,
    fail: bool,
    stopped: bool,
}

impl DiscoveryPort for Port {
    fn poll(&mut self) -> Result<DiscoveryBatch, DiscoveryError> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        if let Some(entered) = self.entered.take() {
            entered.send(()).unwrap();
        }
        if let Some(release) = self.release.take() {
            release
                .recv_timeout(Duration::from_secs(5))
                .expect("fixture releases gated poll; timeout is not a behavioral red");
        }
        if self.fail {
            return Err(DiscoveryError::Backend);
        }
        let events = if std::mem::take(&mut self.record) {
            vec![DiscoveryEvent::Resolved(
                RawDiscoveryRecord::try_from_untrusted(
                    ServiceKind::Trusted.service_type().into(),
                    format!(
                        "00000000000000000000000001.{}",
                        ServiceKind::Trusted.service_type()
                    ),
                    48_103,
                    vec![("v".into(), b"1".to_vec())],
                    vec![RawEndpoint::new("192.168.1.30".parse().unwrap(), 0)],
                    Duration::from_secs(120),
                )
                .unwrap(),
            )]
        } else {
            Vec::new()
        };
        DiscoveryBatch::new(DiscoveryPermission::Granted, events)
    }

    fn shutdown(&mut self) -> Result<(), DiscoveryError> {
        if !self.stopped {
            self.stopped = true;
            self.shutdowns.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

impl Drop for Port {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn port(fail: bool) -> (Port, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let polls = Arc::new(AtomicUsize::new(0));
    let shutdowns = Arc::new(AtomicUsize::new(0));
    (
        Port {
            entered: None,
            release: None,
            polls: Arc::clone(&polls),
            shutdowns: Arc::clone(&shutdowns),
            record: true,
            fail,
            stopped: false,
        },
        polls,
        shutdowns,
    )
}

fn session() -> (TestEnv, Arc<AppSession>) {
    let env = TestEnv::new();
    create_test_vault(&env.paths, "discovery", "discovery cancellation fixture");
    let session = Arc::new(AppSession::for_test(env.paths.clone()).unwrap());
    session
        .unlock(
            "discovery".into(),
            "discovery cancellation fixture".into(),
            None,
        )
        .unwrap();
    (env, session)
}

struct Release(Option<mpsc::Sender<()>>);
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

struct ShutdownObservation {
    revoked: bool,
    poll_canceled: bool,
    candidates: usize,
    shutdowns: usize,
}

fn gated_shutdown() -> ShutdownObservation {
    let (_env, session) = session();
    let (mut backend, _, shutdowns) = port(false);
    let (entered, poll_entered) = mpsc::channel();
    let (release, poll_release) = mpsc::channel();
    let release = Release(Some(release));
    backend.entered = Some(entered);
    backend.release = Some(poll_release);
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    let caller = Arc::clone(&session);
    let poll = thread::spawn(move || caller.poll_local_discovery());
    poll_entered.recv_timeout(Duration::from_secs(3)).unwrap();
    let (entered, shutdown_entered) = mpsc::channel();
    let caller = Arc::clone(&session);
    let shutdown = thread::spawn(move || {
        BEFORE_SHUTDOWN_WAIT.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move || entered.send(()).unwrap()));
        });
        caller.shutdown();
    });
    shutdown_entered
        .recv_timeout(Duration::from_secs(3))
        .unwrap();
    let revoked = {
        let state = session.lock_state();
        state.dead && state.vault.is_none() && state.credentials.is_none()
    };
    drop(release);
    let result = poll.join().unwrap();
    shutdown.join().unwrap();
    let candidates = session
        .lock_state()
        .discovery_cache
        .candidates(ServiceKind::Trusted)
        .len();
    ShutdownObservation {
        revoked,
        poll_canceled: matches!(result, Err(HidlinsApiError::SyncCanceled)),
        candidates,
        shutdowns: shutdowns.load(Ordering::SeqCst),
    }
}

#[test]
fn shutdown_revokes_vault_and_credentials_before_discovery_wait() {
    let observed = gated_shutdown();
    assert!(
        observed.revoked,
        "shutdown must revoke the session before waiting for a discovery owner"
    );
    assert_eq!(observed.shutdowns, 1);
}

#[test]
fn shutdown_prevents_gated_poll_from_publishing_routes() {
    let observed = gated_shutdown();
    assert!(observed.poll_canceled, "superseded poll must be canceled");
    assert_eq!(observed.candidates, 0, "canceled poll must not publish");
    assert_eq!(observed.shutdowns, 1);
}

#[test]
fn dead_session_never_polls_or_republishes_an_injected_backend() {
    let (_env, session) = session();
    session.shutdown();
    let (backend, polls, shutdowns) = port(false);
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    let result = session.poll_local_discovery();
    session.shutdown();
    assert!(matches!(result, Err(HidlinsApiError::SyncCanceled)));
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn poisoned_session_never_begins_discovery() {
    let (_env, session) = session();
    let (backend, polls, _) = port(false);
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _guard = session.inner.lock().unwrap();
        panic!("controlled discovery state poison");
    }));
    let result = session.poll_local_discovery();
    session.shutdown();
    assert!(matches!(result, Err(HidlinsApiError::SyncCanceled)));
    assert_eq!(polls.load(Ordering::SeqCst), 0);
}

#[test]
fn ordinary_poll_retains_routes_and_shuts_backend_once() {
    let (_env, session) = session();
    let (backend, _, shutdowns) = port(false);
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    let status = session.poll_local_discovery().unwrap();
    assert_eq!(status.candidates.len(), 1);
    assert!(session.has_vault());
    session.shutdown();
    session.shutdown();
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn backend_error_releases_browser_for_shutdown() {
    let (_env, session) = session();
    let (backend, _, shutdowns) = port(true);
    *session.discovery_port.lock().unwrap() = Some(Box::new(backend));
    assert!(session.poll_local_discovery().is_err());
    assert!(session.has_vault());
    session.shutdown();
    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

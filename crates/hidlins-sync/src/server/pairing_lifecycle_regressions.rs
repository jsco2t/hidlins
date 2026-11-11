//! Deterministic interleavings at the private lifecycle boundary.
use super::*;
use crate::{client::ClientPairingSession, discovery::ServiceKind, identity::PublicIdentity};
use std::{cell::RefCell, sync::mpsc};

thread_local! {
    static EXPIRY_INTERLEAVING: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

pub(super) fn after_expiry_check() {
    EXPIRY_INTERLEAVING.with(|hook| {
        let callback = hook.borrow_mut().take();
        if let Some(callback) = callback {
            callback();
        }
    });
}

fn server(
    authority: Option<Arc<dyn PairingAuthority>>,
) -> (ServerController, super::super::HostProcessor) {
    let (queue, processor) = HostQueue::new();
    let server = ServerController::start_on_bind_endpoint_with_pairing_authority(
        LocalBindEndpoint::ephemeral("127.0.0.1".parse().unwrap(), 0).unwrap(),
        Arc::new(NoiseKeypair::generate().unwrap()),
        [],
        queue,
        authority,
    )
    .unwrap();
    server.open_pairing().unwrap();
    (server, processor)
}

fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "bounded runtime state transition"
        );
        thread::yield_now();
    }
}

fn seed_replacement(pairing: &PairingRuntimeState) -> (PairingAuthorization, SasCode) {
    let permission = pairing.authorization().unwrap();
    let peer = PublicIdentity::new([31; 32]);
    let sas = pairing
        .window
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .begin_candidate(peer, [32; 32], Instant::now())
        .unwrap();
    pairing
        .trusted
        .prepare(peer.into_bytes(), &permission)
        .unwrap();
    (permission, sas)
}

fn assert_replacement_survives(
    pairing: &PairingRuntimeState,
    permission: &PairingAuthorization,
    sas: SasCode,
) {
    permission
        .check()
        .expect("replacement window remains authorized");
    assert!(pairing.authorization().unwrap().same_window(permission));
    assert!(pairing.trusted.recovery_matches([31; 32], permission));
    let mut window = pairing.window.lock().unwrap();
    assert_eq!(
        window.as_ref().unwrap().failures(),
        0,
        "older worker cannot consume new failure budget"
    );
    window
        .as_mut()
        .unwrap()
        .confirm(sas, Instant::now())
        .expect("new candidate remains pending");
    drop(window);
    assert!(pairing
        .active_advertisements(Instant::now())
        .iter()
        .any(|ad| ad.kind() == ServiceKind::Pairing));
}

#[test]
fn expiry_observed_before_replacement_cannot_close_the_replacement() {
    let (server, _processor) = server(None);
    let pairing = Arc::clone(&server.pairing);
    *pairing.window.lock().unwrap() = Some(PairingWindow::open_at(
        Instant::now().checked_sub(PAIRING_WINDOW).unwrap(),
    ));
    let replacement = Arc::new(Mutex::new(None));
    let captured = Arc::clone(&replacement);
    let replacing = Arc::clone(&pairing);
    EXPIRY_INTERLEAVING.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            // Actual new-window transition interleaves after the expiry observation.
            replacing.open().unwrap();
            *captured.lock().unwrap() = Some(seed_replacement(&replacing));
        }));
    });
    pairing.expire(Instant::now());
    let (permission, sas) = replacement.lock().unwrap().take().unwrap();
    assert_replacement_survives(&pairing, &permission, sas);
}

fn start_bad_handshake(server: &ServerController) -> TcpStream {
    let mut stream = TcpStream::connect(server.endpoint().socket_addr()).unwrap();
    stream
        .write_all(&Preface::new(PrefaceMode::Pairing).encode())
        .unwrap();
    wait_until(|| {
        server
            .pairing
            .sockets
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .any(|c| c.identity == ConnectionIdentity::Pairing)
    });
    stream
}

fn finish_bad_handshake(mut stream: TcpStream, server: &ServerController) {
    // A replacement may already have closed this old-generation socket.
    let _ = stream.write_all(&[0, 1, 0]);
    drop(stream);
    wait_until(|| {
        server
            .pairing
            .sockets
            .lock()
            .unwrap()
            .iter()
            .all(Option::is_none)
    });
}

#[test]
fn old_admitted_handshake_failure_does_not_reject_the_new_candidate() {
    let (server, _processor) = server(None);
    let stream = start_bad_handshake(&server);
    server.open_pairing().unwrap();
    let (permission, sas) = seed_replacement(&server.pairing);
    finish_bad_handshake(stream, &server);
    assert_replacement_survives(&server.pairing, &permission, sas);
}

struct DelayedRejection {
    entered: mpsc::SyncSender<()>,
    release: Mutex<mpsc::Receiver<()>>,
}

impl PairingAuthority for DelayedRejection {
    fn confirm(
        &self,
        _peer: PublicIdentity,
        _sas: SasCode,
        _timeout: Duration,
    ) -> Result<String, PairingAuthorityError> {
        self.entered.send(()).unwrap();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        Err(PairingAuthorityError::Canceled)
    }
    fn prepare(
        &self,
        _tx: &PairingTransaction,
        _proof: &ConfirmedPairing,
        _name: String,
    ) -> Result<(), PairingAuthorityError> {
        unreachable!("the test always rejects confirmation")
    }
    fn activate(&self, _tx: &PairingTransaction) -> Result<(), PairingAuthorityError> {
        unreachable!("no prepare")
    }
    fn recover(&self, _peer: PublicIdentity) -> Result<PairingTransaction, PairingAuthorityError> {
        unreachable!("no prepared receipt")
    }
    fn shutdown(&self) {}
}

#[test]
fn old_confirmation_rejection_does_not_reject_the_new_candidate() {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let authority = Arc::new(DelayedRejection {
        entered: entered_tx,
        release: Mutex::new(release_rx),
    });
    let (server, _processor) = server(Some(authority));
    let identity = NoiseKeypair::generate().unwrap();
    let old_session = ClientPairingSession::begin(&identity, [server.endpoint()]).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    server.open_pairing().unwrap();
    let (permission, sas) = seed_replacement(&server.pairing);
    release_tx.send(()).unwrap();
    wait_until(|| {
        server
            .pairing
            .sockets
            .lock()
            .unwrap()
            .iter()
            .all(Option::is_none)
    });
    drop(old_session);
    assert_replacement_survives(&server.pairing, &permission, sas);
}

#[test]
fn current_generation_handshake_failures_still_exhaust_the_window() {
    let (server, _processor) = server(None);
    for _ in 0..crate::protocol::MAX_PAIRING_FAILURES {
        let stream = start_bad_handshake(&server);
        finish_bad_handshake(stream, &server);
    }
    assert!(!server.pairing.is_open(Instant::now()));
    assert!(server.pairing.authorization().is_none());
    assert!(!server
        .pairing
        .active_advertisements(Instant::now())
        .iter()
        .any(|ad| ad.kind() == ServiceKind::Pairing));
}

#[test]
fn distinct_windows_with_identical_deadlines_have_distinct_permission() {
    let deadline = Instant::now() + PAIRING_WINDOW;
    let old = PairingAuthorization::new(deadline);
    let new = PairingAuthorization::new(deadline);
    assert!(!old.same_window(&new));
    old.cancel();
    assert!(old.check().is_err());
    new.check().unwrap();
}

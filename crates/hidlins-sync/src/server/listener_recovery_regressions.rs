//! A recovered interface snapshot must restore a suspended listener.

use std::cell::RefCell;

use super::*;

thread_local! {
    static SNAPSHOT: RefCell<Option<Result<Vec<LocalEndpoint>, DiscoveryError>>> = const { RefCell::new(None) };
}

pub(super) fn routes(port: u16) -> Result<Vec<LocalEndpoint>, DiscoveryError> {
    SNAPSHOT
        .with(|snapshot| snapshot.borrow_mut().take())
        .unwrap_or_else(|| crate::discovery::desktop::allowed_interface_endpoints(port))
}

fn fixture() -> (Mutex<ListenerState>, PairingRuntimeState, LocalEndpoint) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = checked_socket(listener.local_addr().unwrap()).unwrap();
    let now = Instant::now();
    let pairing = PairingRuntimeState {
        lifecycle: Mutex::new(()),
        window: Mutex::new(None),
        authorization: Mutex::new(None),
        trusted: Arc::new(AuthorizationRegistry::new([])),
        authority: None,
        sockets: Arc::new(Mutex::new(Vec::new())),
        advertisements: Mutex::new(AdvertisementSet::start(endpoint.port()).unwrap()),
        advertiser: Mutex::new(Some(
            crate::discovery::desktop::DesktopAdvertiser::start(endpoint, now).unwrap(),
        )),
    };
    (
        Mutex::new(ListenerState {
            listener: Some(listener),
            endpoint,
        }),
        pairing,
        endpoint,
    )
}

fn refresh(
    state: &Mutex<ListenerState>,
    pairing: &PairingRuntimeState,
    snapshot: Result<Vec<LocalEndpoint>, DiscoveryError>,
) {
    SNAPSHOT.with(|value| *value.borrow_mut() = Some(snapshot));
    refresh_desktop_listener(state, pairing);
}

// Observe the backlog without accepting: the queued-connection test must
// establish readiness before refresh can accidentally replace the listener.
fn wait_readable(listener: &TcpListener, deadline: Instant) -> std::io::Result<()> {
    let mut watched = mio::net::TcpListener::from_std(listener.try_clone()?);
    let mut poll = mio::Poll::new()?;
    poll.registry()
        .register(&mut watched, mio::Token(0), mio::Interest::READABLE)?;
    let mut events = mio::Events::with_capacity(4);
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::TimedOut))?;
        match poll.poll(&mut events, Some(remaining)) {
            Ok(()) => {
                if events.iter().any(mio::event::Event::is_readable) {
                    return Ok(());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

fn accept_queued(listener: &TcpListener) -> (TcpStream, std::net::SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match listener.accept() {
            Ok(connection) => return connection,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                wait_readable(listener, deadline).expect("owned listener becomes readable");
            }
            Err(error) => panic!("owned listener accept failed: {error}"),
        }
    }
}

#[test]
fn same_endpoint_recovers_after_transient_enumeration_error() {
    let (state, pairing, endpoint) = fixture();
    refresh(&state, &pairing, Err(DiscoveryError::Backend));
    assert!(state.lock().unwrap().listener.is_none());
    assert!(pairing.active_advertisements(Instant::now()).is_empty());
    refresh(&state, &pairing, Ok(vec![endpoint]));
    assert!(
        state.lock().unwrap().listener.is_some(),
        "a recovered unchanged route must rebind its missing listener"
    );
    assert_eq!(state.lock().unwrap().endpoint, endpoint);
    assert!(pairing
        .active_advertisements(Instant::now())
        .iter()
        .any(|ad| ad.kind() == crate::discovery::ServiceKind::Trusted
            && ad.port() == endpoint.port()));
    let client =
        TcpStream::connect_timeout(&endpoint.socket_addr(), Duration::from_secs(1)).unwrap();
    let (_accepted, peer) = accept_queued(state.lock().unwrap().listener.as_ref().unwrap());
    assert_eq!(peer, client.local_addr().unwrap());
}

#[test]
fn current_listening_route_retains_its_queued_connection() {
    let (state, pairing, endpoint) = fixture();
    let client =
        TcpStream::connect_timeout(&endpoint.socket_addr(), Duration::from_secs(1)).unwrap();
    wait_readable(
        state.lock().unwrap().listener.as_ref().unwrap(),
        Instant::now() + Duration::from_secs(2),
    )
    .expect("connection is queued before route refresh");
    refresh(&state, &pairing, Ok(vec![endpoint]));
    let (_accepted, peer) = accept_queued(state.lock().unwrap().listener.as_ref().unwrap());
    assert_eq!(peer, client.local_addr().unwrap());
    assert!(pairing
        .active_advertisements(Instant::now())
        .iter()
        .any(|ad| ad.kind() == crate::discovery::ServiceKind::Trusted));
}

#[test]
fn repeated_enumeration_errors_keep_listener_and_advertisements_suspended() {
    let (state, pairing, _) = fixture();
    for _ in 0..2 {
        refresh(&state, &pairing, Err(DiscoveryError::Backend));
        assert!(state.lock().unwrap().listener.is_none());
        assert!(pairing.active_advertisements(Instant::now()).is_empty());
    }
}

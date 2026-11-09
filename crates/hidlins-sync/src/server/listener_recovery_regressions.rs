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
    let _client =
        TcpStream::connect_timeout(&endpoint.socket_addr(), Duration::from_secs(1)).unwrap();
    let (_accepted, _) = state
        .lock()
        .unwrap()
        .listener
        .as_ref()
        .unwrap()
        .accept()
        .unwrap();
}

#[test]
fn current_listening_route_retains_its_queued_connection() {
    let (state, pairing, endpoint) = fixture();
    let _client =
        TcpStream::connect_timeout(&endpoint.socket_addr(), Duration::from_secs(1)).unwrap();
    refresh(&state, &pairing, Ok(vec![endpoint]));
    let (_accepted, _) = state
        .lock()
        .unwrap()
        .listener
        .as_ref()
        .unwrap()
        .accept()
        .unwrap();
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

//! Bounded local-only TCP and Noise server runtime.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    address::{LocalBindEndpoint, LocalEndpoint},
    discovery::{Advertisement, AdvertisementSet, DiscoveryError},
    framing::{Preface, PrefaceMode, PREFACE_LENGTH},
    noise::{HandshakeSession, NoiseKeypair, SecureTransport, SessionMode, HANDSHAKE_TIMEOUT},
    pairing::{
        ConfirmedPairing, PairingError, PairingTransaction, PairingWindow, SasCode, TransactionId,
        TranscriptDigest,
    },
    protocol::{
        Direction, ErrorCode, LocalRole, Message, ProtocolState, AUTHENTICATED_IDLE_TIMEOUT,
        MAX_APPLICATION_CHUNK, MAX_CONNECTIONS_PER_VAULT, PAIRING_WINDOW, SYNC_SESSION_TIMEOUT,
    },
};

use super::{
    AuthorizationPermit, CanonicalSnapshot, HostOperation, HostQueue, HostResponse, ServerError,
    StagedUpload,
};

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const NOISE_OVERHEAD: usize = 16;

/// Secret-free runtime setup failure.
#[derive(Debug, thiserror::Error)]
pub enum ServerRuntimeError {
    /// Listener creation or local-address inspection failed.
    #[error("local sync listener could not start")]
    Listener,
    /// Discovery lifecycle state could not be created.
    #[error("local sync advertisement could not start")]
    Discovery(#[from] DiscoveryError),
}

/// Application-owned persistence and human-confirmation boundary for XX pairing.
pub trait PairingAuthority: Send + Sync {
    /// Block only this pairing worker until the local user accepts or rejects the SAS.
    fn confirm(
        &self,
        peer: crate::identity::PublicIdentity,
        sas: SasCode,
        timeout: Duration,
    ) -> Result<String, PairingAuthorityError>;

    /// Atomically persist the non-authorizing server-side provisional record.
    fn prepare(
        &self,
        transaction: &PairingTransaction,
        confirmation: &ConfirmedPairing,
        display_name: String,
    ) -> Result<(), PairingAuthorityError>;

    /// Atomically activate the exact prepared client.
    fn activate(&self, transaction: &PairingTransaction) -> Result<(), PairingAuthorityError>;

    /// Wake only the currently pending confirmation; future windows remain usable.
    fn cancel_pending(&self) {}

    /// Wake pending confirmations during lock, stop, shutdown, or panic containment.
    fn shutdown(&self);
}

/// Secret-free failure returned by the application-owned pairing boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PairingAuthorityError {
    /// The prompt was rejected, expired, or canceled by lifecycle shutdown.
    #[error("pairing authorization was canceled")]
    Canceled,
    /// Provisional or active trust could not be committed durably.
    #[error("pairing trust persistence failed")]
    Persistence,
}

/// Running local-only server. Dropping it closes admission and advertisements.
pub struct ServerController {
    listener_state: Arc<Mutex<ListenerState>>,
    trusted: Arc<AuthorizationRegistry>,
    pairing: Arc<PairingRuntimeState>,
    stopped: Arc<AtomicBool>,
    listener: Option<JoinHandle<()>>,
}

struct ListenerState {
    listener: Option<TcpListener>,
    endpoint: LocalEndpoint,
}

struct AuthorizationRegistry {
    permits: RwLock<BTreeMap<[u8; 32], AuthorizationPermit>>,
}

impl AuthorizationRegistry {
    fn new(keys: impl IntoIterator<Item = [u8; 32]>) -> Self {
        Self {
            permits: RwLock::new(
                keys.into_iter()
                    .map(|key| (key, AuthorizationPermit::active()))
                    .collect(),
            ),
        }
    }

    fn authorize(&self, peer: [u8; 32]) -> Option<AuthorizationPermit> {
        self.permits.read().ok()?.get(&peer).cloned()
    }

    fn activate(&self, peer: [u8; 32]) {
        self.permits
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(peer)
            .or_insert_with(AuthorizationPermit::active);
    }

    fn replace(&self, keys: impl IntoIterator<Item = [u8; 32]>) -> BTreeSet<[u8; 32]> {
        let replacement: BTreeSet<_> = keys.into_iter().collect();
        let mut permits = self
            .permits
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let removed: BTreeSet<_> = permits
            .keys()
            .copied()
            .filter(|key| !replacement.contains(key))
            .collect();
        for key in &removed {
            if let Some(permit) = permits.remove(key) {
                permit.revoke();
            }
        }
        for key in replacement {
            permits
                .entry(key)
                .or_insert_with(AuthorizationPermit::active);
        }
        removed
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ConnectionIdentity {
    Pending,
    Pairing,
    Trusted([u8; 32]),
}

struct LiveConnection {
    socket: TcpStream,
    identity: ConnectionIdentity,
}

struct PairingRuntimeState {
    window: Mutex<Option<PairingWindow>>,
    authority: Option<Arc<dyn PairingAuthority>>,
    sockets: Arc<Mutex<Vec<Option<LiveConnection>>>>,
    advertisements: Mutex<AdvertisementSet>,
    #[cfg(all(
        feature = "desktop-discovery",
        any(target_os = "macos", target_os = "linux")
    ))]
    advertiser: Mutex<Option<crate::discovery::desktop::DesktopAdvertiser>>,
}

impl PairingRuntimeState {
    fn open(self: &Arc<Self>) -> Result<(), ServerRuntimeError> {
        let now = Instant::now();
        self.advertisements
            .lock()
            .map_err(|_| ServerRuntimeError::Listener)?
            .open_pairing_at(now)?;
        #[cfg(all(
            feature = "desktop-discovery",
            any(target_os = "macos", target_os = "linux")
        ))]
        if self
            .advertiser
            .lock()
            .map_err(|_| ServerRuntimeError::Listener)?
            .as_mut()
            .ok_or(ServerRuntimeError::Listener)?
            .open_pairing_at(now)
            .is_err()
        {
            if let Ok(mut advertisements) = self.advertisements.lock() {
                advertisements.close_pairing();
            }
            return Err(ServerRuntimeError::Discovery(DiscoveryError::Backend));
        }
        *self
            .window
            .lock()
            .map_err(|_| ServerRuntimeError::Listener)? = Some(PairingWindow::open_at(now));

        let weak = Arc::downgrade(self);
        let _ = thread::Builder::new()
            .name("hidlins-sync-pairing-expiry".into())
            .spawn(move || {
                thread::sleep(PAIRING_WINDOW);
                if let Some(state) = weak.upgrade() {
                    state.expire(Instant::now());
                }
            });
        Ok(())
    }

    fn is_open(&self, now: Instant) -> bool {
        self.window
            .lock()
            .is_ok_and(|window| window.as_ref().is_some_and(|window| window.is_open(now)))
    }

    fn deadline(&self) -> Option<Instant> {
        self.window
            .lock()
            .ok()?
            .as_ref()
            .map(PairingWindow::deadline)
    }

    fn begin_candidate(
        &self,
        peer: crate::identity::PublicIdentity,
        handshake_hash: [u8; 32],
        now: Instant,
    ) -> Result<SasCode, PairingError> {
        self.window
            .lock()
            .map_err(|_| PairingError::Closed)?
            .as_mut()
            .ok_or(PairingError::Closed)?
            .begin_candidate(peer, handshake_hash, now)
    }

    fn confirm_candidate(
        &self,
        sas: SasCode,
        now: Instant,
    ) -> Result<ConfirmedPairing, PairingError> {
        self.window
            .lock()
            .map_err(|_| PairingError::Closed)?
            .as_mut()
            .ok_or(PairingError::Closed)?
            .confirm(sas, now)
    }

    fn reject_candidate(&self, now: Instant) {
        let exhausted = self.window.lock().map_or(true, |mut window| {
            let Some(window) = window.as_mut() else {
                return true;
            };
            let _ = window.reject(now);
            !window.is_open(now)
        });
        if exhausted {
            self.close();
        }
    }

    fn record_failed_attempt(&self, now: Instant) {
        let exhausted = self.window.lock().map_or(true, |mut window| {
            let Some(window) = window.as_mut() else {
                return true;
            };
            let _ = window.record_failed_attempt(now);
            !window.is_open(now)
        });
        if exhausted {
            self.close();
        }
    }

    fn expire(&self, now: Instant) {
        if !self.is_open(now) {
            self.close();
        }
    }

    fn close(&self) {
        if let Ok(mut window) = self.window.lock() {
            *window = None;
        }
        if let Ok(mut advertisements) = self.advertisements.lock() {
            advertisements.close_pairing();
        }
        #[cfg(all(
            feature = "desktop-discovery",
            any(target_os = "macos", target_os = "linux")
        ))]
        if let Ok(mut advertiser) = self.advertiser.lock() {
            if let Some(advertiser) = advertiser.as_mut() {
                let _ = advertiser.close_pairing(Instant::now());
            }
        }
        if let Some(authority) = self.authority.as_ref() {
            authority.cancel_pending();
        }
        if let Ok(sockets) = self.sockets.lock() {
            for connection in sockets.iter().flatten() {
                if connection.identity == ConnectionIdentity::Pairing {
                    let _ = connection.socket.shutdown(Shutdown::Both);
                }
            }
        }
    }

    fn active_advertisements(&self, now: Instant) -> Vec<Advertisement> {
        self.expire(now);
        #[cfg(all(
            feature = "desktop-discovery",
            any(target_os = "macos", target_os = "linux")
        ))]
        if let Ok(mut advertiser) = self.advertiser.lock() {
            if let Some(advertiser) = advertiser.as_mut() {
                let _ = advertiser.tick(now);
            }
        }
        self.advertisements
            .lock()
            .map_or_else(|_| Vec::new(), |mut set| set.active(now))
    }

    #[cfg(any(
        all(
            feature = "desktop-discovery",
            any(target_os = "macos", target_os = "linux")
        ),
        test
    ))]
    fn replace_endpoint(&self, endpoint: LocalEndpoint, now: Instant) -> Result<(), ()> {
        let pairing_open = self.is_open(now);
        let mut replacement = AdvertisementSet::start(endpoint.port()).map_err(|_| ())?;
        if pairing_open {
            replacement.open_pairing_at(now).map_err(|_| ())?;
        }
        #[cfg(all(
            feature = "desktop-discovery",
            any(target_os = "macos", target_os = "linux")
        ))]
        self.advertiser
            .lock()
            .map_err(|_| ())?
            .as_mut()
            .ok_or(())?
            .replace_endpoint(endpoint, now)
            .map_err(|_| ())?;
        *self.advertisements.lock().map_err(|_| ())? = replacement;
        Ok(())
    }

    #[cfg(all(
        feature = "desktop-discovery",
        any(target_os = "macos", target_os = "linux")
    ))]
    fn suspend(&self) {
        if let Ok(mut advertisements) = self.advertisements.lock() {
            advertisements.stop();
        }
        #[cfg(all(
            feature = "desktop-discovery",
            any(target_os = "macos", target_os = "linux")
        ))]
        if let Ok(mut advertiser) = self.advertiser.lock() {
            if let Some(advertiser) = advertiser.as_mut() {
                let _ = advertiser.suspend();
            }
        }
    }
}

#[cfg(all(
    feature = "desktop-discovery",
    any(target_os = "macos", target_os = "linux")
))]
fn refresh_desktop_listener(listener_state: &Mutex<ListenerState>, pairing: &PairingRuntimeState) {
    let current = match listener_state.lock() {
        Ok(state) => state.endpoint,
        Err(_) => return,
    };
    let Ok(current_port_routes) =
        crate::discovery::desktop::allowed_interface_endpoints(current.port())
    else {
        if let Ok(mut state) = listener_state.lock() {
            state.listener = None;
        }
        pairing.suspend();
        return;
    };
    if current_port_routes.contains(&current) {
        return;
    }

    if replace_listener_from_socket_candidates(
        listener_state,
        pairing,
        current_port_routes
            .into_iter()
            .map(LocalEndpoint::socket_addr),
    ) {
        return;
    }
    if replace_listener_from_socket_candidates(
        listener_state,
        pairing,
        crate::discovery::desktop::allowed_interface_bind_endpoints()
            .unwrap_or_default()
            .into_iter()
            .map(LocalBindEndpoint::socket_addr),
    ) {
        return;
    }
    if let Ok(mut state) = listener_state.lock() {
        state.listener = None;
    }
    pairing.suspend();
}

#[cfg(any(
    all(
        feature = "desktop-discovery",
        any(target_os = "macos", target_os = "linux")
    ),
    test
))]
fn replace_listener_from_socket_candidates(
    listener_state: &Mutex<ListenerState>,
    pairing: &PairingRuntimeState,
    candidates: impl IntoIterator<Item = SocketAddr>,
) -> bool {
    for candidate in candidates {
        let Ok(listener) = TcpListener::bind(candidate) else {
            continue;
        };
        if listener.set_nonblocking(true).is_err() {
            continue;
        }
        let Ok(bound) = listener
            .local_addr()
            .map_err(|_| ())
            .and_then(checked_socket)
        else {
            continue;
        };
        if pairing.replace_endpoint(bound, Instant::now()).is_err() {
            continue;
        }
        if let Ok(mut state) = listener_state.lock() {
            state.listener = Some(listener);
            state.endpoint = bound;
        }
        return true;
    }
    false
}

fn reap_finished_workers(workers: &mut Vec<JoinHandle<()>>) {
    let mut index = 0;
    while index < workers.len() {
        if workers[index].is_finished() {
            let worker = workers.swap_remove(index);
            let _ = worker.join();
        } else {
            index += 1;
        }
    }
}

impl ServerController {
    /// Bind an already policy-approved interface and begin bounded admission.
    #[allow(
        clippy::too_many_lines,
        reason = "listener ownership and every cloned shutdown handle remain visible in one constructor"
    )]
    pub fn start(
        endpoint: LocalEndpoint,
        identity: Arc<NoiseKeypair>,
        trusted_keys: impl IntoIterator<Item = [u8; 32]>,
        host: HostQueue,
    ) -> Result<Self, ServerRuntimeError> {
        Self::start_with_pairing_authority(endpoint, identity, trusted_keys, host, None)
    }

    /// Start with an application-owned pairing persistence/confirmation boundary.
    #[allow(clippy::too_many_lines)] // Listener ownership and cleanup stay visibly co-located.
    pub fn start_with_pairing_authority(
        endpoint: LocalEndpoint,
        identity: Arc<NoiseKeypair>,
        trusted_keys: impl IntoIterator<Item = [u8; 32]>,
        host: HostQueue,
        pairing_authority: Option<Arc<dyn PairingAuthority>>,
    ) -> Result<Self, ServerRuntimeError> {
        Self::start_on_socket(
            endpoint.socket_addr(),
            identity,
            trusted_keys,
            host,
            pairing_authority,
        )
    }

    /// Bind a policy-approved interface with an operating-system-assigned
    /// port, then expose only the resulting nonzero [`LocalEndpoint`].
    pub fn start_on_bind_endpoint_with_pairing_authority(
        endpoint: LocalBindEndpoint,
        identity: Arc<NoiseKeypair>,
        trusted_keys: impl IntoIterator<Item = [u8; 32]>,
        host: HostQueue,
        pairing_authority: Option<Arc<dyn PairingAuthority>>,
    ) -> Result<Self, ServerRuntimeError> {
        Self::start_on_socket(
            endpoint.socket_addr(),
            identity,
            trusted_keys,
            host,
            pairing_authority,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn start_on_socket(
        bind_address: SocketAddr,
        identity: Arc<NoiseKeypair>,
        trusted_keys: impl IntoIterator<Item = [u8; 32]>,
        host: HostQueue,
        pairing_authority: Option<Arc<dyn PairingAuthority>>,
    ) -> Result<Self, ServerRuntimeError> {
        let listener = TcpListener::bind(bind_address).map_err(|_| ServerRuntimeError::Listener)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| ServerRuntimeError::Listener)?;
        let bound = checked_socket(
            listener
                .local_addr()
                .map_err(|_| ServerRuntimeError::Listener)?,
        )
        .map_err(|()| ServerRuntimeError::Listener)?;
        let trusted = Arc::new(AuthorizationRegistry::new(trusted_keys));
        let stopped = Arc::new(AtomicBool::new(false));
        let sockets = Arc::new(Mutex::new(
            std::iter::repeat_with(|| None)
                .take(MAX_CONNECTIONS_PER_VAULT)
                .collect::<Vec<Option<LiveConnection>>>(),
        ));
        let pairing = Arc::new(PairingRuntimeState {
            window: Mutex::new(None),
            authority: pairing_authority,
            sockets: Arc::clone(&sockets),
            advertisements: Mutex::new(AdvertisementSet::start(bound.port())?),
            #[cfg(all(
                feature = "desktop-discovery",
                any(target_os = "macos", target_os = "linux")
            ))]
            advertiser: Mutex::new(Some(crate::discovery::desktop::DesktopAdvertiser::start(
                bound,
                Instant::now(),
            )?)),
        });
        let listener_state = Arc::new(Mutex::new(ListenerState {
            listener: Some(listener),
            endpoint: bound,
        }));

        let worker_stopped = Arc::clone(&stopped);
        let worker_trusted = Arc::clone(&trusted);
        let worker_pairing = Arc::clone(&pairing);
        let worker_sockets = Arc::clone(&sockets);
        let worker_listener = Arc::clone(&listener_state);
        let listener_handle = thread::Builder::new()
            .name("hidlins-sync-listener".into())
            .spawn(move || {
                let mut workers = Vec::new();
                #[cfg(all(
                    feature = "desktop-discovery",
                    any(target_os = "macos", target_os = "linux")
                ))]
                let mut next_interface_check = Instant::now();
                while !worker_stopped.load(Ordering::Acquire) {
                    reap_finished_workers(&mut workers);
                    #[cfg(all(
                        feature = "desktop-discovery",
                        any(target_os = "macos", target_os = "linux")
                    ))]
                    if Instant::now() >= next_interface_check {
                        refresh_desktop_listener(&worker_listener, &worker_pairing);
                        next_interface_check = Instant::now() + Duration::from_secs(1);
                    }
                    let accepted = worker_listener.lock().map_or_else(
                        |_| Err(io::ErrorKind::Other.into()),
                        |state| {
                            state.listener.as_ref().map_or_else(
                                || Err(io::ErrorKind::WouldBlock.into()),
                                TcpListener::accept,
                            )
                        },
                    );
                    match accepted {
                        Ok((stream, peer)) => {
                            let _ = stream.set_nonblocking(false);
                            if checked_socket(peer).is_err() {
                                let _ = stream.shutdown(Shutdown::Both);
                                continue;
                            }
                            let slot = worker_sockets.lock().ok().and_then(|mut open_sockets| {
                                let slot = open_sockets.iter().position(Option::is_none)?;
                                open_sockets[slot] = Some(LiveConnection {
                                    socket: stream.try_clone().ok()?,
                                    identity: ConnectionIdentity::Pending,
                                });
                                Some(slot)
                            });
                            let Some(slot) = slot else {
                                let _ = stream.shutdown(Shutdown::Both);
                                continue;
                            };
                            let guarded_sockets = Arc::clone(&worker_sockets);
                            let stop = Arc::clone(&worker_stopped);
                            let keys = Arc::clone(&worker_trusted);
                            let pairing = Arc::clone(&worker_pairing);
                            let identity = Arc::clone(&identity);
                            let host = host.clone();
                            if let Ok(handle) = thread::Builder::new()
                                .name("hidlins-sync-connection".into())
                                .spawn(move || {
                                    let _connection = ConnectionGuard {
                                        sockets: Arc::clone(&guarded_sockets),
                                        slot,
                                    };
                                    serve_connection(
                                        stream,
                                        &identity,
                                        &keys,
                                        &pairing,
                                        &stop,
                                        &host,
                                        &guarded_sockets,
                                        slot,
                                    );
                                })
                            {
                                workers.push(handle);
                            } else if let Ok(mut open_sockets) = worker_sockets.lock() {
                                open_sockets[slot] = None;
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(POLL_INTERVAL);
                        }
                        Err(_) => break,
                    }
                }
                for worker in workers {
                    let _ = worker.join();
                }
            })
            .map_err(|_| ServerRuntimeError::Listener)?;

        Ok(Self {
            listener_state,
            trusted,
            pairing,
            stopped,
            listener: Some(listener_handle),
        })
    }

    /// Return the actual validated listener endpoint.
    #[must_use]
    pub fn endpoint(&self) -> LocalEndpoint {
        self.listener_state.lock().map_or_else(
            |poison| poison.into_inner().endpoint,
            |state| state.endpoint,
        )
    }

    /// Atomically replace the active authorization set after trust changes.
    pub fn replace_trusted(&self, keys: impl IntoIterator<Item = [u8; 32]>) {
        let removed = self.trusted.replace(keys);
        if removed.is_empty() {
            return;
        }
        if let Ok(sockets) = self.pairing.sockets.lock() {
            for connection in sockets.iter().flatten() {
                if matches!(
                    connection.identity,
                    ConnectionIdentity::Trusted(peer) if removed.contains(&peer)
                ) {
                    let _ = connection.socket.shutdown(Shutdown::Both);
                }
            }
        }
    }

    /// Open the explicit bounded pairing admission and advertisement window.
    pub fn open_pairing(&self) -> Result<(), ServerRuntimeError> {
        self.pairing.open()
    }

    /// Close pairing admission and withdraw only its advertisement.
    pub fn close_pairing(&self) {
        self.pairing.close();
    }

    /// Snapshot currently valid logical advertisements.
    pub fn advertisements(&self) -> Vec<Advertisement> {
        self.pairing.active_advertisements(Instant::now())
    }

    /// Stop admission and pairing. This is shared by UI stop, lock, and signal paths.
    pub fn stop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(authority) = self.pairing.authority.as_ref() {
            authority.shutdown();
        }
        self.close_pairing();
        if let Ok(mut advertisements) = self.pairing.advertisements.lock() {
            advertisements.stop();
        }
        #[cfg(all(
            feature = "desktop-discovery",
            any(target_os = "macos", target_os = "linux")
        ))]
        if let Ok(mut advertiser) = self.pairing.advertiser.lock() {
            if let Some(mut advertiser) = advertiser.take() {
                let _ = advertiser.shutdown();
            }
        }
        if let Ok(mut sockets) = self.pairing.sockets.lock() {
            for connection in sockets.iter_mut().filter_map(Option::take) {
                let _ = connection.socket.shutdown(Shutdown::Both);
            }
        }
        // Close admission before joining. The accept loop is nonblocking and
        // observes shutdown within one short poll interval; retaining the
        // shared listener here would leave the advertised port connectable
        // after an explicit stop.
        if let Ok(mut state) = self.listener_state.lock() {
            state.listener = None;
        }
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
    }
}

impl Drop for ServerController {
    fn drop(&mut self) {
        self.stop();
    }
}

struct ConnectionGuard {
    sockets: Arc<Mutex<Vec<Option<LiveConnection>>>>,
    slot: usize,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if let Ok(mut sockets) = self.sockets.lock() {
            sockets[self.slot] = None;
        }
    }
}

fn checked_socket(address: SocketAddr) -> Result<LocalEndpoint, ()> {
    let scope = match address {
        SocketAddr::V4(_) => 0,
        SocketAddr::V6(address) => address.scope_id(),
    };
    LocalEndpoint::new(address.ip(), address.port(), scope).map_err(|_| ())
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the connection's ordered authentication and protocol boundaries stay explicit"
)]
fn serve_connection(
    mut stream: TcpStream,
    identity: &NoiseKeypair,
    trusted: &AuthorizationRegistry,
    pairing: &PairingRuntimeState,
    stopped: &AtomicBool,
    host: &HostQueue,
    sockets: &Mutex<Vec<Option<LiveConnection>>>,
    slot: usize,
) {
    let started = Instant::now();
    let handshake_deadline = started + HANDSHAKE_TIMEOUT;
    if configure_socket_deadline(&stream, handshake_deadline).is_err() {
        return;
    }
    let mut preface_bytes = [0_u8; PREFACE_LENGTH];
    if stream.read_exact(&mut preface_bytes).is_err() {
        return;
    }
    let Ok(preface) = Preface::decode(&preface_bytes) else {
        return;
    };
    let mode = match preface.mode() {
        PrefaceMode::Trusted => SessionMode::Trusted,
        PrefaceMode::Pairing if pairing.is_open(Instant::now()) => SessionMode::Pairing,
        PrefaceMode::Pairing => return,
    };
    if mode == SessionMode::Pairing {
        identify_connection(sockets, slot, ConnectionIdentity::Pairing);
    }
    let Ok(mut transport) = establish_transport(&mut stream, identity, mode, handshake_deadline)
    else {
        if mode == SessionMode::Pairing {
            pairing.record_failed_attempt(Instant::now());
        }
        return;
    };
    if mode == SessionMode::Pairing {
        serve_pairing(&mut stream, &mut transport, identity, pairing, trusted);
        return;
    }
    let peer = transport.peer_static();
    let Some(authorization) = trusted.authorize(peer) else {
        return;
    };
    identify_connection(sockets, slot, ConnectionIdentity::Trusted(peer));

    let mut protocol = ProtocolState::new(SessionMode::Trusted, LocalRole::Server);
    protocol.authenticate();
    let mut upload: Option<(u32, StagedUpload)> = None;
    let mut plain = vec![0_u8; u16::MAX as usize].into_boxed_slice();
    let session_deadline = started + SYNC_SESSION_TIMEOUT;
    while !stopped.load(Ordering::Acquire) && Instant::now() < session_deadline {
        if !authorization.is_authorized() {
            return;
        }
        if configure_socket_deadline(
            &stream,
            session_deadline.min(Instant::now() + AUTHENTICATED_IDLE_TIMEOUT),
        )
        .is_err()
        {
            return;
        }
        let Ok(ciphertext) = read_frame(&mut stream) else {
            return;
        };
        let Ok(length) = transport.read_message(&ciphertext, &mut plain) else {
            return;
        };
        let Ok(decoded) = Message::decode(&plain[..length]) else {
            return;
        };
        if protocol
            .apply(Direction::Receive, decoded.request_id, &decoded.message)
            .is_err()
        {
            let _ = send_message(
                &mut stream,
                &mut transport,
                &mut protocol,
                decoded.request_id,
                &Message::Error {
                    code: ErrorCode::InvalidRequest,
                },
            );
            return;
        }
        let result = handle_message(
            decoded.request_id,
            &decoded.message,
            &mut upload,
            host,
            &authorization,
            session_deadline,
        );
        match result {
            Ok(HostReply::Messages(messages)) => {
                for message in messages {
                    if configure_socket_deadline(
                        &stream,
                        session_deadline.min(Instant::now() + AUTHENTICATED_IDLE_TIMEOUT),
                    )
                    .is_err()
                    {
                        return;
                    }
                    if send_message(
                        &mut stream,
                        &mut transport,
                        &mut protocol,
                        decoded.request_id,
                        &message,
                    )
                    .is_err()
                    {
                        return;
                    }
                }
            }
            Ok(HostReply::Fetch(snapshot)) => {
                if send_fetch(
                    &mut stream,
                    &mut transport,
                    &mut protocol,
                    decoded.request_id,
                    snapshot,
                    &authorization,
                    stopped,
                    started + SYNC_SESSION_TIMEOUT,
                )
                .is_err()
                {
                    return;
                }
            }
            Err(error) => {
                if configure_socket_deadline(
                    &stream,
                    session_deadline.min(Instant::now() + AUTHENTICATED_IDLE_TIMEOUT),
                )
                .is_err()
                {
                    return;
                }
                let _ = send_message(
                    &mut stream,
                    &mut transport,
                    &mut protocol,
                    decoded.request_id,
                    &Message::Error {
                        code: error_code(error),
                    },
                );
            }
        }
    }
}

fn serve_pairing(
    stream: &mut TcpStream,
    transport: &mut SecureTransport,
    identity: &NoiseKeypair,
    pairing: &PairingRuntimeState,
    trusted: &AuthorizationRegistry,
) {
    let peer = crate::identity::PublicIdentity::new(transport.peer_static());
    let handshake_hash = *transport.handshake_hash();
    let Ok(sas) = pairing.begin_candidate(peer, handshake_hash, Instant::now()) else {
        return;
    };
    let Some(authority) = pairing.authority.as_deref() else {
        pairing.reject_candidate(Instant::now());
        return;
    };
    let Some(deadline) = pairing.deadline() else {
        pairing.reject_candidate(Instant::now());
        return;
    };
    let Some(timeout) = deadline.checked_duration_since(Instant::now()) else {
        pairing.reject_candidate(Instant::now());
        return;
    };
    let Ok(display_name) = authority.confirm(peer, sas.clone(), timeout) else {
        pairing.reject_candidate(Instant::now());
        return;
    };
    let Ok(confirmation) = pairing.confirm_candidate(sas, Instant::now()) else {
        return;
    };
    if complete_pairing(
        stream,
        transport,
        identity,
        authority,
        trusted,
        peer,
        handshake_hash,
        &confirmation,
        display_name,
        deadline,
    )
    .is_err()
    {
        pairing.record_failed_attempt(Instant::now());
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_pairing(
    stream: &mut TcpStream,
    transport: &mut SecureTransport,
    identity: &NoiseKeypair,
    authority: &dyn PairingAuthority,
    trusted: &AuthorizationRegistry,
    peer: crate::identity::PublicIdentity,
    handshake_hash: [u8; 32],
    confirmation: &ConfirmedPairing,
    display_name: String,
    deadline: Instant,
) -> Result<(), ()> {
    let mut protocol = ProtocolState::new(SessionMode::Pairing, LocalRole::Server);
    protocol.authenticate();
    let (request_id, message) = receive_message(stream, transport, &mut protocol, deadline)?;
    let Message::PairCommit {
        transaction_id,
        transcript_digest,
    } = message
    else {
        return Err(());
    };
    let transaction = PairingTransaction::from_authenticated(
        TransactionId::new(transaction_id),
        TranscriptDigest::new(transcript_digest),
        crate::identity::PublicIdentity::new(identity.public_key()),
        peer,
    );
    if !transaction.validates(&handshake_hash)
        || authority
            .prepare(&transaction, confirmation, display_name)
            .is_err()
    {
        return Err(());
    }
    configure_socket_deadline(stream, deadline)?;
    if send_message(
        stream,
        transport,
        &mut protocol,
        request_id,
        &Message::PairPrepared {
            transaction_id,
            transcript_digest,
        },
    )
    .is_err()
    {
        return Err(());
    }
    let (activate_id, activate) = receive_message(stream, transport, &mut protocol, deadline)?;
    if activate_id != request_id
        || activate
            != (Message::PairActivate {
                transaction_id,
                transcript_digest,
            })
        || authority.activate(&transaction).is_err()
    {
        return Err(());
    }
    trusted.activate(peer.into_bytes());
    configure_socket_deadline(stream, deadline)?;
    send_message(
        stream,
        transport,
        &mut protocol,
        request_id,
        &Message::PairActivated {
            transaction_id,
            transcript_digest,
        },
    )
}

fn receive_message(
    stream: &mut TcpStream,
    transport: &mut SecureTransport,
    protocol: &mut ProtocolState,
    deadline: Instant,
) -> Result<(u32, Message), ()> {
    configure_socket_deadline(stream, deadline)?;
    let ciphertext = read_frame(stream).map_err(|_| ())?;
    let mut plain = vec![0_u8; ciphertext.len()];
    let length = transport
        .read_message(&ciphertext, &mut plain)
        .map_err(|_| ())?;
    let decoded = Message::decode(&plain[..length]).map_err(|_| ())?;
    protocol
        .apply(Direction::Receive, decoded.request_id, &decoded.message)
        .map_err(|_| ())?;
    Ok((decoded.request_id, decoded.message))
}

fn establish_transport(
    stream: &mut TcpStream,
    identity: &NoiseKeypair,
    mode: SessionMode,
    deadline: Instant,
) -> Result<SecureTransport, ()> {
    let handshake = match mode {
        SessionMode::Trusted => HandshakeSession::trusted_responder_for_authorization(identity),
        SessionMode::Pairing => HandshakeSession::pairing_responder(identity),
    };
    let mut handshake = handshake.map_err(|_| ())?;
    let mut noise = vec![0_u8; u16::MAX as usize].into_boxed_slice();
    let mut plain = vec![0_u8; u16::MAX as usize].into_boxed_slice();
    configure_socket_deadline(stream, deadline)?;
    let first = read_frame(stream).map_err(|_| ())?;
    handshake
        .read_handshake(&first, &mut plain)
        .map_err(|_| ())?;
    let length = handshake.write_handshake(&[], &mut noise).map_err(|_| ())?;
    configure_socket_deadline(stream, deadline)?;
    write_frame(stream, &noise[..length]).map_err(|_| ())?;
    if mode == SessionMode::Pairing {
        configure_socket_deadline(stream, deadline)?;
        let third = read_frame(stream).map_err(|_| ())?;
        handshake
            .read_handshake(&third, &mut plain)
            .map_err(|_| ())?;
    }
    handshake.finish().map_err(|_| ())
}

fn handle_message(
    request_id: u32,
    message: &Message,
    upload: &mut Option<(u32, StagedUpload)>,
    host: &HostQueue,
    authorization: &AuthorizationPermit,
    session_deadline: Instant,
) -> Result<HostReply, ServerError> {
    if !authorization.is_authorized() {
        return Err(ServerError::NotAuthorized);
    }
    match message {
        Message::HeadRequest => {
            match wait(host, HostOperation::Head, authorization, session_deadline)? {
                HostResponse::Head(version) => {
                    Ok(HostReply::Messages(vec![Message::HeadResponse {
                        version: Some(version),
                    }]))
                }
                _ => Err(ServerError::Internal),
            }
        }
        Message::FetchRequest { known } => {
            match wait(
                host,
                HostOperation::Fetch { known: *known },
                authorization,
                session_deadline,
            )? {
                HostResponse::Fetch(None) => Ok(HostReply::Messages(vec![Message::FetchUnchanged])),
                HostResponse::Fetch(Some(snapshot)) => Ok(HostReply::Fetch(snapshot)),
                _ => Err(ServerError::Internal),
            }
        }
        Message::UploadBegin {
            expected,
            total_length,
            digest,
        } => {
            if upload.is_some() {
                return Err(ServerError::Busy);
            }
            match wait(
                host,
                HostOperation::BeginUpload {
                    expected: *expected,
                    total_length: *total_length,
                    digest: *digest,
                },
                authorization,
                session_deadline,
            )? {
                HostResponse::UploadStage(stage) => {
                    *upload = Some((request_id, stage));
                    Ok(HostReply::Messages(Vec::new()))
                }
                _ => Err(ServerError::Internal),
            }
        }
        Message::UploadChunk { sequence, bytes } => {
            let (_, stage) = upload
                .as_mut()
                .filter(|(id, _)| *id == request_id)
                .ok_or(ServerError::InvalidUpload)?;
            stage.write_chunk(*sequence, bytes)?;
            Ok(HostReply::Messages(Vec::new()))
        }
        Message::UploadCommit { chunk_count } => {
            let (_, stage) = upload
                .take()
                .filter(|(id, _)| *id == request_id)
                .ok_or(ServerError::InvalidUpload)?;
            let stage = stage.finish(*chunk_count)?;
            match wait(
                host,
                HostOperation::Commit(stage),
                authorization,
                session_deadline,
            )? {
                HostResponse::Committed(version) => {
                    Ok(HostReply::Messages(vec![Message::UploadAccepted {
                        version,
                    }]))
                }
                _ => Err(ServerError::Internal),
            }
        }
        Message::Cancel => {
            *upload = None;
            Err(ServerError::Cancelled)
        }
        _ => Err(ServerError::InvalidUpload),
    }
}

enum HostReply {
    Messages(Vec<Message>),
    Fetch(CanonicalSnapshot),
}

#[allow(clippy::too_many_arguments)]
fn send_fetch(
    stream: &mut TcpStream,
    transport: &mut SecureTransport,
    protocol: &mut ProtocolState,
    request_id: u32,
    mut snapshot: CanonicalSnapshot,
    authorization: &AuthorizationPermit,
    stopped: &AtomicBool,
    session_deadline: Instant,
) -> Result<(), ()> {
    let version = snapshot.version();
    configure_socket_deadline(
        stream,
        session_deadline.min(Instant::now() + AUTHENTICATED_IDLE_TIMEOUT),
    )?;
    send_message(
        stream,
        transport,
        protocol,
        request_id,
        &Message::FetchBegin {
            version,
            total_length: snapshot.length(),
            digest: *version.as_bytes(),
        },
    )?;
    let mut buffer = vec![0_u8; MAX_APPLICATION_CHUNK];
    let mut sequence = 0_u32;
    loop {
        if stopped.load(Ordering::Acquire)
            || !authorization.is_authorized()
            || Instant::now() >= session_deadline
        {
            return Err(());
        }
        let read = snapshot.read_chunk(&mut buffer).map_err(|_| ())?;
        if read == 0 {
            break;
        }
        configure_socket_deadline(
            stream,
            session_deadline.min(Instant::now() + AUTHENTICATED_IDLE_TIMEOUT),
        )?;
        send_message(
            stream,
            transport,
            protocol,
            request_id,
            &Message::FetchChunk {
                sequence,
                bytes: buffer[..read].to_vec(),
            },
        )?;
        sequence = sequence.checked_add(1).ok_or(())?;
    }
    configure_socket_deadline(
        stream,
        session_deadline.min(Instant::now() + AUTHENTICATED_IDLE_TIMEOUT),
    )?;
    send_message(
        stream,
        transport,
        protocol,
        request_id,
        &Message::FetchCommit {
            chunk_count: sequence,
        },
    )
}

fn wait(
    host: &HostQueue,
    operation: HostOperation,
    authorization: &AuthorizationPermit,
    session_deadline: Instant,
) -> Result<HostResponse, ServerError> {
    let remaining = session_deadline
        .checked_duration_since(Instant::now())
        .ok_or(ServerError::TimedOut)?
        .min(AUTHENTICATED_IDLE_TIMEOUT);
    host.submit_authorized(operation, authorization)?
        .wait_timeout(remaining)
}

fn configure_socket_deadline(stream: &TcpStream, deadline: Instant) -> Result<(), ()> {
    let remaining = deadline.checked_duration_since(Instant::now()).ok_or(())?;
    if remaining.is_zero() {
        return Err(());
    }
    stream.set_read_timeout(Some(remaining)).map_err(|_| ())?;
    stream.set_write_timeout(Some(remaining)).map_err(|_| ())
}

fn identify_connection(
    sockets: &Mutex<Vec<Option<LiveConnection>>>,
    slot: usize,
    identity: ConnectionIdentity,
) {
    if let Ok(mut sockets) = sockets.lock() {
        if let Some(connection) = sockets.get_mut(slot).and_then(Option::as_mut) {
            connection.identity = identity;
        }
    }
}

fn send_message(
    stream: &mut TcpStream,
    transport: &mut SecureTransport,
    protocol: &mut ProtocolState,
    request_id: u32,
    message: &Message,
) -> Result<(), ()> {
    protocol
        .apply(Direction::Send, request_id, message)
        .map_err(|_| ())?;
    let payload = message.encode(request_id).map_err(|_| ())?;
    let mut encrypted = vec![0_u8; payload.len() + NOISE_OVERHEAD];
    let length = transport
        .write_message(&payload, &mut encrypted)
        .map_err(|_| ())?;
    write_frame(stream, &encrypted[..length]).map_err(|_| ())
}

fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut prefix = [0_u8; 2];
    stream.read_exact(&mut prefix)?;
    let length = usize::from(u16::from_be_bytes(prefix));
    if length == 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut bytes = vec![0_u8; length];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn write_frame(stream: &mut TcpStream, bytes: &[u8]) -> io::Result<()> {
    let length = u16::try_from(bytes.len()).map_err(|_| io::ErrorKind::InvalidData)?;
    if length == 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(bytes)
}

const fn error_code(error: ServerError) -> ErrorCode {
    match error {
        ServerError::NotAuthorized => ErrorCode::NotAuthorized,
        ServerError::Busy => ErrorCode::Busy,
        ServerError::StaleVersion => ErrorCode::StaleVersion,
        ServerError::TooLarge => ErrorCode::TooLarge,
        ServerError::Cancelled => ErrorCode::Cancelled,
        ServerError::TimedOut => ErrorCode::TimedOut,
        ServerError::InvalidUpload => ErrorCode::InvalidRequest,
        ServerError::Stopped | ServerError::Internal => ErrorCode::InternalFailure,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::mpsc,
        thread::{self, JoinHandle},
    };

    use super::reap_finished_workers;
    #[cfg(not(feature = "desktop-discovery"))]
    use {
        super::{replace_listener_from_socket_candidates, ListenerState, PairingRuntimeState},
        crate::{
            address::LocalEndpoint, discovery::AdvertisementSet,
            protocol::MAX_CONNECTIONS_PER_VAULT,
        },
        std::{
            net::{IpAddr, Ipv4Addr, TcpListener},
            sync::{Arc, Mutex},
            time::Instant,
        },
    };

    #[test]
    fn finished_connection_workers_are_reaped_while_server_is_running() {
        let (release_tx, release_rx) = mpsc::channel();
        let active = thread::spawn(move || {
            let _ = release_rx.recv();
        });
        let finished = thread::spawn(|| {});
        while !finished.is_finished() {
            thread::yield_now();
        }
        let mut workers: Vec<JoinHandle<()>> = vec![active, finished];

        reap_finished_workers(&mut workers);

        assert_eq!(workers.len(), 1);
        assert!(!workers[0].is_finished());
        release_tx.send(()).expect("active worker is waiting");
        workers
            .pop()
            .expect("active worker remains")
            .join()
            .unwrap();
    }

    // LNS-REVIEW-010
    #[test]
    #[cfg(not(feature = "desktop-discovery"))]
    fn listener_refresh_rebinds_to_an_injected_allowed_snapshot() {
        let initial = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        initial.set_nonblocking(true).unwrap();
        let initial_endpoint = LocalEndpoint::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            initial.local_addr().unwrap().port(),
            0,
        )
        .unwrap();
        let candidate_socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let candidate = LocalEndpoint::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            candidate_socket.local_addr().unwrap().port(),
            0,
        )
        .unwrap();
        drop(candidate_socket);
        let sockets = Arc::new(Mutex::new(
            std::iter::repeat_with(|| None)
                .take(MAX_CONNECTIONS_PER_VAULT)
                .collect(),
        ));
        let pairing = PairingRuntimeState {
            window: Mutex::new(None),
            authority: None,
            sockets,
            advertisements: Mutex::new(AdvertisementSet::start(initial_endpoint.port()).unwrap()),
        };
        let state = Mutex::new(ListenerState {
            listener: Some(initial),
            endpoint: initial_endpoint,
        });

        assert!(replace_listener_from_socket_candidates(
            &state,
            &pairing,
            [candidate.socket_addr()]
        ));
        assert_eq!(state.lock().unwrap().endpoint, candidate);
        assert!(pairing
            .active_advertisements(Instant::now())
            .iter()
            .all(|advertisement| advertisement.port() == candidate.port()));
    }
}

//! Pinned-identity local-network client transport.

use std::{
    collections::{BTreeSet, HashSet},
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use sha2::{Digest, Sha256};
use std::path::Path;

use hidlins_core::{Keyfile, MasterPassword, RegisteredVault, Vault, VaultRegistry};

use crate::{
    address::LocalEndpoint,
    config::local::{LocalConfigError, LocalSyncConfig},
    encoding::encode_lower,
    framing::{Preface, PrefaceMode},
    noise::{HandshakeSession, NoiseKeypair, SecureTransport, SessionMode, HANDSHAKE_TIMEOUT},
    pairing::{ConfirmedPairing, PairingTransaction, SasCode},
    protocol::{
        Direction, ErrorCode, LocalRole, Message, ProtocolState, RemoteVersion,
        AUTHENTICATED_IDLE_TIMEOUT, CANDIDATE_CONNECT_TIMEOUT, MAX_APPLICATION_CHUNK,
        MAX_CANDIDATE_ATTEMPTS, SYNC_SESSION_TIMEOUT, TOTAL_CONNECT_BUDGET,
    },
    sync::{run_state_machine, SyncOptions, SyncOutcome},
    transport::{IsPreconditionFailed, SyncSnapshot, SyncTransport, SyncVersion},
};

/// An authenticated XX session retained only until explicit local SAS confirmation.
pub struct ClientPairingSession {
    session: LanSession,
    confirmation: ConfirmedPairing,
    server_key: crate::identity::PublicIdentity,
    endpoint: LocalEndpoint,
}

impl ClientPairingSession {
    /// Complete first-contact XX against the first allowed candidate.
    pub fn begin(
        identity: &NoiseKeypair,
        candidates: impl IntoIterator<Item = LocalEndpoint>,
    ) -> Result<Self, LanError> {
        let connect_deadline = Instant::now() + TOTAL_CONNECT_BUDGET;
        let pairing_deadline = Instant::now() + crate::protocol::PAIRING_WINDOW;
        for endpoint in candidates.into_iter().take(MAX_CANDIDATE_ATTEMPTS) {
            let Some(remaining) = connect_deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            let Ok(mut stream) = TcpStream::connect_timeout(
                &endpoint.socket_addr(),
                remaining.min(CANDIDATE_CONNECT_TIMEOUT),
            ) else {
                continue;
            };
            if checked_peer(stream.peer_addr().map_err(|_| LanError::Unreachable)?).is_err() {
                continue;
            }
            let handshake_deadline = connect_deadline.min(Instant::now() + HANDSHAKE_TIMEOUT);
            configure_socket_deadline(&stream, handshake_deadline)?;
            stream
                .write_all(&Preface::new(PrefaceMode::Pairing).encode())
                .map_err(|_| LanError::Unreachable)?;
            let mut handshake = HandshakeSession::pairing_initiator(identity)
                .map_err(|_| LanError::Authentication)?;
            let mut buffer = vec![0_u8; u16::MAX as usize].into_boxed_slice();
            let first = handshake
                .write_handshake(&[], &mut buffer)
                .map_err(|_| LanError::Authentication)?;
            configure_socket_deadline(&stream, handshake_deadline)?;
            write_frame(&mut stream, &buffer[..first]).map_err(|_| LanError::Unreachable)?;
            configure_socket_deadline(&stream, handshake_deadline)?;
            let second = read_frame(&mut stream).map_err(|_| LanError::Authentication)?;
            handshake
                .read_handshake(&second, &mut buffer)
                .map_err(|_| LanError::Authentication)?;
            let third = handshake
                .write_handshake(&[], &mut buffer)
                .map_err(|_| LanError::Authentication)?;
            configure_socket_deadline(&stream, handshake_deadline)?;
            write_frame(&mut stream, &buffer[..third]).map_err(|_| LanError::Unreachable)?;
            let transport = handshake.finish().map_err(|_| LanError::Authentication)?;
            let server_key = crate::identity::PublicIdentity::new(transport.peer_static());
            let confirmation =
                ConfirmedPairing::from_authenticated(server_key, *transport.handshake_hash());
            let mut protocol = ProtocolState::new(SessionMode::Pairing, LocalRole::Client);
            protocol.authenticate();
            return Ok(Self {
                session: LanSession {
                    stream,
                    transport,
                    protocol,
                    deadline: pairing_deadline,
                },
                confirmation,
                server_key,
                endpoint,
            });
        }
        Err(LanError::Unreachable)
    }

    /// Return the locally derived display-only SAS.
    #[must_use]
    pub fn sas(&self) -> SasCode {
        SasCode::derive(self.confirmation.handshake_hash())
    }

    /// Return the policy-checked route that completed the authenticated handshake.
    #[must_use]
    pub const fn endpoint(&self) -> LocalEndpoint {
        self.endpoint
    }

    /// Run authenticated prepare/activate messages after local confirmation.
    pub fn confirm(
        mut self,
        config: &mut LocalSyncConfig,
        server_display_name: String,
        now_epoch_seconds: u64,
    ) -> Result<PairingTransaction, LanError> {
        let transaction = self
            .confirmation
            .transaction(self.server_key, config.identity().public_key())
            .map_err(|_| LanError::Protocol)?;
        let id = transaction.transaction_id().into_bytes();
        let digest = transaction.transcript_digest().into_bytes();
        self.session.send(
            1,
            &Message::PairCommit {
                transaction_id: id,
                transcript_digest: digest,
            },
        )?;
        match self.session.receive()? {
            (
                1,
                Message::PairPrepared {
                    transaction_id,
                    transcript_digest,
                },
            ) if transaction_id == id && transcript_digest == digest => {}
            (1, Message::Error { code }) => return Err(from_error(code)),
            _ => return Err(LanError::Protocol),
        }
        config
            .prepare_server(
                &transaction,
                &self.confirmation,
                server_display_name,
                now_epoch_seconds,
                now_epoch_seconds.saturating_add(crate::protocol::PAIRING_WINDOW.as_secs()),
            )
            .map_err(|_| LanError::Protocol)?;
        self.session.send(
            1,
            &Message::PairActivate {
                transaction_id: id,
                transcript_digest: digest,
            },
        )?;
        match self.session.receive()? {
            (
                1,
                Message::PairActivated {
                    transaction_id,
                    transcript_digest,
                },
            ) if transaction_id == id && transcript_digest == digest => {}
            (1, Message::Error { code }) => return Err(from_error(code)),
            _ => return Err(LanError::Protocol),
        }
        config
            .activate_server(&transaction, now_epoch_seconds)
            .map_err(|_| LanError::Protocol)?;
        Ok(transaction)
    }
}

impl std::fmt::Debug for ClientPairingSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ClientPairingSession([REDACTED])")
    }
}

/// Atomic pair-and-import failure with no secret-bearing detail.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// Trust is provisional, revoked, or configured for the server role.
    #[error("pairing is not active for client import")]
    NotAuthorized,
    /// The fetched bytes did not authenticate as a complete KDBX.
    #[error("imported vault validation failed")]
    InvalidVault,
    /// The destination already exists or could not be written atomically.
    #[error("imported vault could not be installed")]
    Install,
    /// The complete vault could not be registered atomically.
    #[error("imported vault registration failed")]
    Registry,
    /// The local-sync config could not be attached.
    #[error("imported vault sync configuration failed")]
    Config(#[from] LocalConfigError),
}

/// Validate, atomically install, and atomically register an active paired vault.
pub fn import_paired_vault(
    encrypted_kdbx: &[u8],
    target: &Path,
    vault_name: &str,
    master_password: &MasterPassword,
    keyfile: Option<&Keyfile>,
    config: &LocalSyncConfig,
    registry: &mut VaultRegistry,
) -> Result<(), ImportError> {
    if !config.is_active_client() {
        return Err(ImportError::NotAuthorized);
    }
    registry
        .paths()
        .ensure_exists()
        .map_err(|_| ImportError::Install)?;
    if target.exists() || Vault::open_from_bytes(encrypted_kdbx, master_password, keyfile).is_err()
    {
        return Err(if target.exists() {
            ImportError::Install
        } else {
            ImportError::InvalidVault
        });
    }
    let mut entry = RegisteredVault {
        name: vault_name.to_string(),
        path: target.to_path_buf(),
        created_at: chrono::Utc::now().to_rfc3339(),
        keyfile_path: keyfile.and_then(Keyfile::path).map(Path::to_path_buf),
        extra: toml::Table::new(),
    };
    config.write_to_entry(&mut entry)?;
    hidlins_core::atomic::write_atomic(target, encrypted_kdbx).map_err(|_| ImportError::Install)?;
    entry.path = if let Ok(canonical) = std::fs::canonicalize(target) {
        canonical
    } else {
        let _ = std::fs::remove_file(target);
        return Err(ImportError::Install);
    };
    if registry.register_and_save(entry).is_err() {
        std::fs::remove_file(target).map_err(|_| ImportError::Registry)?;
        return Err(ImportError::Registry);
    }
    Ok(())
}

/// Fetch the first complete canonical KDBX after pairing, without installing it.
pub fn fetch_paired_vault(
    vault_name: &str,
    master_password: &MasterPassword,
    config: &LocalSyncConfig,
    discovered: impl IntoIterator<Item = LocalEndpoint>,
) -> Result<(Vec<u8>, crate::protocol::RemoteVersion), LanError> {
    if !config.is_active_client() {
        return Err(LanError::Authentication);
    }
    let identity = config
        .identity()
        .unlock(vault_name, config.role(), master_password)
        .map_err(|_| LanError::Authentication)?;
    let pinned = config
        .pinned_server()
        .ok_or(LanError::Authentication)?
        .public_key()
        .into_bytes();
    let mut transport = LanTransport::new(identity, pinned, config.routing_hint(), discovered)?;
    let snapshot = transport
        .fetch_if_changed(None)?
        .ok_or(LanError::Protocol)?;
    let version = parse_object_version(&snapshot.version)?;
    Ok((snapshot.bytes, version))
}

const NOISE_OVERHEAD: usize = 16;

/// Explicit scheduling cause; the core never schedules after mutations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncTrigger {
    /// First configured unlock observed in this process.
    StartupUnlock,
    /// Direct user request; always repeatable while configured.
    Manual,
    /// A local save or mutation; explicitly unsupported as a sync trigger.
    PostMutation,
}

/// Process-local record of vaults whose one startup attempt was consumed.
#[derive(Debug, Default)]
pub struct StartupSyncTracker {
    attempted: HashSet<String>,
}

/// Cloneable cooperative cancellation shared with a UI/session owner.
#[derive(Clone, Debug, Default)]
pub struct LanCancellation(Arc<AtomicBool>);

impl LanCancellation {
    /// Cancel candidate selection or the next stream boundary.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Return whether the owner has requested cancellation.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    fn check(&self) -> Result<(), LanError> {
        if self.is_cancelled() {
            Err(LanError::Cancelled)
        } else {
            Ok(())
        }
    }
}

impl StartupSyncTracker {
    /// Return whether the caller should attempt now, consuming startup even on failure.
    pub fn should_attempt(
        &mut self,
        vault_name: &str,
        configured: bool,
        trigger: SyncTrigger,
    ) -> bool {
        if !configured {
            return false;
        }
        match trigger {
            SyncTrigger::Manual => true,
            SyncTrigger::PostMutation => false,
            SyncTrigger::StartupUnlock => self.attempted.insert(vault_name.to_string()),
        }
    }
}

/// Stable, secret-free LAN client failure categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LanError {
    /// No policy-approved candidate authenticated within the fixed budget.
    #[error("no authorized local sync server was reachable")]
    Unreachable,
    /// The peer key, ciphertext, or handshake authentication failed.
    #[error("local sync server authentication failed")]
    Authentication,
    /// A frame or application transition violated protocol V1.
    #[error("local sync protocol failed")]
    Protocol,
    /// The authority rejected a stale compare-and-swap version.
    #[error("authoritative vault advanced concurrently")]
    StaleVersion,
    /// A fixed connection, object, or queue limit was reached.
    #[error("local sync resource limit reached")]
    Busy,
    /// The operation was cancelled.
    #[error("local sync operation was cancelled")]
    Cancelled,
    /// The operation deadline elapsed.
    #[error("local sync operation timed out")]
    TimedOut,
    /// The authenticated server reported a generic internal failure.
    #[error("local sync server failed")]
    Internal,
}

impl IsPreconditionFailed for LanError {
    fn is_precondition_failed(&self) -> bool {
        *self == Self::StaleVersion
    }
}

/// Production local-network transport over allowed candidates and pinned IK.
pub struct LanTransport {
    candidates: Vec<LocalEndpoint>,
    identity: NoiseKeypair,
    pinned_server: [u8; 32],
    cancellation: LanCancellation,
}

impl LanTransport {
    /// Build a transport with the routing hint first and all candidates deduplicated.
    pub fn new(
        identity: NoiseKeypair,
        pinned_server: [u8; 32],
        routing_hint: Option<LocalEndpoint>,
        discovered: impl IntoIterator<Item = LocalEndpoint>,
    ) -> Result<Self, LanError> {
        let mut unique = BTreeSet::new();
        let mut candidates = Vec::new();
        if let Some(hint) = routing_hint {
            unique.insert(hint);
            candidates.push(hint);
        }
        for endpoint in discovered {
            if unique.insert(endpoint) {
                candidates.push(endpoint);
            }
            if candidates.len() == MAX_CANDIDATE_ATTEMPTS {
                break;
            }
        }
        if candidates.is_empty() {
            return Err(LanError::Unreachable);
        }
        Ok(Self {
            candidates,
            identity,
            pinned_server,
            cancellation: LanCancellation::default(),
        })
    }

    /// Attach a caller-owned cancellation signal.
    #[must_use]
    pub fn with_cancellation(mut self, cancellation: LanCancellation) -> Self {
        self.cancellation = cancellation;
        self
    }

    fn connect(&self) -> Result<LanSession, LanError> {
        let started = Instant::now();
        let mut authentication_failed = false;
        for endpoint in self.candidates.iter().take(MAX_CANDIDATE_ATTEMPTS) {
            self.cancellation.check()?;
            let remaining = TOTAL_CONNECT_BUDGET.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                break;
            }
            let timeout = remaining.min(CANDIDATE_CONNECT_TIMEOUT);
            let Ok(stream) = TcpStream::connect_timeout(&endpoint.socket_addr(), timeout) else {
                continue;
            };
            if checked_peer(stream.peer_addr().map_err(|_| LanError::Unreachable)?).is_err() {
                let _ = stream.shutdown(Shutdown::Both);
                continue;
            }
            let handshake_deadline =
                (started + TOTAL_CONNECT_BUDGET).min(Instant::now() + HANDSHAKE_TIMEOUT);
            match LanSession::handshake(
                stream,
                &self.identity,
                self.pinned_server,
                handshake_deadline,
            ) {
                Ok(session) => return Ok(session),
                Err(LanError::Authentication) => authentication_failed = true,
                Err(_) => {}
            }
        }
        if authentication_failed {
            Err(LanError::Authentication)
        } else {
            Err(LanError::Unreachable)
        }
    }
}

/// Run the established no-data-loss state machine for a configured client.
pub fn sync_vault(
    vault: &mut Vault,
    vault_name: &str,
    registry: &mut VaultRegistry,
    master_password: &MasterPassword,
    keyfile: Option<&Keyfile>,
    discovered: impl IntoIterator<Item = LocalEndpoint>,
    options: SyncOptions,
) -> Result<SyncOutcome, crate::SyncError> {
    sync_vault_with_cancellation(
        vault,
        vault_name,
        registry,
        master_password,
        keyfile,
        discovered,
        options,
        LanCancellation::default(),
    )
}

/// Run local sync with a caller-owned cooperative cancellation signal.
#[allow(clippy::too_many_arguments)] // Mirrors sync_vault plus its lifecycle-owned cancellation.
pub fn sync_vault_with_cancellation(
    vault: &mut Vault,
    vault_name: &str,
    registry: &mut VaultRegistry,
    master_password: &MasterPassword,
    keyfile: Option<&Keyfile>,
    discovered: impl IntoIterator<Item = LocalEndpoint>,
    options: SyncOptions,
    cancellation: LanCancellation,
) -> Result<SyncOutcome, crate::SyncError> {
    let entry = registry.get(vault_name).ok_or_else(|| {
        crate::SyncError::Vault(hidlins_core::VaultError::NotRegistered {
            name: vault_name.to_string(),
        })
    })?;
    let mut config =
        LocalSyncConfig::from_vault_entry(entry).ok_or(crate::SyncError::NotConfigured)?;
    if !config.is_active_client() {
        return Err(LanError::Authentication.into());
    }
    let identity = config
        .identity()
        .unlock(vault_name, config.role(), master_password)
        .map_err(|_| LanError::Authentication)?;
    let pinned = config
        .pinned_server()
        .ok_or(LanError::Authentication)?
        .public_key()
        .into_bytes();
    let previous_remote = config
        .last_synced_remote_version()
        .map(|value| encode_lower(value.as_bytes()));
    let previous_local = config
        .last_synced_local_version()
        .map(|value| encode_lower(value.as_bytes()));
    let mut transport = LanTransport::new(identity, pinned, config.routing_hint(), discovered)?
        .with_cancellation(cancellation);
    let (outcome, pointers) = run_state_machine(
        vault,
        master_password,
        keyfile,
        &mut transport,
        previous_remote,
        previous_local.as_deref(),
        options,
    )?;
    let remote = pointers
        .remote_version
        .as_deref()
        .map(parse_version_text)
        .transpose()?;
    let local = pointers
        .local_sha256
        .as_deref()
        .map(parse_version_text)
        .transpose()?;
    config.set_sync_versions(remote, local);
    config.persist(registry, vault_name)?;
    Ok(outcome)
}

impl std::fmt::Debug for LanTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LanTransport([REDACTED])")
    }
}

impl SyncTransport for LanTransport {
    type Error = LanError;

    fn head(&mut self) -> Result<Option<SyncVersion>, Self::Error> {
        let mut session = self.connect()?;
        session.send(1, &Message::HeadRequest)?;
        match session.receive()? {
            (1, Message::HeadResponse { version }) => Ok(version.map(to_object_version)),
            (_, Message::Error { code }) => Err(from_error(code)),
            _ => Err(LanError::Protocol),
        }
    }

    fn fetch_if_changed(
        &mut self,
        previous: Option<&SyncVersion>,
    ) -> Result<Option<SyncSnapshot>, Self::Error> {
        let known = previous.map(parse_object_version).transpose()?;
        let mut session = self.connect()?;
        session.send(1, &Message::FetchRequest { known })?;
        match session.receive()? {
            (1, Message::FetchUnchanged) => Ok(None),
            (
                1,
                Message::FetchBegin {
                    version,
                    total_length,
                    digest,
                },
            ) => {
                let capacity = usize::try_from(total_length).map_err(|_| LanError::Busy)?;
                let mut bytes = Vec::with_capacity(capacity);
                let mut chunks = 0_u32;
                loop {
                    self.cancellation.check()?;
                    match session.receive()? {
                        (
                            1,
                            Message::FetchChunk {
                                sequence,
                                bytes: chunk,
                            },
                        ) if sequence == chunks => {
                            bytes.extend_from_slice(&chunk);
                            chunks = chunks.checked_add(1).ok_or(LanError::Busy)?;
                            if bytes.len() > capacity {
                                return Err(LanError::Protocol);
                            }
                        }
                        (1, Message::FetchCommit { chunk_count }) if chunk_count == chunks => break,
                        (1, Message::Error { code }) => return Err(from_error(code)),
                        _ => return Err(LanError::Protocol),
                    }
                }
                if bytes.len() != capacity
                    || <[u8; 32]>::from(Sha256::digest(&bytes)) != digest
                    || digest != *version.as_bytes()
                {
                    return Err(LanError::Protocol);
                }
                Ok(Some(SyncSnapshot {
                    version: to_object_version(version),
                    bytes,
                }))
            }
            (1, Message::Error { code }) => Err(from_error(code)),
            _ => Err(LanError::Protocol),
        }
    }

    fn commit_conditional(
        &mut self,
        bytes: &[u8],
        expected: Option<&SyncVersion>,
    ) -> Result<SyncVersion, Self::Error> {
        let expected = expected.map(parse_object_version).transpose()?;
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let total_length = u64::try_from(bytes.len()).map_err(|_| LanError::Busy)?;
        let mut session = self.connect()?;
        session.send(
            1,
            &Message::UploadBegin {
                expected,
                total_length,
                digest,
            },
        )?;
        let mut chunks = 0_u32;
        for chunk in bytes.chunks(MAX_APPLICATION_CHUNK) {
            self.cancellation.check()?;
            session.send(
                1,
                &Message::UploadChunk {
                    sequence: chunks,
                    bytes: chunk.to_vec(),
                },
            )?;
            chunks = chunks.checked_add(1).ok_or(LanError::Busy)?;
        }
        session.send(
            1,
            &Message::UploadCommit {
                chunk_count: chunks,
            },
        )?;
        match session.receive()? {
            (1, Message::UploadAccepted { version }) => Ok(to_object_version(version)),
            (1, Message::Error { code }) => Err(from_error(code)),
            _ => Err(LanError::Protocol),
        }
    }
}

struct LanSession {
    stream: TcpStream,
    transport: SecureTransport,
    protocol: ProtocolState,
    deadline: Instant,
}

impl LanSession {
    fn handshake(
        mut stream: TcpStream,
        identity: &NoiseKeypair,
        pinned: [u8; 32],
        handshake_deadline: Instant,
    ) -> Result<Self, LanError> {
        configure_socket_deadline(&stream, handshake_deadline)?;
        stream
            .write_all(&Preface::new(PrefaceMode::Trusted).encode())
            .map_err(|_| LanError::Unreachable)?;
        let mut handshake = HandshakeSession::trusted_initiator(identity, pinned)
            .map_err(|_| LanError::Authentication)?;
        let mut buffer = vec![0_u8; u16::MAX as usize].into_boxed_slice();
        let length = handshake
            .write_handshake(&[], &mut buffer)
            .map_err(|_| LanError::Authentication)?;
        configure_socket_deadline(&stream, handshake_deadline)?;
        write_frame(&mut stream, &buffer[..length]).map_err(|_| LanError::Unreachable)?;
        configure_socket_deadline(&stream, handshake_deadline)?;
        let response = read_frame(&mut stream).map_err(|_| LanError::Authentication)?;
        handshake
            .read_handshake(&response, &mut buffer)
            .map_err(|_| LanError::Authentication)?;
        let transport = handshake.finish().map_err(|_| LanError::Authentication)?;
        let mut protocol = ProtocolState::new(SessionMode::Trusted, LocalRole::Client);
        protocol.authenticate();
        Ok(Self {
            stream,
            transport,
            protocol,
            deadline: Instant::now() + SYNC_SESSION_TIMEOUT,
        })
    }

    fn send(&mut self, request_id: u32, message: &Message) -> Result<(), LanError> {
        configure_socket_deadline(
            &self.stream,
            self.deadline
                .min(Instant::now() + AUTHENTICATED_IDLE_TIMEOUT),
        )?;
        self.protocol
            .apply(Direction::Send, request_id, message)
            .map_err(|_| LanError::Protocol)?;
        let plain = message.encode(request_id).map_err(|_| LanError::Protocol)?;
        let mut encrypted = vec![0_u8; plain.len() + NOISE_OVERHEAD];
        let length = self
            .transport
            .write_message(&plain, &mut encrypted)
            .map_err(|_| LanError::Authentication)?;
        write_frame(&mut self.stream, &encrypted[..length]).map_err(|error| map_io(&error))
    }

    fn receive(&mut self) -> Result<(u32, Message), LanError> {
        configure_socket_deadline(
            &self.stream,
            self.deadline
                .min(Instant::now() + AUTHENTICATED_IDLE_TIMEOUT),
        )?;
        let encrypted = read_frame(&mut self.stream).map_err(|error| map_io(&error))?;
        let mut plain = vec![0_u8; encrypted.len()];
        let length = self
            .transport
            .read_message(&encrypted, &mut plain)
            .map_err(|_| LanError::Authentication)?;
        let decoded = Message::decode(&plain[..length]).map_err(|_| LanError::Protocol)?;
        self.protocol
            .apply(Direction::Receive, decoded.request_id, &decoded.message)
            .map_err(|_| LanError::Protocol)?;
        Ok((decoded.request_id, decoded.message))
    }
}

fn checked_peer(address: SocketAddr) -> Result<LocalEndpoint, LanError> {
    let scope = match address {
        SocketAddr::V4(_) => 0,
        SocketAddr::V6(value) => value.scope_id(),
    };
    LocalEndpoint::new(address.ip(), address.port(), scope).map_err(|_| LanError::Unreachable)
}

fn configure_socket_deadline(stream: &TcpStream, deadline: Instant) -> Result<(), LanError> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or(LanError::TimedOut)?;
    if remaining.is_zero() {
        return Err(LanError::TimedOut);
    }
    stream
        .set_read_timeout(Some(remaining))
        .map_err(|_| LanError::Unreachable)?;
    stream
        .set_write_timeout(Some(remaining))
        .map_err(|_| LanError::Unreachable)
}

fn write_frame(stream: &mut TcpStream, bytes: &[u8]) -> io::Result<()> {
    let length = u16::try_from(bytes.len()).map_err(|_| io::ErrorKind::InvalidData)?;
    if length == 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(bytes)
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

fn parse_object_version(version: &SyncVersion) -> Result<RemoteVersion, LanError> {
    parse_version_text(&version.0)
}

fn parse_version_text(version: &str) -> Result<RemoteVersion, LanError> {
    crate::identity::decode_hex::<32>(version)
        .map(RemoteVersion::new)
        .map_err(|()| LanError::Protocol)
}

fn to_object_version(version: RemoteVersion) -> SyncVersion {
    SyncVersion(encode_lower(version.as_bytes()))
}

const fn from_error(code: ErrorCode) -> LanError {
    match code {
        ErrorCode::StaleVersion => LanError::StaleVersion,
        ErrorCode::Busy | ErrorCode::TooLarge => LanError::Busy,
        ErrorCode::TimedOut => LanError::TimedOut,
        ErrorCode::Cancelled => LanError::Cancelled,
        ErrorCode::NotAuthorized => LanError::Authentication,
        ErrorCode::InvalidRequest => LanError::Protocol,
        ErrorCode::InternalFailure => LanError::Internal,
    }
}

fn map_io(error: &io::Error) -> LanError {
    if matches!(
        error.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
    ) {
        LanError::TimedOut
    } else {
        LanError::Unreachable
    }
}

//! Bounded application messages and strict local-sync protocol state.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::HashSet, fmt, time::Duration};

use crate::{encoding::encode_lower, identity::decode_hex};

pub use crate::noise::SessionMode;

/// Largest encrypted KDBX object accepted by V1.
pub const MAX_ENCRYPTED_VAULT: u64 = 268_435_456;
/// Largest application data chunk accepted by V1.
pub const MAX_APPLICATION_CHUNK: usize = 61_440;
/// Maximum simultaneous accepted connections per vault.
pub const MAX_CONNECTIONS_PER_VAULT: usize = 8;
/// Maximum pending operations in the authoritative host queue.
pub const MAX_PENDING_HOST_OPERATIONS: usize = 16;
/// Maximum cached normalized discovery endpoints.
pub const MAX_DISCOVERY_ENDPOINTS: usize = 32;
/// Maximum candidate connection attempts per operation.
pub const MAX_CANDIDATE_ATTEMPTS: usize = 8;
/// Maximum pairing-window failures.
pub const MAX_PAIRING_FAILURES: usize = 3;
/// Maximum concurrently displayed SAS candidates.
pub const MAX_ACTIVE_SAS_CANDIDATES: usize = 1;
/// Additional conditional-commit attempts after the initial attempt.
pub const MAX_CONDITIONAL_COMMIT_RETRIES: usize = 1;
/// Duration of one explicit pairing window.
pub const PAIRING_WINDOW: Duration = Duration::from_secs(180);
/// Per-candidate TCP connection timeout.
pub const CANDIDATE_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// Complete connection-attempt budget for one operation.
pub const TOTAL_CONNECT_BUDGET: Duration = Duration::from_secs(10);
/// Authenticated connection idle timeout.
pub const AUTHENTICATED_IDLE_TIMEOUT: Duration = Duration::from_secs(15);
/// Complete ordinary synchronization-session deadline.
pub const SYNC_SESSION_TIMEOUT: Duration = Duration::from_secs(300);

/// Numeric V1 resource boundaries shared by codecs and later network paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceLimit {
    /// Complete encrypted-vault byte length.
    EncryptedVault,
    /// One application data chunk in bytes.
    ApplicationChunk,
    /// One Noise ciphertext frame in bytes.
    NoiseFrame,
    /// Failures allowed during one pairing window.
    PairingFailures,
    /// Concurrent SAS candidates during pairing.
    ActiveSasCandidates,
    /// Candidate sockets attempted for one operation.
    CandidateAttempts,
    /// Concurrent accepted connections for one vault.
    ConnectionsPerVault,
    /// Pending authoritative-host operations.
    PendingHostOperations,
    /// Conditional commit retries after the first attempt.
    ConditionalCommitRetries,
    /// Cached normalized discovery endpoints.
    DiscoveryEndpoints,
}

impl ResourceLimit {
    /// Return the inclusive maximum for this resource.
    #[must_use]
    pub const fn maximum(self) -> u64 {
        match self {
            Self::EncryptedVault => MAX_ENCRYPTED_VAULT,
            Self::ApplicationChunk => MAX_APPLICATION_CHUNK as u64,
            Self::NoiseFrame => u16::MAX as u64,
            Self::PairingFailures => MAX_PAIRING_FAILURES as u64,
            Self::ActiveSasCandidates => MAX_ACTIVE_SAS_CANDIDATES as u64,
            Self::CandidateAttempts => MAX_CANDIDATE_ATTEMPTS as u64,
            Self::ConnectionsPerVault => MAX_CONNECTIONS_PER_VAULT as u64,
            Self::PendingHostOperations => MAX_PENDING_HOST_OPERATIONS as u64,
            Self::ConditionalCommitRetries => MAX_CONDITIONAL_COMMIT_RETRIES as u64,
            Self::DiscoveryEndpoints => MAX_DISCOVERY_ENDPOINTS as u64,
        }
    }

    /// Return whether a claimed or resulting quantity is within the limit.
    #[must_use]
    pub const fn allows(self, value: u64) -> bool {
        value <= self.maximum()
    }
}

/// A SHA-256 digest of the exact canonical encrypted KDBX bytes.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RemoteVersion([u8; 32]);

impl RemoteVersion {
    /// Construct a fixed-width remote version.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the exact 32-byte representation.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Borrow the exact 32-byte representation.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for RemoteVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RemoteVersion([REDACTED])")
    }
}

impl Serialize for RemoteVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&encode_lower(&self.0))
    }
}

impl<'de> Deserialize<'de> for RemoteVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        decode_hex::<32>(&encoded)
            .map(Self)
            .map_err(|()| serde::de::Error::custom("invalid remote version"))
    }
}

/// Generic post-authentication application error codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ErrorCode {
    /// The request violates the V1 protocol.
    InvalidRequest = 0x0001,
    /// The authenticated peer is not authorized for the operation.
    NotAuthorized = 0x0002,
    /// A bounded resource or operation is already occupied.
    Busy = 0x0003,
    /// The conditional version no longer matches.
    StaleVersion = 0x0004,
    /// The declared object or record exceeds a fixed limit.
    TooLarge = 0x0005,
    /// The bounded operation deadline elapsed.
    TimedOut = 0x0006,
    /// The operation was cancelled.
    Cancelled = 0x0007,
    /// The peer encountered an internal failure.
    InternalFailure = 0x0008,
}

impl TryFrom<u16> for ErrorCode {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            0x0001 => Ok(Self::InvalidRequest),
            0x0002 => Ok(Self::NotAuthorized),
            0x0003 => Ok(Self::Busy),
            0x0004 => Ok(Self::StaleVersion),
            0x0005 => Ok(Self::TooLarge),
            0x0006 => Ok(Self::TimedOut),
            0x0007 => Ok(Self::Cancelled),
            0x0008 => Ok(Self::InternalFailure),
            _ => Err(ProtocolError::Malformed),
        }
    }
}

/// One canonical V1 application message.
#[derive(Clone, Eq, PartialEq)]
pub enum Message {
    /// Request the current authoritative version, if any.
    HeadRequest,
    /// Return the current authoritative version, if any.
    HeadResponse {
        /// Current version, or absence when the authority has no object.
        version: Option<RemoteVersion>,
    },
    /// Fetch unless the optional known version is still current.
    FetchRequest {
        /// Version already held by the requester, if any.
        known: Option<RemoteVersion>,
    },
    /// Report that the supplied known version remains current.
    FetchUnchanged,
    /// Begin a bounded encrypted-vault download.
    FetchBegin {
        /// Version of the complete encrypted bytes.
        version: RemoteVersion,
        /// Declared complete encrypted length.
        total_length: u64,
        /// SHA-256 of the complete encrypted bytes.
        digest: [u8; 32],
    },
    /// Carry the next contiguous download chunk.
    FetchChunk {
        /// Zero-based contiguous sequence number.
        sequence: u32,
        /// Bounded encrypted bytes.
        bytes: Vec<u8>,
    },
    /// Finish a download after the declared number of chunks.
    FetchCommit {
        /// Number of chunks sent in this stream.
        chunk_count: u32,
    },
    /// Begin a conditional encrypted-vault upload.
    UploadBegin {
        /// Required prior version, or absence for a first seed.
        expected: Option<RemoteVersion>,
        /// Declared complete encrypted length.
        total_length: u64,
        /// SHA-256 of the complete encrypted bytes.
        digest: [u8; 32],
    },
    /// Carry the next contiguous upload chunk.
    UploadChunk {
        /// Zero-based contiguous sequence number.
        sequence: u32,
        /// Bounded encrypted bytes.
        bytes: Vec<u8>,
    },
    /// Finish an upload after the declared number of chunks.
    UploadCommit {
        /// Number of chunks sent in this stream.
        chunk_count: u32,
    },
    /// Confirm the newly committed authoritative version.
    UploadAccepted {
        /// Version of the newly authoritative encrypted bytes.
        version: RemoteVersion,
    },
    /// Cancel the one active stream with this request ID.
    Cancel,
    /// Return a fixed, string-free application failure.
    Error {
        /// Fixed generic failure code.
        code: ErrorCode,
    },
    /// Begin the two-party trust persistence transaction.
    PairCommit {
        /// CSPRNG transaction identifier.
        transaction_id: [u8; 16],
        /// Digest of the canonical pairing transcript binding.
        transcript_digest: [u8; 32],
    },
    /// Confirm non-authorizing provisional server persistence.
    PairPrepared {
        /// Matching transaction identifier.
        transaction_id: [u8; 16],
        /// Matching transcript digest.
        transcript_digest: [u8; 32],
    },
    /// Request activation after client provisional persistence.
    PairActivate {
        /// Matching transaction identifier.
        transaction_id: [u8; 16],
        /// Matching transcript digest.
        transcript_digest: [u8; 32],
    },
    /// Confirm active server persistence.
    PairActivated {
        /// Matching transaction identifier.
        transaction_id: [u8; 16],
        /// Matching transcript digest.
        transcript_digest: [u8; 32],
    },
}

impl fmt::Debug for Message {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::HeadRequest => "HeadRequest",
            Self::HeadResponse { .. } => "HeadResponse",
            Self::FetchRequest { .. } => "FetchRequest",
            Self::FetchUnchanged => "FetchUnchanged",
            Self::FetchBegin { .. } => "FetchBegin",
            Self::FetchChunk { .. } => "FetchChunk",
            Self::FetchCommit { .. } => "FetchCommit",
            Self::UploadBegin { .. } => "UploadBegin",
            Self::UploadChunk { .. } => "UploadChunk",
            Self::UploadCommit { .. } => "UploadCommit",
            Self::UploadAccepted { .. } => "UploadAccepted",
            Self::Cancel => "Cancel",
            Self::Error { .. } => "Error",
            Self::PairCommit { .. } => "PairCommit",
            Self::PairPrepared { .. } => "PairPrepared",
            Self::PairActivate { .. } => "PairActivate",
            Self::PairActivated { .. } => "PairActivated",
        };
        write!(formatter, "Message::{name}([REDACTED])")
    }
}

/// A decoded request identifier and typed message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedMessage {
    /// Nonzero operation ID, except a connection-scoped generic error.
    pub request_id: u32,
    /// Canonically decoded application message.
    pub message: Message,
}

impl Message {
    /// Encode one exact application record.
    pub fn encode(&self, request_id: u32) -> Result<Vec<u8>, ProtocolError> {
        if request_id == 0 && !matches!(self, Self::Error { .. }) {
            return Err(ProtocolError::Malformed);
        }
        self.validate_limits()?;

        let mut payload = Vec::with_capacity(self.encoded_payload_len());
        self.encode_payload(&mut payload)?;
        let payload_length = u32::try_from(payload.len()).map_err(|_| ProtocolError::TooLarge)?;
        let total = 9_usize
            .checked_add(payload.len())
            .ok_or(ProtocolError::TooLarge)?;
        let mut output = Vec::with_capacity(total);
        output.push(self.message_type());
        output.extend_from_slice(&request_id.to_be_bytes());
        output.extend_from_slice(&payload_length.to_be_bytes());
        output.extend_from_slice(&payload);
        Ok(output)
    }

    /// Decode one exact application record, validating claims before copying.
    pub fn decode(record: &[u8]) -> Result<DecodedMessage, ProtocolError> {
        let header = record.get(..9).ok_or(ProtocolError::Malformed)?;
        let message_type = header[0];
        let request_id = u32::from_be_bytes(
            header[1..5]
                .try_into()
                .map_err(|_| ProtocolError::Malformed)?,
        );
        let payload_length = u32::from_be_bytes(
            header[5..9]
                .try_into()
                .map_err(|_| ProtocolError::Malformed)?,
        );
        let payload_length =
            usize::try_from(payload_length).map_err(|_| ProtocolError::TooLarge)?;
        let expected = 9_usize
            .checked_add(payload_length)
            .ok_or(ProtocolError::TooLarge)?;
        if record.len() != expected {
            return Err(ProtocolError::Malformed);
        }
        let payload = &record[9..];
        let message = Self::decode_payload(message_type, payload)?;
        if request_id == 0 && !matches!(message, Self::Error { .. }) {
            return Err(ProtocolError::Malformed);
        }
        message.validate_limits()?;
        Ok(DecodedMessage {
            request_id,
            message,
        })
    }

    fn validate_limits(&self) -> Result<(), ProtocolError> {
        match self {
            Self::FetchBegin { total_length, .. } | Self::UploadBegin { total_length, .. }
                if !ResourceLimit::EncryptedVault.allows(*total_length) =>
            {
                Err(ProtocolError::TooLarge)
            }
            Self::FetchChunk { bytes, .. } | Self::UploadChunk { bytes, .. }
                if !ResourceLimit::ApplicationChunk.allows(bytes.len() as u64) =>
            {
                Err(ProtocolError::TooLarge)
            }
            _ => Ok(()),
        }
    }

    const fn message_type(&self) -> u8 {
        match self {
            Self::HeadRequest => 0x01,
            Self::HeadResponse { .. } => 0x02,
            Self::FetchRequest { .. } => 0x03,
            Self::FetchUnchanged => 0x04,
            Self::FetchBegin { .. } => 0x05,
            Self::FetchChunk { .. } => 0x06,
            Self::FetchCommit { .. } => 0x07,
            Self::UploadBegin { .. } => 0x08,
            Self::UploadChunk { .. } => 0x09,
            Self::UploadCommit { .. } => 0x0a,
            Self::UploadAccepted { .. } => 0x0b,
            Self::Cancel => 0x0c,
            Self::Error { .. } => 0x0d,
            Self::PairCommit { .. } => 0x20,
            Self::PairPrepared { .. } => 0x21,
            Self::PairActivate { .. } => 0x22,
            Self::PairActivated { .. } => 0x23,
        }
    }

    const fn encoded_payload_len(&self) -> usize {
        match self {
            Self::HeadRequest | Self::FetchUnchanged | Self::Cancel => 0,
            Self::HeadResponse { version } | Self::FetchRequest { known: version } => {
                1 + if version.is_some() { 32 } else { 0 }
            }
            Self::FetchBegin { .. } => 72,
            Self::FetchChunk { bytes, .. } | Self::UploadChunk { bytes, .. } => 6 + bytes.len(),
            Self::FetchCommit { .. } | Self::UploadCommit { .. } => 4,
            Self::UploadBegin { expected, .. } => 41 + if expected.is_some() { 32 } else { 0 },
            Self::UploadAccepted { .. } => 32,
            Self::Error { .. } => 2,
            Self::PairCommit { .. } => 49,
            Self::PairPrepared { .. } | Self::PairActivate { .. } | Self::PairActivated { .. } => {
                48
            }
        }
    }

    fn encode_payload(&self, output: &mut Vec<u8>) -> Result<(), ProtocolError> {
        match self {
            Self::HeadRequest | Self::FetchUnchanged | Self::Cancel => {}
            Self::HeadResponse { version } | Self::FetchRequest { known: version } => {
                put_optional_version(output, *version);
            }
            Self::FetchBegin {
                version,
                total_length,
                digest,
            } => {
                output.extend_from_slice(version.as_bytes());
                output.extend_from_slice(&total_length.to_be_bytes());
                output.extend_from_slice(digest);
            }
            Self::FetchChunk { sequence, bytes } | Self::UploadChunk { sequence, bytes } => {
                output.extend_from_slice(&sequence.to_be_bytes());
                let length = u16::try_from(bytes.len()).map_err(|_| ProtocolError::TooLarge)?;
                output.extend_from_slice(&length.to_be_bytes());
                output.extend_from_slice(bytes);
            }
            Self::FetchCommit { chunk_count } | Self::UploadCommit { chunk_count } => {
                output.extend_from_slice(&chunk_count.to_be_bytes());
            }
            Self::UploadBegin {
                expected,
                total_length,
                digest,
            } => {
                put_optional_version(output, *expected);
                output.extend_from_slice(&total_length.to_be_bytes());
                output.extend_from_slice(digest);
            }
            Self::UploadAccepted { version } => output.extend_from_slice(version.as_bytes()),
            Self::Error { code } => output.extend_from_slice(&(*code as u16).to_be_bytes()),
            Self::PairCommit {
                transaction_id,
                transcript_digest,
            } => {
                output.extend_from_slice(transaction_id);
                output.extend_from_slice(transcript_digest);
                output.push(1);
            }
            Self::PairPrepared {
                transaction_id,
                transcript_digest,
            }
            | Self::PairActivate {
                transaction_id,
                transcript_digest,
            }
            | Self::PairActivated {
                transaction_id,
                transcript_digest,
            } => {
                output.extend_from_slice(transaction_id);
                output.extend_from_slice(transcript_digest);
            }
        }
        Ok(())
    }

    fn decode_payload(message_type: u8, payload: &[u8]) -> Result<Self, ProtocolError> {
        let mut cursor = Cursor::new(payload);
        let message = match message_type {
            0x01 => Self::HeadRequest,
            0x02 => Self::HeadResponse {
                version: cursor.optional_version()?,
            },
            0x03 => Self::FetchRequest {
                known: cursor.optional_version()?,
            },
            0x04 => Self::FetchUnchanged,
            0x05 => Self::FetchBegin {
                version: cursor.version()?,
                total_length: cursor.u64()?,
                digest: cursor.array()?,
            },
            0x06 => Self::FetchChunk {
                sequence: cursor.u32()?,
                bytes: cursor.chunk()?,
            },
            0x07 => Self::FetchCommit {
                chunk_count: cursor.u32()?,
            },
            0x08 => Self::UploadBegin {
                expected: cursor.optional_version()?,
                total_length: cursor.u64()?,
                digest: cursor.array()?,
            },
            0x09 => Self::UploadChunk {
                sequence: cursor.u32()?,
                bytes: cursor.chunk()?,
            },
            0x0a => Self::UploadCommit {
                chunk_count: cursor.u32()?,
            },
            0x0b => Self::UploadAccepted {
                version: cursor.version()?,
            },
            0x0c => Self::Cancel,
            0x0d => Self::Error {
                code: ErrorCode::try_from(cursor.u16()?)?,
            },
            0x20 => {
                let transaction_id = cursor.array()?;
                let transcript_digest = cursor.array()?;
                if cursor.u8()? != 1 {
                    return Err(ProtocolError::Malformed);
                }
                Self::PairCommit {
                    transaction_id,
                    transcript_digest,
                }
            }
            0x21 => Self::PairPrepared {
                transaction_id: cursor.array()?,
                transcript_digest: cursor.array()?,
            },
            0x22 => Self::PairActivate {
                transaction_id: cursor.array()?,
                transcript_digest: cursor.array()?,
            },
            0x23 => Self::PairActivated {
                transaction_id: cursor.array()?,
                transcript_digest: cursor.array()?,
            },
            _ => return Err(ProtocolError::Malformed),
        };
        if !cursor.is_empty() {
            return Err(ProtocolError::Malformed);
        }
        Ok(message)
    }
}

fn put_optional_version(output: &mut Vec<u8>, version: Option<RemoteVersion>) {
    if let Some(version) = version {
        output.push(1);
        output.extend_from_slice(version.as_bytes());
    } else {
        output.push(0);
    }
}

struct Cursor<'a> {
    remaining: &'a [u8],
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProtocolError> {
        let (value, remaining) = self
            .remaining
            .split_at_checked(length)
            .ok_or(ProtocolError::Malformed)?;
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProtocolError> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProtocolError::Malformed)
    }

    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ProtocolError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    fn version(&mut self) -> Result<RemoteVersion, ProtocolError> {
        Ok(RemoteVersion::new(self.array()?))
    }

    fn optional_version(&mut self) -> Result<Option<RemoteVersion>, ProtocolError> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(self.version()?)),
            _ => Err(ProtocolError::Malformed),
        }
    }

    fn chunk(&mut self) -> Result<Vec<u8>, ProtocolError> {
        let length = usize::from(self.u16()?);
        if length > MAX_APPLICATION_CHUNK {
            return Err(ProtocolError::TooLarge);
        }
        Ok(self.take(length)?.to_vec())
    }
}

/// Which endpoint this state machine represents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalRole {
    /// The peer initiating pairing and vault requests.
    Client,
    /// The authoritative vault-owning peer.
    Server,
}

/// Whether an observed record is sent or received by the local endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// The local endpoint sends the record.
    Send,
    /// The local endpoint receives the record.
    Receive,
}

/// Secret-free message or state failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProtocolError {
    /// The record is not the one canonical V1 encoding.
    #[error("malformed application record")]
    Malformed,
    /// A declared or actual size exceeds a fixed V1 limit.
    #[error("application record exceeds a fixed limit")]
    TooLarge,
    /// The record is not valid in the current protocol state.
    #[error("invalid application protocol state")]
    InvalidState,
    /// A request identifier was reused within the session.
    #[error("duplicate application request identifier")]
    DuplicateRequestId,
    /// Application data was attempted before authentication completed.
    #[error("application session is not authorized")]
    NotAuthorized,
}

#[derive(Clone)]
enum ActiveOperation {
    FetchAwait {
        request_id: u32,
    },
    FetchStream {
        request_id: u32,
        next_sequence: u32,
        chunks: u32,
        received: u64,
        total: u64,
    },
    UploadStream {
        request_id: u32,
        next_sequence: u32,
        chunks: u32,
        received: u64,
        total: u64,
    },
    UploadAwaitResult {
        request_id: u32,
    },
    Pair {
        request_id: u32,
        transaction_id: [u8; 16],
        transcript_digest: [u8; 32],
        phase: PairPhase,
    },
}

impl ActiveOperation {
    const fn request_id(&self) -> u32 {
        match self {
            Self::FetchAwait { request_id }
            | Self::FetchStream { request_id, .. }
            | Self::UploadStream { request_id, .. }
            | Self::UploadAwaitResult { request_id }
            | Self::Pair { request_id, .. } => *request_id,
        }
    }

    const fn is_cancellable_stream(&self, request_id: u32) -> bool {
        matches!(
            self,
            Self::FetchStream { request_id: id, .. }
                | Self::UploadStream { request_id: id, .. }
                if *id == request_id
        )
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PairPhase {
    Prepared,
    Activate,
    Activated,
}

/// Per-connection V1 ordering and authorization validator.
///
/// Failed transitions are applied to a clone, so they cannot partially mutate
/// request IDs, sequence counters, or the active-stream state.
#[derive(Clone)]
pub struct ProtocolState {
    mode: SessionMode,
    local_role: LocalRole,
    authenticated: bool,
    last_request_id: u32,
    pending_heads: HashSet<u32>,
    active: Option<ActiveOperation>,
}

impl ProtocolState {
    /// Construct a pre-authentication state that rejects all application data.
    #[must_use]
    pub fn new(mode: SessionMode, local_role: LocalRole) -> Self {
        Self {
            mode,
            local_role,
            authenticated: false,
            last_request_id: 0,
            pending_heads: HashSet::new(),
            active: None,
        }
    }

    /// Mark the fixed Noise handshake and peer authorization as complete.
    pub fn authenticate(&mut self) {
        self.authenticated = true;
    }

    /// Validate one sent or received typed record and atomically advance state.
    pub fn apply(
        &mut self,
        direction: Direction,
        request_id: u32,
        message: &Message,
    ) -> Result<(), ProtocolError> {
        let mut next = self.clone();
        next.apply_inner(direction, request_id, message)?;
        *self = next;
        Ok(())
    }

    fn apply_inner(
        &mut self,
        direction: Direction,
        request_id: u32,
        message: &Message,
    ) -> Result<(), ProtocolError> {
        if !self.authenticated {
            return Err(ProtocolError::NotAuthorized);
        }
        message.validate_limits()?;
        if request_id == 0 && !matches!(message, Message::Error { .. }) {
            return Err(ProtocolError::InvalidState);
        }
        let sender = self.sender(direction);
        match self.mode {
            SessionMode::Pairing => self.apply_pairing(sender, request_id, message),
            SessionMode::Trusted => self.apply_trusted(sender, request_id, message),
        }
    }

    const fn sender(&self, direction: Direction) -> LocalRole {
        match (self.local_role, direction) {
            (role, Direction::Send) => role,
            (LocalRole::Client, Direction::Receive) => LocalRole::Server,
            (LocalRole::Server, Direction::Receive) => LocalRole::Client,
        }
    }

    fn start_request(&mut self, request_id: u32) -> Result<(), ProtocolError> {
        if request_id <= self.last_request_id {
            return Err(ProtocolError::DuplicateRequestId);
        }
        self.last_request_id = request_id;
        Ok(())
    }

    fn apply_trusted(
        &mut self,
        sender: LocalRole,
        request_id: u32,
        message: &Message,
    ) -> Result<(), ProtocolError> {
        match (sender, message) {
            (LocalRole::Client, Message::HeadRequest) => {
                if self.pending_heads.len() >= MAX_PENDING_HOST_OPERATIONS {
                    return Err(ProtocolError::InvalidState);
                }
                self.start_request(request_id)?;
                self.pending_heads.insert(request_id);
                Ok(())
            }
            (LocalRole::Server, Message::HeadResponse { .. })
                if self.pending_heads.remove(&request_id) =>
            {
                Ok(())
            }
            (LocalRole::Client, Message::FetchRequest { .. }) if self.active.is_none() => {
                self.start_request(request_id)?;
                self.active = Some(ActiveOperation::FetchAwait { request_id });
                Ok(())
            }
            (LocalRole::Server, Message::FetchUnchanged) if matches!(self.active, Some(ActiveOperation::FetchAwait { request_id: id }) if id == request_id) =>
            {
                self.active = None;
                Ok(())
            }
            (LocalRole::Server, Message::FetchBegin { total_length, .. }) if matches!(self.active, Some(ActiveOperation::FetchAwait { request_id: id }) if id == request_id) =>
            {
                self.active = Some(ActiveOperation::FetchStream {
                    request_id,
                    next_sequence: 0,
                    chunks: 0,
                    received: 0,
                    total: *total_length,
                });
                Ok(())
            }
            (LocalRole::Server, Message::FetchChunk { sequence, bytes }) => {
                apply_chunk(&mut self.active, request_id, *sequence, bytes.len(), false)
            }
            (LocalRole::Server, Message::FetchCommit { chunk_count }) => {
                finish_stream(&mut self.active, request_id, *chunk_count, false)
            }
            (LocalRole::Client, Message::UploadBegin { total_length, .. })
                if self.active.is_none() =>
            {
                self.start_request(request_id)?;
                self.active = Some(ActiveOperation::UploadStream {
                    request_id,
                    next_sequence: 0,
                    chunks: 0,
                    received: 0,
                    total: *total_length,
                });
                Ok(())
            }
            (LocalRole::Client, Message::UploadChunk { sequence, bytes }) => {
                apply_chunk(&mut self.active, request_id, *sequence, bytes.len(), true)
            }
            (LocalRole::Client, Message::UploadCommit { chunk_count }) => {
                finish_stream(&mut self.active, request_id, *chunk_count, true)
            }
            (LocalRole::Server, Message::UploadAccepted { .. }) if matches!(self.active, Some(ActiveOperation::UploadAwaitResult { request_id: id }) if id == request_id) =>
            {
                self.active = None;
                Ok(())
            }
            (LocalRole::Client, Message::Cancel)
                if self
                    .active
                    .as_ref()
                    .is_some_and(|active| active.is_cancellable_stream(request_id)) =>
            {
                self.active = None;
                Ok(())
            }
            (LocalRole::Server, Message::Error { .. }) => self.finish_error(request_id),
            _ => Err(ProtocolError::InvalidState),
        }
    }

    fn apply_pairing(
        &mut self,
        sender: LocalRole,
        request_id: u32,
        message: &Message,
    ) -> Result<(), ProtocolError> {
        match (sender, message) {
            (
                LocalRole::Client,
                Message::PairCommit {
                    transaction_id,
                    transcript_digest,
                },
            ) if self.active.is_none() => {
                self.start_request(request_id)?;
                self.active = Some(ActiveOperation::Pair {
                    request_id,
                    transaction_id: *transaction_id,
                    transcript_digest: *transcript_digest,
                    phase: PairPhase::Prepared,
                });
                Ok(())
            }
            (
                LocalRole::Server,
                Message::PairPrepared {
                    transaction_id,
                    transcript_digest,
                },
            ) if pair_matches(
                self.active.as_ref(),
                request_id,
                transaction_id,
                transcript_digest,
                PairPhase::Prepared,
            ) =>
            {
                set_pair_phase(&mut self.active, PairPhase::Activate);
                Ok(())
            }
            (
                LocalRole::Client,
                Message::PairActivate {
                    transaction_id,
                    transcript_digest,
                },
            ) if pair_matches(
                self.active.as_ref(),
                request_id,
                transaction_id,
                transcript_digest,
                PairPhase::Activate,
            ) =>
            {
                set_pair_phase(&mut self.active, PairPhase::Activated);
                Ok(())
            }
            (
                LocalRole::Server,
                Message::PairActivated {
                    transaction_id,
                    transcript_digest,
                },
            ) if pair_matches(
                self.active.as_ref(),
                request_id,
                transaction_id,
                transcript_digest,
                PairPhase::Activated,
            ) =>
            {
                self.active = None;
                Ok(())
            }
            (LocalRole::Server, Message::Error { .. }) => self.finish_error(request_id),
            _ => Err(ProtocolError::InvalidState),
        }
    }

    fn finish_error(&mut self, request_id: u32) -> Result<(), ProtocolError> {
        if request_id == 0 {
            return Ok(());
        }
        if self.pending_heads.remove(&request_id) {
            return Ok(());
        }
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.request_id() == request_id)
        {
            self.active = None;
            return Ok(());
        }
        Err(ProtocolError::InvalidState)
    }
}

fn apply_chunk(
    active: &mut Option<ActiveOperation>,
    request_id: u32,
    sequence: u32,
    length: usize,
    upload: bool,
) -> Result<(), ProtocolError> {
    let state = active.as_mut().ok_or(ProtocolError::InvalidState)?;
    let (id, next_sequence, chunks, received, total) = match state {
        ActiveOperation::UploadStream {
            request_id,
            next_sequence,
            chunks,
            received,
            total,
        } if upload => (request_id, next_sequence, chunks, received, total),
        ActiveOperation::FetchStream {
            request_id,
            next_sequence,
            chunks,
            received,
            total,
        } if !upload => (request_id, next_sequence, chunks, received, total),
        _ => return Err(ProtocolError::InvalidState),
    };
    if *id != request_id || *next_sequence != sequence || length > MAX_APPLICATION_CHUNK {
        return Err(ProtocolError::InvalidState);
    }
    let length = u64::try_from(length).map_err(|_| ProtocolError::TooLarge)?;
    let next_received = received
        .checked_add(length)
        .ok_or(ProtocolError::TooLarge)?;
    if next_received > *total || next_received > MAX_ENCRYPTED_VAULT {
        return Err(ProtocolError::InvalidState);
    }
    *received = next_received;
    *next_sequence = next_sequence
        .checked_add(1)
        .ok_or(ProtocolError::InvalidState)?;
    *chunks = chunks.checked_add(1).ok_or(ProtocolError::InvalidState)?;
    Ok(())
}

fn finish_stream(
    active: &mut Option<ActiveOperation>,
    request_id: u32,
    chunk_count: u32,
    upload: bool,
) -> Result<(), ProtocolError> {
    let state = active.as_ref().ok_or(ProtocolError::InvalidState)?;
    let (id, chunks, received, total) = match state {
        ActiveOperation::UploadStream {
            request_id,
            chunks,
            received,
            total,
            ..
        } if upload => (*request_id, *chunks, *received, *total),
        ActiveOperation::FetchStream {
            request_id,
            chunks,
            received,
            total,
            ..
        } if !upload => (*request_id, *chunks, *received, *total),
        _ => return Err(ProtocolError::InvalidState),
    };
    if id != request_id || chunks != chunk_count || received != total {
        return Err(ProtocolError::InvalidState);
    }
    *active = if upload {
        Some(ActiveOperation::UploadAwaitResult { request_id })
    } else {
        None
    };
    Ok(())
}

fn pair_matches(
    active: Option<&ActiveOperation>,
    request_id: u32,
    transaction_id: &[u8; 16],
    transcript_digest: &[u8; 32],
    phase: PairPhase,
) -> bool {
    matches!(
        active,
        Some(ActiveOperation::Pair {
            request_id: id,
            transaction_id: transaction,
            transcript_digest: digest,
            phase: actual,
        }) if *id == request_id
            && transaction == transaction_id
            && digest == transcript_digest
            && *actual == phase
    )
}

fn set_pair_phase(active: &mut Option<ActiveOperation>, phase: PairPhase) {
    if let Some(ActiveOperation::Pair { phase: current, .. }) = active {
        *current = phase;
    }
}

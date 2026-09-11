//! Human-confirmed XX pairing window and transcript binding.

use std::{fmt, time::Instant};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{
    encoding::encode_lower,
    identity::{decode_hex, PublicIdentity},
    protocol::{MAX_PAIRING_FAILURES, PAIRING_WINDOW},
};

const SAS_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const PAIRING_BINDING: &[u8] = b"HIDLINS-PAIR\0";

/// Six Crockford Base32 characters derived from 30 transcript bits.
#[derive(Clone, Eq, PartialEq, Zeroize, ZeroizeOnDrop)]
pub struct SasCode([u8; 6]);

impl SasCode {
    /// Derive the first 30 hash bits in network bit order.
    #[must_use]
    pub fn derive(handshake_hash: &[u8; 32]) -> Self {
        let bits = u32::from_be_bytes([
            handshake_hash[0],
            handshake_hash[1],
            handshake_hash[2],
            handshake_hash[3],
        ]) >> 2;
        let mut code = [0_u8; 6];
        for (index, shift) in [25_u32, 20, 15, 10, 5, 0].into_iter().enumerate() {
            code[index] = SAS_ALPHABET[((bits >> shift) & 0x1f) as usize];
        }
        Self(code)
    }
}

impl fmt::Display for SasCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let first = std::str::from_utf8(&self.0[..3]).map_err(|_| fmt::Error)?;
        let second = std::str::from_utf8(&self.0[3..]).map_err(|_| fmt::Error)?;
        write!(formatter, "{first}-{second}")
    }
}

impl fmt::Debug for SasCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SasCode([REDACTED])")
    }
}

/// CSPRNG identifier for one durable pairing transaction.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct TransactionId([u8; 16]);

impl TransactionId {
    /// Generate a fresh transaction identifier.
    pub fn generate() -> Result<Self, PairingError> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| PairingError::EntropyUnavailable)?;
        Ok(Self(bytes))
    }

    /// Construct from authenticated wire bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Return authenticated wire bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Debug for TransactionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TransactionId([REDACTED])")
    }
}

impl Serialize for TransactionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&encode_lower(&self.0))
    }
}

impl<'de> Deserialize<'de> for TransactionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        decode_hex::<16>(&encoded)
            .map(Self)
            .map_err(|()| D::Error::custom("invalid pairing transaction"))
    }
}

/// SHA-256 binding of the complete authenticated pairing context.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct TranscriptDigest([u8; 32]);

impl TranscriptDigest {
    /// Construct from authenticated wire bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return authenticated wire bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for TranscriptDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TranscriptDigest([REDACTED])")
    }
}

impl Serialize for TranscriptDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&encode_lower(&self.0))
    }
}

impl<'de> Deserialize<'de> for TranscriptDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        decode_hex::<32>(&encoded)
            .map(Self)
            .map_err(|()| D::Error::custom("invalid pairing transcript"))
    }
}

/// Canonical authenticated inputs for one pairing persistence transaction.
#[derive(Clone, Eq, PartialEq)]
pub struct PairingTransaction {
    transaction_id: TransactionId,
    transcript_digest: TranscriptDigest,
    server_key: PublicIdentity,
    client_key: PublicIdentity,
}

impl PairingTransaction {
    /// Generate a fresh transaction bound to the completed XX transcript.
    fn generate(
        handshake_hash: &[u8; 32],
        server_key: PublicIdentity,
        client_key: PublicIdentity,
    ) -> Result<Self, PairingError> {
        let transaction_id = TransactionId::generate()?;
        Ok(Self::from_authenticated(
            transaction_id,
            binding_digest(handshake_hash, server_key, client_key, transaction_id),
            server_key,
            client_key,
        ))
    }

    /// Reconstruct authenticated wire fields for local verification.
    #[must_use]
    pub const fn from_authenticated(
        transaction_id: TransactionId,
        transcript_digest: TranscriptDigest,
        server_key: PublicIdentity,
        client_key: PublicIdentity,
    ) -> Self {
        Self {
            transaction_id,
            transcript_digest,
            server_key,
            client_key,
        }
    }

    /// Verify that no transcript, role, key, or transaction input changed.
    #[must_use]
    pub fn validates(&self, handshake_hash: &[u8; 32]) -> bool {
        self.transcript_digest
            == binding_digest(
                handshake_hash,
                self.server_key,
                self.client_key,
                self.transaction_id,
            )
    }

    /// Return the transaction identifier.
    #[must_use]
    pub const fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }

    /// Return the canonical transcript digest.
    #[must_use]
    pub const fn transcript_digest(&self) -> TranscriptDigest {
        self.transcript_digest
    }

    /// Return the authoritative server identity.
    #[must_use]
    pub const fn server_key(&self) -> PublicIdentity {
        self.server_key
    }

    /// Return the client identity.
    #[must_use]
    pub const fn client_key(&self) -> PublicIdentity {
        self.client_key
    }
}

impl fmt::Debug for PairingTransaction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PairingTransaction([REDACTED])")
    }
}

/// Locally confirmed pairing material; never serialized.
#[derive(ZeroizeOnDrop)]
pub struct ConfirmedPairing {
    #[zeroize(skip)]
    peer_key: PublicIdentity,
    handshake_hash: Zeroizing<[u8; 32]>,
}

impl ConfirmedPairing {
    /// Construct from a completed fixed-suite XX transcript.
    pub(crate) fn from_authenticated(peer_key: PublicIdentity, handshake_hash: [u8; 32]) -> Self {
        Self {
            peer_key,
            handshake_hash: Zeroizing::new(handshake_hash),
        }
    }

    /// Return the authenticated peer public key.
    #[must_use]
    pub const fn peer_key(&self) -> PublicIdentity {
        self.peer_key
    }

    /// Create the client's fresh durable pairing transaction.
    pub fn transaction(
        &self,
        server_key: PublicIdentity,
        client_key: PublicIdentity,
    ) -> Result<PairingTransaction, PairingError> {
        PairingTransaction::generate(&self.handshake_hash, server_key, client_key)
    }

    pub(crate) fn handshake_hash(&self) -> &[u8; 32] {
        &self.handshake_hash
    }
}

impl fmt::Debug for ConfirmedPairing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ConfirmedPairing([REDACTED])")
    }
}

struct Candidate {
    peer_key: PublicIdentity,
    handshake_hash: Zeroizing<[u8; 32]>,
    sas: SasCode,
}

/// One explicit, bounded three-minute pairing window.
pub struct PairingWindow {
    deadline: Instant,
    failures: usize,
    candidate: Option<Candidate>,
}

impl PairingWindow {
    /// Open a new window using the supplied monotonic instant.
    #[must_use]
    pub fn open_at(now: Instant) -> Self {
        Self {
            deadline: now + PAIRING_WINDOW,
            failures: 0,
            candidate: None,
        }
    }

    /// Admit the sole active SAS candidate after an authenticated XX handshake.
    pub fn begin_candidate(
        &mut self,
        peer_key: PublicIdentity,
        handshake_hash: [u8; 32],
        now: Instant,
    ) -> Result<SasCode, PairingError> {
        self.ensure_open(now)?;
        if self.candidate.is_some() {
            return Err(PairingError::Busy);
        }
        let sas = SasCode::derive(&handshake_hash);
        self.candidate = Some(Candidate {
            peer_key,
            handshake_hash: Zeroizing::new(handshake_hash),
            sas: sas.clone(),
        });
        Ok(sas)
    }

    /// Record explicit local confirmation of the locally derived SAS.
    pub fn confirm(
        &mut self,
        mut sas: SasCode,
        now: Instant,
    ) -> Result<ConfirmedPairing, PairingError> {
        self.ensure_open(now)?;
        let Some(candidate) = self.candidate.as_ref() else {
            sas.zeroize();
            return Err(PairingError::NoCandidate);
        };
        let matches = candidate.sas == sas;
        sas.zeroize();
        if !matches {
            self.record_failure();
            return Err(PairingError::Mismatch);
        }
        let candidate = self.candidate.take().ok_or(PairingError::NoCandidate)?;
        Ok(ConfirmedPairing {
            peer_key: candidate.peer_key,
            handshake_hash: candidate.handshake_hash,
        })
    }

    /// Reject or disconnect the current candidate and count one failure.
    pub fn reject(&mut self, now: Instant) -> Result<(), PairingError> {
        self.ensure_open(now)?;
        if self.candidate.take().is_none() {
            return Err(PairingError::NoCandidate);
        }
        self.record_failure();
        Ok(())
    }

    /// Count a failed XX handshake that did not reach SAS presentation.
    pub fn record_failed_attempt(&mut self, now: Instant) -> Result<(), PairingError> {
        self.ensure_open(now)?;
        self.record_failure();
        Ok(())
    }

    /// Return whether the time and failure bounds still permit a candidate.
    #[must_use]
    pub fn is_open(&self, now: Instant) -> bool {
        now < self.deadline && self.failures < MAX_PAIRING_FAILURES
    }

    /// Return the number of failed handshakes/confirmations in this window.
    #[must_use]
    pub const fn failures(&self) -> usize {
        self.failures
    }

    /// Return the fixed monotonic deadline for runtime I/O budgeting.
    #[must_use]
    pub const fn deadline(&self) -> Instant {
        self.deadline
    }

    fn ensure_open(&mut self, now: Instant) -> Result<(), PairingError> {
        if !self.is_open(now) {
            self.candidate = None;
            return Err(PairingError::Closed);
        }
        Ok(())
    }

    fn record_failure(&mut self) {
        self.candidate = None;
        self.failures = self.failures.saturating_add(1);
    }
}

impl fmt::Debug for PairingWindow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairingWindow")
            .field("failures", &self.failures)
            .field("candidate", &self.candidate.as_ref().map(|_| "[REDACTED]"))
            .finish_non_exhaustive()
    }
}

/// Secret-free pairing failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PairingError {
    /// The bounded window expired or reached its failure limit.
    #[error("pairing window is closed")]
    Closed,
    /// One SAS candidate is already awaiting local action.
    #[error("pairing candidate is already active")]
    Busy,
    /// No candidate is awaiting local action.
    #[error("no pairing candidate is active")]
    NoCandidate,
    /// The local confirmation does not match the derived SAS.
    #[error("pairing confirmation did not match")]
    Mismatch,
    /// The operating-system CSPRNG could not create a transaction ID.
    #[error("pairing randomness is unavailable")]
    EntropyUnavailable,
}

fn binding_digest(
    handshake_hash: &[u8; 32],
    server_key: PublicIdentity,
    client_key: PublicIdentity,
    transaction_id: TransactionId,
) -> TranscriptDigest {
    let mut hasher = Sha256::new();
    hasher.update(PAIRING_BINDING);
    hasher.update([1_u8]);
    hasher.update(handshake_hash);
    hasher.update(server_key.as_bytes());
    hasher.update(client_key.as_bytes());
    hasher.update([1_u8, 2_u8]);
    hasher.update(transaction_id.into_bytes());
    TranscriptDigest::new(hasher.finalize().into())
}

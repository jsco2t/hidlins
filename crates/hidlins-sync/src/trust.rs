//! Durable pinned-peer and provisional pairing state.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    config::local::LocalSyncConfig,
    identity::{PublicIdentity, SyncRole},
    noise::{HandshakeError, SecureTransport, SessionMode},
    pairing::{ConfirmedPairing, PairingTransaction, TransactionId, TranscriptDigest},
    protocol::PAIRING_WINDOW,
};

/// Durable authorization state for a paired peer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerStatus {
    /// The exact public key may authenticate through trusted Noise IK.
    Active,
    /// The key remains as local audit metadata but cannot authorize.
    Revoked,
}

/// One locally named, pinned peer identity.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PeerRecord {
    public_key: PublicIdentity,
    display_name: String,
    status: PeerStatus,
}

impl PeerRecord {
    /// Construct a locally named active peer.
    pub fn active(public_key: PublicIdentity, display_name: String) -> Result<Self, TrustError> {
        validate_display_name(&display_name)?;
        Ok(Self {
            public_key,
            display_name,
            status: PeerStatus::Active,
        })
    }

    /// Return the pinned public identity.
    #[must_use]
    pub const fn public_key(&self) -> PublicIdentity {
        self.public_key
    }

    /// Return the local-only display name.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Return the durable authorization status.
    #[must_use]
    pub const fn status(&self) -> PeerStatus {
        self.status
    }
}

impl fmt::Debug for PeerRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PeerRecord")
            .field("identity", &"[REDACTED]")
            .field("display_name", &"[REDACTED]")
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// Non-authorizing durable half of a pairing transaction.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProvisionalPairing {
    transaction_id: TransactionId,
    transcript_digest: TranscriptDigest,
    peer: PeerRecord,
    created_at_epoch_seconds: u64,
    expires_at_epoch_seconds: u64,
}

impl ProvisionalPairing {
    pub(crate) fn is_valid(&self) -> bool {
        self.peer.is_valid()
            && self.peer.status == PeerStatus::Active
            && self
                .expires_at_epoch_seconds
                .checked_sub(self.created_at_epoch_seconds)
                .is_some_and(|lifetime| lifetime > 0 && lifetime <= PAIRING_WINDOW.as_secs())
    }

    /// Return the authenticated peer admitted only for bounded pairing recovery.
    #[must_use]
    pub const fn peer_key(&self) -> PublicIdentity {
        self.peer.public_key
    }
}

impl fmt::Debug for ProvisionalPairing {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProvisionalPairing([REDACTED])")
    }
}

/// Secret-free trust and authorization failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TrustError {
    /// The local configuration role cannot perform this operation.
    #[error("sync role cannot perform this trust operation")]
    WrongRole,
    /// The peer name is empty or exceeds the fixed local metadata bound.
    #[error("invalid peer display name")]
    InvalidDisplayName,
    /// An active or revoked record already uses this identity.
    #[error("peer identity is already known")]
    DuplicatePeer,
    /// A client already pins another authority and requires explicit reset.
    #[error("server re-designation requires explicit reset and fresh pairing")]
    RedesignationRequired,
    /// The transcript, key, transaction, or role binding does not match.
    #[error("pairing transaction did not authenticate")]
    TransactionMismatch,
    /// No matching provisional transaction exists.
    #[error("pairing transaction is not prepared")]
    NotPrepared,
    /// The provisional transaction is expired.
    #[error("pairing transaction expired")]
    Expired,
    /// The peer is absent or already revoked.
    #[error("peer is not active")]
    NotActive,
}

/// A trusted-mode Noise transport whose exact peer key passed durable policy.
pub struct AuthorizedTransport {
    inner: SecureTransport,
    peer: PublicIdentity,
}

impl AuthorizedTransport {
    /// Return the exact pinned identity authorized for this channel.
    #[must_use]
    pub const fn peer(&self) -> PublicIdentity {
        self.peer
    }

    /// Encrypt one bounded application payload on the authorized channel.
    pub fn write_message(
        &mut self,
        payload: &[u8],
        message: &mut [u8],
    ) -> Result<usize, HandshakeError> {
        self.inner.write_message(payload, message)
    }

    /// Authenticate and decrypt one bounded application payload.
    pub fn read_message(
        &mut self,
        message: &[u8],
        payload: &mut [u8],
    ) -> Result<usize, HandshakeError> {
        self.inner.read_message(message, payload)
    }
}

impl fmt::Debug for AuthorizedTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AuthorizedTransport([REDACTED])")
    }
}

impl LocalSyncConfig {
    /// Return whether an exact pinned, active identity may enter trusted IK.
    #[must_use]
    pub fn authorizes(&self, peer_key: PublicIdentity) -> bool {
        match self.role() {
            SyncRole::Server => self
                .trusted_peers()
                .iter()
                .any(|peer| peer.public_key == peer_key && peer.status == PeerStatus::Active),
            SyncRole::Client => self.pinned_server().is_some_and(|peer| {
                peer.public_key == peer_key && peer.status == PeerStatus::Active
            }),
        }
    }

    /// Consume a completed IK channel only when its exact peer pin is active.
    pub fn authorize_transport(
        &self,
        transport: SecureTransport,
    ) -> Result<AuthorizedTransport, TrustError> {
        if transport.mode() != SessionMode::Trusted {
            return Err(TrustError::NotActive);
        }
        let peer = PublicIdentity::new(transport.peer_static());
        if !self.authorizes(peer) {
            return Err(TrustError::NotActive);
        }
        Ok(AuthorizedTransport {
            inner: transport,
            peer,
        })
    }

    /// Persist a non-authorizing provisional client record on the server.
    pub fn prepare_client(
        &mut self,
        transaction: &PairingTransaction,
        confirmation: &ConfirmedPairing,
        display_name: String,
        now_epoch_seconds: u64,
        expires_at_epoch_seconds: u64,
    ) -> Result<(), TrustError> {
        if self.role() != SyncRole::Server
            || transaction.server_key() != self.identity().public_key()
        {
            return Err(TrustError::WrongRole);
        }
        if confirmation.peer_key() != transaction.client_key() {
            return Err(TrustError::TransactionMismatch);
        }
        self.prepare(
            transaction,
            confirmation,
            transaction.client_key(),
            display_name,
            now_epoch_seconds,
            expires_at_epoch_seconds,
        )
    }

    /// Persist a non-authorizing provisional server pin on the client.
    pub fn prepare_server(
        &mut self,
        transaction: &PairingTransaction,
        confirmation: &ConfirmedPairing,
        display_name: String,
        now_epoch_seconds: u64,
        expires_at_epoch_seconds: u64,
    ) -> Result<(), TrustError> {
        if self.role() != SyncRole::Client
            || transaction.client_key() != self.identity().public_key()
        {
            return Err(TrustError::WrongRole);
        }
        if self.pinned_server().is_some() {
            return Err(TrustError::RedesignationRequired);
        }
        if confirmation.peer_key() != transaction.server_key() {
            return Err(TrustError::TransactionMismatch);
        }
        self.prepare(
            transaction,
            confirmation,
            transaction.server_key(),
            display_name,
            now_epoch_seconds,
            expires_at_epoch_seconds,
        )
    }

    fn prepare(
        &mut self,
        transaction: &PairingTransaction,
        confirmation: &ConfirmedPairing,
        peer_key: PublicIdentity,
        display_name: String,
        now_epoch_seconds: u64,
        expires_at_epoch_seconds: u64,
    ) -> Result<(), TrustError> {
        if !transaction.validates(confirmation.handshake_hash()) {
            return Err(TrustError::TransactionMismatch);
        }
        validate_expiry(now_epoch_seconds, expires_at_epoch_seconds)?;
        if let Some(previous) = self.provisional().cloned() {
            if previous.peer.public_key == peer_key {
                let recovered = self.recover_pairing_transaction(peer_key, now_epoch_seconds)?;
                return if recovered == *transaction {
                    Ok(())
                } else {
                    Err(TrustError::TransactionMismatch)
                };
            }
            // A server retains an activated provisional as a bounded receipt
            // for lost PairActivated recovery. Once that exact peer is active,
            // a separately authenticated new pairing may replace the receipt.
            let previous_is_active = self.role() == SyncRole::Server
                && self.trusted_peers().iter().any(|peer| {
                    peer.public_key == previous.peer.public_key && peer.status == PeerStatus::Active
                });
            if previous_is_active {
                self.set_provisional(None);
            } else {
                return Err(TrustError::TransactionMismatch);
            }
        }
        if self
            .trusted_peers()
            .iter()
            .any(|peer| peer.public_key == peer_key)
        {
            return Err(TrustError::DuplicatePeer);
        }
        let peer = PeerRecord::active(peer_key, display_name)?;
        self.set_provisional(Some(ProvisionalPairing {
            transaction_id: transaction.transaction_id(),
            transcript_digest: transaction.transcript_digest(),
            peer,
            created_at_epoch_seconds: now_epoch_seconds,
            expires_at_epoch_seconds,
        }));
        Ok(())
    }

    /// Atomically promote a matching server-side provisional client.
    pub fn activate_client(
        &mut self,
        transaction: &PairingTransaction,
        now_epoch_seconds: u64,
    ) -> Result<(), TrustError> {
        if self.role() != SyncRole::Server {
            return Err(TrustError::WrongRole);
        }
        let provisional = matching_provisional(self, transaction, now_epoch_seconds)?.clone();
        if let Some(existing) = self
            .trusted_peers()
            .iter()
            .find(|peer| peer.public_key == provisional.peer.public_key)
        {
            return if existing.status == PeerStatus::Active {
                Ok(())
            } else {
                Err(TrustError::NotActive)
            };
        }
        // Retain the bounded provisional as a non-authorizing receipt so an
        // authenticated peer can recover if PAIR_ACTIVATED is lost.
        self.trusted_peers_mut().push(provisional.peer);
        Ok(())
    }

    /// Atomically promote a matching client-side provisional server pin.
    pub fn activate_server(
        &mut self,
        transaction: &PairingTransaction,
        now_epoch_seconds: u64,
    ) -> Result<(), TrustError> {
        if self.role() != SyncRole::Client {
            return Err(TrustError::WrongRole);
        }
        let provisional = matching_provisional(self, transaction, now_epoch_seconds)?.clone();
        self.set_provisional(None);
        self.set_pinned_server(Some(provisional.peer));
        Ok(())
    }

    /// Remove an expired provisional record without changing active trust.
    pub fn expire_provisional(&mut self, now_epoch_seconds: u64) -> bool {
        if self
            .provisional()
            .is_some_and(|record| now_epoch_seconds >= record.expires_at_epoch_seconds)
        {
            self.set_provisional(None);
            true
        } else {
            false
        }
    }

    /// Reconstruct only the matching unexpired transaction for an
    /// authenticated peer; this does not grant trusted vault operations.
    pub fn recover_pairing_transaction(
        &self,
        authenticated_peer: PublicIdentity,
        now_epoch_seconds: u64,
    ) -> Result<PairingTransaction, TrustError> {
        let provisional = self.provisional().ok_or(TrustError::NotPrepared)?;
        if now_epoch_seconds >= provisional.expires_at_epoch_seconds {
            return Err(TrustError::Expired);
        }
        if provisional.peer.public_key != authenticated_peer {
            return Err(TrustError::TransactionMismatch);
        }
        if self
            .trusted_peers()
            .iter()
            .any(|peer| peer.public_key == authenticated_peer && peer.status == PeerStatus::Revoked)
        {
            return Err(TrustError::NotActive);
        }
        let (server_key, client_key) = match self.role() {
            SyncRole::Server => (self.identity().public_key(), provisional.peer.public_key),
            SyncRole::Client => (provisional.peer.public_key, self.identity().public_key()),
        };
        Ok(PairingTransaction::from_authenticated(
            provisional.transaction_id,
            provisional.transcript_digest,
            server_key,
            client_key,
        ))
    }

    /// Rename a peer's local-only label without changing its identity.
    pub fn rename_peer(
        &mut self,
        public_key: PublicIdentity,
        display_name: String,
    ) -> Result<(), TrustError> {
        validate_display_name(&display_name)?;
        let peer = self.peer_mut(public_key).ok_or(TrustError::NotActive)?;
        peer.display_name = display_name;
        Ok(())
    }

    /// Revoke an exact peer key; live sessions must be closed by the caller.
    pub fn revoke_peer(&mut self, public_key: PublicIdentity) -> Result<(), TrustError> {
        {
            let peer = self
                .peer_mut(public_key)
                .filter(|peer| peer.status == PeerStatus::Active)
                .ok_or(TrustError::NotActive)?;
            peer.status = PeerStatus::Revoked;
        }
        if self
            .provisional()
            .is_some_and(|receipt| receipt.peer_key() == public_key)
        {
            self.set_provisional(None);
        }
        Ok(())
    }

    /// Explicitly clear a client's authority before a mandatory fresh pairing.
    pub fn begin_server_redesignation(&mut self) -> Result<(), TrustError> {
        if self.role() != SyncRole::Client {
            return Err(TrustError::WrongRole);
        }
        self.set_pinned_server(None);
        self.set_provisional(None);
        Ok(())
    }

    fn peer_mut(&mut self, public_key: PublicIdentity) -> Option<&mut PeerRecord> {
        match self.role() {
            SyncRole::Server => self
                .trusted_peers_mut()
                .iter_mut()
                .find(|peer| peer.public_key == public_key),
            SyncRole::Client => self
                .pinned_server_mut()
                .filter(|peer| peer.public_key == public_key),
        }
    }
}

fn matching_provisional<'a>(
    config: &'a LocalSyncConfig,
    transaction: &PairingTransaction,
    now_epoch_seconds: u64,
) -> Result<&'a ProvisionalPairing, TrustError> {
    let provisional = config.provisional().ok_or(TrustError::NotPrepared)?;
    if now_epoch_seconds >= provisional.expires_at_epoch_seconds {
        return Err(TrustError::Expired);
    }
    if provisional.transaction_id != transaction.transaction_id()
        || provisional.transcript_digest != transaction.transcript_digest()
        || provisional.peer.public_key
            != match config.role() {
                SyncRole::Server => transaction.client_key(),
                SyncRole::Client => transaction.server_key(),
            }
    {
        return Err(TrustError::TransactionMismatch);
    }
    Ok(provisional)
}

fn validate_display_name(display_name: &str) -> Result<(), TrustError> {
    if display_name.trim().is_empty() || display_name.len() > 255 {
        Err(TrustError::InvalidDisplayName)
    } else {
        Ok(())
    }
}

impl PeerRecord {
    pub(crate) fn is_valid(&self) -> bool {
        validate_display_name(&self.display_name).is_ok()
    }
}

fn validate_expiry(now: u64, expires: u64) -> Result<(), TrustError> {
    let lifetime = expires.checked_sub(now).ok_or(TrustError::Expired)?;
    if lifetime == 0 || lifetime > PAIRING_WINDOW.as_secs() {
        Err(TrustError::Expired)
    } else {
        Ok(())
    }
}

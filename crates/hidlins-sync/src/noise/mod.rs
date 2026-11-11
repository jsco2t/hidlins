//! Fixed-suite Noise sessions for local-network sync.
//!
//! This is the only product module allowed to construct Snow state. Callers
//! choose pairing or trusted mode, never arbitrary patterns or algorithms.

use std::{
    fmt,
    time::{Duration, Instant},
};

use zeroize::{ZeroizeOnDrop, Zeroizing};

/// Fixed Noise pattern used for first-time pairing.
pub const PAIRING_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
/// Fixed Noise pattern used once both peers have pinned identities.
pub const TRUSTED_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_SHA256";
/// Protocol/version/mode preface bound into every pairing handshake.
pub const PREFACE_PAIRING: &[u8; 12] = b"HIDLINS\0\x01\x01\0\0";
/// Protocol/version/mode preface bound into every trusted handshake.
pub const PREFACE_TRUSTED: &[u8; 12] = b"HIDLINS\0\x01\x02\0\0";
/// Maximum elapsed time permitted for a handshake.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

/// Local-network synchronization session mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionMode {
    /// First-contact mutual authentication using Noise XX.
    Pairing,
    /// Pinned-peer authentication using Noise IK.
    Trusted,
}

/// A static X25519 identity keypair.
///
/// The private half is never exposed and is erased when this value is
/// dropped. The public half may be persisted as a peer identity.
#[derive(ZeroizeOnDrop)]
pub struct NoiseKeypair {
    private: Zeroizing<[u8; 32]>,
    public: [u8; 32],
}

impl NoiseKeypair {
    /// Generate a new static identity using the operating system CSPRNG.
    pub fn generate() -> Result<Self, HandshakeError> {
        let params = PAIRING_PATTERN
            .parse()
            .map_err(|_| HandshakeError::InvalidConfiguration)?;
        let generated = snow::Builder::new(params)
            .generate_keypair()
            .map_err(|_| HandshakeError::CryptographicFailure)?;
        if generated.private.len() != 32 {
            return Err(HandshakeError::CryptographicFailure);
        }
        let mut private = Zeroizing::new([0_u8; 32]);
        private.copy_from_slice(&generated.private);
        let public = generated
            .public
            .as_slice()
            .try_into()
            .map_err(|_| HandshakeError::CryptographicFailure)?;
        Ok(Self { private, public })
    }

    /// Return the public identity key.
    #[must_use]
    pub const fn public_key(&self) -> [u8; 32] {
        self.public
    }

    pub(crate) fn private_key(&self) -> &[u8; 32] {
        &self.private
    }

    pub(crate) fn from_persisted(private: Zeroizing<[u8; 32]>, public: [u8; 32]) -> Self {
        Self { private, public }
    }
}

impl fmt::Debug for NoiseKeypair {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("NoiseKeypair([REDACTED])")
    }
}

/// A deliberately low-detail secure-session failure.
///
/// Snow's detailed errors are never propagated because doing so would expose
/// a useful handshake oracle to an unauthenticated network peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HandshakeError {
    /// A fixed, compile-time protocol configuration could not be constructed.
    #[error("secure-session configuration is unavailable")]
    InvalidConfiguration,
    /// Authentication, decryption, randomness, or message validation failed.
    #[error("secure-session authentication failed")]
    CryptographicFailure,
    /// The five-second handshake deadline elapsed.
    #[error("secure-session handshake timed out")]
    TimedOut,
    /// The session was not ready to enter transport mode.
    #[error("secure-session state is incomplete")]
    Incomplete,
}

/// An in-progress fixed-suite Noise handshake.
pub struct HandshakeSession {
    state: snow::HandshakeState,
    mode: SessionMode,
    expected_remote: Option<[u8; 32]>,
    deadline: Instant,
}

impl HandshakeSession {
    /// Construct the initiating side of a first-contact XX handshake.
    pub fn pairing_initiator(local: &NoiseKeypair) -> Result<Self, HandshakeError> {
        Self::build(local, None, SessionMode::Pairing, true)
    }

    /// Construct the responding side of a first-contact XX handshake.
    pub fn pairing_responder(local: &NoiseKeypair) -> Result<Self, HandshakeError> {
        Self::build(local, None, SessionMode::Pairing, false)
    }

    /// Construct the initiating side of a pinned-peer IK handshake.
    pub fn trusted_initiator(
        local: &NoiseKeypair,
        remote: [u8; 32],
    ) -> Result<Self, HandshakeError> {
        Self::build(local, Some(remote), SessionMode::Trusted, true)
    }

    /// Construct the responding side of a pinned-peer IK handshake.
    pub fn trusted_responder(
        local: &NoiseKeypair,
        remote: [u8; 32],
    ) -> Result<Self, HandshakeError> {
        Self::build(local, Some(remote), SessionMode::Trusted, false)
    }

    /// Construct a responding IK session that authenticates the initiator's
    /// static key for authorization immediately after the handshake.
    ///
    /// Noise IK does not consume the initiator key while constructing the
    /// responder. Callers using this entry point must reject the returned
    /// transport unless [`SecureTransport::peer_static`] is actively trusted.
    pub fn trusted_responder_for_authorization(
        local: &NoiseKeypair,
    ) -> Result<Self, HandshakeError> {
        Self::build(local, None, SessionMode::Trusted, false)
    }

    fn build(
        local: &NoiseKeypair,
        remote: Option<[u8; 32]>,
        mode: SessionMode,
        initiator: bool,
    ) -> Result<Self, HandshakeError> {
        let (pattern, preface) = match mode {
            SessionMode::Pairing => (PAIRING_PATTERN, PREFACE_PAIRING.as_slice()),
            SessionMode::Trusted => (TRUSTED_PATTERN, PREFACE_TRUSTED.as_slice()),
        };
        let params = pattern
            .parse()
            .map_err(|_| HandshakeError::InvalidConfiguration)?;
        let mut builder = snow::Builder::new(params)
            .local_private_key(&local.private[..])
            .and_then(|builder| builder.prologue(preface))
            .map_err(|_| HandshakeError::InvalidConfiguration)?;
        if initiator && mode == SessionMode::Trusted {
            builder = builder
                .remote_public_key(
                    remote
                        .as_ref()
                        .ok_or(HandshakeError::InvalidConfiguration)?,
                )
                .map_err(|_| HandshakeError::InvalidConfiguration)?;
        }
        let state = if initiator {
            builder.build_initiator()
        } else {
            builder.build_responder()
        }
        .map_err(|_| HandshakeError::InvalidConfiguration)?;

        Ok(Self {
            state,
            mode,
            expected_remote: remote,
            deadline: Instant::now() + HANDSHAKE_TIMEOUT,
        })
    }

    fn check_deadline(&self) -> Result<(), HandshakeError> {
        if Instant::now() >= self.deadline {
            Err(HandshakeError::TimedOut)
        } else {
            Ok(())
        }
    }

    /// Write this side's next handshake message.
    pub fn write_handshake(
        &mut self,
        payload: &[u8],
        message: &mut [u8],
    ) -> Result<usize, HandshakeError> {
        self.check_deadline()?;
        self.state
            .write_message(payload, message)
            .map_err(|_| HandshakeError::CryptographicFailure)
    }

    /// Authenticate and read the peer's next handshake message.
    pub fn read_handshake(
        &mut self,
        message: &[u8],
        payload: &mut [u8],
    ) -> Result<usize, HandshakeError> {
        self.check_deadline()?;
        self.state
            .read_message(message, payload)
            .map_err(|_| HandshakeError::CryptographicFailure)
    }

    /// Consume a complete authenticated handshake and enter transport mode.
    pub fn finish(self) -> Result<SecureTransport, HandshakeError> {
        self.check_deadline()?;
        if !self.state.is_handshake_finished() {
            return Err(HandshakeError::Incomplete);
        }

        let peer_static: [u8; 32] = self
            .state
            .get_remote_static()
            .ok_or(HandshakeError::CryptographicFailure)?
            .try_into()
            .map_err(|_| HandshakeError::CryptographicFailure)?;
        if self
            .expected_remote
            .is_some_and(|expected| expected != peer_static)
        {
            return Err(HandshakeError::CryptographicFailure);
        }
        let handshake_hash: [u8; 32] = self
            .state
            .get_handshake_hash()
            .try_into()
            .map_err(|_| HandshakeError::CryptographicFailure)?;
        let state = self
            .state
            .into_transport_mode()
            .map_err(|_| HandshakeError::CryptographicFailure)?;

        Ok(SecureTransport {
            state,
            peer_static,
            handshake_hash: Zeroizing::new(handshake_hash),
            mode: self.mode,
        })
    }
}

impl fmt::Debug for HandshakeSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HandshakeSession")
            .field("mode", &self.mode)
            .field("state", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

/// An authenticated, encrypted Noise transport session.
pub struct SecureTransport {
    state: snow::TransportState,
    peer_static: [u8; 32],
    handshake_hash: Zeroizing<[u8; 32]>,
    mode: SessionMode,
}

impl SecureTransport {
    /// Encrypt and authenticate one application payload.
    pub fn write_message(
        &mut self,
        payload: &[u8],
        message: &mut [u8],
    ) -> Result<usize, HandshakeError> {
        self.state
            .write_message(payload, message)
            .map_err(|_| HandshakeError::CryptographicFailure)
    }

    /// Authenticate and decrypt one application payload.
    pub fn read_message(
        &mut self,
        message: &[u8],
        payload: &mut [u8],
    ) -> Result<usize, HandshakeError> {
        self.state
            .read_message(message, payload)
            .map_err(|_| HandshakeError::CryptographicFailure)
    }

    /// Return the authenticated peer's static public key.
    #[must_use]
    pub const fn peer_static(&self) -> [u8; 32] {
        self.peer_static
    }

    /// Return the channel-binding hash for this handshake.
    #[must_use]
    pub fn handshake_hash(&self) -> &[u8; 32] {
        &self.handshake_hash
    }

    /// Return whether this was a pairing or trusted session.
    #[must_use]
    pub const fn mode(&self) -> SessionMode {
        self.mode
    }
}

impl fmt::Debug for SecureTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecureTransport")
            .field("mode", &self.mode)
            .field("state", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{HandshakeError, HandshakeSession, NoiseKeypair};
    use std::time::Instant;

    #[test]
    fn elapsed_deadline_fails_before_using_noise_state() {
        let keypair = NoiseKeypair::generate().expect("keypair");
        let mut session = HandshakeSession::pairing_initiator(&keypair).expect("session");
        session.deadline = Instant::now();
        let mut output = [0_u8; 128];

        assert_eq!(
            session.write_handshake(b"", &mut output),
            Err(HandshakeError::TimedOut)
        );
    }
}

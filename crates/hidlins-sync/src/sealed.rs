//! Generic versioned at-rest container for small application secrets.

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    ChaCha20Poly1305, Key, Nonce,
};
use hidlins_core::MasterPassword;
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroizing;

const MAGIC: &[u8; 4] = b"HS01";
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 12;
const TAG_LENGTH: usize = 16;
const KEY_LENGTH: usize = 32;
const ARGON2_MEMORY_KIB: u32 = 16_384;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_LANES: u32 = 1;

/// Domain separation for independently meaningful sealed values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecretDomain {
    /// Per-vault, per-installation static Noise identity.
    LocalSyncIdentity,
}

impl SecretDomain {
    const fn label(self) -> &'static [u8] {
        match self {
            Self::LocalSyncIdentity => b"hidlins/local-sync/identity/v1",
        }
    }
}

/// An authenticated, versioned, base64-encoded sealed secret.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SealedSecret(String);

impl SealedSecret {
    /// Seal bytes with a master password, domain, and canonical public context.
    pub fn seal(
        plaintext: &[u8],
        master_password: &MasterPassword,
        domain: SecretDomain,
        context: &[u8],
    ) -> Result<Self, SealedSecretError> {
        let mut salt = [0_u8; SALT_LENGTH];
        let mut nonce_bytes = [0_u8; NONCE_LENGTH];
        getrandom::fill(&mut salt).map_err(|_| SealedSecretError::EntropyUnavailable)?;
        getrandom::fill(&mut nonce_bytes).map_err(|_| SealedSecretError::EntropyUnavailable)?;

        let key = derive_key(master_password, &salt)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&*key));
        let aad = associated_data(domain, context)?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| SealedSecretError::EncryptionFailed)?;

        let mut container =
            Vec::with_capacity(MAGIC.len() + SALT_LENGTH + NONCE_LENGTH + ciphertext.len());
        container.extend_from_slice(MAGIC);
        container.extend_from_slice(&salt);
        container.extend_from_slice(&nonce_bytes);
        container.extend_from_slice(&ciphertext);
        Ok(Self(STANDARD.encode(container)))
    }

    /// Authenticate and decrypt into zeroizing owned bytes.
    pub fn open(
        &self,
        master_password: &MasterPassword,
        domain: SecretDomain,
        context: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, SealedSecretError> {
        let container = STANDARD
            .decode(self.0.as_bytes())
            .map_err(|_| SealedSecretError::Malformed)?;
        let header_length = MAGIC.len() + SALT_LENGTH + NONCE_LENGTH;
        if container.len() < header_length + TAG_LENGTH || &container[..MAGIC.len()] != MAGIC {
            return Err(SealedSecretError::Malformed);
        }

        let salt: &[u8; SALT_LENGTH] = container[MAGIC.len()..MAGIC.len() + SALT_LENGTH]
            .try_into()
            .map_err(|_| SealedSecretError::Malformed)?;
        let nonce: &[u8; NONCE_LENGTH] = container[MAGIC.len() + SALT_LENGTH..header_length]
            .try_into()
            .map_err(|_| SealedSecretError::Malformed)?;
        let key = derive_key(master_password, salt)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&*key));
        let aad = associated_data(domain, context)?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: &container[header_length..],
                    aad: &aad,
                },
            )
            .map_err(|_| SealedSecretError::AuthenticationFailed)?;
        Ok(Zeroizing::new(plaintext))
    }

    /// Re-seal the identical plaintext under a new master password.
    pub fn rewrap(
        &self,
        old_password: &MasterPassword,
        new_password: &MasterPassword,
        domain: SecretDomain,
        context: &[u8],
    ) -> Result<Self, SealedSecretError> {
        let plaintext = self.open(old_password, domain, context)?;
        Self::seal(&plaintext, new_password, domain, context)
    }

    /// Return the opaque encoded container for persistence.
    #[must_use]
    pub fn encoded(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SealedSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SealedSecret([REDACTED])")
    }
}

/// Secret-free sealed-container failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SealedSecretError {
    /// The operating system CSPRNG could not provide salt or nonce bytes.
    #[error("secure storage randomness is unavailable")]
    EntropyUnavailable,
    /// The fixed Argon2id configuration could not run.
    #[error("secure storage key derivation is unavailable")]
    KeyDerivationFailed,
    /// The fixed AEAD could not encrypt a valid input.
    #[error("secure storage encryption failed")]
    EncryptionFailed,
    /// The encoded container is truncated, corrupt, or an unknown version.
    #[error("sealed secret is malformed")]
    Malformed,
    /// Password, associated context, ciphertext, or tag authentication failed.
    #[error("sealed secret authentication failed")]
    AuthenticationFailed,
}

fn associated_data(domain: SecretDomain, context: &[u8]) -> Result<Vec<u8>, SealedSecretError> {
    let context_length = u32::try_from(context.len()).map_err(|_| SealedSecretError::Malformed)?;
    let mut aad = Vec::with_capacity(MAGIC.len() + domain.label().len() + 4 + context.len());
    aad.extend_from_slice(MAGIC);
    aad.extend_from_slice(domain.label());
    aad.extend_from_slice(&context_length.to_be_bytes());
    aad.extend_from_slice(context);
    Ok(aad)
}

fn derive_key(
    master_password: &MasterPassword,
    salt: &[u8; SALT_LENGTH],
) -> Result<Zeroizing<[u8; KEY_LENGTH]>, SealedSecretError> {
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_LANES,
        Some(KEY_LENGTH),
    )
    .map_err(|_| SealedSecretError::KeyDerivationFailed)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0_u8; KEY_LENGTH]);
    argon2
        .hash_password_into(master_password.as_bytes(), salt, &mut *key)
        .map_err(|_| SealedSecretError::KeyDerivationFailed)?;
    Ok(key)
}

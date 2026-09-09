//! Per-vault, per-installation static Noise identity management.

use std::fmt;

use hidlins_core::MasterPassword;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

use crate::{
    encoding::encode_lower,
    noise::NoiseKeypair,
    sealed::{SealedSecret, SealedSecretError, SecretDomain},
};

/// Whether this installation owns the authoritative vault or is a client.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncRole {
    /// Owns and conditionally commits the canonical KDBX file.
    Server,
    /// Pins one authoritative server and initiates synchronization.
    Client,
}

impl SyncRole {
    const fn wire_byte(self) -> u8 {
        match self {
            Self::Server => 1,
            Self::Client => 2,
        }
    }
}

/// Public X25519 static identity used as the trust-store key.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PublicIdentity([u8; 32]);

impl PublicIdentity {
    /// Construct from the exact X25519 public-key bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the exact X25519 public-key bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }

    /// Borrow the exact X25519 public-key bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for PublicIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PublicIdentity([REDACTED])")
    }
}

impl Serialize for PublicIdentity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&encode_lower(&self.0))
    }
}

impl<'de> Deserialize<'de> for PublicIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        decode_hex::<32>(&encoded)
            .map(Self)
            .map_err(|()| D::Error::custom("invalid public identity"))
    }
}

/// Random stable identifier for one installation's relationship to one vault.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstallationId([u8; 16]);

impl InstallationId {
    /// Generate a fresh identifier from the operating-system CSPRNG.
    pub fn generate() -> Result<Self, IdentityError> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| IdentityError::EntropyUnavailable)?;
        Ok(Self(bytes))
    }

    /// Return the exact identifier bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for InstallationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InstallationId([REDACTED])")
    }
}

impl Serialize for InstallationId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&encode_lower(&self.0))
    }
}

impl<'de> Deserialize<'de> for InstallationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        decode_hex::<16>(&encoded)
            .map(Self)
            .map_err(|()| D::Error::custom("invalid installation identity"))
    }
}

/// Persistable public identity plus its sealed private half.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SealedIdentity {
    installation_id: InstallationId,
    public_key: PublicIdentity,
    sealed_private_key: SealedSecret,
}

impl SealedIdentity {
    /// Generate and seal a distinct static identity.
    pub fn generate(
        vault_name: &str,
        role: SyncRole,
        master_password: &MasterPassword,
    ) -> Result<Self, IdentityError> {
        let keypair = NoiseKeypair::generate().map_err(|_| IdentityError::KeyUnavailable)?;
        let installation_id = InstallationId::generate()?;
        let public_key = PublicIdentity::new(keypair.public_key());
        let context = identity_context(vault_name, role, installation_id, public_key)?;
        let sealed_private_key = SealedSecret::seal(
            keypair.private_key(),
            master_password,
            SecretDomain::LocalSyncIdentity,
            &context,
        )?;
        Ok(Self {
            installation_id,
            public_key,
            sealed_private_key,
        })
    }

    /// Return the public identity.
    #[must_use]
    pub const fn public_key(&self) -> PublicIdentity {
        self.public_key
    }

    /// Return the per-vault/per-installation relationship identifier.
    #[must_use]
    pub const fn installation_id(&self) -> InstallationId {
        self.installation_id
    }

    /// Open the private key into zeroizing Noise-owned state.
    pub fn unlock(
        &self,
        vault_name: &str,
        role: SyncRole,
        master_password: &MasterPassword,
    ) -> Result<NoiseKeypair, IdentityError> {
        let context = identity_context(vault_name, role, self.installation_id, self.public_key)?;
        let plaintext = self.sealed_private_key.open(
            master_password,
            SecretDomain::LocalSyncIdentity,
            &context,
        )?;
        if plaintext.len() != 32 {
            return Err(IdentityError::Malformed);
        }
        let mut private = Zeroizing::new([0_u8; 32]);
        private.copy_from_slice(&plaintext);
        Ok(NoiseKeypair::from_persisted(
            private,
            self.public_key.into_bytes(),
        ))
    }

    /// Preserve the identity while changing the wrapping master password.
    pub fn rewrap(
        &mut self,
        vault_name: &str,
        role: SyncRole,
        old_password: &MasterPassword,
        new_password: &MasterPassword,
    ) -> Result<(), IdentityError> {
        let context = identity_context(vault_name, role, self.installation_id, self.public_key)?;
        let replacement = self.sealed_private_key.rewrap(
            old_password,
            new_password,
            SecretDomain::LocalSyncIdentity,
            &context,
        )?;
        self.sealed_private_key = replacement;
        Ok(())
    }

    /// Return the opaque sealed value for static persistence tests.
    #[must_use]
    pub fn sealed_value(&self) -> &str {
        self.sealed_private_key.encoded()
    }
}

impl fmt::Debug for SealedIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SealedIdentity([REDACTED])")
    }
}

/// Secret-free identity storage failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum IdentityError {
    /// The operating system CSPRNG is unavailable.
    #[error("identity randomness is unavailable")]
    EntropyUnavailable,
    /// The fixed Noise identity implementation could not generate a key.
    #[error("identity generation is unavailable")]
    KeyUnavailable,
    /// The stored identity is invalid.
    #[error("stored identity is malformed")]
    Malformed,
    /// Sealed identity authentication or storage failed.
    #[error("stored identity could not be authenticated")]
    Sealed,
}

impl From<SealedSecretError> for IdentityError {
    fn from(_: SealedSecretError) -> Self {
        Self::Sealed
    }
}

fn identity_context(
    vault_name: &str,
    role: SyncRole,
    installation_id: InstallationId,
    public_key: PublicIdentity,
) -> Result<Vec<u8>, IdentityError> {
    let name_length = u32::try_from(vault_name.len()).map_err(|_| IdentityError::Malformed)?;
    let mut context = Vec::with_capacity(1 + 16 + 32 + 4 + vault_name.len());
    context.push(role.wire_byte());
    context.extend_from_slice(installation_id.as_bytes());
    context.extend_from_slice(public_key.as_bytes());
    context.extend_from_slice(&name_length.to_be_bytes());
    context.extend_from_slice(vault_name.as_bytes());
    Ok(context)
}

pub(crate) fn decode_hex<const N: usize>(encoded: &str) -> Result<[u8; N], ()> {
    if encoded.len() != N * 2 || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(());
    }
    let mut output = [0_u8; N];
    for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(output)
}

const fn hex_nibble(byte: u8) -> Result<u8, ()> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(()),
    }
}

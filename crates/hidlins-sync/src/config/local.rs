//! V1-only local-network sync registry schema.

use std::fmt;

use hidlins_core::{MasterPassword, RegisteredVault, VaultError, VaultRegistry};
use serde::{Deserialize, Serialize};

use crate::{
    address::LocalEndpoint,
    identity::{IdentityError, SealedIdentity, SyncRole},
    protocol::RemoteVersion,
    trust::{PeerRecord, PeerStatus, ProvisionalPairing, TrustError},
};

const SYNC_KEY: &str = "sync";
const LOCAL_SCHEMA_VERSION: u8 = 1;

/// The only transport kind recognized by the local V1 schema.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalTransportKind {
    /// Fixed Noise-authenticated local-network synchronization.
    Local,
}

/// Secret-free status for presentation surfaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalSyncStatus {
    /// Configured authority/client role.
    pub role: SyncRole,
    /// Whether this client has an active server pin.
    pub paired: bool,
    /// Number of active server-side client authorizations.
    pub active_clients: usize,
}

/// V1 local-network configuration stored beneath one registered vault.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalSyncConfig {
    kind: LocalTransportKind,
    schema_version: u8,
    role: SyncRole,
    identity: SealedIdentity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    trusted_peers: Vec<PeerRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pinned_server: Option<PeerRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provisional: Option<ProvisionalPairing>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    routing_hint: Option<LocalEndpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_synced_remote_version: Option<RemoteVersion>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_synced_local_version: Option<RemoteVersion>,
    #[serde(default, flatten, skip_serializing_if = "toml::Table::is_empty")]
    extra: toml::Table,
}

impl LocalSyncConfig {
    /// Configure a registered vault for a fresh V1 local role atomically.
    pub fn configure(
        registry: &mut VaultRegistry,
        vault_name: &str,
        role: SyncRole,
        master_password: &MasterPassword,
    ) -> Result<Self, LocalConfigError> {
        if registry.get(vault_name).is_none() {
            return Err(VaultError::NotRegistered {
                name: vault_name.to_string(),
            }
            .into());
        }
        let config = Self::create(vault_name, role, master_password)?;
        config.persist(registry, vault_name)?;
        Ok(config)
    }

    /// Generate a distinct sealed identity and empty trust store.
    pub fn create(
        vault_name: &str,
        role: SyncRole,
        master_password: &MasterPassword,
    ) -> Result<Self, LocalConfigError> {
        Ok(Self {
            kind: LocalTransportKind::Local,
            schema_version: LOCAL_SCHEMA_VERSION,
            role,
            identity: SealedIdentity::generate(vault_name, role, master_password)?,
            trusted_peers: Vec::new(),
            pinned_server: None,
            provisional: None,
            routing_hint: None,
            last_synced_remote_version: None,
            last_synced_local_version: None,
            extra: toml::Table::new(),
        })
    }

    /// Parse only the exact local V1 schema; unknown kinds are absent.
    #[must_use]
    pub fn from_vault_entry(entry: &RegisteredVault) -> Option<Self> {
        Self::from_extra(&entry.extra)
    }

    fn from_extra(extra: &toml::Table) -> Option<Self> {
        let table = extra.get(SYNC_KEY)?.as_table()?;
        if table.get("kind")?.as_str()? != "local"
            || table.get("schema_version")?.as_integer()? != i64::from(LOCAL_SCHEMA_VERSION)
        {
            return None;
        }
        let config: Self = toml::Value::Table(table.clone()).try_into().ok()?;
        config.valid().then_some(config)
    }

    /// Persist with the registry's locked reload/update/atomic-save path.
    pub fn persist(
        &self,
        registry: &mut VaultRegistry,
        vault_name: &str,
    ) -> Result<(), LocalConfigError> {
        let value = toml::Value::try_from(self).map_err(|_| LocalConfigError::Malformed)?;
        registry.update_registered_extra(vault_name, |extra| {
            extra.insert(SYNC_KEY.to_string(), value);
        })?;
        Ok(())
    }

    /// Transactionally mutate the latest local config under the registry lock.
    pub fn transactional_update(
        registry: &mut VaultRegistry,
        vault_name: &str,
        update: impl FnOnce(&mut Self) -> Result<(), TrustError>,
    ) -> Result<(), LocalConfigError> {
        let mut update = Some(update);
        let mut semantic_result = Ok(());
        registry.update_registered_extra(vault_name, |extra| {
            let Some(mut config) = Self::from_extra(extra) else {
                semantic_result = Err(TrustError::NotPrepared);
                return;
            };
            let Some(operation) = update.take() else {
                semantic_result = Err(TrustError::TransactionMismatch);
                return;
            };
            if let Err(error) = operation(&mut config) {
                semantic_result = Err(error);
                return;
            }
            match toml::Value::try_from(&config) {
                Ok(value) => {
                    extra.insert(SYNC_KEY.to_string(), value);
                }
                Err(_) => semantic_result = Err(TrustError::TransactionMismatch),
            }
        })?;
        semantic_result?;
        Ok(())
    }

    /// Re-seal the same identity for a master-password change.
    pub fn rewrap_identity(
        &mut self,
        vault_name: &str,
        old_password: &MasterPassword,
        new_password: &MasterPassword,
    ) -> Result<(), LocalConfigError> {
        self.identity
            .rewrap(vault_name, self.role, old_password, new_password)?;
        Ok(())
    }

    /// Rewrap the latest registered identity and persist it atomically.
    pub fn rewrap_registered_identity(
        registry: &mut VaultRegistry,
        vault_name: &str,
        old_password: &MasterPassword,
        new_password: &MasterPassword,
    ) -> Result<(), LocalConfigError> {
        let mut identity_result = Ok(());
        registry.update_registered_extra(vault_name, |extra| {
            let Some(mut config) = Self::from_extra(extra) else {
                identity_result = Err(IdentityError::Malformed);
                return;
            };
            if let Err(error) =
                config
                    .identity
                    .rewrap(vault_name, config.role, old_password, new_password)
            {
                identity_result = Err(error);
                return;
            }
            match toml::Value::try_from(config) {
                Ok(value) => {
                    extra.insert(SYNC_KEY.to_string(), value);
                }
                Err(_) => identity_result = Err(IdentityError::Malformed),
            }
        })?;
        identity_result?;
        Ok(())
    }

    /// Return the configured authority/client role.
    #[must_use]
    pub const fn role(&self) -> SyncRole {
        self.role
    }

    /// Return a secret-free configuration and trust summary.
    #[must_use]
    pub fn status(&self) -> LocalSyncStatus {
        LocalSyncStatus {
            role: self.role,
            paired: self.is_active_client(),
            active_clients: self
                .trusted_peers
                .iter()
                .filter(|peer| peer.status() == PeerStatus::Active)
                .count(),
        }
    }

    /// Return the sealed local identity.
    #[must_use]
    pub const fn identity(&self) -> &SealedIdentity {
        &self.identity
    }

    /// Return all server-side peer records, active and revoked.
    #[must_use]
    pub fn trusted_peers(&self) -> &[PeerRecord] {
        &self.trusted_peers
    }

    /// Return the client-side pinned server, if paired.
    #[must_use]
    pub const fn pinned_server(&self) -> Option<&PeerRecord> {
        self.pinned_server.as_ref()
    }

    /// Return the non-authorizing provisional transaction, if any.
    #[must_use]
    pub const fn provisional(&self) -> Option<&ProvisionalPairing> {
        self.provisional.as_ref()
    }

    /// Return the untrusted, policy-validated last-known/manual route.
    #[must_use]
    pub const fn routing_hint(&self) -> Option<LocalEndpoint> {
        self.routing_hint
    }

    /// Replace the untrusted routing hint with a checked endpoint.
    pub fn set_routing_hint(&mut self, endpoint: Option<LocalEndpoint>) {
        self.routing_hint = endpoint;
    }

    /// Return the last authoritative remote version pointer.
    #[must_use]
    pub const fn last_synced_remote_version(&self) -> Option<RemoteVersion> {
        self.last_synced_remote_version
    }

    /// Return the last local encrypted-byte version pointer.
    #[must_use]
    pub const fn last_synced_local_version(&self) -> Option<RemoteVersion> {
        self.last_synced_local_version
    }

    /// Atomically replace both transport-neutral divergence pointers in memory.
    pub fn set_sync_versions(
        &mut self,
        remote: Option<RemoteVersion>,
        local: Option<RemoteVersion>,
    ) {
        self.last_synced_remote_version = remote;
        self.last_synced_local_version = local;
    }

    /// Return whether a client has completed bilateral trust activation.
    #[must_use]
    pub fn is_active_client(&self) -> bool {
        self.role == SyncRole::Client
            && self.provisional.is_none()
            && self
                .pinned_server
                .as_ref()
                .is_some_and(|peer| peer.status() == PeerStatus::Active)
    }

    /// Attach this validated local configuration to a new registration.
    pub fn write_to_entry(&self, entry: &mut RegisteredVault) -> Result<(), LocalConfigError> {
        if !self.valid() {
            return Err(LocalConfigError::Malformed);
        }
        let value = toml::Value::try_from(self).map_err(|_| LocalConfigError::Malformed)?;
        entry.extra.insert(SYNC_KEY.to_string(), value);
        Ok(())
    }

    pub(crate) fn trusted_peers_mut(&mut self) -> &mut Vec<PeerRecord> {
        &mut self.trusted_peers
    }

    pub(crate) fn pinned_server_mut(&mut self) -> Option<&mut PeerRecord> {
        self.pinned_server.as_mut()
    }

    pub(crate) fn set_pinned_server(&mut self, peer: Option<PeerRecord>) {
        self.pinned_server = peer;
    }

    pub(crate) fn set_provisional(&mut self, provisional: Option<ProvisionalPairing>) {
        self.provisional = provisional;
    }

    fn valid(&self) -> bool {
        if self.schema_version != LOCAL_SCHEMA_VERSION || self.kind != LocalTransportKind::Local {
            return false;
        }
        if self.trusted_peers.iter().any(|peer| !peer.is_valid())
            || self.trusted_peers.iter().enumerate().any(|(index, peer)| {
                self.trusted_peers[index + 1..]
                    .iter()
                    .any(|other| peer.public_key() == other.public_key())
            })
            || self
                .provisional
                .as_ref()
                .is_some_and(|pairing| !pairing.is_valid())
        {
            return false;
        }
        match self.role {
            SyncRole::Server => {
                self.pinned_server.is_none()
                    && self.provisional.as_ref().is_none_or(|pairing| {
                        self.trusted_peers
                            .iter()
                            .filter(|peer| peer.public_key() == pairing.peer_key())
                            .all(|peer| peer.status() == PeerStatus::Active)
                    })
            }
            SyncRole::Client => {
                self.trusted_peers.is_empty()
                    && self.pinned_server.as_ref().is_none_or(PeerRecord::is_valid)
                    && !(self.pinned_server.is_some() && self.provisional.is_some())
            }
        }
    }
}

impl fmt::Debug for LocalSyncConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalSyncConfig")
            .field("role", &self.role)
            .field("identity", &"[REDACTED]")
            .field("trusted_peer_count", &self.trusted_peers.len())
            .field(
                "pinned_server",
                &self.pinned_server.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "provisional",
                &self.provisional.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "routing_hint",
                &self.routing_hint.as_ref().map(|_| "[REDACTED]"),
            )
            .finish_non_exhaustive()
    }
}

/// Secret-free local registry/configuration failure.
#[derive(Debug, thiserror::Error)]
pub enum LocalConfigError {
    /// Identity generation, authentication, or rewrap failed.
    #[error("local sync identity operation failed")]
    Identity(#[from] IdentityError),
    /// Trust-state validation failed.
    #[error("local sync trust operation failed")]
    Trust(#[from] TrustError),
    /// Registry persistence failed atomically.
    #[error("local sync registry update failed")]
    Registry(#[from] VaultError),
    /// The local V1 schema could not be represented.
    #[error("local sync configuration is malformed")]
    Malformed,
}

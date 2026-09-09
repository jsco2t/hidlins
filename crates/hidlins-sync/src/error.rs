//! Secret-free errors surfaced by local-network synchronization.

use std::path::PathBuf;

use crate::client::LanError;
use crate::config::local::LocalConfigError;
use crate::merge::MergeError;

/// Errors surfaced by the sync layer.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SyncError {
    /// Local-network routing, authentication, framing, or protocol failed.
    #[error("local-network sync failed: {0}")]
    LocalNetwork(#[from] LanError),

    /// Local-network registry/configuration update failed.
    #[error("local sync configuration failed: {0}")]
    LocalConfig(#[from] LocalConfigError),

    /// The vault has no valid local-network sync configuration.
    #[error("local-network sync is not configured for this vault")]
    NotConfigured,

    /// A transport fixture or adapter could not reach its authority.
    #[error("sync authority unreachable: {endpoint} ({source})")]
    RemoteUnreachable {
        /// A secret-free authority description.
        endpoint: String,
        /// The underlying transport error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The conditional-commit retry loop exhausted its budget.
    #[error("conditional commit exhausted after {attempts} attempts; authority advanced concurrently; retry sync")]
    ConditionalCommitExhausted {
        /// Number of attempts consumed before exhaustion.
        attempts: usize,
    },

    /// Local and authoritative KDF parameters differ.
    #[error("master password or KDF parameters differ between local and authoritative vault")]
    MasterPasswordMismatch,

    /// A merge could not be auto-resolved.
    #[error("merge cannot proceed: {reason}; pre-merge state preserved at {backup_path}")]
    Unresolvable {
        /// Non-secret description of the conflict.
        reason: String,
        /// Path to the preserved recovery snapshot.
        backup_path: PathBuf,
    },

    /// Writing the pre-merge recovery snapshot failed.
    #[error("pre-merge backup creation failed: {source}")]
    BackupFailed {
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// The merge engine returned an error.
    #[error("merge engine error: {0}")]
    Merge(#[from] MergeError),

    /// A vault-layer error occurred.
    #[error("vault error: {0}")]
    Vault(#[from] hidlins_core::VaultError),

    /// Reading or writing the encrypted vault failed during sync.
    #[error("vault I/O during sync: {path:?}: {source}")]
    VaultIo {
        /// The file path involved.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

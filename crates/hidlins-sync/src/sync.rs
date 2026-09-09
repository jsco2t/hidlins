//! Transport-neutral synchronization state machine.
//!
//! The local-network client supplies an authenticated, pinned transport. This
//! module owns only divergence detection, no-data-loss merge behavior, backup
//! creation, bounded compare-and-swap retries, and activity notifications.

use std::time::Duration;

use hidlins_core::{Keyfile, MasterPassword, Vault};
use sha2::{Digest, Sha256};

use crate::backup;
use crate::error::SyncError;
use crate::merge::{self, EntryDelta};
use crate::transport::{IsPreconditionFailed, SyncSnapshot, SyncTransport, SyncVersion};

/// Default conditional-commit attempt budget when [`SyncOptions::max_commit_attempts`]
/// is unset.
pub const DEFAULT_MAX_COMMIT_ATTEMPTS: usize = 2;

// ===========================================================================
// SyncOptions
// ===========================================================================

/// Caller-supplied options for a sync cycle.
///
/// Defaults: two total conditional-commit attempts (one retry); no activity pinger.
pub struct SyncOptions {
    /// conditional commit retry budget. The orchestrator's `Merged` path
    /// re-fetches and re-merges on each precondition failure up to this
    /// many times; on exhaustion → `SyncError::ConditionalCommitExhausted`.
    pub max_commit_attempts: usize,

    /// Activity pinger called at every network boundary (version check, fetch, commit)
    /// and after the merge engine returns. The TUI uses this callback to keep
    /// its auto-lock controller active during long syncs. `None` is appropriate
    /// for headless and one-shot CLI contexts.
    pub on_activity: Option<Box<dyn FnMut()>>,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            max_commit_attempts: DEFAULT_MAX_COMMIT_ATTEMPTS,
            on_activity: None,
        }
    }
}

impl std::fmt::Debug for SyncOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncOptions")
            .field("max_commit_attempts", &self.max_commit_attempts)
            .field("on_activity", &self.on_activity.is_some())
            .finish()
    }
}

// ===========================================================================
// SyncPointers
// ===========================================================================

/// The two transport-neutral divergence pointers the orchestrator maintains.
///
/// Returned by [`run_state_machine`] so the caller can write them back
/// to the registry atomically — the state machine itself never touches
/// the registry, keeping it pure-function-shaped for tests.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncPointers {
    /// The authoritative version of the just-synced encrypted vault.
    pub remote_version: Option<String>,
    /// Hex-encoded SHA-256 of the just-synced local bytes (or, on
    /// `AlreadyInSync`, the unchanged prior pointer).
    pub local_sha256: Option<String>,
}

// ===========================================================================
// SyncOutcome
// ===========================================================================

/// The outcome of a successful a sync cycle.
///
/// `#[non_exhaustive]` so future Phase-1 outcomes (e.g. a dry-run variant)
/// don't break existing exhaustive matches.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SyncOutcome {
    /// Both `(remote, local)` pointers matched the current state; version check
    /// was the only network call.
    AlreadyInSync,

    /// Local diverged from the last-synced state; remote did not. A
    /// (possibly conditional) commit uploaded the new local bytes.
    Pushed {
        /// `true` for the first-ever sync against an empty target —
        /// the unconditional first-seed commit path. `false` for
        /// steady-state pushes.
        is_first_seed: bool,
    },

    /// Remote advanced; local did not. The remote bytes replaced the
    /// local in-memory database and were saved to disk; the merge
    /// engine was NOT invoked. `.kdbx.bak` was still written as a
    /// belt-and-suspenders safeguard.
    FastReplaced,

    /// Both sides diverged. A merge folded the remote into the local
    /// database, the result was saved, and a conditional commit uploaded
    /// the merged state. `attempts` reports how many conditional commit
    /// attempts were needed (1 = no contention; ≥2 = at least one
    /// `PreconditionFailed` triggered a re-fetch + re-merge).
    Merged {
        /// How the local database changed.
        delta: EntryDelta,
        /// conditional commit attempts consumed (≥1; bounded by
        /// `opts.max_commit_attempts`).
        attempts: usize,
    },
}

/// Run the four-state truth table over `(remote_changed?, local_changed?)`.
///
/// Returns `(outcome, new_pointers)` on success: the caller writes
/// `new_pointers` into the local registry configuration and persists to the registry
/// atomically. The state machine itself never touches the registry,
/// keeping it pure-function-shaped for tests.
///
/// Generic over the transport: production passes the local authenticated transport;
/// tests pass a `MemoryTransport`. The
/// orchestrator never touches network or authentication internals directly;
/// the transport hides those details behind the three trait methods.
#[allow(clippy::too_many_arguments)] // The state machine genuinely needs every input.
#[allow(clippy::too_many_lines)] // The four-state truth table reads best as one cohesive function.
pub fn run_state_machine<T: SyncTransport>(
    vault: &mut Vault,
    master_password: &MasterPassword,
    keyfile: Option<&Keyfile>,
    transport: &mut T,
    last_synced_remote_version: Option<String>,
    last_synced_local_sha256: Option<&str>,
    mut opts: SyncOptions,
) -> Result<(SyncOutcome, SyncPointers), SyncError>
where
    T::Error: Into<SyncError>,
{
    let prev_remote = last_synced_remote_version.clone().map(SyncVersion);
    let prev_sha = last_synced_local_sha256.map(str::to_string);

    // 1. Check the authority's current version.
    let remote_now = transport
        .head()
        .map_err(<T::Error as Into<SyncError>>::into)?;
    ping(&mut opts);

    // 2. Read the local vault file ONCE and hash it. Reading twice
    // (once for the SHA, once for the eventual commit body) would open a
    // window where an external process (advisory locks are advisory)
    // edits the file between reads and we commit one byte stream while
    // recording the SHA of another. Holding the bytes in memory is
    // cheap — KDBX vaults are typically <5 MiB.
    let local_bytes = read_vault_bytes(vault.path())?;
    let local_sha = sha256_of(&local_bytes);

    // 3. Compare against the last-synced pointers
    let remote_changed = remote_now.as_ref() != prev_remote.as_ref();
    let local_changed = Some(local_sha.as_str()) != last_synced_local_sha256;

    match (remote_changed, local_changed) {
        // -----------------------------------------------------------------
        // (false, false) — nothing to do; pointers unchanged.
        // -----------------------------------------------------------------
        (false, false) => Ok((
            SyncOutcome::AlreadyInSync,
            SyncPointers {
                remote_version: last_synced_remote_version,
                local_sha256: prev_sha,
            },
        )),

        // -----------------------------------------------------------------
        // (false, true) — push only
        // -----------------------------------------------------------------
        (false, true) => {
            let new_version = transport
                .commit_conditional(&local_bytes, prev_remote.as_ref())
                .map_err(<T::Error as Into<SyncError>>::into)?;
            ping(&mut opts);
            let is_first_seed = prev_remote.is_none();
            Ok((
                SyncOutcome::Pushed { is_first_seed },
                SyncPointers {
                    remote_version: Some(new_version.0),
                    local_sha256: Some(local_sha),
                },
            ))
        }

        // -----------------------------------------------------------------
        // (true, false) — fast-replace
        // -----------------------------------------------------------------
        (true, false) => {
            let snapshot = transport
                .fetch_if_changed(prev_remote.as_ref())
                .map_err(<T::Error as Into<SyncError>>::into)?;
            ping(&mut opts);
            let Some(SyncSnapshot { version, bytes }) = snapshot else {
                // The transport reported no change despite our version check
                // having said the version moved — typically a race
                // where the remote rolled back between version check and fetch.
                // Treat as `AlreadyInSync` per impl plan §4.7 pseudocode.
                return Ok((
                    SyncOutcome::AlreadyInSync,
                    SyncPointers {
                        remote_version: last_synced_remote_version,
                        local_sha256: prev_sha,
                    },
                ));
            };

            // Decrypt + KDF-mismatch sanity check
            let remote_db = Vault::open_from_bytes(&bytes, master_password, keyfile)?;
            if vault.database().config.kdf_config != remote_db.config.kdf_config {
                return Err(SyncError::MasterPasswordMismatch);
            }

            // .kdbx.bak even on fast-replace (TC-SYNC-003 acceptance)
            backup::snapshot_pre_merge(vault.path())?;
            ping(&mut opts);

            vault.replace_database(remote_db)?;
            vault.save()?;
            // Re-hash the on-disk bytes — Vault::save re-encrypts with a
            // fresh AES IV + KDF seed, so the saved bytes have a SHA
            // distinct from the snapshot's `bytes`.
            let new_local_sha = sha256_of(&read_vault_bytes(vault.path())?);
            Ok((
                SyncOutcome::FastReplaced,
                SyncPointers {
                    remote_version: Some(version.0),
                    local_sha256: Some(new_local_sha),
                },
            ))
        }

        // -----------------------------------------------------------------
        // (true, true) — diverged → merge → conditional commit → maybe retry
        // -----------------------------------------------------------------
        (true, true) => {
            // .kdbx.bak BEFORE any merge work. Keep the returned path: the
            // orchestrator is the only place that has it, and an
            // unresolvable merge must carry it to the caller.
            let backup_path = backup::snapshot_pre_merge(vault.path())?;

            let mut attempts: usize = 0;
            let mut total_delta: EntryDelta;
            let max = opts.max_commit_attempts.max(1);
            loop {
                attempts += 1;

                // (Re-)fetch the remote unconditionally — we need the
                // bytes AND the version every iteration.
                let snapshot = transport
                    .fetch_if_changed(None)
                    .map_err(<T::Error as Into<SyncError>>::into)?
                    .ok_or_else(|| SyncError::RemoteUnreachable {
                        endpoint: "<transport>".to_string(),
                        source: "transport returned no snapshot during merge".into(),
                    })?;
                ping(&mut opts);

                let remote_db = Vault::open_from_bytes(&snapshot.bytes, master_password, keyfile)?;
                if vault.database().config.kdf_config != remote_db.config.kdf_config {
                    return Err(SyncError::MasterPasswordMismatch);
                }

                // Merge in place. An unresolvable merge (same-second
                // divergence with differing content) must surface as the
                // user-facing `SyncError::Unresolvable` carrying the
                // pre-merge backup path — NOT the generic
                // `SyncError::Merge(_)` the `?`-`#[from]` would produce.
                // The CLI maps `Unresolvable` to exit 3 and the TUI renders
                // it prominently with the `.kdbx.bak` pointer; both are dead
                // paths if this stays `Merge(_)`.
                // `MergeError` is `#[non_exhaustive]` but currently has the
                // single `Unresolvable` variant, so this intra-crate match is
                // exhaustive without a wildcard; a future variant would fail
                // to compile here, forcing an explicit exit-code decision
                // rather than silently defaulting to internal.
                let summary = match merge::reconcile(vault.database_mut(), &remote_db) {
                    Ok(summary) => summary,
                    Err(merge::MergeError::Unresolvable { reason }) => {
                        return Err(SyncError::Unresolvable {
                            reason,
                            backup_path: backup_path.clone(),
                        });
                    }
                };
                total_delta = summary.delta;
                ping(&mut opts);

                vault.save()?;

                let merged_bytes = read_vault_bytes(vault.path())?;
                match transport.commit_conditional(&merged_bytes, Some(&snapshot.version)) {
                    Ok(new_version) => {
                        ping(&mut opts);
                        let merged_sha = sha256_of(&merged_bytes);
                        return Ok((
                            SyncOutcome::Merged {
                                delta: total_delta,
                                attempts,
                            },
                            SyncPointers {
                                remote_version: Some(new_version.0),
                                local_sha256: Some(merged_sha),
                            },
                        ));
                    }
                    Err(e) => {
                        if !e.is_precondition_failed() {
                            return Err(e.into());
                        }
                        if attempts >= max {
                            return Err(SyncError::ConditionalCommitExhausted { attempts: max });
                        }
                        // else: precondition failed → loop and refetch.
                    }
                }
            }
        }
    }
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Read the on-disk vault bytes. Distinct error variant
/// ([`SyncError::VaultIo`]) so the caller can distinguish "I/O during
/// sync" from "vault-layer logical error" without parsing strings.
fn read_vault_bytes(path: &std::path::Path) -> Result<Vec<u8>, SyncError> {
    std::fs::read(path).map_err(|source| SyncError::VaultIo {
        path: path.to_path_buf(),
        source,
    })
}

/// Hex-encoded SHA-256 of `bytes`. Used for the `last_synced_local_sha256`
/// pointer.
fn sha256_of(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    crate::encoding::encode_lower(&digest)
}

/// Call the activity pinger if one is registered.
fn ping(opts: &mut SyncOptions) {
    if let Some(p) = opts.on_activity.as_mut() {
        p();
    }
}

/// Format a one-line summary of an outcome for human-readable stderr.
#[must_use]
pub fn format_outcome(outcome: &SyncOutcome, duration: Duration) -> String {
    crate::sync_log::format(outcome, duration)
}

#[cfg(any(test, feature = "test-helpers"))]
impl From<crate::transport::memory::MemoryTransportError> for SyncError {
    fn from(error: crate::transport::memory::MemoryTransportError) -> Self {
        use crate::transport::memory::MemoryTransportError;
        match error {
            MemoryTransportError::PreconditionFailed => {
                SyncError::LocalNetwork(crate::client::LanError::StaleVersion)
            }
            MemoryTransportError::NotFound => SyncError::RemoteUnreachable {
                endpoint: "memory authority".to_string(),
                source: "memory transport has no authoritative snapshot".into(),
            },
        }
    }
}

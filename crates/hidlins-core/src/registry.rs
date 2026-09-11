//! Vault registry — load and save `vaults.toml`.
//!
//! Per design §2.2.8 and §2.3.1, the registry tracks the user's named
//! KDBX vaults. The on-disk schema is TOML at
//! `$HOME/.local/state/hidlins/vaults.toml`:
//!
//! ```toml
//! version = 1
//!
//! [[vault]]
//! name = "personal"
//! path = "/Users/jason/.local/state/hidlins/personal.kdbx"
//! created_at = "2026-05-16T22:00:00-06:00"
//! # keyfile_path = "..."     # optional
//! ```
//!
//! ## Forward compatibility
//!
//! Shipped sync and per-vault auto-lock settings use nested data attached to
//! each `[[vault]]` entry. The registry preserves those and any unknown TOML
//! keys verbatim via
//! `#[serde(flatten)] extra: toml::Table` at both the top level and the
//! per-vault level. This is the same forward-compat principle KDBX uses
//! internally — never drop data we don't understand.
//!
//! ## Durability
//!
//! [`VaultRegistry::save`] routes through [`crate::atomic::write_atomic`],
//! inheriting the same crash-durability guarantees as KDBX vault saves
//! (FR-054 / NFR-006). The file is created with POSIX mode `0600`.
//!
//! ## Concurrency
//!
//! Writes are serialized with a sibling advisory lock. Field-level updates use
//! [`VaultRegistry::update_registered_extra`] to reload under that lock before
//! applying the mutation, preventing stale snapshots from losing another
//! process's completed registry edit.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::atomic::{install_staged_file, remove_file_durable, write_atomic};
use crate::error::VaultError;
use crate::locking::acquire_exclusive;
use crate::paths::HidlinsPaths;
use crate::vault::Vault;
use crate::MasterPassword;

/// The only schema version this build understands. Loading a registry
/// with a different `version` returns [`VaultError::RegistryMalformed`].
pub const SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single registered vault.
///
/// Fields beyond `name`, `path`, `created_at`, and `keyfile_path` are
/// captured in `extra` and round-tripped verbatim on save. This includes the
/// shipped sync and auto-lock extensions and preserves future unknown fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredVault {
    /// Unique human-readable name for the vault.
    pub name: String,

    /// Absolute path to the `.kdbx` file on disk.
    pub path: PathBuf,

    /// RFC 3339 timestamp string recording when this vault was
    /// registered. Stored as `String` rather than a typed timestamp so
    /// the registry stays human-editable and round-trips unknown
    /// timezone offsets.
    pub created_at: String,

    /// Optional path to a keyfile required to open this vault.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyfile_path: Option<PathBuf>,

    /// Unknown TOML keys belonging to this vault entry, preserved
    /// verbatim across load/save. Future features (sync, auto-lock)
    /// write into and read out of this map.
    #[serde(default, flatten)]
    pub extra: toml::Table,
}

/// In-memory representation of `vaults.toml`.
///
/// Construct via [`Self::load`] (production) or [`Self::with_paths`]
/// (tests). Mutate via [`Self::register`]. Persist via [`Self::save`].
///
/// The registry intentionally does not enforce that the underlying
/// `.kdbx` files exist on disk — `Vault::open` is where missing-file
/// errors surface (design §2.2.8, decision #9).
#[derive(Debug)]
pub struct VaultRegistry {
    paths: HidlinsPaths,
    /// Schema version of this in-memory registry. Always
    /// [`SCHEMA_VERSION`] on save; this field exists so that load can
    /// verify the on-disk file's version.
    version: u32,
    vaults: Vec<RegisteredVault>,
    /// Unknown top-level TOML keys preserved verbatim across save.
    extra_top_level: toml::Table,
    /// Exact bytes read (or `None` for a missing registry), used for optimistic
    /// conflict detection under the write lock.
    baseline: Option<String>,
}

// ---------------------------------------------------------------------------
// Internal serde shape
//
// Kept private so the public API can evolve independently of the TOML
// surface. Note the `#[serde(rename = "vault")]` on the array — the
// TOML uses `[[vault]]` (singular), which is the conventional spelling
// for arrays of tables.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct OnDisk {
    version: u32,

    #[serde(default, rename = "vault")]
    vaults: Vec<RegisteredVault>,

    #[serde(default, flatten)]
    extra: toml::Table,
}

const ROTATION_VERSION: u32 = 1;
const ROTATION_PREFIX: &str = ".hidlins-password-rotation-";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RotationPhase {
    Prepared,
    VaultCommitStarted,
    VaultCommitted,
    RegistryCommitted,
}

#[derive(Debug, Serialize, Deserialize)]
struct RotationMarker {
    version: u32,
    vault_name: String,
    extra_key: String,
    phase: RotationPhase,
    old_vault_sha256: String,
    new_vault_sha256: String,
    old_registry_sha256: String,
    new_registry_sha256: String,
    old_extra_sha256: String,
    new_extra_sha256: String,
}

struct RotationArtifacts {
    marker: PathBuf,
    registry_stage: PathBuf,
    vault_stage: PathBuf,
}

// ---------------------------------------------------------------------------
// VaultRegistry — public API
// ---------------------------------------------------------------------------

impl VaultRegistry {
    /// Load the registry from `paths.vaults_toml()`.
    ///
    /// Returns an **empty** registry (no error) when the file does not
    /// yet exist — first-run UX requires that creating the very first
    /// vault works without a pre-existing `vaults.toml`.
    ///
    /// # Errors
    ///
    /// - [`VaultError::Io`] for filesystem failures other than
    ///   "file not found".
    /// - [`VaultError::RegistryMalformed`] when the file is not valid
    ///   TOML or its `version` is not [`SCHEMA_VERSION`].
    pub fn load(paths: HidlinsPaths) -> Result<Self, VaultError> {
        let file = paths.vaults_toml();
        let contents = match std::fs::read_to_string(&file) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::with_paths(paths));
            }
            Err(source) => {
                return Err(VaultError::Io { source, path: file });
            }
        };

        let on_disk: OnDisk =
            toml::from_str(&contents).map_err(|source| VaultError::RegistryMalformed {
                source: Some(source),
            })?;

        if on_disk.version != SCHEMA_VERSION {
            // A registry written by a newer version of Hidlins is
            // surfaced as malformed rather than parsed loosely. Refusing
            // to read a newer file protects users who downgrade from
            // having their richer registry silently truncated on
            // re-save.
            return Err(VaultError::RegistryMalformed { source: None });
        }

        Ok(Self {
            paths,
            version: on_disk.version,
            vaults: on_disk.vaults,
            extra_top_level: on_disk.extra,
            baseline: Some(contents),
        })
    }

    /// Construct an empty registry rooted at `paths`. Used internally
    /// by [`Self::load`] when `vaults.toml` is missing, and by tests.
    pub fn with_paths(paths: HidlinsPaths) -> Self {
        Self {
            paths,
            version: SCHEMA_VERSION,
            vaults: Vec::new(),
            extra_top_level: toml::Table::new(),
            baseline: None,
        }
    }

    /// Serialize the registry to `paths.vaults_toml()` via the atomic
    /// write helper. Creates the state directory if needed.
    ///
    /// # Errors
    ///
    /// - [`VaultError::Io`] for filesystem failures (parent-dir
    ///   creation, temp-file write, rename, parent-dir fsync).
    /// - [`VaultError::RegistrySerializationFailed`] if the in-memory
    ///   registry cannot be expressed as TOML. Nearly impossible in
    ///   practice — values that came from a valid load always
    ///   re-serialize, and the registry's own typed fields are
    ///   trivially serializable.
    /// - [`VaultError::Contended`] if another process currently holds the
    ///   registry write lock.
    /// - [`VaultError::RegistryChanged`] if the file changed since this
    ///   snapshot was loaded; reload, reapply the edit, and retry.
    pub fn save(&mut self) -> Result<(), VaultError> {
        self.paths.ensure_exists()?;
        let _guard = crate::locking::acquire_exclusive(&self.paths.vaults_toml())?;
        let current = match std::fs::read_to_string(self.paths.vaults_toml()) {
            Ok(contents) => Some(contents),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(VaultError::Io {
                    source,
                    path: self.paths.vaults_toml(),
                })
            }
        };
        if current != self.baseline {
            return Err(VaultError::RegistryChanged);
        }
        self.save_unlocked()
    }

    fn save_unlocked(&mut self) -> Result<(), VaultError> {
        let on_disk = OnDisk {
            version: SCHEMA_VERSION,
            vaults: self.vaults.clone(),
            extra: self.extra_top_level.clone(),
        };

        let body = toml::to_string(&on_disk)
            .map_err(|source| VaultError::RegistrySerializationFailed { source })?;

        write_atomic(&self.paths.vaults_toml(), body.as_bytes())?;
        self.baseline = Some(body);
        Ok(())
    }

    /// Transactionally update one registered vault. The latest registry is
    /// reloaded while holding the write lock, so unrelated concurrent changes
    /// survive. `self` is replaced only after the atomic save succeeds.
    pub fn update_registered_extra(
        &mut self,
        name: &str,
        update: impl FnOnce(&mut toml::Table),
    ) -> Result<(), VaultError> {
        self.paths.ensure_exists()?;
        let path = self.paths.vaults_toml();
        let _guard = crate::locking::acquire_exclusive(&path)?;
        let mut latest = Self::load(self.paths.clone())?;
        let vault = latest
            .vaults
            .iter_mut()
            .find(|vault| vault.name == name)
            .ok_or_else(|| VaultError::NotRegistered {
                name: name.to_string(),
            })?;
        update(&mut vault.extra);
        latest.save_unlocked()?;
        *self = latest;
        Ok(())
    }

    /// Change a vault master password and one password-sealed registry value
    /// as one crash-recoverable transaction.
    ///
    /// The caller must already hold the vault's exclusive lock through
    /// `vault`. This method then acquires the registry lock, establishing the
    /// stable lock order used by restart recovery: vault first, registry
    /// second. `expected_old` prevents a stale caller from overwriting a
    /// concurrent change to the same registry value; unrelated registry edits
    /// are preserved by reloading under the lock.
    ///
    /// # Errors
    ///
    /// Returns an authentication error for a wrong current password, a
    /// concurrency error for a changed registry value, or a durable I/O /
    /// recovery error. Once the marker is durable, callers must fail locked on
    /// error and let [`Self::recover_password_rotations`] finish the commit.
    #[allow(clippy::too_many_arguments)]
    pub fn change_master_password_and_extra(
        &mut self,
        vault: &mut Vault,
        vault_name: &str,
        current: &MasterPassword,
        new: &MasterPassword,
        extra_key: &str,
        expected_old: &toml::Value,
        replacement: &toml::Value,
    ) -> Result<(), VaultError> {
        validate_rotation_name(vault_name)?;
        validate_extra_key(extra_key)?;
        self.paths.ensure_exists()?;

        // A prior interrupted transaction must be resolved before a new one
        // can reuse its fixed artifact names.
        if rotation_artifacts(&self.paths, vault.path(), vault_name)
            .marker
            .exists()
        {
            return Err(VaultError::PasswordRotationRecoveryFailed {
                reason: "a prior password rotation requires restart recovery",
            });
        }

        // `vault` owns the first lock. Acquiring the registry second is the
        // documented global order for this two-file operation.
        let registry_path = self.paths.vaults_toml();
        let _registry_lock = acquire_exclusive(&registry_path)?;
        let mut latest = Self::load(self.paths.clone())?;
        let target = latest
            .vaults
            .iter_mut()
            .find(|entry| entry.name == vault_name)
            .ok_or_else(|| VaultError::NotRegistered {
                name: vault_name.to_string(),
            })?;
        if target.path != vault.path() {
            return Err(VaultError::RegistryChanged);
        }
        if target.extra.get(extra_key) != Some(expected_old) {
            return Err(VaultError::RegistryChanged);
        }

        let old_registry =
            latest
                .baseline
                .clone()
                .ok_or(VaultError::PasswordRotationRecoveryFailed {
                    reason: "registered vault has no durable registry",
                })?;
        target
            .extra
            .insert(extra_key.to_string(), replacement.clone());
        let new_registry = latest.serialize()?;
        let artifacts = rotation_artifacts(&self.paths, vault.path(), vault_name);

        // No arbitrary path enters this protocol: every artifact is derived
        // from the configured registry target, registered vault path, and a
        // hash of the validated vault name.
        remove_file_durable(&artifacts.vault_stage)?;
        remove_file_durable(&artifacts.registry_stage)?;

        let staged_vault =
            vault.stage_master_password_change(current, new, artifacts.vault_stage.clone())?;
        if let Err(error) = write_atomic(&artifacts.registry_stage, new_registry.as_bytes()) {
            let _ = remove_file_durable(&artifacts.vault_stage);
            return Err(error);
        }
        maybe_signal_rotation_phase("stages_durable")?;

        let mut marker = RotationMarker {
            version: ROTATION_VERSION,
            vault_name: vault_name.to_string(),
            extra_key: extra_key.to_string(),
            phase: RotationPhase::Prepared,
            old_vault_sha256: hash_file(vault.path())?,
            new_vault_sha256: hash_file(&artifacts.vault_stage)?,
            old_registry_sha256: hash_bytes(old_registry.as_bytes()),
            new_registry_sha256: hash_bytes(new_registry.as_bytes()),
            old_extra_sha256: hash_toml_value(expected_old)?,
            new_extra_sha256: hash_toml_value(replacement)?,
        };
        if let Err(error) = write_rotation_marker(&artifacts.marker, &marker) {
            let _ = remove_file_durable(&artifacts.vault_stage);
            let _ = remove_file_durable(&artifacts.registry_stage);
            return Err(error);
        }
        maybe_signal_rotation_phase("prepared")?;

        marker.phase = RotationPhase::VaultCommitStarted;
        write_rotation_marker(&artifacts.marker, &marker)?;
        maybe_signal_rotation_phase("vault_commit_started")?;
        install_staged_file(&staged_vault.path, vault.path())?;
        vault.finish_staged_password_change(staged_vault);
        maybe_signal_rotation_phase("vault_renamed")?;

        marker.phase = RotationPhase::VaultCommitted;
        write_rotation_marker(&artifacts.marker, &marker)?;
        maybe_signal_rotation_phase("vault_committed")?;
        install_staged_file(&artifacts.registry_stage, &registry_path)?;
        latest.baseline = Some(new_registry);
        *self = latest;
        maybe_signal_rotation_phase("registry_renamed")?;

        marker.phase = RotationPhase::RegistryCommitted;
        write_rotation_marker(&artifacts.marker, &marker)?;
        maybe_signal_rotation_phase("registry_committed")?;
        remove_file_durable(&artifacts.marker)?;
        Ok(())
    }

    /// Recover every interrupted password/registry rotation belonging to this
    /// registry. Recovery uses only encrypted staged files, fixed paths, and
    /// hashes from the non-secret marker; it never needs either password.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed recovery error when a marker or required stage is
    /// missing, malformed, or inconsistent with the live files.
    pub fn recover_password_rotations(paths: &HidlinsPaths) -> Result<(), VaultError> {
        paths.ensure_exists()?;
        let registry_path = paths.vaults_toml();
        let registry_parent = parent_dir(&registry_path);
        let prefix = rotation_registry_prefix(&registry_path);
        let mut markers = Vec::new();
        let mut registry_stages = Vec::new();
        for entry in std::fs::read_dir(registry_parent).map_err(|source| VaultError::Io {
            source,
            path: registry_parent.to_path_buf(),
        })? {
            let entry = entry.map_err(|source| VaultError::Io {
                source,
                path: registry_parent.to_path_buf(),
            })?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with(&prefix) {
                continue;
            }
            if name.ends_with(".txn") {
                markers.push(entry.path());
            } else if name.ends_with(".registry.stage") {
                registry_stages.push(entry.path());
            }
        }
        markers.sort();

        // A crash before the marker became durable leaves the old live pair
        // untouched. Fixed stage names can therefore be removed safely after
        // matching them against the current registry; no path is read from an
        // untrusted artifact.
        if !registry_stages.is_empty() {
            let registry = Self::load(paths.clone())?;
            for stage in registry_stages {
                let mut matched = false;
                for entry in &registry.vaults {
                    let artifacts = rotation_artifacts(paths, &entry.path, &entry.name);
                    if artifacts.registry_stage == stage {
                        if !artifacts.marker.exists() {
                            remove_file_durable(&artifacts.registry_stage)?;
                            remove_file_durable(&artifacts.vault_stage)?;
                        }
                        matched = true;
                        break;
                    }
                }
                if !matched && stage.exists() {
                    return Err(VaultError::PasswordRotationRecoveryFailed {
                        reason: "unrecognized password rotation stage",
                    });
                }
            }
        }

        for marker_path in markers {
            recover_one_rotation(paths, &marker_path)?;
        }
        Ok(())
    }

    fn serialize(&self) -> Result<String, VaultError> {
        let on_disk = OnDisk {
            version: SCHEMA_VERSION,
            vaults: self.vaults.clone(),
            extra: self.extra_top_level.clone(),
        };
        toml::to_string(&on_disk)
            .map_err(|source| VaultError::RegistrySerializationFailed { source })
    }

    /// Transactionally register and persist a vault.
    ///
    /// The latest on-disk registry is reloaded while holding the write lock so
    /// a stale caller cannot overwrite registrations completed by another
    /// process. `self` is replaced only after the atomic save succeeds.
    ///
    /// # Errors
    ///
    /// - [`VaultError::AlreadyRegistered`] if the latest registry already has
    ///   a vault with the same name.
    /// - [`VaultError::Contended`] if another process holds the registry lock.
    /// - The load and save errors documented by [`Self::load`] and
    ///   [`Self::save`].
    pub fn register_and_save(&mut self, vault: RegisteredVault) -> Result<(), VaultError> {
        self.paths.ensure_exists()?;
        let path = self.paths.vaults_toml();
        let _guard = crate::locking::acquire_exclusive(&path)?;
        let mut latest = Self::load(self.paths.clone())?;
        latest.register(vault)?;
        latest.save_unlocked()?;
        *self = latest;
        Ok(())
    }

    /// Add a vault to the registry.
    ///
    /// # Errors
    ///
    /// - [`VaultError::AlreadyRegistered`] if a vault with the same
    ///   name is already present.
    pub fn register(&mut self, vault: RegisteredVault) -> Result<(), VaultError> {
        if self.vaults.iter().any(|v| v.name == vault.name) {
            return Err(VaultError::AlreadyRegistered { name: vault.name });
        }
        self.vaults.push(vault);
        Ok(())
    }

    /// Remove a vault registration by name.
    ///
    /// When `delete_file` is `false`, only the registry entry is
    /// removed and the `.kdbx` file remains untouched. When
    /// `delete_file` is `true`, the registered file is unlinked before
    /// the in-memory entry is removed. A missing file is treated as
    /// already deleted; other I/O failures leave the registry unchanged.
    pub fn deregister(
        &mut self,
        name: &str,
        delete_file: bool,
    ) -> Result<RegisteredVault, VaultError> {
        let index = self
            .vaults
            .iter()
            .position(|vault| vault.name == name)
            .ok_or_else(|| VaultError::NotRegistered {
                name: name.to_string(),
            })?;

        if delete_file {
            let path = self.vaults[index].path.clone();
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => return Err(VaultError::Io { source, path }),
            }
        }

        Ok(self.vaults.remove(index))
    }

    /// Iterate over the registered vaults in insertion order.
    pub fn list(&self) -> impl Iterator<Item = &RegisteredVault> {
        self.vaults.iter()
    }

    /// Look up a vault by name.
    pub fn get(&self, name: &str) -> Option<&RegisteredVault> {
        self.vaults.iter().find(|v| v.name == name)
    }

    /// The [`HidlinsPaths`] this registry is rooted in.
    pub fn paths(&self) -> &HidlinsPaths {
        &self.paths
    }

    /// Return the schema version this registry was loaded with. Always
    /// [`SCHEMA_VERSION`] for in-memory and saved registries.
    pub fn version(&self) -> u32 {
        self.version
    }
}

fn recover_one_rotation(paths: &HidlinsPaths, marker_path: &Path) -> Result<(), VaultError> {
    let marker_body = std::fs::read_to_string(marker_path).map_err(|source| VaultError::Io {
        source,
        path: marker_path.to_path_buf(),
    })?;
    let mut marker: RotationMarker =
        toml::from_str(&marker_body).map_err(|_| VaultError::PasswordRotationRecoveryFailed {
            reason: "password rotation marker is malformed",
        })?;
    if marker.version != ROTATION_VERSION {
        return Err(VaultError::PasswordRotationRecoveryFailed {
            reason: "password rotation marker version is unsupported",
        });
    }
    validate_rotation_name(&marker.vault_name)?;
    validate_extra_key(&marker.extra_key)?;

    let initial = VaultRegistry::load(paths.clone())?;
    let vault_path = initial
        .get(&marker.vault_name)
        .ok_or(VaultError::PasswordRotationRecoveryFailed {
            reason: "password rotation vault is no longer registered",
        })?
        .path
        .clone();
    let artifacts = rotation_artifacts(paths, &vault_path, &marker.vault_name);
    if artifacts.marker != marker_path {
        return Err(VaultError::PasswordRotationRecoveryFailed {
            reason: "password rotation marker name is inconsistent",
        });
    }

    // Same stable order as the live transaction: vault, then registry.
    let _vault_lock = acquire_exclusive(&vault_path)?;
    let registry_path = paths.vaults_toml();
    let _registry_lock = acquire_exclusive(&registry_path)?;
    let mut latest = VaultRegistry::load(paths.clone())?;
    let latest_vault =
        latest
            .get(&marker.vault_name)
            .ok_or(VaultError::PasswordRotationRecoveryFailed {
                reason: "password rotation vault is no longer registered",
            })?;
    if latest_vault.path != vault_path {
        return Err(VaultError::PasswordRotationRecoveryFailed {
            reason: "password rotation vault path changed",
        });
    }

    let live_vault_hash = hash_file(&vault_path)?;
    if live_vault_hash == marker.old_vault_sha256 {
        if matches!(
            marker.phase,
            RotationPhase::VaultCommitted | RotationPhase::RegistryCommitted
        ) {
            return Err(VaultError::PasswordRotationRecoveryFailed {
                reason: "password rotation phase contradicts the live vault",
            });
        }
        require_hash(&artifacts.vault_stage, &marker.new_vault_sha256)?;
        install_staged_file(&artifacts.vault_stage, &vault_path)?;
    } else if live_vault_hash != marker.new_vault_sha256 {
        return Err(VaultError::PasswordRotationRecoveryFailed {
            reason: "live vault does not match either transaction version",
        });
    }

    marker.phase = RotationPhase::VaultCommitted;
    write_rotation_marker(&artifacts.marker, &marker)?;

    recover_registry_value(
        &mut latest,
        &marker,
        &artifacts,
        &vault_path,
        &registry_path,
    )?;

    marker.phase = RotationPhase::RegistryCommitted;
    write_rotation_marker(&artifacts.marker, &marker)?;
    remove_file_durable(&artifacts.vault_stage)?;
    remove_file_durable(&artifacts.registry_stage)?;
    remove_file_durable(&artifacts.marker)?;
    Ok(())
}

fn recover_registry_value(
    latest: &mut VaultRegistry,
    marker: &RotationMarker,
    artifacts: &RotationArtifacts,
    vault_path: &Path,
    registry_path: &Path,
) -> Result<(), VaultError> {
    let current_extra = latest
        .get(&marker.vault_name)
        .and_then(|entry| entry.extra.get(&marker.extra_key))
        .ok_or(VaultError::PasswordRotationRecoveryFailed {
            reason: "password rotation registry value is missing",
        })?;
    let current_extra_hash = hash_toml_value(current_extra)?;
    if current_extra_hash == marker.old_extra_sha256 {
        require_hash(&artifacts.registry_stage, &marker.new_registry_sha256)?;
        let staged_body = std::fs::read_to_string(&artifacts.registry_stage).map_err(|source| {
            VaultError::Io {
                source,
                path: artifacts.registry_stage.clone(),
            }
        })?;
        let staged: OnDisk = toml::from_str(&staged_body).map_err(|_| {
            VaultError::PasswordRotationRecoveryFailed {
                reason: "password rotation registry stage is malformed",
            }
        })?;
        if staged.version != SCHEMA_VERSION {
            return Err(VaultError::PasswordRotationRecoveryFailed {
                reason: "password rotation registry stage version is unsupported",
            });
        }
        let replacement = staged
            .vaults
            .iter()
            .find(|entry| entry.name == marker.vault_name && entry.path == vault_path)
            .and_then(|entry| entry.extra.get(&marker.extra_key))
            .ok_or(VaultError::PasswordRotationRecoveryFailed {
                reason: "password rotation registry stage is inconsistent",
            })?;
        if hash_toml_value(replacement)? != marker.new_extra_sha256 {
            return Err(VaultError::PasswordRotationRecoveryFailed {
                reason: "password rotation registry value hash mismatch",
            });
        }

        let current_registry_hash = hash_file(registry_path)?;
        if current_registry_hash == marker.old_registry_sha256 {
            install_staged_file(&artifacts.registry_stage, registry_path)?;
        } else {
            // Only replace the password-sealed value in the latest registry.
            // Unrelated registrations and fields written after the crash are
            // retained. A concurrent edit to this same value was rejected by
            // the old/new hash check above.
            let target = latest
                .vaults
                .iter_mut()
                .find(|entry| entry.name == marker.vault_name)
                .ok_or(VaultError::PasswordRotationRecoveryFailed {
                    reason: "password rotation vault is no longer registered",
                })?;
            target
                .extra
                .insert(marker.extra_key.clone(), replacement.clone());
            latest.save_unlocked()?;
        }
    } else if current_extra_hash != marker.new_extra_sha256 {
        return Err(VaultError::PasswordRotationRecoveryFailed {
            reason: "password rotation registry value changed unexpectedly",
        });
    }

    Ok(())
}

fn rotation_artifacts(
    paths: &HidlinsPaths,
    vault_path: &Path,
    vault_name: &str,
) -> RotationArtifacts {
    let registry_path = paths.vaults_toml();
    let registry_parent = parent_dir(&registry_path);
    let vault_parent = parent_dir(vault_path);
    let registry_hash = hash_bytes(registry_path.as_os_str().as_encoded_bytes());
    let name_hash = hash_bytes(vault_name.as_bytes());
    let id = format!("{}-{}", &registry_hash[..16], &name_hash[..16]);
    let base = format!("{ROTATION_PREFIX}{id}");
    RotationArtifacts {
        marker: registry_parent.join(format!("{base}.txn")),
        registry_stage: registry_parent.join(format!("{base}.registry.stage")),
        vault_stage: vault_parent.join(format!("{base}.vault.stage")),
    }
}

fn rotation_registry_prefix(registry_path: &Path) -> String {
    let registry_hash = hash_bytes(registry_path.as_os_str().as_encoded_bytes());
    format!("{ROTATION_PREFIX}{}-", &registry_hash[..16])
}

fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

fn validate_rotation_name(name: &str) -> Result<(), VaultError> {
    if name.is_empty() || name.len() > 255 {
        return Err(VaultError::PasswordRotationRecoveryFailed {
            reason: "password rotation vault name is invalid",
        });
    }
    Ok(())
}

fn validate_extra_key(key: &str) -> Result<(), VaultError> {
    if key.is_empty()
        || key.len() > 64
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(VaultError::PasswordRotationRecoveryFailed {
            reason: "password rotation registry key is invalid",
        });
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, VaultError> {
    let bytes = std::fs::read(path).map_err(|source| VaultError::Io {
        source,
        path: path.to_path_buf(),
    })?;
    Ok(hash_bytes(&bytes))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn hash_toml_value(value: &toml::Value) -> Result<String, VaultError> {
    let serialized = toml::to_string(value)
        .map_err(|source| VaultError::RegistrySerializationFailed { source })?;
    Ok(hash_bytes(serialized.as_bytes()))
}

fn require_hash(path: &Path, expected: &str) -> Result<(), VaultError> {
    if !path.exists() {
        return Err(VaultError::PasswordRotationRecoveryFailed {
            reason: "required password rotation stage is missing",
        });
    }
    if hash_file(path)? != expected {
        return Err(VaultError::PasswordRotationRecoveryFailed {
            reason: "password rotation stage hash mismatch",
        });
    }
    Ok(())
}

fn write_rotation_marker(path: &Path, marker: &RotationMarker) -> Result<(), VaultError> {
    let body = toml::to_string(marker)
        .map_err(|source| VaultError::RegistrySerializationFailed { source })?;
    write_atomic(path, body.as_bytes())
}

#[cfg(debug_assertions)]
fn maybe_signal_rotation_phase(phase: &str) -> Result<(), VaultError> {
    const SIGNAL_DIR_ENV: &str = "HIDLINS_ROTATION_FAULT_SIGNAL_DIR";
    const PAUSE_PHASE_ENV: &str = "HIDLINS_ROTATION_FAULT_PAUSE_PHASE";
    if let Ok(signal_dir) = std::env::var(SIGNAL_DIR_ENV) {
        let signal_path = Path::new(&signal_dir).join(phase);
        std::fs::write(&signal_path, phase).map_err(|source| VaultError::Io {
            source,
            path: signal_path,
        })?;
    }
    if std::env::var(PAUSE_PHASE_ENV).as_deref() == Ok(phase) {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    Ok(())
}

#[cfg(not(debug_assertions))]
fn maybe_signal_rotation_phase(_phase: &str) -> Result<(), VaultError> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    /// Build a `HidlinsPaths` rooted in `dir` with the state dir at `<dir>/state`.
    /// Matches the `TestEnv` shape used by integration tests but avoids the
    /// integration-test `common::mod` (this is an in-file unit test).
    fn paths_in(dir: &TempDir) -> HidlinsPaths {
        HidlinsPaths::with_state_dir(dir.path().join("state"))
    }

    /// Construct a representative [`RegisteredVault`].
    fn sample_vault(name: &str, dir: &Path) -> RegisteredVault {
        RegisteredVault {
            name: name.to_string(),
            path: dir.join(format!("{name}.kdbx")),
            created_at: "2026-05-17T10:00:00-06:00".to_string(),
            keyfile_path: None,
            extra: toml::Table::new(),
        }
    }

    // -------------------------------------------------------------------
    // Load / save / register / list / get
    // -------------------------------------------------------------------

    #[test]
    fn load_missing_file_returns_empty_registry() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        let registry = VaultRegistry::load(paths).expect("load missing succeeds");
        assert_eq!(registry.list().count(), 0);
    }

    #[test]
    fn register_and_save_missing_registry_round_trips_every_field() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        let mut registry = VaultRegistry::with_paths(paths.clone());
        let mut vault = sample_vault("personal", tmp.path());
        vault.keyfile_path = Some(tmp.path().join("personal.key"));
        vault.extra.insert(
            "future_setting".to_string(),
            toml::Value::String("preserve me".to_string()),
        );

        registry
            .register_and_save(vault.clone())
            .expect("transactional registration succeeds");

        for registered in [
            registry.get("personal").expect("caller updated"),
            VaultRegistry::load(paths)
                .expect("reload")
                .get("personal")
                .expect("persisted registration"),
        ] {
            assert_eq!(registered.name, vault.name);
            assert_eq!(registered.path, vault.path);
            assert_eq!(registered.created_at, vault.created_at);
            assert_eq!(registered.keyfile_path, vault.keyfile_path);
            assert_eq!(registered.extra, vault.extra);
        }
    }

    #[test]
    fn register_and_save_preserves_unrelated_concurrent_registration() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        let mut initial = VaultRegistry::with_paths(paths.clone());
        initial
            .register_and_save(sample_vault("personal", tmp.path()))
            .expect("initial registration");

        let mut stale = VaultRegistry::load(paths.clone()).expect("stale load");
        let mut concurrent = VaultRegistry::load(paths.clone()).expect("concurrent load");
        concurrent
            .register_and_save(sample_vault("work", tmp.path()))
            .expect("concurrent registration");

        stale
            .register_and_save(sample_vault("archive", tmp.path()))
            .expect("stale caller rebases registration");

        let names: Vec<_> = stale.list().map(|vault| vault.name.as_str()).collect();
        assert_eq!(names, ["personal", "work", "archive"]);
        let reloaded = VaultRegistry::load(paths).expect("reload");
        assert!(reloaded.get("personal").is_some());
        assert!(reloaded.get("work").is_some());
        assert!(reloaded.get("archive").is_some());
    }

    #[test]
    fn register_and_save_duplicate_leaves_disk_and_caller_unchanged() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        let mut registry = VaultRegistry::with_paths(paths.clone());
        registry
            .register_and_save(sample_vault("personal", tmp.path()))
            .expect("initial registration");
        let disk_before = std::fs::read(paths.vaults_toml()).expect("registry bytes");
        let caller_before: Vec<_> = registry
            .list()
            .map(|vault| (vault.name.clone(), vault.path.clone()))
            .collect();

        let mut duplicate = sample_vault("personal", tmp.path());
        duplicate.path = tmp.path().join("different.kdbx");
        let error = registry
            .register_and_save(duplicate)
            .expect_err("duplicate must fail");

        assert!(matches!(
            error,
            VaultError::AlreadyRegistered { ref name } if name == "personal"
        ));
        assert_eq!(std::fs::read(paths.vaults_toml()).unwrap(), disk_before);
        let caller_after: Vec<_> = registry
            .list()
            .map(|vault| (vault.name.clone(), vault.path.clone()))
            .collect();
        assert_eq!(caller_after, caller_before);
    }

    #[test]
    fn transactional_update_preserves_a_concurrent_registry_change() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        let mut stale = VaultRegistry::with_paths(paths.clone());
        stale
            .register(sample_vault("personal", tmp.path()))
            .expect("register");
        stale.save().expect("initial save");

        let mut other = VaultRegistry::load(paths.clone()).expect("other load");
        other
            .register(sample_vault("work", tmp.path()))
            .expect("register work");
        other.save().expect("other save");

        stale
            .update_registered_extra("personal", |extra| {
                extra.insert(
                    "lock".to_string(),
                    toml::Value::String("changed".to_string()),
                );
            })
            .expect("transactional update");

        let reloaded = VaultRegistry::load(paths).expect("reload");
        assert!(
            reloaded.get("work").is_some(),
            "unrelated completed write survives"
        );
        assert_eq!(
            reloaded.get("personal").unwrap().extra["lock"].as_str(),
            Some("changed")
        );
    }

    #[test]
    fn stale_whole_registry_save_is_rejected_instead_of_losing_data() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        let mut initial = VaultRegistry::with_paths(paths.clone());
        initial
            .register(sample_vault("personal", tmp.path()))
            .unwrap();
        initial.save().unwrap();

        let mut first = VaultRegistry::load(paths.clone()).unwrap();
        let mut stale = VaultRegistry::load(paths.clone()).unwrap();
        first.register(sample_vault("work", tmp.path())).unwrap();
        first.save().unwrap();
        stale.register(sample_vault("archive", tmp.path())).unwrap();

        assert!(matches!(stale.save(), Err(VaultError::RegistryChanged)));
        let reloaded = VaultRegistry::load(paths).unwrap();
        assert!(reloaded.get("work").is_some());
        assert!(reloaded.get("archive").is_none());
    }

    #[test]
    fn contended_registry_save_is_non_destructive() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        let mut registry = VaultRegistry::with_paths(paths.clone());
        registry
            .register(sample_vault("personal", tmp.path()))
            .unwrap();
        registry.save().unwrap();
        let path = paths.vaults_toml();
        let before = std::fs::read(&path).unwrap();
        let _guard = crate::locking::acquire_exclusive(&path).unwrap();
        registry.register(sample_vault("work", tmp.path())).unwrap();
        assert!(matches!(registry.save(), Err(VaultError::Contended { .. })));
        assert_eq!(std::fs::read(path).unwrap(), before);
    }

    #[test]
    fn register_then_get_returns_record() {
        let tmp = TempDir::new().expect("tempdir");
        let mut registry = VaultRegistry::with_paths(paths_in(&tmp));

        let vault = sample_vault("personal", tmp.path());
        registry.register(vault.clone()).expect("register succeeds");

        let got = registry.get("personal").expect("get returns the vault");
        assert_eq!(got.name, "personal");
        assert_eq!(got.path, vault.path);
    }

    #[test]
    fn register_then_save_then_load_round_trips() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);

        // Phase 1: write.
        {
            let mut registry = VaultRegistry::with_paths(paths.clone());
            registry
                .register(sample_vault("personal", tmp.path()))
                .expect("register");
            registry
                .register(sample_vault("work", tmp.path()))
                .expect("register");
            registry.save().expect("save succeeds");
        }

        // Phase 2: read in a fresh registry instance.
        let reloaded = VaultRegistry::load(paths).expect("reload succeeds");
        let names: Vec<&str> = reloaded.list().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["personal", "work"]);
        assert_eq!(reloaded.version(), SCHEMA_VERSION);
    }

    #[test]
    fn register_duplicate_name_returns_already_registered() {
        let tmp = TempDir::new().expect("tempdir");
        let mut registry = VaultRegistry::with_paths(paths_in(&tmp));

        registry
            .register(sample_vault("personal", tmp.path()))
            .expect("first register");
        let err = registry
            .register(sample_vault("personal", tmp.path()))
            .expect_err("duplicate register should fail");
        assert!(matches!(err, VaultError::AlreadyRegistered { ref name } if name == "personal"));
    }

    // -------------------------------------------------------------------
    // Forward-compat — unknown-key preservation. The two highest-value
    // tests in the module: a regression here would silently clobber the
    // sync / auto-lock features' future config when vault-core re-saves.
    // -------------------------------------------------------------------

    /// Hand-craft a fixture matching `tests/fixtures/v1_registry.toml`
    /// but inline so the test stays self-contained. The fixture includes:
    /// - a known top-level key (`version`)
    /// - an unknown top-level key (`[future_feature]`)
    /// - one `[[vault]]` entry with a known field set
    /// - an unknown per-vault key (`note = "x"`)
    /// - an unknown per-vault sub-table (`[[vault.sync]]` style)
    fn v1_fixture_toml() -> &'static str {
        r#"
version = 1

[future_feature]
foo = 1
bar = "two"

[[vault]]
name = "personal"
path = "/tmp/personal.kdbx"
created_at = "2026-05-17T10:00:00-06:00"
note = "unknown per-vault key — should survive"

[vault.sync]
remote = "git@example:vault.git"
branch = "main"
"#
    }

    #[test]
    fn unknown_top_level_keys_preserved_across_roundtrip() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        paths.ensure_exists().expect("ensure state dir");
        std::fs::write(paths.vaults_toml(), v1_fixture_toml()).expect("seed fixture");

        // Load, then save unchanged.
        let mut registry = VaultRegistry::load(paths.clone()).expect("load fixture");
        registry.save().expect("save unchanged");

        // Re-read the raw file and verify the unknown top-level table
        // is intact.
        let raw = std::fs::read_to_string(paths.vaults_toml()).expect("read saved");
        let table: toml::Table = toml::from_str(&raw).expect("parse saved");
        let future = table
            .get("future_feature")
            .and_then(toml::Value::as_table)
            .expect("future_feature top-level table preserved");
        assert_eq!(
            future.get("foo").and_then(toml::Value::as_integer),
            Some(1),
            "foo value preserved"
        );
        assert_eq!(
            future.get("bar").and_then(toml::Value::as_str),
            Some("two"),
            "bar value preserved"
        );
    }

    #[test]
    fn unknown_per_vault_keys_preserved_across_roundtrip() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        paths.ensure_exists().expect("ensure state dir");
        std::fs::write(paths.vaults_toml(), v1_fixture_toml()).expect("seed fixture");

        let mut registry = VaultRegistry::load(paths.clone()).expect("load fixture");
        registry.save().expect("save unchanged");

        // Re-read the raw file and inspect the [[vault]] entry.
        let raw = std::fs::read_to_string(paths.vaults_toml()).expect("read saved");
        let table: toml::Table = toml::from_str(&raw).expect("parse saved");
        let vaults = table
            .get("vault")
            .and_then(toml::Value::as_array)
            .expect("[[vault]] array present");
        assert_eq!(vaults.len(), 1, "exactly one vault");
        let vault = vaults[0].as_table().expect("vault entry is a table");

        // Known field still there.
        assert_eq!(
            vault.get("name").and_then(toml::Value::as_str),
            Some("personal")
        );

        // Unknown scalar field preserved.
        assert_eq!(
            vault.get("note").and_then(toml::Value::as_str),
            Some("unknown per-vault key — should survive"),
            "unknown per-vault scalar key preserved"
        );

        // Unknown sub-table preserved.
        let sync = vault
            .get("sync")
            .and_then(toml::Value::as_table)
            .expect("sync sub-table preserved");
        assert_eq!(
            sync.get("remote").and_then(toml::Value::as_str),
            Some("git@example:vault.git"),
            "sync.remote preserved"
        );
        assert_eq!(
            sync.get("branch").and_then(toml::Value::as_str),
            Some("main"),
            "sync.branch preserved"
        );
    }

    // -------------------------------------------------------------------
    // Error paths
    // -------------------------------------------------------------------

    #[test]
    fn load_with_unsupported_version_errors() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        paths.ensure_exists().expect("ensure state dir");
        let fixture = r#"
version = 99

[[vault]]
name = "future"
path = "/tmp/future.kdbx"
created_at = "2026-05-17T10:00:00-06:00"
"#;
        std::fs::write(paths.vaults_toml(), fixture).expect("seed v99 fixture");

        let err = VaultRegistry::load(paths).expect_err("v99 should error");
        assert!(
            matches!(err, VaultError::RegistryMalformed { source: None }),
            "expected RegistryMalformed with no toml source for version mismatch, got {err:?}"
        );
    }

    #[test]
    fn load_malformed_toml_errors() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        paths.ensure_exists().expect("ensure state dir");
        // Unclosed string literal — guaranteed parser failure.
        std::fs::write(paths.vaults_toml(), "version = \"unterminated").expect("seed malformed");

        let err = VaultRegistry::load(paths).expect_err("malformed toml should error");
        assert!(
            matches!(err, VaultError::RegistryMalformed { source: Some(_) }),
            "expected RegistryMalformed with toml::de::Error source, got {err:?}"
        );
    }

    // -------------------------------------------------------------------
    // Permissions
    // -------------------------------------------------------------------

    #[cfg(unix)]
    #[test]
    fn save_creates_file_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        let mut registry = VaultRegistry::with_paths(paths.clone());
        registry
            .register(sample_vault("personal", tmp.path()))
            .expect("register");
        registry.save().expect("save");

        let mode = std::fs::metadata(paths.vaults_toml())
            .expect("stat vaults.toml")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "vaults.toml must be created with mode 0600");
    }

    // -------------------------------------------------------------------
    // Fixture-file tests — confirms hand-crafted fixtures parse correctly.
    // Mirror of v1_fixture_toml above but loaded from disk; protects
    // against accidental fixture-file mutations.
    // -------------------------------------------------------------------

    #[test]
    fn fixture_v1_registry_toml_loads_and_preserves_extras() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        paths.ensure_exists().expect("ensure state dir");

        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1_registry.toml");
        let body = std::fs::read_to_string(&fixture_path)
            .unwrap_or_else(|e| panic!("read fixture {}: {e}", fixture_path.display()));
        std::fs::write(paths.vaults_toml(), body).expect("seed fixture");

        let mut registry = VaultRegistry::load(paths.clone()).expect("load fixture");
        assert!(
            registry.get("personal").is_some(),
            "fixture should contain the 'personal' vault"
        );
        // Save + reload: the fixture's unknown keys should survive.
        registry.save().expect("save fixture");
        let reloaded = VaultRegistry::load(paths.clone()).expect("reload fixture");
        assert!(reloaded.get("personal").is_some());

        let raw = std::fs::read_to_string(paths.vaults_toml()).expect("read saved fixture");
        let table: toml::Table = toml::from_str(&raw).expect("parse saved fixture");

        let future = table
            .get("future_feature")
            .and_then(toml::Value::as_table)
            .expect("future_feature top-level table preserved");
        assert_eq!(future.get("foo").and_then(toml::Value::as_integer), Some(1));
        assert_eq!(future.get("bar").and_then(toml::Value::as_str), Some("two"));

        let vaults = table
            .get("vault")
            .and_then(toml::Value::as_array)
            .expect("[[vault]] array preserved");
        let vault = vaults
            .first()
            .and_then(toml::Value::as_table)
            .expect("first vault table preserved");
        assert_eq!(
            vault.get("note").and_then(toml::Value::as_str),
            Some("an unknown per-vault key — should survive load/save")
        );
        let sync = vault
            .get("sync")
            .and_then(toml::Value::as_table)
            .expect("vault.sync table preserved");
        assert_eq!(
            sync.get("remote").and_then(toml::Value::as_str),
            Some("git@example:vault.git")
        );
        assert_eq!(
            sync.get("branch").and_then(toml::Value::as_str),
            Some("main")
        );
    }

    #[test]
    fn fixture_v99_registry_toml_is_rejected() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        paths.ensure_exists().expect("ensure state dir");

        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v99_registry.toml");
        let body = std::fs::read_to_string(&fixture_path).expect("read v99 fixture");
        std::fs::write(paths.vaults_toml(), body).expect("seed v99 fixture");

        let err = VaultRegistry::load(paths).expect_err("v99 fixture should error");
        assert!(matches!(
            err,
            VaultError::RegistryMalformed { source: None }
        ));
    }

    #[test]
    fn fixture_malformed_toml_is_rejected() {
        let tmp = TempDir::new().expect("tempdir");
        let paths = paths_in(&tmp);
        paths.ensure_exists().expect("ensure state dir");

        let fixture_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/malformed.toml");
        let body = std::fs::read_to_string(&fixture_path).expect("read malformed fixture");
        std::fs::write(paths.vaults_toml(), body).expect("seed malformed fixture");

        let err = VaultRegistry::load(paths).expect_err("malformed fixture should error");
        assert!(matches!(
            err,
            VaultError::RegistryMalformed { source: Some(_) }
        ));
    }
}

//! KDBX vault handles over the `keepass-rs` database model.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use keepass::config::{DatabaseConfig, DatabaseVersion, KdfConfig};
use keepass::db::DatabaseOpenError;
use keepass::Database;

use crate::atomic::write_atomic_with;
use crate::locking::{acquire_exclusive, acquire_shared};
use crate::unlock::{build_database_key, build_database_key_from_material, KeyfileMaterial};
use crate::{ExclusiveLock, Keyfile, MasterPassword, SharedLock, VaultError};

/// KDF settings for newly-created KDBX4 vaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Argon2 memory cost in KiB.
    pub memory_kib: u64,
    /// Argon2 iteration count.
    pub iterations: u64,
    /// Argon2 lane count.
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_kib: 65_536,
            iterations: 3,
            parallelism: 2,
        }
    }
}

impl KdfParams {
    fn database_config(self) -> DatabaseConfig {
        let mut config = DatabaseConfig::default();
        config.kdf_config = KdfConfig::Argon2id {
            iterations: self.iterations,
            memory: self.memory_kib * 1024,
            parallelism: self.parallelism,
            version: argon2::Version::Version13,
        };
        config
    }
}

/// Confirmation marker for the no-recovery warning.
///
/// Callers can only construct this value through [`Self::yes`], after
/// showing the user the no-recovery warning.
///
/// ```compile_fail
/// let _confirmation = hidlins_core::NoRecoveryConfirmed {};
/// ```
#[derive(Debug, Clone, Copy)]
pub struct NoRecoveryConfirmed {
    _private: (),
}

impl NoRecoveryConfirmed {
    /// Construct the confirmation marker after the caller shows the
    /// no-recovery warning to the user.
    pub fn yes() -> Self {
        Self { _private: () }
    }
}

/// Read-write vault handle. Holds an exclusive advisory lock.
pub struct Vault {
    path: PathBuf,
    database: Database,
    key: keepass::DatabaseKey,
    keyfile_material: KeyfileMaterial,
    _lock: ExclusiveLock,
}

/// Read-only vault handle. Holds a shared advisory lock.
pub struct VaultReadOnly {
    path: PathBuf,
    database: Database,
    _lock: SharedLock,
}

impl Vault {
    /// Create a new KDBX4 vault at `path`.
    pub fn create(
        path: &Path,
        master: &MasterPassword,
        keyfile: Option<&Keyfile>,
        kdf: KdfParams,
        _confirmation: NoRecoveryConfirmed,
    ) -> Result<Self, VaultError> {
        if path.exists() {
            return Err(VaultError::PathExists {
                path: path.to_path_buf(),
            });
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| VaultError::Io {
                source,
                path: parent.to_path_buf(),
            })?;
        }

        let reservation = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|source| {
                if source.kind() == std::io::ErrorKind::AlreadyExists {
                    VaultError::PathExists {
                        path: path.to_path_buf(),
                    }
                } else {
                    VaultError::Io {
                        source,
                        path: path.to_path_buf(),
                    }
                }
            })?;
        drop(reservation);

        let keyfile_material = KeyfileMaterial::from_keyfile(keyfile)?;
        let key = build_database_key_from_material(master, &keyfile_material)?;
        let mut database = Database::with_config(kdf.database_config());
        let lock = match acquire_exclusive(path) {
            Ok(lock) => lock,
            Err(err) => {
                let _ = std::fs::remove_file(path);
                return Err(err);
            }
        };

        if let Err(err) = save_database(path, &mut database, key.clone()) {
            let _ = std::fs::remove_file(path);
            return Err(err);
        }

        Ok(Self {
            path: path.to_path_buf(),
            database,
            key,
            keyfile_material,
            _lock: lock,
        })
    }

    /// Open an existing vault for read-write access.
    pub fn open(
        path: &Path,
        master: &MasterPassword,
        keyfile: Option<&Keyfile>,
    ) -> Result<Self, VaultError> {
        if !path.exists() {
            return Err(VaultError::FileNotFound {
                path: path.to_path_buf(),
            });
        }

        let lock = acquire_exclusive(path)?;
        let keyfile_material = KeyfileMaterial::from_keyfile(keyfile)?;
        let key = build_database_key_from_material(master, &keyfile_material)?;
        let database = open_database(path, key.clone())?;

        Ok(Self {
            path: path.to_path_buf(),
            database,
            key,
            keyfile_material,
            _lock: lock,
        })
    }

    /// Save the in-memory database to disk using an atomic replace.
    pub fn save(&mut self) -> Result<(), VaultError> {
        save_database(&self.path, &mut self.database, self.key.clone())
    }

    /// Re-encrypt the vault with a new master password after re-verifying
    /// the current password against the on-disk vault.
    pub fn change_master_password(
        &mut self,
        current: &MasterPassword,
        new: &MasterPassword,
    ) -> Result<(), VaultError> {
        let current_key = build_database_key_from_material(current, &self.keyfile_material)?;
        open_database(&self.path, current_key)?;

        let new_key = build_database_key_from_material(new, &self.keyfile_material)?;
        save_database(&self.path, &mut self.database, new_key.clone())?;
        self.key = new_key;
        Ok(())
    }

    /// Return the on-disk vault path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Return immutable access to the in-memory KDBX database.
    ///
    /// Infrastructure seam for the sync adapter and compatibility tooling.
    /// Presentation crates should use the owned/query APIs instead.
    #[doc(hidden)]
    pub fn database(&self) -> &Database {
        &self.database
    }

    /// Return mutable access to the in-memory KDBX database.
    ///
    /// Infrastructure seam for the sync adapter and compatibility tooling.
    /// Presentation crates must use invariant-preserving mutation APIs instead.
    #[doc(hidden)]
    pub fn database_mut(&mut self) -> &mut Database {
        &mut self.database
    }

    /// Decrypt a KDBX database from in-memory bytes, without touching the
    /// filesystem or acquiring a lock.
    ///
    /// Used by `hidlins-sync` to decrypt a remote vault snapshot fetched over
    /// the sync transport as an encrypted object so it can be fed into the merge.
    /// Returns the parsed [`Database`] rather than a [`Vault`] because the
    /// snapshot is path-less and lock-less — it is consumed read-only by the
    /// merge and then dropped; a partial `Vault` with a panicking `path()`
    /// would be the wrong shape. The key is derived from `master` (and
    /// `keyfile`) exactly as [`Vault::open`] does, so a snapshot encrypted with
    /// a different master password surfaces as [`VaultError::AuthenticationFailed`].
    ///
    /// Public API: used by `hidlins-sync` for merge snapshots and by
    /// `hidlins-api` for bootstrap-from-remote password validation.
    pub fn open_from_bytes(
        bytes: &[u8],
        master: &MasterPassword,
        keyfile: Option<&Keyfile>,
    ) -> Result<Database, VaultError> {
        let key = build_database_key(master, keyfile)?;
        let mut cursor = std::io::Cursor::new(bytes);
        Database::open(&mut cursor, key)
            .map_err(|source| map_open_error(source, Path::new("<memory>")))
    }

    /// Replace the in-memory database with `new_db`, after verifying it
    /// describes the same vault (identical root-group UUID).
    ///
    /// Used by `hidlins-sync` to install a merged or fast-forwarded database
    /// before [`Vault::save`]. The root-group-UUID guard refuses a database
    /// from a different vault — installing one would silently swap the vault's
    /// identity (and desynchronise the registry), so it returns
    /// [`VaultError::DatabaseIdentityMismatch`] rather than corrupting state.
    ///
    /// `#[doc(hidden)]`: cross-crate-internal API for the sync layer.
    ///
    /// # Errors
    ///
    /// Returns [`VaultError::DatabaseIdentityMismatch`] if `new_db`'s root-group
    /// UUID differs from the current database's.
    #[doc(hidden)]
    pub fn replace_database(&mut self, new_db: Database) -> Result<(), VaultError> {
        let expected = self.database.root().id().uuid();
        let found = new_db.root().id().uuid();
        if expected != found {
            return Err(VaultError::DatabaseIdentityMismatch { expected, found });
        }
        self.database = new_db;
        Ok(())
    }
}

impl VaultReadOnly {
    /// Open an existing vault for read-only access.
    pub fn open(
        path: &Path,
        master: &MasterPassword,
        keyfile: Option<&Keyfile>,
    ) -> Result<Self, VaultError> {
        if !path.exists() {
            return Err(VaultError::FileNotFound {
                path: path.to_path_buf(),
            });
        }

        let lock = acquire_shared(path)?;
        let key = build_database_key(master, keyfile)?;
        let database = open_database(path, key)?;

        Ok(Self {
            path: path.to_path_buf(),
            database,
            _lock: lock,
        })
    }

    /// Return the on-disk vault path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Return immutable access to the in-memory KDBX database.
    ///
    /// Infrastructure seam for sync and compatibility tooling. Presentation
    /// crates should use the owned/query APIs instead.
    #[doc(hidden)]
    pub fn database(&self) -> &Database {
        &self.database
    }
}

fn open_database(path: &Path, key: keepass::DatabaseKey) -> Result<Database, VaultError> {
    let mut file = File::open(path).map_err(|source| VaultError::Io {
        source,
        path: path.to_path_buf(),
    })?;

    Database::open(&mut file, key).map_err(|source| map_open_error(source, path))
}

fn save_database(
    path: &Path,
    database: &mut Database,
    key: keepass::DatabaseKey,
) -> Result<(), VaultError> {
    // Hidlins reads KDBX3 for migration but only writes the current KDBX4
    // minor version. KeePassXC can legitimately rewrite a shared vault as
    // KDBX 4.0, while keepass-rs currently serializes KDBX 4.1 only. Promote
    // either legacy form before the next Hidlins save while preserving the
    // parsed cipher and KDF configuration (including legacy AES-KDF).
    let current_version = DatabaseConfig::default().version;
    let DatabaseVersion::KDB4(current_minor) = current_version else {
        unreachable!("keepass-rs default database version must be KDBX4")
    };
    let needs_migration = match database.config.version {
        DatabaseVersion::KDB3(_) => true,
        DatabaseVersion::KDB4(minor) => minor < current_minor,
        _ => false,
    };
    if needs_migration {
        database.config.version = DatabaseVersion::KDB4(current_minor);
    }
    write_atomic_with(path, |file| {
        database
            .save(file, key)
            .map_err(|source| VaultError::WriteFailed { source })
    })
}

fn map_open_error(source: DatabaseOpenError, path: &Path) -> VaultError {
    match source {
        DatabaseOpenError::Io(source) => VaultError::Io {
            source,
            path: path.to_path_buf(),
        },
        DatabaseOpenError::Key(_) | DatabaseOpenError::Cryptography(_) => {
            VaultError::AuthenticationFailed
        }
        DatabaseOpenError::UnexpectedEof
        | DatabaseOpenError::VersionParse(_)
        | DatabaseOpenError::UnsupportedVersion
        | DatabaseOpenError::Format(_) => VaultError::InvalidFormat { source },
        _ => VaultError::AuthenticationFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::{save_database, Database, DatabaseConfig};
    use keepass::config::DatabaseVersion;
    use keepass::DatabaseKey;
    use std::fs::File;

    #[test]
    fn saving_a_legacy_kdbx3_database_migrates_it_to_kdbx4() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("legacy.kdbx");
        let key = DatabaseKey::new().with_password("legacy-test-password");
        let mut database = Database::with_config(DatabaseConfig::default());
        database.config.version = DatabaseVersion::KDB3(1);

        save_database(&path, &mut database, key.clone())
            .expect("Hidlins writes legacy KDBX3 databases back as KDBX4");

        assert!(matches!(database.config.version, DatabaseVersion::KDB4(_)));
        let reopened = Database::open(&mut File::open(path).unwrap(), key).unwrap();
        assert!(matches!(reopened.config.version, DatabaseVersion::KDB4(_)));
    }

    #[test]
    fn saving_a_kdbx4_0_database_migrates_it_to_the_writable_minor_version() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("kdbx-4-0.kdbx");
        let key = DatabaseKey::new().with_password("legacy-test-password");
        let mut database = Database::with_config(DatabaseConfig::default());
        database.config.version = DatabaseVersion::KDB4(0);

        save_database(&path, &mut database, key.clone())
            .expect("Hidlins writes KeePassXC KDBX 4.0 databases as current KDBX4");

        assert_eq!(database.config.version, DatabaseConfig::default().version);
        let reopened = Database::open(&mut File::open(path).unwrap(), key).unwrap();
        assert_eq!(reopened.config.version, DatabaseConfig::default().version);
    }
}

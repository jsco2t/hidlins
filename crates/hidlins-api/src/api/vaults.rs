use std::path::PathBuf;

use hidlins_core::{KdfParams, MasterPassword, NoRecoveryConfirmed, RegisteredVault, Vault};

use super::session::AppSession;
use crate::dto::{KeyfileRef, VaultSummary};
use crate::error::HidlinsApiError;

pub(super) fn protect_keyfile(keyfile: Option<KeyfileRef>) -> Option<hidlins_core::Keyfile> {
    keyfile.map(|value| match value {
        KeyfileRef::Path(path) => hidlins_core::Keyfile::Path(PathBuf::from(path)),
        KeyfileRef::Bytes(bytes) => hidlins_core::Keyfile::Bytes(bytes),
    })
}

pub(crate) fn validate_vault_name(name: &str) -> Result<(), HidlinsApiError> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(HidlinsApiError::InvalidInput {
            field: "name".to_string(),
            reason: "vault name must not be empty or contain path separators / '..'".to_string(),
        });
    }
    Ok(())
}

#[allow(clippy::needless_pass_by_value)] // Public API accepts owned DTOs
impl AppSession {
    pub fn list_vaults(&self) -> Result<Vec<VaultSummary>, HidlinsApiError> {
        let state = self.lock_state();
        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }
        let summaries = state
            .registry
            .list()
            .map(|entry| VaultSummary {
                name: entry.name.clone(),
                path: entry.path.display().to_string(),
                has_keyfile: entry.keyfile_path.is_some(),
                has_sync: hidlins_sync::config::local::LocalSyncConfig::from_vault_entry(entry)
                    .is_some(),
            })
            .collect();
        Ok(summaries)
    }

    pub fn create_vault(
        &self,
        name: String,
        master_password: String,
        file_name: Option<String>,
        keyfile: Option<KeyfileRef>,
        confirmed_no_recovery: bool,
    ) -> Result<VaultSummary, HidlinsApiError> {
        // Zeroize the inbound secret at function entry, before any validation.
        let master = MasterPassword::new(master_password);
        let kf = protect_keyfile(keyfile);

        validate_vault_name(&name)?;

        if !confirmed_no_recovery {
            return Err(HidlinsApiError::InvalidInput {
                field: "confirmed_no_recovery".to_string(),
                reason: "no-recovery warning must be confirmed before vault creation".to_string(),
            });
        }

        // Reject file names containing path separators or parent-directory
        // traversals — the inbound value arrives from FFI and must not
        // write outside the state directory.
        if let Some(ref fname) = file_name {
            if fname.contains('/')
                || fname.contains('\\')
                || fname.contains("..")
                || fname.is_empty()
            {
                return Err(HidlinsApiError::InvalidInput {
                    field: "file_name".to_string(),
                    reason: "file name must not contain path separators or '..'".to_string(),
                });
            }
        }

        let mut state = self.lock_state();
        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }

        // Check name availability before creating the file on disk, so a
        // name collision does not leave an orphan .kdbx.
        if state.registry.get(&name).is_some() {
            return Err(HidlinsApiError::PathExists { path: name });
        }

        let vault_file = file_name.unwrap_or_else(|| format!("{name}.kdbx"));
        let vault_path = self.paths.state_dir().join(&vault_file);

        let _vault = Vault::create(
            &vault_path,
            &master,
            kf.as_ref(),
            KdfParams::default(),
            NoRecoveryConfirmed::yes(),
        )?;

        let registered = RegisteredVault {
            name: name.clone(),
            path: vault_path.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            keyfile_path: kf
                .as_ref()
                .and_then(|k| k.path().map(std::path::Path::to_path_buf)),
            extra: toml::Table::default(),
        };
        if let Err(e) = state.registry.register_and_save(registered) {
            // The registry follows its installed rename even when a later
            // durability step reports an error. Only an uncommitted creation
            // may discard the new KDBX; keep a committed registration usable.
            let installed = state
                .registry
                .get(&name)
                .is_some_and(|entry| entry.path == vault_path);
            if !installed {
                let _ = std::fs::remove_file(&vault_path);
            }
            return Err(e.into());
        }

        Ok(VaultSummary {
            name,
            path: vault_path.display().to_string(),
            has_keyfile: kf.is_some(),
            has_sync: false,
        })
    }

    pub fn register_existing_vault(
        &self,
        name: String,
        kdbx_path: String,
        keyfile: Option<KeyfileRef>,
    ) -> Result<VaultSummary, HidlinsApiError> {
        let keyfile = protect_keyfile(keyfile);
        let path = PathBuf::from(&kdbx_path);
        if !path.exists() {
            return Err(HidlinsApiError::FileNotFound { path: kdbx_path });
        }

        let keyfile_path = keyfile
            .as_ref()
            .and_then(|value| value.path().map(std::path::Path::to_path_buf));

        let registered = RegisteredVault {
            name: name.clone(),
            path: path.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            keyfile_path: keyfile_path.clone(),
            extra: toml::Table::default(),
        };

        let mut state = self.lock_state();
        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }
        state.registry.register(registered)?;
        state.registry.save()?;

        Ok(VaultSummary {
            name,
            path: path.display().to_string(),
            has_keyfile: keyfile_path.is_some(),
            has_sync: false,
        })
    }

    pub fn deregister_vault(&self, name: String, delete_file: bool) -> Result<(), HidlinsApiError> {
        let mut state = self.lock_state();
        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }
        state.registry.deregister(&name, delete_file)?;
        state.registry.save()?;
        Ok(())
    }
}

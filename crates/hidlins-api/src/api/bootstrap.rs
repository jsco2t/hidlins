//! Vault credential rotation coordinated with sealed local-sync identity.

use hidlins_core::MasterPassword;

use super::session::{AppSession, SessionCredentials};
use crate::error::HidlinsApiError;

impl AppSession {
    /// Change the vault password and rewrap (never replace) its local Noise identity.
    #[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
    pub fn change_master_password(
        &self,
        current: String,
        new_password: String,
    ) -> Result<(), HidlinsApiError> {
        let current_master = MasterPassword::new(current);
        let new_master = MasterPassword::new(new_password);
        let mut state = self.lock_state();
        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }

        let vault_path = state
            .vault
            .as_ref()
            .ok_or(HidlinsApiError::VaultLocked)?
            .path()
            .to_path_buf();
        let vault_name = state
            .registry
            .list()
            .find(|entry| entry.path == vault_path)
            .map(|entry| entry.name.clone())
            .ok_or_else(|| HidlinsApiError::Internal {
                context: "unlocked vault is not registered".to_string(),
            })?;
        let keyfile = state
            .credentials
            .as_ref()
            .and_then(|credentials| credentials.keyfile.as_ref());

        // Validate the current credential before changing either durable
        // authority. This uses encrypted KDBX bytes and creates no second lock.
        let bytes = std::fs::read(&vault_path).map_err(|_| HidlinsApiError::Io {
            context: "vault password verification read failed".to_string(),
        })?;
        hidlins_core::Vault::open_from_bytes(&bytes, &current_master, keyfile)?;

        let replacement_config = state
            .registry
            .get(&vault_name)
            .and_then(hidlins_sync::config::local::LocalSyncConfig::from_vault_entry)
            .map(|mut config| {
                config
                    .rewrap_identity(&vault_name, &current_master, &new_master)
                    .map(|()| config)
            })
            .transpose()
            .map_err(|_| HidlinsApiError::LocalSyncConfiguration)?;

        {
            let vault = state
                .vault
                .as_mut()
                .ok_or_else(|| HidlinsApiError::Internal {
                    context: "vault ownership changed during password rotation".to_string(),
                })?;
            vault.change_master_password(&current_master, &new_master)?;
            if let Err(error) = vault.save() {
                let _ = vault.change_master_password(&new_master, &current_master);
                return Err(error.into());
            }
        }

        if let Some(config) = replacement_config {
            if config.persist(&mut state.registry, &vault_name).is_err() {
                // Restore the old KDBX credential. If this rollback itself
                // fails, fail closed: no credentials remain usable in the
                // session and the caller must recover from the atomic file.
                let rollback_failed = if let Some(vault) = state.vault.as_mut() {
                    vault
                        .change_master_password(&new_master, &current_master)
                        .and_then(|()| vault.save())
                        .is_err()
                } else {
                    true
                };
                if rollback_failed {
                    state.do_lock();
                    return Err(HidlinsApiError::Internal {
                        context: "password rotation rollback failed; session locked".to_string(),
                    });
                }
                return Err(HidlinsApiError::LocalSyncConfiguration);
            }
        }

        let old_keyfile = state
            .credentials
            .as_mut()
            .and_then(|value| value.keyfile.take());
        state.credentials = Some(SessionCredentials {
            master: new_master,
            keyfile: old_keyfile,
        });
        Ok(())
    }
}

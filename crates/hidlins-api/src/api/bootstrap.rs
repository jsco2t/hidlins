//! Vault credential rotation coordinated with sealed local-sync identity.

use hidlins_core::MasterPassword;
use hidlins_sync::config::local::SYNC_KEY;

use super::session::{AppSession, SessionCredentials};
use crate::error::HidlinsApiError;

impl AppSession {
    /// Change the vault password and rewrap (never replace) its local Noise identity.
    pub fn change_master_password(
        &self,
        current: String,
        new_password: String,
    ) -> Result<(), HidlinsApiError> {
        self.change_password_for_target(None, current, new_password)
    }

    /// Rotate only when the selected registered vault is currently unlocked.
    /// The target check and credential rotation share the session mutex, so a
    /// stale UI action cannot rotate another vault after a lock or switch.
    #[allow(clippy::needless_pass_by_value)]
    pub fn change_vault_master_password(
        &self,
        vault_name: String,
        current: String,
        new_password: String,
    ) -> Result<(), HidlinsApiError> {
        self.change_password_for_target(Some(&vault_name), current, new_password)
    }

    #[allow(clippy::too_many_lines)]
    fn change_password_for_target(
        &self,
        selected: Option<&str>,
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
        if let Some(selected) = selected {
            if state
                .registry
                .get(selected)
                .is_none_or(|entry| entry.path != vault_path)
            {
                return Err(HidlinsApiError::InvalidInput {
                    field: "vault".to_string(),
                    reason: "unlock the selected vault before changing its password".to_string(),
                });
            }
        }
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

        let sync_value = state
            .registry
            .get(&vault_name)
            .and_then(|entry| entry.extra.get(SYNC_KEY))
            .cloned();
        let replacement = sync_value
            .as_ref()
            .map(|old_value| {
                let mut config = state
                    .registry
                    .get(&vault_name)
                    .and_then(hidlins_sync::config::local::LocalSyncConfig::from_vault_entry)
                    .ok_or(HidlinsApiError::LocalSyncConfiguration)?;
                config
                    .rewrap_identity(&vault_name, &current_master, &new_master)
                    .map_err(|_| HidlinsApiError::LocalSyncConfiguration)?;
                let replacement = toml::Value::try_from(config)
                    .map_err(|_| HidlinsApiError::LocalSyncConfiguration)?;
                Ok::<_, HidlinsApiError>((old_value.clone(), replacement))
            })
            .transpose()?;

        let result = if let Some((old_value, replacement)) = replacement {
            let state = &mut *state;
            let vault = state
                .vault
                .as_mut()
                .ok_or_else(|| HidlinsApiError::Internal {
                    context: "vault ownership changed during password rotation".to_string(),
                })?;
            state.registry.change_master_password_and_extra(
                vault,
                &vault_name,
                &current_master,
                &new_master,
                SYNC_KEY,
                &old_value,
                &replacement,
            )
        } else {
            state
                .vault
                .as_mut()
                .ok_or_else(|| HidlinsApiError::Internal {
                    context: "vault ownership changed during password rotation".to_string(),
                })?
                .change_master_password(&current_master, &new_master)
        };
        if let Err(error) = result {
            // The transaction may already have committed one durable file.
            // Discard all credential-bearing in-memory state and let startup
            // recovery finish from encrypted stages and the non-secret marker.
            state.do_lock();
            return Err(error.into());
        }

        // The same public identity now has a different credential wrapping.
        // Pending confirmation must not persist its old sealed configuration.
        state.pairing_api.cancel_all();
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

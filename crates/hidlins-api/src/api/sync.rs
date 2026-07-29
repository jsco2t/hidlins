//! Sync API — configure, status, sync-now with worker-thread model.
//!
//! `sync_now` releases the session mutex for network I/O using the
//! same three-phase pattern as `unlock_inner`: claim (move vault +
//! registry out, set `syncing`) → run sync without the mutex →
//! commit or discard based on `lock_pending`. While `syncing` is set,
//! all vault-touching queries and mutations return `VaultBusySyncing`.
//! The worker body is wrapped in `catch_unwind` so a panic emits a
//! `Failed` error without poisoning the session mutex.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use hidlins_sync::{CredentialSource, S3Config, Sync, SyncConfig, SyncOptions};

use super::session::AppSession;
use crate::dto::{S3ConfigDto, SyncOutcomeDto, SyncStatusDto};
use crate::error::HidlinsApiError;

#[allow(clippy::needless_pass_by_value)]
impl AppSession {
    pub fn configure_sync(&self, cfg: S3ConfigDto) -> Result<(), HidlinsApiError> {
        let plaintext = zeroize::Zeroizing::new(cfg.secret_access_key.clone());

        let mut guard = self.lock_state();
        let state = &mut *guard;

        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }

        let vault = state.require_vault()?;
        let creds = state
            .credentials
            .as_ref()
            .ok_or(HidlinsApiError::VaultLocked)?;

        let vault_path = vault.path().to_path_buf();
        let vault_name = state
            .registry
            .list()
            .find(|e| e.path == vault_path)
            .map(|e| e.name.clone())
            .ok_or_else(|| HidlinsApiError::Internal {
                context: "unlocked vault is not registered".to_string(),
            })?;

        let encrypted = hidlins_sync::encrypt_credential(&plaintext, &creds.master)
            .map_err(|e| HidlinsApiError::from(hidlins_sync::SyncError::Auth(e)))?;

        let mut s3_config = S3Config::new(
            cfg.bucket.clone(),
            cfg.key.clone(),
            cfg.region.clone(),
            CredentialSource::RstCred1 {
                access_key_id: cfg.access_key_id.clone(),
                secret_access_key_encrypted: encrypted,
            },
        );
        if let Some(ref endpoint) = cfg.endpoint {
            s3_config.set_endpoint(Some(endpoint.clone()));
        }
        if cfg.path_style {
            s3_config.set_path_style(true);
        }

        Sync::configure_remote(&mut state.registry, &vault_name, s3_config, &creds.master)?;

        Ok(())
    }

    pub fn sync_status(&self) -> Result<SyncStatusDto, HidlinsApiError> {
        let guard = self.lock_state();
        let state = &*guard;

        let in_flight = state.syncing;

        if state.syncing {
            return Ok(SyncStatusDto {
                configured: true,
                in_flight,
                last_outcome: None,
            });
        }

        let vault = state.require_vault()?;
        let vault_path = vault.path().to_path_buf();

        let configured = state
            .registry
            .list()
            .find(|e| e.path == vault_path)
            .and_then(SyncConfig::from_vault_entry)
            .is_some();

        Ok(SyncStatusDto {
            configured,
            in_flight,
            last_outcome: None,
        })
    }

    /// Run a sync cycle using the three-phase protocol.
    ///
    /// Phase 1 (short guard): validate, resolve vault name, clone
    ///   credentials, move vault + registry out, set `syncing = true`.
    /// Phase 2 (no guard): run `Sync::sync_now` wrapped in
    ///   `catch_unwind`. The session mutex is NOT held across the
    ///   network round-trips.
    /// Phase 3 (short guard): if `lock_pending` was set during phase 2,
    ///   discard the returned vault + registry and credentials, lock
    ///   the session. On panic, also lock (vault may be corrupted).
    ///   On success, restore vault + registry and clear the TOTP cache
    ///   (sync may have merged changed OTP fields).
    ///
    /// # Panics
    ///
    /// Internal `expect` for state that was validated in the same
    /// critical section — unreachable in normal operation.
    #[allow(clippy::too_many_lines)]
    pub fn sync_now(&self) -> Result<SyncOutcomeDto, HidlinsApiError> {
        // Phase 1: claim under a short guard.
        //
        // Resolve vault_name BEFORE taking the vault out, so the `?`
        // error path never leaves the session with a missing vault.
        let (mut vault, mut registry, master_clone, keyfile_clone, vault_name) = {
            let mut state = self.lock_state();

            if state.syncing {
                return Err(HidlinsApiError::VaultBusySyncing);
            }
            if state.vault.is_none() {
                return Err(HidlinsApiError::VaultLocked);
            }
            let creds = state
                .credentials
                .as_ref()
                .ok_or(HidlinsApiError::VaultLocked)?;

            // Resolve vault name while vault is still in state (no take yet).
            let vault_path = state
                .vault
                .as_ref()
                .expect("checked above")
                .path()
                .to_path_buf();
            let vault_name = state
                .registry
                .list()
                .find(|e| e.path == vault_path)
                .map(|e| e.name.clone())
                .ok_or_else(|| HidlinsApiError::Internal {
                    context: "unlocked vault is not registered".to_string(),
                })?;

            // Clone credential material BEFORE moving the vault out.
            let master_clone = hidlins_core::MasterPassword::new(
                String::from_utf8_lossy(creds.master.as_bytes()).into_owned(),
            );
            let keyfile_clone = creds.keyfile.as_ref().map(|kf| match kf {
                hidlins_core::Keyfile::Path(p) => hidlins_core::Keyfile::Path(p.clone()),
                hidlins_core::Keyfile::Bytes(b) => hidlins_core::Keyfile::Bytes(b.clone()),
            });

            // Now safe to take — all fallible operations above succeeded.
            let vault = state.vault.take().expect("checked above");

            // Move registry out so the worker can mutate it.
            let placeholder_paths = state.registry.paths().clone();
            let registry = std::mem::replace(
                &mut state.registry,
                hidlins_core::VaultRegistry::with_paths(placeholder_paths),
            );

            state.syncing = true;
            state.push_sync_event(crate::dto::SyncEvent::Started);
            (vault, registry, master_clone, keyfile_clone, vault_name)
        };
        // Session lock dropped — network I/O below.

        // Phase 2: sync without the session mutex, with panic containment.
        let engine = Arc::clone(&self.sync_engine);
        let sync_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            engine.sync_now(
                &mut vault,
                &vault_name,
                &mut registry,
                &master_clone,
                keyfile_clone.as_ref(),
                SyncOptions::default(),
            )
        }));

        // Phase 3: commit or discard under a short guard.
        let mut state = self.lock_state();
        state.syncing = false;
        let lock_requested = std::mem::take(&mut state.lock_pending);

        // Always clear the TOTP cache — sync may have merged changed
        // OTP fields, and the discard path needs it cleared too.
        self.clear_totp_cache();

        if lock_requested || state.dead {
            // Discard the vault (may be mutated mid-sync) but restore
            // the registry — it is structurally sound and losing it
            // would leave the session believing zero vaults are
            // registered for the rest of its lifetime.
            state.registry = registry;
            state.credentials = None;
            state
                .controller
                .lock_now(hidlins_security::OsLockReason::Manual);
            state.push_sync_event(crate::dto::SyncEvent::Failed(HidlinsApiError::VaultLocked));
            if !state.dead {
                state.push_lock_event(crate::dto::LockEvent::Locked);
            }
            return Err(HidlinsApiError::VaultLocked);
        }

        match sync_result {
            Ok(Ok(outcome)) => {
                let dto = crate::dto::sync_outcome_from_core(&outcome);
                state.push_sync_event(crate::dto::SyncEvent::Done(dto.clone()));
                state.vault = Some(vault);
                state.registry = registry;
                Ok(dto)
            }
            Ok(Err(sync_err)) => {
                let api_err = HidlinsApiError::from(sync_err);
                state.push_sync_event(crate::dto::SyncEvent::Failed(api_err.clone()));
                state.vault = Some(vault);
                state.registry = registry;
                Err(api_err)
            }
            Err(_panic) => {
                // Worker panicked — vault may be in an inconsistent
                // state (sync mutates in place). Do NOT restore it;
                // lock the session so the user re-opens from disk.
                //
                // Reload the registry from disk rather than restoring
                // the in-memory copy (sync may have partially mutated
                // it before panicking). An empty placeholder would
                // make list_vaults() return nothing for the rest of
                // the process lifetime.
                if let Ok(reloaded) =
                    hidlins_core::VaultRegistry::load(state.registry.paths().clone())
                {
                    state.registry = reloaded;
                }
                let api_err = HidlinsApiError::Internal {
                    context: "sync worker panicked".to_string(),
                };
                // Push Failed so a sync-events subscriber that saw
                // Started always receives a terminal — symmetric with
                // the Ok(Err) arm.
                state.push_sync_event(crate::dto::SyncEvent::Failed(api_err.clone()));
                state.credentials = None;
                state
                    .controller
                    .lock_now(hidlins_security::OsLockReason::Manual);
                state.push_lock_event(crate::dto::LockEvent::Locked);
                Err(api_err)
            }
        }
    }

    pub fn clear_sync_config(&self, name: String) -> Result<(), HidlinsApiError> {
        let mut guard = self.lock_state();
        let state = &mut *guard;

        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }

        state.registry.update_registered_extra(&name, |extra| {
            extra.remove("sync");
        })?;

        Ok(())
    }
}

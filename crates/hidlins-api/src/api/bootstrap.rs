use std::path::PathBuf;

use hidlins_core::{Keyfile, MasterPassword, RegisteredVault};
use hidlins_sync::{CredentialSource, S3Config, Sync, SyncConfig, SyncError, SyncOptions};
use secrecy::ExposeSecret;
use zeroize::Zeroizing;

use super::session::{AppSession, KeyfileRef};
use crate::dto::{S3ConfigDto, VaultSummary};
use crate::error::HidlinsApiError;

fn reencrypt_sync_credentials(
    registry: &mut hidlins_core::VaultRegistry,
    name: &str,
    old_master: &MasterPassword,
    new_creds: &super::session::SessionCredentials,
) -> Result<(), HidlinsApiError> {
    let Some(entry) = registry.get(name) else {
        return Ok(());
    };
    let Some(sync_cfg) = SyncConfig::from_vault_entry(entry) else {
        return Ok(());
    };
    let Some(ref s3) = sync_cfg.s3 else {
        return Ok(());
    };
    if !matches!(s3.credentials(), CredentialSource::RstCred1 { .. }) {
        return Ok(());
    }

    let resolved =
        hidlins_sync::auth::resolve(s3.credentials(), old_master, &hidlins_sync::SystemEnvSource)
            .map_err(|e| HidlinsApiError::Io {
            context: format!(
            "password changed but S3 credential decryption failed — re-enter S3 credentials: {e}"
        ),
        })?;

    let plaintext: &str = resolved.secret_access_key.expose_secret();
    let re_encrypted = hidlins_sync::encrypt_credential(plaintext, &new_creds.master)
        .map_err(|e| HidlinsApiError::Io {
            context: format!("password changed but S3 credential re-encryption failed — re-enter S3 credentials: {e}"),
        })?;

    let mut new_s3 = s3.clone();
    new_s3.set_credentials(CredentialSource::RstCred1 {
        access_key_id: resolved.access_key_id.clone(),
        secret_access_key_encrypted: re_encrypted,
    });
    Sync::configure_remote(registry, name, new_s3, &new_creds.master)?;
    Ok(())
}

impl AppSession {
    /// Change the master password (design A5, FR-004).
    ///
    /// Sequence: verify current → change vault password → REPLACE
    /// `SessionCredentials` immediately (before the registry write) →
    /// if RST-CRED-1 sync credentials exist, decrypt with old master,
    /// re-encrypt with new master, persist → registry save → vault save.
    ///
    /// On registry-write failure the vault change AND the credential
    /// replacement both stand; the error instructs re-entering S3
    /// credentials (no silent auth breakage).
    ///
    /// # Panics
    ///
    /// Panics if the vault is present but not registered (impossible via
    /// the session API).
    #[allow(clippy::needless_pass_by_value)]
    pub fn change_master_password(
        &self,
        current: String,
        new_password: String,
    ) -> Result<(), HidlinsApiError> {
        let current_master = MasterPassword::new(current);
        let new_master = MasterPassword::new(new_password);

        let mut guard = self.lock_state();
        let state = &mut *guard;

        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }

        {
            let vault = state.vault.as_mut().ok_or(HidlinsApiError::VaultLocked)?;
            vault.change_master_password(&current_master, &new_master)?;
        }

        // Get the vault path for registry lookup before credential replacement.
        let vault_path = state
            .vault
            .as_ref()
            .expect("just changed password")
            .path()
            .to_path_buf();
        let vault_name = state
            .registry
            .list()
            .find(|v| v.path == vault_path)
            .map(|v| v.name.clone());

        // Replace SessionCredentials IMMEDIATELY at vault-change success,
        // BEFORE the registry write — the session must always match the
        // vault's actual password.
        let old_keyfile = state.credentials.as_mut().and_then(|c| c.keyfile.take());
        state.credentials = Some(super::session::SessionCredentials {
            master: new_master,
            keyfile: old_keyfile,
        });

        if let Some(ref name) = vault_name {
            reencrypt_sync_credentials(
                &mut state.registry,
                name,
                &current_master,
                state.credentials.as_ref().expect("just replaced"),
            )?;
        }

        // Save registry — may fail; vault change + credential replacement
        // both stand. The error instructs re-entering S3 credentials.
        if let Err(e) = state.registry.save() {
            return Err(HidlinsApiError::Io {
                context: format!(
                    "password changed but registry save failed — re-enter S3 credentials: {e}"
                ),
            });
        }

        let vault = state.vault.as_mut().expect("just changed password");
        vault.save()?;
        Ok(())
    }

    /// Bootstrap a vault from a remote S3 target (design A3).
    ///
    /// State machine:
    /// 1. Guards (duplicate target, name/path exists)
    /// 2. Build transport from config, fetch remote object
    /// 3. Validate password/keyfile via `Vault::open_from_bytes` —
    ///    wrong password fails BEFORE any install
    /// 4. `write_atomic` install into the state dir
    /// 5. Register vault + persist pre-sealed sync config
    /// 6. One standard `sync_now` establishes synced-base pointers
    /// 7. Rollback on any failure after step 4
    #[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
    pub fn bootstrap_vault_from_remote(
        &self,
        name: String,
        cfg: S3ConfigDto,
        master_password: String,
        keyfile: Option<KeyfileRef>,
    ) -> Result<VaultSummary, HidlinsApiError> {
        // Take ownership of every inbound secret before any fallible work so
        // early validation/guard returns wipe the original DTO and password.
        let cfg = Zeroizing::new(cfg);
        let master = MasterPassword::new(master_password);
        let kf = keyfile.map(|kr| match kr {
            KeyfileRef::Path(p) => Keyfile::Path(PathBuf::from(p)),
            KeyfileRef::Bytes(b) => Keyfile::Bytes(b),
        });
        super::vaults::validate_vault_name(&name)?;

        // Step 1: guards (under session lock)
        let vault_path;
        let s3_config;
        {
            let state = self.lock_state();
            let paths = state.registry.paths().clone();

            vault_path = paths.state_dir().join(format!("{name}.kdbx"));
            if vault_path.exists() {
                return Err(HidlinsApiError::PathExists {
                    path: vault_path.display().to_string(),
                });
            }
            if state.registry.get(&name).is_some() {
                return Err(HidlinsApiError::PathExists { path: name });
            }

            let dup = SyncConfig::find_duplicate_target(
                &state.registry,
                cfg.endpoint.as_deref(),
                &cfg.bucket,
                &cfg.key,
                None,
            );
            if let Some(existing) = dup {
                return Err(HidlinsApiError::SyncDuplicateTarget {
                    existing_vault: existing,
                });
            }

            // Seal credentials
            let encrypted_secret =
                hidlins_sync::encrypt_credential(&cfg.secret_access_key, &master)
                    .map_err(|e| HidlinsApiError::from(SyncError::Auth(e)))?;

            let cred = CredentialSource::RstCred1 {
                access_key_id: cfg.access_key_id.clone(),
                secret_access_key_encrypted: encrypted_secret,
            };
            let mut config = S3Config::new(
                cfg.bucket.clone(),
                cfg.key.clone(),
                cfg.region.clone(),
                cred,
            );
            if let Some(ref ep) = cfg.endpoint {
                config.set_endpoint(Some(ep.clone()));
            }
            config.set_path_style(cfg.path_style);
            s3_config = config;
        }
        // Session lock dropped — network I/O below.

        // Step 2: fetch remote via the sync crate's transport layer
        let snapshot = Sync::fetch_remote_snapshot(&s3_config, &master)?.ok_or(
            HidlinsApiError::SyncRemoteUnreachable {
                endpoint: cfg.endpoint.clone(),
            },
        )?;

        // Step 3: validate password BEFORE install
        let _db = hidlins_core::Vault::open_from_bytes(&snapshot.bytes, &master, kf.as_ref())?;

        // Step 4: atomic write install (interrupted download leaves no
        // partial file — write_atomic uses write-then-rename)
        hidlins_core::atomic::write_atomic(&vault_path, &snapshot.bytes)?;

        // Steps 5–6: register + sync config + establishing sync
        // Re-acquire session lock for registry mutations.
        let mut guard = self.lock_state();
        let state = &mut *guard;

        // Re-validate name guard after re-acquiring lock (TOCTOU defense).
        if state.registry.get(&name).is_some() {
            let _ = std::fs::remove_file(&vault_path);
            return Err(HidlinsApiError::PathExists { path: name });
        }

        let result = (|| -> Result<VaultSummary, HidlinsApiError> {
            let entry = RegisteredVault {
                name: name.clone(),
                path: vault_path.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
                keyfile_path: kf.as_ref().and_then(|k| k.path().map(PathBuf::from)),
                extra: toml::Table::default(),
            };
            state.registry.register(entry)?;

            Sync::configure_remote(&mut state.registry, &name, s3_config.clone(), &master)?;
            state.registry.save()?;

            // Step 6: open the installed vault and run one sync to
            // establish the synced-base pointers.
            let mut vault = hidlins_core::Vault::open(&vault_path, &master, kf.as_ref())?;
            let _outcome = Sync::sync_now(
                &mut vault,
                &name,
                &mut state.registry,
                &master,
                kf.as_ref(),
                SyncOptions::default(),
            )?;

            Ok(VaultSummary {
                name: name.clone(),
                path: vault_path.display().to_string(),
                has_keyfile: kf.is_some(),
                has_sync: true,
            })
        })();

        match result {
            Ok(summary) => Ok(summary),
            Err(e) => {
                // Step 7: rollback — remove file, deregister, persist
                let _ = std::fs::remove_file(&vault_path);
                let _ = state.registry.deregister(&name, false);
                let _ = state.registry.save();
                Err(e)
            }
        }
    }
}

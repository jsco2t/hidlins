use hidlins_core::Uuid;

use super::session::AppSession;
use crate::dto::{CopyField, RevealField};
use crate::error::HidlinsApiError;

#[cfg(feature = "desktop")]
const CLIPBOARD_AUTO_CLEAR_SECS: u64 = 30;

impl AppSession {
    #[allow(clippy::needless_pass_by_value)]
    pub fn reveal_field(
        &self,
        uuid: String,
        field: RevealField,
    ) -> Result<String, HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let state = self.lock_state();
        let vault = state.require_vault()?;

        match field {
            RevealField::Password => {
                let entry = vault.get_entry(id)?;
                Ok(entry.password().to_string())
            }
            RevealField::CustomField(name) => {
                let entry = vault.get_entry(id)?;
                entry.custom_field(&name).map(String::from).ok_or_else(|| {
                    HidlinsApiError::InvalidInput {
                        field: "custom_field".to_string(),
                        reason: format!("field not found: {name}"),
                    }
                })
            }
            RevealField::TotpUri => {
                let cache = match self.totp_cache.read() {
                    Ok(guard) => guard,
                    Err(poison) => {
                        drop(poison);
                        self.clear_totp_cache();
                        return Err(HidlinsApiError::Internal {
                            context: "totp not cached".to_string(),
                        });
                    }
                };
                cache
                    .get(&id)
                    .map(|snap| snap.secret_uri().to_string())
                    .ok_or_else(|| HidlinsApiError::Internal {
                        context: "totp not cached".to_string(),
                    })
            }
        }
    }

    #[cfg(feature = "desktop")]
    #[allow(clippy::needless_pass_by_value)]
    pub fn copy_entry_field(&self, uuid: String, field: CopyField) -> Result<(), HidlinsApiError> {
        let text = self.resolve_copy_field(&uuid, &field)?;
        self.clipboard_copy(text)
    }

    #[cfg(not(feature = "desktop"))]
    #[allow(clippy::needless_pass_by_value)]
    pub fn take_secret_for_clipboard(
        &self,
        uuid: String,
        field: CopyField,
    ) -> Result<String, HidlinsApiError> {
        self.resolve_copy_field(&uuid, &field)
    }

    fn resolve_copy_field(&self, uuid: &str, field: &CopyField) -> Result<String, HidlinsApiError> {
        let id = parse_uuid(uuid)?;
        let state = self.lock_state();
        let vault = state.require_vault()?;

        match field {
            CopyField::Username => {
                let view = vault.get_entry(id)?;
                Ok(view.username().to_string())
            }
            CopyField::Password => {
                let view = vault.get_entry(id)?;
                Ok(view.password().to_string())
            }
            CopyField::TotpCode => {
                drop(state);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let Ok(cache) = self.totp_cache.read() else {
                    self.clear_totp_cache();
                    return Err(HidlinsApiError::Internal {
                        context: "totp not cached".to_string(),
                    });
                };
                let snap = cache.get(&id).ok_or_else(|| HidlinsApiError::Internal {
                    context: "totp not cached".to_string(),
                })?;
                Ok(snap.totp.code_at(now))
            }
            CopyField::CustomField(name) => {
                let entry = vault.get_entry(id)?;
                entry.custom_field(name).map(String::from).ok_or_else(|| {
                    HidlinsApiError::InvalidInput {
                        field: "custom_field".to_string(),
                        reason: format!("field not found: {name}"),
                    }
                })
            }
        }
    }

    #[cfg(feature = "desktop")]
    fn clipboard_copy(&self, text: String) -> Result<(), HidlinsApiError> {
        if let Some(ref port) = self.clipboard {
            port.copy_with_autoclear(
                text,
                std::time::Duration::from_secs(CLIPBOARD_AUTO_CLEAR_SECS),
            )?;
        }
        Ok(())
    }
}

fn parse_uuid(s: &str) -> Result<Uuid, HidlinsApiError> {
    s.parse::<Uuid>()
        .map_err(|_| HidlinsApiError::InvalidInput {
            field: "uuid".to_string(),
            reason: "invalid UUID".to_string(),
        })
}

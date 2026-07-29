use hidlins_core::{EntryBuilder, EntryKind, Tag, Totp, Uuid};

use super::session::{AppSession, TotpSnapshot};
use crate::dto::{
    AttachmentMeta, EntryDraftDto, EntryEditDto, EntryKindDto, GroupDeleteBehaviorDto,
    HistorySummary, VaultTree,
};
use crate::error::HidlinsApiError;

fn parse_uuid(s: &str) -> Result<Uuid, HidlinsApiError> {
    s.parse::<Uuid>()
        .map_err(|_| HidlinsApiError::InvalidInput {
            field: "uuid".to_string(),
            reason: "invalid UUID".to_string(),
        })
}

/// Convert the `GroupDeleteBehaviorDto` to the core enum.
fn delete_behavior(dto: GroupDeleteBehaviorDto) -> hidlins_core::GroupDeleteBehavior {
    match dto {
        GroupDeleteBehaviorDto::Refuse => hidlins_core::GroupDeleteBehavior::Refuse,
        GroupDeleteBehaviorDto::Recurse => hidlins_core::GroupDeleteBehavior::Recurse,
    }
}

#[allow(clippy::needless_pass_by_value)] // frb FFI passes owned DTOs
impl AppSession {
    pub fn vault_tree(&self) -> Result<VaultTree, HidlinsApiError> {
        let state = self.lock_state();
        let vault = state.require_vault()?;
        Ok(crate::dto::vault_tree_from_database(
            vault.database(),
            chrono::Utc::now(),
        ))
    }

    pub fn entry_detail(&self, uuid: String) -> Result<crate::dto::EntryDetail, HidlinsApiError> {
        let id = parse_uuid(&uuid)?;

        // Extract everything needed from the vault under one guard scope,
        // so the mutex is released at a single predictable point regardless
        // of whether this entry has TOTP data.
        let (detail, totp_data) = {
            let state = self.lock_state();
            let vault = state.require_vault()?;
            let view = vault.get_entry(id)?;
            let detail = crate::dto::entry_detail_from_view(&view);
            let totp = if view.kind() == EntryKind::Totp {
                view.custom_field(hidlins_core::fields::OTP)
                    .and_then(|uri| {
                        Totp::from_otpauth_uri(uri)
                            .ok()
                            .map(|totp| (totp, uri.to_string()))
                    })
            } else {
                None
            };
            (detail, totp)
        }; // state guard dropped here unconditionally

        // Populate the TOTP cache outside the session mutex.
        if let Some((totp, uri)) = totp_data {
            match self.totp_cache.write() {
                Ok(mut cache) => {
                    cache.insert(id, TotpSnapshot::new(totp, uri));
                }
                Err(poison) => {
                    let mut cache = poison.into_inner();
                    cache.clear();
                    cache.insert(id, TotpSnapshot::new(totp, uri));
                }
            }
        }

        Ok(detail)
    }

    pub fn create_entry(
        &self,
        group: String,
        mut draft: EntryDraftDto,
    ) -> Result<String, HidlinsApiError> {
        let group_uuid = parse_uuid(&group)?;

        let builder = match draft.kind {
            EntryKindDto::Credential => EntryBuilder::credential(&draft.title),
            EntryKindDto::SecureNote => EntryBuilder::secure_note(&draft.title),
            EntryKindDto::Totp => {
                let uri =
                    draft
                        .totp_uri
                        .as_deref()
                        .ok_or_else(|| HidlinsApiError::InvalidInput {
                            field: "totp_uri".to_string(),
                            reason: "TOTP entries require an otpauth URI".to_string(),
                        })?;
                EntryBuilder::totp(&draft.title, uri)?
            }
        };

        let mut builder = builder;
        if let Some(ref u) = draft.username {
            builder = builder.username(u.as_str());
        }
        if let Some(ref p) = draft.password {
            builder = builder.password(p.as_str());
        }
        if let Some(ref u) = draft.url {
            builder = builder.url(u.as_str());
        }
        if let Some(ref n) = draft.notes {
            builder = builder.notes(n.as_str());
        }
        for tag_str in &draft.tags {
            let tag = Tag::from(tag_str.clone())?;
            builder = builder.tag(tag);
        }
        for cf in &draft.custom_fields {
            builder = builder.custom_field(cf.name.as_str(), cf.value.as_str(), cf.protected);
        }

        let entry_draft = builder.build();

        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        let new_uuid = vault.add_entry(group_uuid, entry_draft)?;
        vault.save()?;

        zeroize::Zeroize::zeroize(&mut draft);
        drop(state);
        self.maybe_sync_after_save();

        Ok(new_uuid.hyphenated().to_string())
    }

    pub fn update_entry(
        &self,
        uuid: String,
        mut edit: EntryEditDto,
    ) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;

        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;

        // Collect existing custom-field names before the mutable closure so
        // the immutable borrow on the vault is dropped first.
        let existing_custom_fields: Vec<String> = if edit.custom_fields.is_some() {
            let view = vault.get_entry(id)?;
            view.custom_field_names()
                .into_iter()
                .map(str::to_string)
                .collect()
        } else {
            Vec::new()
        };

        // Validate the TOTP URI before entering the update closure so a
        // malformed URI does not leave a half-applied edit.
        if let Some(ref uri) = edit.totp_uri {
            Totp::from_otpauth_uri(uri)?;
        }

        vault.update_entry(id, |view| {
            if let Some(ref title) = edit.title {
                view.set_title(title.as_str());
            }
            if let Some(ref username) = edit.username {
                view.set_username(username.as_str());
            }
            if let Some(ref password) = edit.password {
                view.set_password(password.as_str());
            }
            if let Some(ref url) = edit.url {
                view.set_url(url.as_str());
            }
            if let Some(ref notes) = edit.notes {
                view.set_notes(notes.as_str());
            }
            if let Some(ref tags) = edit.tags {
                let core_tags: Vec<Tag> = tags
                    .iter()
                    .map(|t| Tag::from(t.clone()))
                    .collect::<Result<_, _>>()?;
                view.set_tags(core_tags);
            }

            // Replace custom fields: remove old ones not in the new set,
            // then set the new ones. Exclude the OTP field from removal —
            // it is managed by the totp_uri path below.
            if let Some(ref custom_fields) = edit.custom_fields {
                for name in &existing_custom_fields {
                    if name == hidlins_core::fields::OTP {
                        continue;
                    }
                    if !custom_fields.iter().any(|f| f.name == *name) {
                        view.remove_custom_field(name);
                    }
                }
                for field in custom_fields {
                    view.set_custom_field(
                        field.name.as_str(),
                        field.value.as_str(),
                        field.protected,
                    );
                }
            }

            if let Some(ref uri) = edit.totp_uri {
                // Already validated above; set the protected OTP field.
                view.set_custom_field(hidlins_core::fields::OTP, uri.as_str(), true);
            }

            Ok(())
        })?;

        vault.save()?;

        zeroize::Zeroize::zeroize(&mut edit);
        drop(state);
        self.invalidate_totp(&id);
        self.maybe_sync_after_save();
        Ok(())
    }

    pub fn delete_entry(&self, uuid: String) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        vault.delete_entry(id)?;
        vault.save()?;
        drop(state);
        self.invalidate_totp(&id);
        self.maybe_sync_after_save();
        Ok(())
    }

    pub fn purge_entry(&self, uuid: String) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        vault.purge_entry(id)?;
        vault.save()?;
        drop(state);
        self.invalidate_totp(&id);
        self.maybe_sync_after_save();
        Ok(())
    }

    pub fn move_entry(&self, uuid: String, group: String) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let group_uuid = parse_uuid(&group)?;
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        vault.move_entry(id, group_uuid)?;
        vault.save()?;
        drop(state);
        self.maybe_sync_after_save();
        Ok(())
    }

    pub fn entry_history(&self, uuid: String) -> Result<Vec<HistorySummary>, HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let state = self.lock_state();
        let vault = state.require_vault()?;
        let view = vault.get_entry(id)?;
        Ok(crate::dto::history_summaries_from_view(&view))
    }

    pub fn create_group(&self, parent: String, name: String) -> Result<String, HidlinsApiError> {
        let parent_uuid = parse_uuid(&parent)?;
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        let new_uuid = vault.create_group(parent_uuid, &name)?;
        vault.save()?;
        drop(state);
        self.maybe_sync_after_save();
        Ok(new_uuid.hyphenated().to_string())
    }

    pub fn rename_group(&self, uuid: String, name: String) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        vault.rename_group(id, &name)?;
        vault.save()?;
        drop(state);
        self.maybe_sync_after_save();
        Ok(())
    }

    pub fn move_group(&self, uuid: String, parent: String) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let parent_uuid = parse_uuid(&parent)?;
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        vault.move_group(id, parent_uuid)?;
        vault.save()?;
        drop(state);
        self.maybe_sync_after_save();
        Ok(())
    }

    pub fn delete_group(
        &self,
        uuid: String,
        behavior: GroupDeleteBehaviorDto,
    ) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        vault.delete_group(id, delete_behavior(behavior))?;
        vault.save()?;
        drop(state);
        self.maybe_sync_after_save();
        Ok(())
    }

    pub fn list_tags(&self) -> Result<Vec<String>, HidlinsApiError> {
        let state = self.lock_state();
        let vault = state.require_vault()?;
        let tags = vault
            .list_tags()
            .into_iter()
            .map(|t| t.as_str().to_string())
            .collect();
        Ok(tags)
    }

    pub fn set_expiration(&self, uuid: String, epoch_secs: i64) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let when = chrono::DateTime::from_timestamp(epoch_secs, 0).ok_or_else(|| {
            HidlinsApiError::InvalidInput {
                field: "epoch_secs".to_string(),
                reason: "timestamp is out of representable range".to_string(),
            }
        })?;
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        vault.set_expiration(id, when)?;
        vault.save()?;
        drop(state);
        self.maybe_sync_after_save();
        Ok(())
    }

    pub fn clear_expiration(&self, uuid: String) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        vault.clear_expiration(id)?;
        vault.save()?;
        drop(state);
        self.maybe_sync_after_save();
        Ok(())
    }

    pub fn list_attachments(&self, uuid: String) -> Result<Vec<AttachmentMeta>, HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let state = self.lock_state();
        let vault = state.require_vault()?;
        let attachments = vault
            .list_attachments(id)?
            .into_iter()
            .map(|a| AttachmentMeta {
                name: a.name,
                size_bytes: a.size_bytes,
            })
            .collect();
        Ok(attachments)
    }

    #[cfg(feature = "desktop")]
    pub fn add_attachment(&self, uuid: String, source_path: String) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;

        let file_size = std::fs::metadata(&source_path)
            .map_err(|e| HidlinsApiError::Io {
                context: format!("reading attachment source: {e}"),
            })?
            .len();
        if file_size > hidlins_core::MAX_ATTACHMENT_BYTES_UPPER_BOUND {
            return Err(HidlinsApiError::InvalidInput {
                field: "attachment".to_string(),
                reason: format!(
                    "{file_size} bytes exceeds maximum of {} bytes",
                    hidlins_core::MAX_ATTACHMENT_BYTES_UPPER_BOUND
                ),
            });
        }

        let bytes = zeroize::Zeroizing::new(std::fs::read(&source_path).map_err(|e| {
            HidlinsApiError::Io {
                context: format!("reading attachment source: {e}"),
            }
        })?);
        let file_name = std::path::Path::new(&source_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("attachment");
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        vault.add_attachment(id, file_name, &bytes)?;
        vault.save()?;
        drop(bytes);
        drop(state);
        self.maybe_sync_after_save();
        Ok(())
    }

    #[cfg(feature = "desktop")]
    pub fn remove_attachment(&self, uuid: String, key: String) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let mut state = self.lock_state();
        let vault = state.require_vault_mut()?;
        vault.remove_attachment(id, &key)?;
        vault.save()?;
        drop(state);
        self.maybe_sync_after_save();
        Ok(())
    }

    #[cfg(feature = "desktop")]
    pub fn save_attachment_to(
        &self,
        uuid: String,
        key: String,
        dest_path: String,
    ) -> Result<(), HidlinsApiError> {
        let id = parse_uuid(&uuid)?;
        let state = self.lock_state();
        let vault = state.require_vault()?;
        let bytes = vault.get_attachment(id, &key)?;
        drop(state);
        hidlins_core::atomic::write_atomic(std::path::Path::new(&dest_path), &bytes)?;
        Ok(())
    }
}

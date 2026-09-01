//! Domain-action adapters from edit-form snapshots to core mutations.

use hidlins_core::{EntryBuilder, EntryKind, Uuid, Vault, VaultError};

use crate::overlay::edit::EditValues;

pub(crate) fn add(vault: &mut Vault, group: Uuid, values: &EditValues) -> Result<Uuid, VaultError> {
    let mut builder = match values.kind {
        EntryKind::Credential => EntryBuilder::credential(values.title.clone()),
        EntryKind::SecureNote => EntryBuilder::secure_note(values.title.clone()),
        EntryKind::Totp => EntryBuilder::totp(values.title.clone(), &values.totp_uri)?,
    };
    if matches!(values.kind, EntryKind::Credential | EntryKind::Totp) {
        builder = builder
            .username(values.username.clone())
            .url(values.url.clone());
    }
    if values.kind == EntryKind::Credential {
        builder = builder.password(values.password.as_str());
    }
    builder = builder
        .notes(values.notes.clone())
        .tags(values.tags.clone());
    for (name, value, protected) in &values.custom {
        builder = builder.custom_field(name.clone(), value.as_str().to_string(), *protected);
    }
    vault.add_entry(group, builder.build())
}

pub(crate) fn update(vault: &mut Vault, uuid: Uuid, values: &EditValues) -> Result<(), VaultError> {
    vault.update_entry(uuid, |view| {
        view.set_title(values.title.clone());
        match values.kind {
            EntryKind::Credential => {
                view.set_username(values.username.clone());
                view.set_password(values.password.as_str());
                view.set_url(values.url.clone());
            }
            EntryKind::Totp => {
                view.set_username(values.username.clone());
                view.set_url(values.url.clone());
            }
            EntryKind::SecureNote => {}
        }
        view.set_notes(values.notes.clone());
        view.set_tags(values.tags.clone());
        for (name, value, protected) in &values.custom {
            view.set_custom_field(name.clone(), value.as_str().to_string(), *protected);
        }
        for name in &values.removed_custom {
            view.remove_custom_field(name);
        }
        Ok(())
    })
}

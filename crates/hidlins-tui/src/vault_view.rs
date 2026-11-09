//! Borrowed presentation queries over the core's distinct read/write handles.

#[cfg(test)]
use std::path::Path;

use chrono::{DateTime, Utc};
use hidlins_core::{
    EntrySummary, EntryView, GroupSummary, GroupView, SearchOptions, SearchResult, Uuid, Vault,
    VaultError, VaultReadOnly,
};

/// Contains only immutable borrows; mutation and worker ownership stay separate.
#[derive(Clone, Copy)]
pub(crate) enum VaultView<'a> {
    Writable(&'a Vault),
    ReadOnly(&'a VaultReadOnly),
}

impl<'a> From<&'a Vault> for VaultView<'a> {
    fn from(vault: &'a Vault) -> Self {
        Self::Writable(vault)
    }
}

impl<'a> From<&'a VaultReadOnly> for VaultView<'a> {
    fn from(vault: &'a VaultReadOnly) -> Self {
        Self::ReadOnly(vault)
    }
}

impl<'a> VaultView<'a> {
    #[cfg(test)]
    pub(crate) fn backing_path(self) -> &'a Path {
        match self {
            Self::Writable(vault) => vault.backing_path(),
            Self::ReadOnly(vault) => vault.backing_path(),
        }
    }

    pub(crate) fn root_group_uuid(self) -> Uuid {
        match self {
            Self::Writable(vault) => vault.root_group_uuid(),
            Self::ReadOnly(vault) => vault.root_group_uuid(),
        }
    }

    pub(crate) fn group_view(self, uuid: Uuid) -> Result<GroupView<'a>, VaultError> {
        match self {
            Self::Writable(vault) => vault.group_view(uuid),
            Self::ReadOnly(vault) => vault.group_view(uuid),
        }
    }

    pub(crate) fn get_entry(self, uuid: Uuid) -> Result<EntryView<'a>, VaultError> {
        match self {
            Self::Writable(vault) => vault.get_entry(uuid),
            Self::ReadOnly(vault) => vault.get_entry(uuid),
        }
    }

    pub(crate) fn entry_summaries(self) -> Vec<EntrySummary> {
        match self {
            Self::Writable(vault) => vault.entry_summaries(),
            Self::ReadOnly(vault) => vault.entry_summaries(),
        }
    }

    #[cfg(test)]
    pub(crate) fn entry_uuids_by_title(self, title: &str) -> Vec<Uuid> {
        match self {
            Self::Writable(vault) => vault.entry_uuids_by_title(title),
            Self::ReadOnly(vault) => vault.entry_uuids_by_title(title),
        }
    }

    pub(crate) fn group_summaries(self) -> Vec<GroupSummary> {
        match self {
            Self::Writable(vault) => vault.group_summaries(),
            Self::ReadOnly(vault) => vault.group_summaries(),
        }
    }

    pub(crate) fn search(self, options: SearchOptions) -> Result<Vec<SearchResult>, VaultError> {
        match self {
            Self::Writable(vault) => vault.search(options),
            Self::ReadOnly(vault) => vault.search(options),
        }
    }

    pub(crate) fn totp(self, uuid: Uuid) -> Result<(String, u64), VaultError> {
        match self {
            Self::Writable(vault) => vault.totp(uuid),
            Self::ReadOnly(vault) => vault.totp(uuid),
        }
    }

    pub(crate) fn is_expired(self, uuid: Uuid, now: DateTime<Utc>) -> Result<bool, VaultError> {
        match self {
            Self::Writable(vault) => vault.is_expired(uuid, now),
            Self::ReadOnly(vault) => vault.is_expired(uuid, now),
        }
    }
}

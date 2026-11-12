//! Owned, presentation-safe query records for vault entries and groups.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use keepass::db::{EntryId, EntryRef, GroupRef};
use keepass::Database;

use crate::entry::crud::{find_entry_id, is_entry_in_recycle_bin};
use crate::{EntryKind, EntryView, Tag, Vault, VaultError, VaultReadOnly};

/// Secret-free entry metadata suitable for list, search, and navigation UIs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntrySummary {
    uuid: uuid::Uuid,
    title: String,
    group_uuid: uuid::Uuid,
    group_name: String,
    tags: Vec<Tag>,
    expires: Option<DateTime<Utc>>,
    kind: EntryKind,
    has_attachments: bool,
    recycled: bool,
}

impl EntrySummary {
    /// Return the entry UUID.
    pub fn uuid(&self) -> uuid::Uuid {
        self.uuid
    }

    /// Return the title, or an empty string when absent.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Return the containing group UUID.
    pub fn group_uuid(&self) -> uuid::Uuid {
        self.group_uuid
    }

    /// Return the containing group name.
    pub fn group_name(&self) -> &str {
        &self.group_name
    }

    /// Return validated entry tags.
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }

    /// Return the expiration timestamp when the entry expires.
    pub fn expires(&self) -> Option<DateTime<Utc>> {
        self.expires
    }

    /// Return the inferred entry kind.
    pub fn kind(&self) -> EntryKind {
        self.kind
    }

    /// Return whether the entry has one or more attachments.
    pub fn has_attachments(&self) -> bool {
        self.has_attachments
    }

    /// Return whether the entry is inside the recycle-bin subtree.
    pub fn is_recycled(&self) -> bool {
        self.recycled
    }
}

/// Owned group metadata suitable for selectors and navigation trees.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupSummary {
    uuid: uuid::Uuid,
    name: String,
    child_group_uuids: Vec<uuid::Uuid>,
    entry_uuids: Vec<uuid::Uuid>,
    recycle_bin: bool,
}

impl GroupSummary {
    /// Return the group UUID.
    pub fn uuid(&self) -> uuid::Uuid {
        self.uuid
    }

    /// Return the group name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return direct child group UUIDs in stable UUID order.
    pub fn child_group_uuids(&self) -> &[uuid::Uuid] {
        &self.child_group_uuids
    }

    /// Return direct child entry UUIDs in stable UUID order.
    pub fn entry_uuids(&self) -> &[uuid::Uuid] {
        &self.entry_uuids
    }

    /// Return whether this is the configured recycle-bin group.
    pub fn is_recycle_bin(&self) -> bool {
        self.recycle_bin
    }
}

impl Vault {
    /// Return secret-free metadata for one entry.
    pub fn entry_summary(&self, uuid: uuid::Uuid) -> Result<EntrySummary, VaultError> {
        entry_summary(self.database(), uuid)
    }

    /// Return secret-free metadata for every entry in database traversal order.
    pub fn entry_summaries(&self) -> Vec<EntrySummary> {
        entry_summaries(self.database())
    }

    /// Return UUIDs whose titles equal `title` ignoring ASCII case.
    pub fn entry_uuids_by_title(&self, title: &str) -> Vec<uuid::Uuid> {
        entry_uuids_by_title(self.database(), title)
    }

    /// Return owned metadata for every group in database traversal order.
    pub fn group_summaries(&self) -> Vec<GroupSummary> {
        group_summaries(self.database())
    }
}

impl VaultReadOnly {
    /// Return a read-only entry view without exposing the KDBX database model.
    pub fn get_entry(&self, uuid: uuid::Uuid) -> Result<EntryView<'_>, VaultError> {
        let entry_id = find_entry_id(self.database(), uuid)?;
        self.database()
            .entry(entry_id)
            .map(EntryView::new)
            .ok_or(VaultError::EntryNotFound { uuid })
    }

    /// Return secret-free metadata for one entry.
    pub fn entry_summary(&self, uuid: uuid::Uuid) -> Result<EntrySummary, VaultError> {
        entry_summary(self.database(), uuid)
    }

    /// Return secret-free metadata for every entry in database traversal order.
    pub fn entry_summaries(&self) -> Vec<EntrySummary> {
        entry_summaries(self.database())
    }

    /// Return UUIDs whose titles equal `title` ignoring ASCII case.
    pub fn entry_uuids_by_title(&self, title: &str) -> Vec<uuid::Uuid> {
        entry_uuids_by_title(self.database(), title)
    }

    /// Return owned metadata for every group in database traversal order.
    pub fn group_summaries(&self) -> Vec<GroupSummary> {
        group_summaries(self.database())
    }
}

fn entry_summary(db: &Database, uuid: uuid::Uuid) -> Result<EntrySummary, VaultError> {
    let entry_id = find_entry_id(db, uuid)?;
    let entry = db
        .entry(entry_id)
        .ok_or(VaultError::EntryNotFound { uuid })?;
    Ok(entry_summary_from_ref(
        entry,
        is_entry_in_recycle_bin(db, entry_id),
    ))
}

fn entry_summary_from_ref(entry: EntryRef<'_>, recycled: bool) -> EntrySummary {
    let uuid = entry.id().uuid();
    let (group_uuid, group_name) = {
        let parent = entry.parent();
        (parent.id().uuid(), parent.name.clone())
    };
    let view = EntryView::new(entry);
    EntrySummary {
        uuid,
        title: view.title().to_string(),
        group_uuid,
        group_name,
        tags: view.tags(),
        expires: view.expires(),
        kind: view.kind(),
        has_attachments: view.has_attachments(),
        recycled,
    }
}

fn entry_summaries(db: &Database) -> Vec<EntrySummary> {
    let recycled = recycled_entry_ids(db);
    db.iter_all_entries()
        .map(|entry| {
            let is_recycled = recycled.contains(&entry.id());
            entry_summary_from_ref(entry, is_recycled)
        })
        .collect()
}

fn recycled_entry_ids(db: &Database) -> HashSet<EntryId> {
    let mut ids = HashSet::new();
    if let Some(recycle_bin) = db.recycle_bin() {
        collect_entry_ids(&recycle_bin, &mut ids);
    }
    ids
}

fn collect_entry_ids(group: &GroupRef<'_>, ids: &mut HashSet<EntryId>) {
    ids.extend(group.entries().map(|entry| entry.id()));
    for child in group.groups() {
        collect_entry_ids(&child, ids);
    }
}

fn entry_uuids_by_title(db: &Database, title: &str) -> Vec<uuid::Uuid> {
    db.iter_all_entries()
        .filter(|entry| {
            entry
                .get(keepass::db::fields::TITLE)
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(title))
        })
        .map(|entry| entry.id().uuid())
        .collect()
}

fn group_summaries(db: &Database) -> Vec<GroupSummary> {
    let recycle_bin_uuid = db.recycle_bin().map(|group| group.id().uuid());
    db.iter_all_groups()
        .map(|group| {
            let mut child_group_uuids: Vec<_> =
                group.groups().map(|child| child.id().uuid()).collect();
            child_group_uuids.sort_unstable();
            let mut entry_uuids: Vec<_> = group.entries().map(|entry| entry.id().uuid()).collect();
            entry_uuids.sort_unstable();
            GroupSummary {
                uuid: group.id().uuid(),
                name: group.name.clone(),
                child_group_uuids,
                entry_uuids,
                recycle_bin: recycle_bin_uuid == Some(group.id().uuid()),
            }
        })
        .collect()
}

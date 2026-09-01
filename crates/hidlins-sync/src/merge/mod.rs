//! Two-way KDBX merge adapter (FR-043).
//!
//! # Why an adapter, not a hand-written engine
//!
//! `keepass-rs` ships a UUID-keyed, timestamp-ordered database merge
//! (`Database::merge`, gated behind its `_merge` feature, which
//! `hidlins-core` enables). It already implements the FR-043 reconciliation
//! we need — entry identity by UUID, newer-`last_modification` wins, and the
//! loser of a collision preserved as a KDBX history entry under the same UUID
//! — against the crate's real (flat, id-keyed) data model. Re-deriving that
//! against the same data structures is impossible from this crate: every
//! UUID-preserving insert API (`Group::add_entry_with_id`, `EntryId::from_uuid`)
//! is `pub(crate)`. So we wrap the upstream merge behind the single chokepoint
//! [`reconcile`] and pin its observable behaviour with a defensive
//! characterization suite (`tests/merge_semantics.rs`).
//!
//! This supersedes the original three-way design (ADR-005); see design
//! **ADR-008** and `kb/three-way-merge.md`.
//!
//! # No-data-loss guarantee
//!
//! [`reconcile`] folds `remote` into `local` in place. `Database::merge` alone
//! is asymmetric — it preserves the *destination*'s loser as history only when
//! that entry already carries history, and never preserves the *source*'s
//! losing current value. Because a silently dropped secret version is often
//! unrecoverable, [`reconcile`] adds a defensive backfill pass that enforces a
//! single invariant: **every pre-merge entry version, from either side, that is
//! not the merged current value survives as a history entry under the same
//! UUID.** Collisions are still resolved by newer `last_modification` (the
//! winner becomes current); the loser — whichever side it is, and regardless of
//! whether it carried prior history — is recovered into history. The backfill
//! is idempotent (a version already present is detected and not re-added, so
//! repeated syncs do not grow history).
//!
//! # Attachment handling
//!
//! Upstream `Database::merge` does not merge the attachment binary pool
//! (`merge.rs` has an explicit `TODO`). Hidlins repairs all three affected
//! surfaces here:
//!
//! - **Repaired: entries newly added from `remote`.** Upstream clones a
//!   source-only entry wholesale — including its attachment map, whose values
//!   are *remote-pool indices* — without copying the referenced bytes. Left
//!   alone, those references dangle (the attachment silently disappears on
//!   the next open) or, worse, alias an unrelated binary in the local pool
//!   (the entry shows the wrong attachment). [`reconcile`] runs a repair pass
//!   (`repair_added_entry_attachments`) that copies the referenced bytes
//!   from `remote`'s pool into `local`'s and re-points the references.
//! - **Repaired: attachment edits on entries present on both sides.** When
//!   `remote` wins a both-sides collision, upstream copies its fields but not
//!   its attachments (same pool-merge `TODO`), so an attachment added, replaced,
//!   or removed on the winning side would not reach the merged current value.
//!   [`reconcile`] runs `propagate_both_side_attachments`, which reconciles
//!   the merged entry's attachment set to the winner's (remote-wins only;
//!   compare-before-mutate; `last_modification` bumped on mutation so a re-sync
//!   is a no-op — REQUIRED because `have_entries_diverged` compares the
//!   attachment map by pool id). This is last-writer-wins at *entry*
//!   granularity: an attachment edit still loses if the other side has a newer
//!   *field* edit on the same entry.
//! - **Repaired: loser and pre-existing history versions.** Both sides are
//!   snapshotted with their resolved attachment bytes before the merge. After
//!   history merge/backfill, every matching historical version is reconciled
//!   to those bytes through a narrow, version-pinned Keepass API patch. The
//!   patch makes historical mutation history-index-aware and causes unresolved
//!   pool references to fail closed instead of producing a dereference panic.
//!   Snapshot bytes are held in [`zeroize::Zeroizing`] buffers.
//!
//! ## Remaining merge limitation
//!
//! - **Cross-device pool-id divergence can surface `Unresolvable`.** Because the
//!   repaired/added attachment gets a fresh local pool id, two devices whose
//!   pools assign the same logical attachment a different numeric id can meet at
//!   an equal timestamp that upstream's `have_entries_diverged` (which compares
//!   pool ids, not bytes) reads as diverged. There is no public API to force
//!   matching ids. This fails safe (`.kdbx.bak` preserved, no silent loss) and
//!   is strictly better than the pre-fix behaviour, which silently dropped the
//!   attachment. Removals converge cleanly (an empty map equals an empty map
//!   regardless of ids). Shared with `repair_added_entry_attachments`; folded
//!   into the long-term upstream improvement (make Keepass attachment handling
//!   merge-aware through content-based comparison / a real pool merge).
//! - **Same-second divergence is unresolvable.** Two devices editing the same
//!   entry within KDBX's one-second timestamp granularity, with differing
//!   content, cannot be auto-ordered — surfaced as [`MergeError::Unresolvable`]
//!   with the pre-merge `.kdbx.bak` preserved. Vanishingly rare for a single
//!   user; fails safe (no merge is written).
//!
//! Every behaviour above is asserted in the characterization suite
//! (`tests/merge_semantics.rs`) so a future `keepass-rs` bump that changes it
//! fails CI loudly.

use std::{collections::BTreeMap, sync::Arc};

use chrono::NaiveDateTime;
use hidlins_core::{Database, Entry, EntryRef, GroupRef, Times, Uuid, Value};
use zeroize::Zeroizing;

#[derive(Clone, PartialEq, Eq)]
struct AttachmentSnapshot {
    name: String,
    data: Arc<AttachmentSnapshotData>,
}

#[derive(PartialEq, Eq)]
struct AttachmentSnapshotData {
    protected: bool,
    bytes: Zeroizing<Vec<u8>>,
}

impl AttachmentSnapshot {
    fn value(&self) -> Value<Vec<u8>> {
        if self.data.protected {
            Value::protected(self.data.bytes.as_slice().to_vec())
        } else {
            Value::unprotected(self.data.bytes.as_slice().to_vec())
        }
    }
}

struct VersionSnapshot {
    entry: Entry,
    attachments: Vec<AttachmentSnapshot>,
}

struct EntrySnapshot {
    current: VersionSnapshot,
    history: Vec<VersionSnapshot>,
}

impl EntrySnapshot {
    fn versions(&self) -> impl Iterator<Item = &VersionSnapshot> {
        std::iter::once(&self.current).chain(self.history.iter())
    }
}

/// Entries added / removed / modified on `local` by a [`reconcile`] call.
#[derive(Debug, Default, Clone)]
pub struct EntryDelta {
    /// UUIDs present after the merge but not before (folded in from `remote`).
    pub added: Vec<Uuid>,
    /// UUIDs present before the merge but not after (removed via a `remote`
    /// deletion tombstone).
    pub removed: Vec<Uuid>,
    /// UUIDs present both before and after whose `last_modification` changed
    /// (i.e. `remote` won a contested entry).
    pub modified: Vec<Uuid>,
}

impl EntryDelta {
    /// `true` when the merge changed no entries on `local`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

/// Outcome of a successful [`reconcile`].
#[derive(Debug, Default, Clone)]
pub struct MergeSummary {
    /// How `local` changed as a result of folding in `remote`.
    pub delta: EntryDelta,
}

/// Errors returned by [`reconcile`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MergeError {
    /// The two vault snapshots diverged in a way the merge cannot
    /// auto-resolve. In practice this is the same entry edited on two devices
    /// within KDBX's one-second timestamp granularity, leaving differing
    /// content with identical modification times. The orchestrator preserves
    /// the pre-merge state (`.kdbx.bak`) and surfaces this to the user.
    #[error("merge cannot be auto-resolved: {reason}")]
    Unresolvable {
        /// Non-secret description of the conflict. Carries only entry/group
        /// UUIDs and timing facts — never field values.
        reason: String,
    },
}

/// Fold `remote` into `local` in place, UUID-keyed and timestamp-ordered.
///
/// On success `local` contains the union of both sides' entries, with
/// per-entry collisions resolved by newer `last_modification` (loser preserved
/// as history, subject to the preconditions in the module docs). `remote` is
/// left unchanged.
///
/// # Errors
///
/// Returns [`MergeError::Unresolvable`] when the underlying merge cannot decide
/// a winner (same-second divergence). `local` may be left partially modified on
/// error, which is why the orchestrator always snapshots `.kdbx.bak` before
/// calling this.
pub fn reconcile(local: &mut Database, remote: &Database) -> Result<MergeSummary, MergeError> {
    let before = entry_times(local);

    // Snapshot both sides' live entries *before* the merge, so we can enforce
    // the no-data-loss invariant afterwards (see `backfill_lost_versions`).
    let local_pre = collect_live_entries(local);
    let remote_pre = collect_live_entries(remote);

    // Numeric attachment ids are local to a database. If two replicas claim
    // the exact same KDBX version metadata but resolve those references to
    // different bytes, no ordering rule can choose safely. Detect that case
    // before upstream mistakes equal pool ids for equal content.
    ensure_unambiguous_attachment_versions(&local_pre, &remote_pre)?;

    // Relies on entry-management's UUID-stability contract (US-011): entries
    // never silently regenerate their UUIDs, so UUID identity equals
    // logical-entry identity for the merge. The upstream `Database::merge`
    // (keepass `_merge` feature) performs the reconciliation; we only map its
    // error, enforce no-data-loss, and summarize the change.
    local.merge(remote).map_err(|e| MergeError::Unresolvable {
        reason: e.to_string(),
    })?;

    // Defensive no-data-loss pass. `Database::merge` is asymmetric: it
    // preserves the *destination*'s losing value as history only when that
    // entry already carries history, and never preserves the *source*'s losing
    // current value at all. For a secrets manager a silently dropped version is
    // often unrecoverable, so we enforce the invariant directly: every
    // pre-merge entry version (from either side) that is not the merged current
    // value must survive as a history entry under the same UUID.
    backfill_lost_versions(local, &local_pre);
    backfill_lost_versions(local, &remote_pre);
    local.rebuild_attachment_references();

    // Repair history while its non-attachment version metadata still matches
    // the pre-merge snapshots. This includes both newly-backfilled losers and
    // pre-existing histories merged from either replica.
    repair_history_attachments(local, &local_pre);
    repair_history_attachments(local, &remote_pre);

    // AFTER the backfills (their content comparison must see the merged
    // entries exactly as `Database::merge` left them): copy attachment
    // bytes for entries newly added from `remote` — see the module docs'
    // "Attachment handling" section.
    repair_added_entry_attachments(local, &local_pre, &remote_pre);

    // Propagate attachment add/replace/remove for entries present on BOTH
    // sides where `remote` won — the complement of the added-entry repair
    // above. Same "AFTER the backfills" ordering requirement.
    propagate_both_side_attachments(local, &local_pre, &remote_pre);
    local.rebuild_attachment_references();

    let after = entry_times(local);
    Ok(MergeSummary {
        delta: delta_between(&before, &after),
    })
}

/// Clone every live entry reachable from the database root. Used to capture a
/// side's pre-merge state for the no-data-loss backfill.
fn collect_live_entries(db: &Database) -> Vec<EntrySnapshot> {
    fn snapshot_version(
        entry: &EntryRef<'_>,
        pool: &mut BTreeMap<usize, Arc<AttachmentSnapshotData>>,
    ) -> VersionSnapshot {
        VersionSnapshot {
            entry: (**entry).clone(),
            attachments: snapshot_attachments(entry, pool),
        }
    }

    fn walk(
        group: &GroupRef<'_>,
        pool: &mut BTreeMap<usize, Arc<AttachmentSnapshotData>>,
        out: &mut Vec<EntrySnapshot>,
    ) {
        for entry in group.entries() {
            let history_len = entry
                .history
                .as_ref()
                .map_or(0, |history| history.get_entries().len());
            let history = (0..history_len)
                .filter_map(|index| entry.historical(index))
                .map(|historical| snapshot_version(&historical, pool))
                .collect();
            out.push(EntrySnapshot {
                current: snapshot_version(&entry, pool),
                history,
            });
        }
        for sub in group.groups() {
            walk(&sub, pool, out);
        }
    }
    let mut out = Vec::new();
    let mut pool = BTreeMap::new();
    walk(&db.root(), &mut pool, &mut out);
    out
}

/// For each `pre`-merge entry, if the merged database still holds that UUID but
/// its current value differs from `pre` and `pre`'s version is not already in
/// the merged entry's history, append `pre` to history. This recovers any
/// losing version that `Database::merge` dropped, in either direction.
///
/// Entries absent from the merged database (e.g. resolved deletions via a
/// remote tombstone) are intentionally skipped — those are deletions, not
/// dropped content. Idempotent: a version already preserved is detected by the
/// dedup check and not re-added, so repeated syncs do not grow history.
fn backfill_lost_versions(local: &mut Database, pre_entries: &[EntrySnapshot]) {
    for pre in pre_entries {
        let version = &pre.current;
        let id = version.entry.id();
        let should_backfill = match local.entry(id) {
            Some(merged) => {
                content_diverged(&merged, version) && !history_has_version(&merged, version)
            }
            None => false,
        };
        if should_backfill {
            if let Some(mut merged) = local.entry_mut(id) {
                merged
                    .history
                    .get_or_insert_default()
                    .add_entry(version.entry.clone());
            }
        }
    }
}

fn ensure_unambiguous_attachment_versions(
    local: &[EntrySnapshot],
    remote: &[EntrySnapshot],
) -> Result<(), MergeError> {
    let mut remote_by_uuid: BTreeMap<Uuid, Vec<&VersionSnapshot>> = BTreeMap::new();
    for version in remote.iter().flat_map(EntrySnapshot::versions) {
        remote_by_uuid
            .entry(version.entry.id().uuid())
            .or_default()
            .push(version);
    }

    for left in local.iter().flat_map(EntrySnapshot::versions) {
        let Some(remote_versions) = remote_by_uuid.get(&left.entry.id().uuid()) else {
            continue;
        };
        for right in remote_versions {
            if same_non_attachment_version(&left.entry, &right.entry)
                && left.attachments != right.attachments
            {
                return Err(MergeError::Unresolvable {
                    reason: format!(
                        "entry {} has identical version metadata but different attachment bytes",
                        left.entry.id()
                    ),
                });
            }
        }
    }
    Ok(())
}

fn repair_history_attachments(local: &mut Database, snapshots: &[EntrySnapshot]) {
    for snapshot in snapshots.iter().flat_map(EntrySnapshot::versions) {
        let id = snapshot.entry.id();
        let indices: Vec<usize> = match local.entry(id) {
            Some(entry) => {
                let history_len = entry
                    .history
                    .as_ref()
                    .map_or(0, |history| history.get_entries().len());
                (0..history_len)
                    .filter(|&index| {
                        entry.historical(index).is_some_and(|historical| {
                            same_non_attachment_version(&historical, &snapshot.entry)
                        })
                    })
                    .collect()
            }
            None => continue,
        };

        for index in indices {
            let already_matches = if let Some(entry) = local.entry(id) {
                entry.historical(index).is_some_and(|historical| {
                    attachment_state_matches(&historical, &snapshot.attachments)
                })
            } else {
                false
            };
            if already_matches {
                continue;
            }

            if let Some(mut entry) = local.entry_mut(id) {
                if let Some(mut historical) = entry.historical(index) {
                    let names: Vec<String> = historical
                        .as_ref()
                        .attachment_names()
                        .map(str::to_string)
                        .collect();
                    for name in names {
                        historical.remove_attachment_by_name(&name);
                    }
                    for attachment in &snapshot.attachments {
                        historical.add_attachment(attachment.name.clone(), attachment.value());
                    }
                }
            }
        }
    }
}

/// Copy attachment bytes for entries `Database::merge` added from `remote`.
///
/// A source-only entry is cloned into `local` wholesale, so its attachment
/// map still holds *remote-pool* indices; the referenced bytes were never
/// copied. For every such entry (present in `remote_pre`, absent from
/// `local_pre`, still live after the merge), re-add each named attachment
/// with the bytes read from `remote`'s pool: `EntryMut::add_attachment`
/// inserts the bytes into `local`'s pool under a fresh id and drops the
/// stale same-named reference (upstream handles a stale id that is absent
/// from the pool gracefully, and an aliased id's other referents are left
/// untouched — only this entry's reference is replaced).
///
/// Attachment bytes come from the fail-closed, pre-merge remote snapshot, so
/// pool-id aliasing introduced by `Database::merge` cannot affect the source.
///
/// The repair advances the entry's `last_modification` (content change ⇒
/// timestamp bump, upstream's own merge invariant), so a later merge
/// against a stale copy of the same remote resolves in the repaired
/// entry's favor instead of aborting as a same-timestamp divergence.
///
/// Idempotent across repeated syncs: once the entry exists locally it
/// appears in `local_pre` and is skipped.
fn repair_added_entry_attachments(
    local: &mut Database,
    local_pre: &[EntrySnapshot],
    remote_pre: &[EntrySnapshot],
) {
    let local_pre_uuids: std::collections::BTreeSet<Uuid> = local_pre
        .iter()
        .map(|e| e.current.entry.id().uuid())
        .collect();

    for pre in remote_pre {
        let id = pre.current.entry.id();
        if local_pre_uuids.contains(&id.uuid()) {
            continue;
        }
        let already_matches = local
            .entry(id)
            .is_some_and(|entry| attachment_state_matches(&entry, &pre.current.attachments));
        if already_matches {
            continue;
        }
        if let Some(mut merged) = local.entry_mut(id) {
            let names: Vec<String> = merged
                .as_ref()
                .attachment_names()
                .map(str::to_string)
                .collect();
            for name in names {
                merged.remove_attachment_by_name(&name);
            }
            for attachment in &pre.current.attachments {
                merged.add_attachment(attachment.name.clone(), attachment.value());
            }
            // Re-pointing the references changed the entry's content
            // (`Entry::eq` includes the attachment map), and upstream's
            // merge invariant is "content change ⇒ `last_modification`
            // bump" — without it, a later merge against a stale copy of
            // this remote sees a same-timestamp divergence and aborts
            // `Unresolvable`. Advancing the timestamp instead makes the
            // repaired entry *win* that comparison, so sync self-heals.
            // `max(now, lm + 1s)` guards a skewed clock: the repaired
            // entry must never lose to the stale remote copy of itself,
            // which would resurrect the dangling references.
            let advanced = merged
                .times
                .last_modification
                .map(|lm| lm + chrono::Duration::seconds(1));
            let now = Times::now();
            merged.times.last_modification = Some(match advanced {
                Some(adv) if adv > now => adv,
                _ => now,
            });
        }
    }
}

/// Propagate attachment edits (add / replace / remove) for entries present on
/// **both** sides where `remote` won the collision.
///
/// `Database::merge` copies the winner's *fields* into the destination entry but
/// leaves the destination's attachment map untouched (`merge.rs` has an explicit
/// `// TODO: attachments`). So when `remote` wins a both-sides entry, the merged
/// current value carries `remote`'s fields but `local`'s (stale) attachments.
/// This pass reconciles the merged entry's attachment set to `remote`'s:
/// attachments only on `local` are removed, and `remote`'s are (re-)added from
/// the resolved, zeroizing pre-merge snapshot.
fn propagate_both_side_attachments(
    local: &mut Database,
    local_pre: &[EntrySnapshot],
    remote_pre: &[EntrySnapshot],
) {
    // uuid -> local pre-merge last_modification; drives both the both-sides
    // membership test and the winner comparison.
    let local_pre_lm: BTreeMap<Uuid, Option<NaiveDateTime>> = local_pre
        .iter()
        .map(|e| {
            (
                e.current.entry.id().uuid(),
                e.current.entry.times.last_modification,
            )
        })
        .collect();

    for pre in remote_pre {
        let id = pre.current.entry.id();
        // Only entries present on BOTH sides (the complement of
        // `repair_added_entry_attachments`, which handles remote-only adds).
        let Some(&local_lm) = local_pre_lm.get(&id.uuid()) else {
            continue;
        };
        // Act only when `remote` STRICTLY won; when `local` won (or tied), the
        // merged entry already carries `local`'s attachments and is correct.
        if pre.current.entry.times.last_modification <= local_lm {
            continue;
        }
        let already_matches = local
            .entry(id)
            .is_some_and(|entry| attachment_state_matches(&entry, &pre.current.attachments));
        if already_matches {
            continue;
        }

        if let Some(mut merged) = local.entry_mut(id) {
            let names: Vec<String> = merged
                .as_ref()
                .attachment_names()
                .map(str::to_string)
                .collect();
            for name in names {
                merged.remove_attachment_by_name(&name);
            }
            for attachment in &pre.current.attachments {
                merged.add_attachment(attachment.name.clone(), attachment.value());
            }
            // Advance `last_modification` whenever the pass mutates (added or
            // removed anything) — REQUIRED, not cosmetic. keepass's
            // `have_entries_diverged` compares the attachment map by
            // `AttachmentId` (pool-local, not content), so the repaired entry
            // and the remote copy — identical bytes, distinct pool ids — read as
            // diverged. Without the bump, the next sync collides at the
            // (converged) equal timestamp and aborts `Unresolvable`; the bump
            // makes the repaired entry win that comparison so the re-sync is a
            // clean no-op. Mirrors `repair_added_entry_attachments`'
            // `max(now, lm + 1s)` (guards a skewed clock).
            let advanced = merged
                .times
                .last_modification
                .map(|lm| lm + chrono::Duration::seconds(1));
            let now = Times::now();
            merged.times.last_modification = Some(match advanced {
                Some(adv) if adv > now => adv,
                _ => now,
            });
        }
    }
}

fn snapshot_attachments(
    entry: &EntryRef<'_>,
    pool: &mut BTreeMap<usize, Arc<AttachmentSnapshotData>>,
) -> Vec<AttachmentSnapshot> {
    let mut attachments: Vec<_> = entry
        .attachments_named()
        .map(|(name, attachment)| {
            let id = attachment.id().id();
            let data = pool
                .entry(id)
                .or_insert_with(|| {
                    Arc::new(AttachmentSnapshotData {
                        protected: attachment.data.is_protected(),
                        bytes: Zeroizing::new(attachment.data.as_slice().to_vec()),
                    })
                })
                .clone();
            AttachmentSnapshot {
                name: name.to_string(),
                data,
            }
        })
        .collect();
    attachments.sort_by(|left, right| left.name.cmp(&right.name));
    attachments
}

fn attachment_state_matches(entry: &EntryRef<'_>, expected: &[AttachmentSnapshot]) -> bool {
    entry.attachment_names().count() == expected.len()
        && expected.iter().all(|snapshot| {
            entry
                .attachment_by_name(&snapshot.name)
                .is_some_and(|attachment| {
                    attachment.data.is_protected() == snapshot.data.protected
                        && attachment.data.as_slice() == snapshot.data.bytes.as_slice()
                })
        })
}

/// Compare all entry content except timestamps, history, and pool-local
/// attachment ids. Parent identity remains content so moves are preserved.
fn same_non_attachment_content(a: &Entry, b: &Entry) -> bool {
    a.id() == b.id()
        && a.parent_id() == b.parent_id()
        && a.previous_parent_id() == b.previous_parent_id()
        && a.fields == b.fields
        && a.autotype == b.autotype
        && a.tags == b.tags
        && a.custom_data == b.custom_data
        && a.icon() == b.icon()
        && a.foreground_color == b.foreground_color
        && a.background_color == b.background_color
        && a.override_url == b.override_url
        && a.quality_check == b.quality_check
}

fn same_non_attachment_version(a: &Entry, b: &Entry) -> bool {
    same_non_attachment_content(a, b) && a.times == b.times
}

/// Whether the merged current value differs semantically from a snapshot,
/// ignoring timestamps/history and comparing attachment bytes rather than ids.
fn content_diverged(entry: &EntryRef<'_>, candidate: &VersionSnapshot) -> bool {
    !same_non_attachment_content(entry, &candidate.entry)
        || !attachment_state_matches(entry, &candidate.attachments)
}

/// Whether `candidate`'s version is already present in `entry`'s history,
/// matched by both `last_modification` and content (second-precision
/// timestamps mean two same-second-but-different versions can legitimately
/// coexist, so neither key alone is sufficient).
fn history_has_version(entry: &EntryRef<'_>, candidate: &VersionSnapshot) -> bool {
    let history_len = entry
        .history
        .as_ref()
        .map_or(0, |history| history.get_entries().len());
    (0..history_len).any(|index| {
        entry.historical(index).is_some_and(|past| {
            same_non_attachment_version(&past, &candidate.entry)
                && attachment_state_matches(&past, &candidate.attachments)
        })
    })
}

/// Collect every entry's UUID → `last_modification` time by walking the group
/// tree through the public read API. Used to summarize what a merge changed.
fn entry_times(db: &Database) -> BTreeMap<Uuid, Option<NaiveDateTime>> {
    fn walk(group: &GroupRef<'_>, out: &mut BTreeMap<Uuid, Option<NaiveDateTime>>) {
        for entry in group.entries() {
            out.insert(entry.id().uuid(), entry.times.last_modification);
        }
        for sub in group.groups() {
            walk(&sub, out);
        }
    }
    let mut out = BTreeMap::new();
    walk(&db.root(), &mut out);
    out
}

/// Classify the difference between two UUID → time snapshots of `local`.
fn delta_between(
    before: &BTreeMap<Uuid, Option<NaiveDateTime>>,
    after: &BTreeMap<Uuid, Option<NaiveDateTime>>,
) -> EntryDelta {
    let mut delta = EntryDelta::default();
    for (uuid, after_time) in after {
        match before.get(uuid) {
            None => delta.added.push(*uuid),
            Some(before_time) if before_time != after_time => delta.modified.push(*uuid),
            Some(_) => {}
        }
    }
    for uuid in before.keys() {
        if !after.contains_key(uuid) {
            delta.removed.push(*uuid);
        }
    }
    delta
}

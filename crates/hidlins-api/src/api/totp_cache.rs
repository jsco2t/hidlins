//! Revocable per-entry ownership for TOTP cache fills.

use std::collections::HashMap;
use std::sync::Arc;

use hidlins_core::Uuid;

use super::session::TotpSnapshot;

struct Slot {
    identity: Arc<()>,
    snapshot: Option<TotpSnapshot>,
}

#[derive(Default)]

pub(crate) struct TotpCache {
    slots: HashMap<Uuid, Slot>,
}

impl TotpCache {
    /// Called only while source extraction owns the session mutex.
    pub(crate) fn begin_fill(&mut self, uuid: Uuid) -> Arc<()> {
        Arc::clone(
            &self
                .slots
                .entry(uuid)
                .or_insert_with(|| Slot {
                    identity: Arc::new(()),
                    snapshot: None,
                })
                .identity,
        )
    }

    /// Publication cannot recreate a slot removed by a lock or mutation.
    pub(crate) fn publish(&mut self, uuid: Uuid, identity: &Arc<()>, snapshot: TotpSnapshot) {
        if let Some(slot) = self.slots.get_mut(&uuid) {
            if Arc::ptr_eq(&slot.identity, identity) {
                slot.snapshot = Some(snapshot);
            }
        }
    }

    pub(crate) fn get(&self, uuid: &Uuid) -> Option<&TotpSnapshot> {
        self.slots.get(uuid)?.snapshot.as_ref()
    }

    pub(crate) fn remove(&mut self, uuid: &Uuid) {
        self.slots.remove(uuid);
    }

    pub(crate) fn clear(&mut self) {
        self.slots.clear();
    }
}

//! Process-local ownership of outbound pairing work through its final commit.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex, Weak};

use super::{pairing_handle, HidlinsApiError, PendingClientPairing, ServerPairingDecision};
use hidlins_sync::client::LanCancellation;

pub(crate) struct PairingApiState {
    clients: Mutex<ClientPairings>,
    pub(super) server_decisions: Mutex<HashMap<String, mpsc::SyncSender<ServerPairingDecision>>>,
}

struct ClientPairings {
    pending: HashMap<String, PendingClientPairing>,
    active: HashMap<String, Arc<ActivePairing>>,
}

struct ActivePairing {
    cancellation: LanCancellation,
    import_target: Option<(String, PathBuf)>,
}

/// Weak ownership avoids a pending-map -> registration -> owner Arc cycle.
/// Membership plus the retained Arc identity supplies the operation generation.
pub(super) struct PairingRegistration {
    owner: Weak<PairingApiState>,
    handle: String,
    operation: Arc<ActivePairing>,
}
impl PairingApiState {
    pub(crate) fn new() -> Self {
        Self {
            clients: Mutex::new(ClientPairings {
                pending: HashMap::new(),
                active: HashMap::new(),
            }),
            server_decisions: Mutex::new(HashMap::new()),
        }
    }
    pub(super) fn reserve(
        self: &Arc<Self>,
        import_target: Option<(String, PathBuf)>,
    ) -> Result<PairingRegistration, HidlinsApiError> {
        let handle = pairing_handle()?;
        let mut clients = self.clients.lock().map_err(|_| unavailable())?;
        if let Some((name, path)) = &import_target {
            if clients.active.values().any(|operation| {
                operation
                    .import_target
                    .as_ref()
                    .is_some_and(|(other_name, other_path)| {
                        name == other_name || path == other_path
                    })
            }) {
                return Err(HidlinsApiError::SyncBusy);
            }
        }
        if clients.active.contains_key(&handle) || clients.pending.contains_key(&handle) {
            return Err(unavailable());
        }
        let operation = Arc::new(ActivePairing {
            cancellation: LanCancellation::default(),
            import_target,
        });
        clients
            .active
            .insert(handle.clone(), Arc::clone(&operation));
        Ok(PairingRegistration {
            owner: Arc::downgrade(self),
            handle,
            operation,
        })
    }
    pub(super) fn publish(&self, pending: PendingClientPairing) -> Result<(), HidlinsApiError> {
        let mut clients = self.clients.lock().map_err(|_| unavailable())?;
        if !pending.registration.matches(&clients) {
            drop(clients);
            return Err(HidlinsApiError::SyncCanceled);
        }
        let handle = pending.registration.handle.clone();
        if clients.pending.contains_key(&handle) {
            // In particular, never replace/drop another registration while
            // holding its owner mutex.
            drop(clients);
            return Err(unavailable());
        }
        clients.pending.insert(handle, pending);
        Ok(())
    }
    pub(super) fn take_pending(
        &self,
        handle: &str,
    ) -> Result<PendingClientPairing, HidlinsApiError> {
        let pending = {
            let mut clients = self.clients.lock().map_err(|_| unavailable())?;
            clients.pending.remove(handle)
        };
        pending.ok_or(HidlinsApiError::SyncCanceled)
    }
    pub(crate) fn cancel_all(&self) {
        let (active, pending) = {
            let mut clients = self
                .clients
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let active = std::mem::take(&mut clients.active);
            let pending = std::mem::take(&mut clients.pending);
            self.clients.clear_poison();
            (active, pending)
        };
        // A registration's destructor takes clients again. Both maps must be
        // detached before tokens are canceled or pending sessions are dropped.
        for operation in active.into_values() {
            operation.cancellation.cancel();
        }
        drop(pending);
        let mut decisions = self
            .server_decisions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (_, sender) in decisions.drain() {
            let _ = sender.try_send(ServerPairingDecision::Reject);
        }
        self.server_decisions.clear_poison();
    }
}
impl PairingRegistration {
    pub(super) fn handle(&self) -> &str {
        &self.handle
    }
    pub(super) fn cancellation(&self) -> LanCancellation {
        self.operation.cancellation.clone()
    }
    pub(super) fn cancel(&self) {
        self.operation.cancellation.cancel();
    }
    pub(super) fn is_current(&self) -> bool {
        self.with_current(|| ()).is_ok()
    }
    fn matches(&self, clients: &ClientPairings) -> bool {
        !self.operation.cancellation.is_cancelled()
            && clients
                .active
                .get(&self.handle)
                .is_some_and(|current| Arc::ptr_eq(current, &self.operation))
    }
    /// The caller owns State first. Retain clients through the actual commit,
    /// including against cancellation routes that do not acquire State.
    pub(super) fn with_current<T>(&self, action: impl FnOnce() -> T) -> Result<T, HidlinsApiError> {
        let owner = self.owner.upgrade().ok_or(HidlinsApiError::SyncCanceled)?;
        let clients = owner.clients.lock().map_err(|_| unavailable())?;
        if !self.matches(&clients) {
            return Err(HidlinsApiError::SyncCanceled);
        }
        let result = action();
        drop(clients);
        Ok(result)
    }
}
impl Drop for PairingRegistration {
    fn drop(&mut self) {
        self.cancel();
        if let Some(owner) = self.owner.upgrade() {
            let mut clients = owner
                .clients
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if clients
                .active
                .get(&self.handle)
                .is_some_and(|current| Arc::ptr_eq(current, &self.operation))
            {
                clients.active.remove(&self.handle);
            }
        }
    }
}
fn unavailable() -> HidlinsApiError {
    HidlinsApiError::Internal {
        context: "pairing state unavailable".into(),
    }
}

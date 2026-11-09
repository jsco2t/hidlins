use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use hidlins_core::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::session::AppSession;
use crate::dto::{ClipboardTransferTicket, CopyField, GeneratedSecret, RevealField};
use crate::error::HidlinsApiError;

#[cfg(feature = "desktop")]
const CLIPBOARD_AUTO_CLEAR_SECS: u64 = 30;

const MOBILE_CLIPBOARD_TTL: Duration = Duration::from_secs(30);
static NEXT_CLIPBOARD_OWNER: AtomicU64 = AtomicU64::new(1);
#[cfg(any(not(feature = "desktop"), test))]
static NEXT_CLIPBOARD_TRANSFER: AtomicU64 = AtomicU64::new(1);
static CLIPBOARD_TRANSFERS: OnceLock<Mutex<HashMap<String, ClipboardTransfer>>> = OnceLock::new();
static ACTIVE_CLIPBOARD_TRANSFERS: OnceLock<Mutex<HashMap<String, ActiveClipboardTransfer>>> =
    OnceLock::new();

struct ClipboardTransfer {
    owner: u64,
    created: Instant,
    value: Zeroizing<String>,
}

struct ClipboardCompletion {
    done: Mutex<bool>,
    finished: Condvar,
}

struct ActiveClipboardTransfer {
    owner: u64,
    completion: Arc<ClipboardCompletion>,
}

/// Native locals retain this guard through their synchronous publication.
/// It is deliberately non-cloneable and never exported through the bridge.
#[flutter_rust_bridge::frb(ignore)]
pub(crate) struct ConsumedClipboardTransfer {
    value: Zeroizing<String>,
    id: String,
    completion: Arc<ClipboardCompletion>,
}

impl Deref for ConsumedClipboardTransfer {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl DerefMut for ConsumedClipboardTransfer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl ZeroizeOnDrop for ConsumedClipboardTransfer {}

impl Drop for ConsumedClipboardTransfer {
    fn drop(&mut self) {
        // Field destruction follows custom Drop: explicitly erase before
        // waking revocation, rather than relying on the field's later Drop.
        self.value.zeroize();
        {
            let mut done = self
                .completion
                .done
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *done = true;
            self.completion.finished.notify_all();
        }
        // Never acquire a session or pending-registry lock here. Release
        // completion before active; purges wait without either global lock.
        active_transfers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
    }
}

/// Protected copy input belongs to the exact origin that resolved it. The
/// owner changes on revocation, so relocking and reopening the same vault
/// cannot authorize an old value again.
#[flutter_rust_bridge::frb(ignore)]
struct ResolvedCopy {
    owner: u64,
    value: Zeroizing<String>,
}

fn transfers() -> &'static Mutex<HashMap<String, ClipboardTransfer>> {
    CLIPBOARD_TRANSFERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn active_transfers() -> &'static Mutex<HashMap<String, ActiveClipboardTransfer>> {
    ACTIVE_CLIPBOARD_TRANSFERS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn new_clipboard_owner() -> u64 {
    NEXT_CLIPBOARD_OWNER.fetch_add(1, Ordering::Relaxed)
}

#[cfg(any(not(feature = "desktop"), test))]
fn insert_clipboard_transfer(
    owner: u64,
    value: Zeroizing<String>,
    created: Instant,
) -> ClipboardTransferTicket {
    let serial = NEXT_CLIPBOARD_TRANSFER.fetch_add(1, Ordering::Relaxed);
    let id = format!("hct-{owner:016x}-{serial:016x}");
    let transfer = ClipboardTransfer {
        owner,
        created,
        value,
    };
    let mut registry = transfers().lock().unwrap_or_else(|poison| {
        let mut registry = poison.into_inner();
        registry.clear();
        transfers().clear_poison();
        registry
    });
    registry.retain(|_, item| item.created.elapsed() <= MOBILE_CLIPBOARD_TTL);
    registry.insert(id.clone(), transfer);
    ClipboardTransferTicket {
        id,
        expires_after_secs: MOBILE_CLIPBOARD_TTL
            .as_secs()
            .try_into()
            .expect("clipboard TTL fits in the bridge DTO"),
    }
}

/// Native platform adapters consume a ticket through a target-specific export.
/// This function is deliberately crate-private so FRB cannot generate a Dart
/// plaintext-returning door. The returned local must span the synchronous
/// native callback; revocation waits for its drop, including on errors/unwind.
#[flutter_rust_bridge::frb(ignore)]
#[allow(dead_code)]
pub(crate) fn consume_clipboard_transfer(
    id: &str,
) -> Result<ConsumedClipboardTransfer, HidlinsApiError> {
    let mut registry = transfers().lock().map_err(|_| HidlinsApiError::Internal {
        context: "clipboard transfer registry unavailable".to_string(),
    })?;
    let transfer = registry
        .remove(id)
        .ok_or_else(|| HidlinsApiError::InvalidInput {
            field: "clipboard_transfer".to_string(),
            reason: "missing or already consumed".to_string(),
        })?;
    if transfer.created.elapsed() > MOBILE_CLIPBOARD_TTL {
        return Err(HidlinsApiError::InvalidInput {
            field: "clipboard_transfer".to_string(),
            reason: "expired".to_string(),
        });
    }
    let completion = Arc::new(ClipboardCompletion {
        done: Mutex::new(false),
        finished: Condvar::new(),
    });
    // Atomic pending -> active transition with the same lock order as purge.
    // Poison recovery must preserve active fences, never clear this map.
    active_transfers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            id.to_string(),
            ActiveClipboardTransfer {
                owner: transfer.owner,
                completion: Arc::clone(&completion),
            },
        );
    drop(registry);
    Ok(ConsumedClipboardTransfer {
        value: transfer.value,
        id: id.to_string(),
        completion,
    })
}

pub(crate) fn purge_clipboard_transfers(owner: u64) {
    let mut registry = match transfers().lock() {
        Ok(registry) => registry,
        Err(poison) => {
            let mut registry = poison.into_inner();
            registry.clear();
            transfers().clear_poison();
            registry
        }
    };
    registry.retain(|_, item| item.owner != owner);
    let completions: Vec<_> = active_transfers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .values()
        .filter(|item| item.owner == owner)
        .map(|item| Arc::clone(&item.completion))
        .collect();
    // Keep active entries until guard Drop, so a concurrent purge observes
    // the same fence. Release both global registries before any wait: other
    // owners can consume and revoke independently of this native receiver.
    drop(registry);
    for completion in completions {
        let mut done = completion
            .done
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while !*done {
            done = completion
                .finished
                .wait(done)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

pub(crate) fn purge_expired_clipboard_transfers() {
    match transfers().lock() {
        Ok(mut registry) => {
            registry.retain(|_, item| item.created.elapsed() <= MOBILE_CLIPBOARD_TTL);
        }
        Err(poison) => {
            poison.into_inner().clear();
            transfers().clear_poison();
        }
    }
}

impl AppSession {
    /// Copy the exact displayed generated result through the Rust clipboard.
    pub fn copy_generated_secret(&self, result: GeneratedSecret) -> Result<(), HidlinsApiError> {
        let text = resolve_generated_copy(result);
        #[cfg(feature = "desktop")]
        {
            let state = self.lock_state();
            state.require_vault()?;
            if state.clipboard_owner != text.owner {
                return Err(HidlinsApiError::VaultLocked);
            }
            self.clipboard_copy(text.value)
        }
        #[cfg(not(feature = "desktop"))]
        {
            drop(text);
            Err(super::desktop_only(
                "copy_generated_secret",
                "mobile uses prepare_generated_clipboard_transfer",
            ))
        }
    }

    /// Prepare the displayed generated result for the existing native-only,
    /// single-use clipboard handoff. Dart receives only the transfer ticket.
    pub fn prepare_generated_clipboard_transfer(
        &self,
        result: GeneratedSecret,
    ) -> Result<ClipboardTransferTicket, HidlinsApiError> {
        let text = resolve_generated_copy(result);
        #[cfg(feature = "desktop")]
        {
            drop(text);
            Err(HidlinsApiError::UnsupportedPlatform {
                capability: "clipboard".to_string(),
            })
        }
        #[cfg(not(feature = "desktop"))]
        {
            self.stage_clipboard_transfer(text, Instant::now())
        }
    }

    /// Prepare a non-secret, single-use native clipboard handoff.
    ///
    /// Desktop uses the Rust clipboard directly and therefore returns a typed
    /// unsupported result. Mobile Dart receives only this ticket; the
    /// protected value remains zeroizing Rust state until a native adapter
    /// consumes it or the session locks.
    #[allow(clippy::needless_pass_by_value)]
    pub fn prepare_clipboard_transfer(
        &self,
        uuid: String,
        field: CopyField,
    ) -> Result<ClipboardTransferTicket, HidlinsApiError> {
        #[cfg(feature = "desktop")]
        {
            let _ = (uuid, field);
            Err(HidlinsApiError::UnsupportedPlatform {
                capability: "clipboard".to_string(),
            })
        }
        #[cfg(not(feature = "desktop"))]
        {
            let value = self.resolve_copy_field(&uuid, &field)?;
            #[cfg(test)]
            copy_origin_regressions::after_resolve(&value.value);
            self.stage_clipboard_transfer(value, Instant::now())
        }
    }

    #[cfg(any(not(feature = "desktop"), test))]
    fn stage_clipboard_transfer(
        &self,
        value: ResolvedCopy,
        created: Instant,
    ) -> Result<ClipboardTransferTicket, HidlinsApiError> {
        // Validate the captured origin and insert under the same guard. A
        // later lock purges this owner; a prior lock, including same-vault
        // reopening, cannot authorize this old protected value.
        let state = self.lock_state();
        state.require_vault()?;
        if state.clipboard_owner != value.owner {
            return Err(HidlinsApiError::VaultLocked);
        }
        Ok(insert_clipboard_transfer(
            state.clipboard_owner,
            value.value,
            created,
        ))
    }

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
        #[cfg(test)]
        copy_origin_regressions::after_resolve(&text.value);
        let state = self.lock_state();
        state.require_vault()?;
        if state.clipboard_owner != text.owner {
            return Err(HidlinsApiError::VaultLocked);
        }
        // Publish before releasing this guard. A lock that follows cannot
        // return while this clipboard write could still publish afterward.
        self.clipboard_copy(text.value)
    }

    #[cfg(not(feature = "desktop"))]
    #[allow(clippy::unused_self, clippy::needless_pass_by_value)]
    pub fn copy_entry_field(
        &self,
        _uuid: String,
        _field: CopyField,
    ) -> Result<(), HidlinsApiError> {
        Err(super::desktop_only(
            "copy_entry_field",
            "mobile uses prepare_clipboard_transfer",
        ))
    }

    fn resolve_copy_field(
        &self,
        uuid: &str,
        field: &CopyField,
    ) -> Result<ResolvedCopy, HidlinsApiError> {
        let id = parse_uuid(uuid)?;
        let state = self.lock_state();
        let vault = state.require_vault()?;
        let owner = state.clipboard_owner;

        let value = match field {
            CopyField::Username => {
                let view = vault.get_entry(id)?;
                Ok(Zeroizing::new(view.username().to_string()))
            }
            CopyField::Password => {
                let view = vault.get_entry(id)?;
                Ok(Zeroizing::new(view.password().to_string()))
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
                Ok(Zeroizing::new(snap.totp.code_at(now)))
            }
            CopyField::CustomField(name) => {
                let entry = vault.get_entry(id)?;
                entry
                    .custom_field(name)
                    .map(|value| Zeroizing::new(value.to_string()))
                    .ok_or_else(|| HidlinsApiError::InvalidInput {
                        field: "custom_field".to_string(),
                        reason: format!("field not found: {name}"),
                    })
            }
        }?;
        Ok(ResolvedCopy { owner, value })
    }

    #[cfg(feature = "desktop")]
    fn clipboard_copy(&self, mut text: Zeroizing<String>) -> Result<(), HidlinsApiError> {
        let port = self.clipboard.as_ref().ok_or_else(|| HidlinsApiError::Io {
            context: "clipboard unavailable".to_string(),
        })?;
        port.copy_with_autoclear(
            std::mem::take(&mut *text),
            std::time::Duration::from_secs(CLIPBOARD_AUTO_CLEAR_SECS),
        )
    }
}

fn parse_uuid(s: &str) -> Result<Uuid, HidlinsApiError> {
    s.parse::<Uuid>()
        .map_err(|_| HidlinsApiError::InvalidInput {
            field: "uuid".to_string(),
            reason: "invalid UUID".to_string(),
        })
}

fn resolve_generated_copy(result: GeneratedSecret) -> ResolvedCopy {
    // Guard the entire inbound DTO before any validation/unsupported return;
    // move its string allocation into the existing zeroizing copy path.
    let mut result = Zeroizing::new(result);
    ResolvedCopy {
        owner: result.copy_origin,
        value: Zeroizing::new(std::mem::take(&mut result.value)),
    }
}

#[cfg(test)]
#[path = "copy_origin_regressions.rs"]
mod copy_origin_regressions;

#[cfg(test)]
#[path = "generated_copy_regressions.rs"]
mod generated_copy_regressions;

#[cfg(test)]
fn insert_clipboard_transfer_for_test(
    owner: u64,
    value: &str,
    created: Instant,
) -> ClipboardTransferTicket {
    insert_clipboard_transfer(owner, Zeroizing::new(value.to_string()), created)
}

#[cfg(test)]
fn clipboard_transfer_count_for_test() -> usize {
    transfers().lock().map_or(0, |registry| registry.len())
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};
    use std::time::{Duration, Instant};

    use zeroize::Zeroize;

    use super::*;

    static CLIPBOARD_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn isolated_clipboard_transfers() -> MutexGuard<'static, ()> {
        let guard = CLIPBOARD_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        transfers()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        guard
    }

    #[test]
    fn stable_clipboard_transfer_is_typed_unsupported_on_desktop() {
        let temp = tempfile::tempdir().expect("tempdir");
        let session = AppSession::for_test(hidlins_core::paths::HidlinsPaths::with_state_dir(
            temp.path().to_path_buf(),
        ))
        .expect("session");
        let error = session
            .prepare_clipboard_transfer("unused".to_string(), CopyField::Password)
            .expect_err("desktop must not expose the mobile transfer door");
        assert_eq!(
            error,
            HidlinsApiError::UnsupportedPlatform {
                capability: "clipboard".to_string()
            }
        );
    }

    #[test]
    fn clipboard_transfer_is_one_shot_expiring_and_zeroizing() {
        let _guard = isolated_clipboard_transfers();
        let ticket =
            insert_clipboard_transfer_for_test(7, "clipboard-secret-canary", Instant::now());
        let mut value = consume_clipboard_transfer(&ticket.id).expect("first consume");
        assert_eq!(&**value, "clipboard-secret-canary");
        assert!(consume_clipboard_transfer(&ticket.id).is_err());
        value.zeroize();
        assert!(value.is_empty());

        let expired = insert_clipboard_transfer_for_test(
            7,
            "expired-secret-canary",
            Instant::now()
                .checked_sub(Duration::from_secs(31))
                .expect("31 seconds fits in the monotonic clock range"),
        );
        purge_expired_clipboard_transfers();
        assert_eq!(clipboard_transfer_count_for_test(), 0);
        assert!(consume_clipboard_transfer(&expired.id).is_err());
    }

    #[test]
    fn locking_owner_purges_unconsumed_transfers() {
        let _guard = isolated_clipboard_transfers();
        let first = insert_clipboard_transfer_for_test(41, "first-canary", Instant::now());
        let second = insert_clipboard_transfer_for_test(42, "second-canary", Instant::now());
        purge_clipboard_transfers(41);
        assert!(consume_clipboard_transfer(&first.id).is_err());
        let mut surviving = consume_clipboard_transfer(&second.id).expect("other owner survives");
        surviving.zeroize();
        assert!(surviving.is_empty());
    }

    #[test]
    fn locked_session_cannot_stage_a_resolved_transfer() {
        let _guard = isolated_clipboard_transfers();
        let temp = tempfile::tempdir().expect("tempdir");
        let session = AppSession::for_test(hidlins_core::paths::HidlinsPaths::with_state_dir(
            temp.path().to_path_buf(),
        ))
        .expect("session");
        let before = clipboard_transfer_count_for_test();
        let owner = session.lock_state().clipboard_owner;

        assert_eq!(
            session.stage_clipboard_transfer(
                ResolvedCopy {
                    owner,
                    value: Zeroizing::new("race-secret-canary".to_string()),
                },
                Instant::now(),
            ),
            Err(HidlinsApiError::VaultLocked)
        );
        assert_eq!(clipboard_transfer_count_for_test(), before);
    }
}

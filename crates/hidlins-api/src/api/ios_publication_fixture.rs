//! Controls for one dedicated debug-simulator `XCTest` artifact.
//!
//! No plaintext values are returned. The same linked API registry supplies the
//! real session ticket and the production iOS consumer. All ABI errors are
//! payload-free; the caller serializes create/finish and owns its scratch root.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use hidlins_core::{EntryBuilder, MasterPassword, Vault};
use zeroize::Zeroize;

use super::session::{init_app, AppSession};
use crate::dto::{AppInitConfig, CopyField};

const MASTER: &str = "ios-publication-fixture-master";
const VALUE: &str = "ios-publication-native-canary";
static OWNER: AtomicU64 = AtomicU64::new(0);
static WAITING: AtomicBool = AtomicBool::new(false);
static DONE: AtomicBool = AtomicBool::new(false);
static LOCK_OK: AtomicBool = AtomicBool::new(false);
static LOCKED: AtomicBool = AtomicBool::new(false);
static FIXTURE: OnceLock<Mutex<Option<Fixture>>> = OnceLock::new();

struct Fixture {
    session: Arc<AppSession>,
    locking: Option<JoinHandle<()>>,
}

fn fixture() -> &'static Mutex<Option<Fixture>> {
    FIXTURE.get_or_init(|| Mutex::new(None))
}

pub(super) fn note_wait(owner: u64) {
    if owner == OWNER.load(Ordering::Acquire) {
        WAITING.store(true, Ordering::Release);
    }
}

/// Create an isolated real vault/session and return the opaque ticket length.
/// Uses production KDF defaults; only the synthetic fixture input is fixed.
///
/// # Safety
/// `root` references `root_len` readable UTF-8 bytes; `ticket` references
/// `capacity` writable bytes. Buffers must not overlap. Their lifetimes span
/// this call. The root is an absolute, caller-owned temporary directory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hidlins_ios_publication_fixture_create(
    root: *const u8,
    root_len: usize,
    ticket: *mut u8,
    capacity: usize,
) -> i32 {
    std::panic::catch_unwind(|| -> Result<i32, ()> {
        if root.is_null() || ticket.is_null() || root_len == 0 || root_len > 4096 {
            return Err(());
        }
        // SAFETY: the native test caller owns this input for the call; bounds
        // and null are checked above, and it is never retained as a pointer.
        let root = unsafe { std::slice::from_raw_parts(root, root_len) };
        let root = PathBuf::from(std::str::from_utf8(root).map_err(|_| ())?);
        if !root.is_absolute() {
            return Err(());
        }
        let mut slot = fixture().lock().map_err(|_| ())?;
        if slot.is_some() {
            return Err(());
        }
        let session = Arc::new(
            init_app(AppInitConfig {
                state_dir: Some(root.join("state").to_string_lossy().into_owned()),
                config_dir: Some(root.join("config").to_string_lossy().into_owned()),
            })
            .map_err(|_| ())?,
        );
        let summary = session
            .create_vault("publication".into(), MASTER.into(), None, None, true)
            .map_err(|_| ())?;
        let entry = {
            let mut vault = Vault::open(
                &PathBuf::from(summary.path),
                &MasterPassword::new(MASTER.into()),
                None,
            )
            .map_err(|_| ())?;
            let entry = vault
                .add_entry(
                    vault.root_group_uuid(),
                    EntryBuilder::credential("native publication")
                        .password(VALUE)
                        .build(),
                )
                .map_err(|_| ())?;
            vault.save().map_err(|_| ())?;
            entry.to_string()
        };
        session
            .unlock("publication".into(), MASTER.into(), None)
            .map_err(|_| ())?;
        let mut transfer = session
            .prepare_clipboard_transfer(entry, CopyField::Password)
            .map_err(|_| ())?;
        let length = transfer.id.len();
        if capacity < length {
            transfer.id.zeroize();
            return Err(());
        }
        let length_i32 = i32::try_from(length).map_err(|_| ())?;
        // SAFETY: the caller supplies capacity writable non-overlapping bytes;
        // the checked length is bounded by capacity and the source String.
        unsafe { std::ptr::copy_nonoverlapping(transfer.id.as_ptr(), ticket, length) };
        transfer.id.zeroize();
        OWNER.store(session.lock_state().clipboard_owner, Ordering::Release);
        WAITING.store(false, Ordering::Release);
        DONE.store(false, Ordering::Release);
        LOCK_OK.store(false, Ordering::Release);
        LOCKED.store(false, Ordering::Release);
        *slot = Some(Fixture {
            session,
            locking: None,
        });
        Ok(length_i32)
    })
    .ok()
    .and_then(Result::ok)
    .unwrap_or(-1)
}

/// Start actual session revocation off the native publication thread.
#[unsafe(no_mangle)]
pub extern "C" fn hidlins_ios_publication_fixture_lock() -> i32 {
    std::panic::catch_unwind(|| -> Result<(), ()> {
        let mut slot = fixture().lock().map_err(|_| ())?;
        let fixture = slot.as_mut().ok_or(())?;
        if fixture.locking.is_some() {
            return Err(());
        }
        let session = Arc::clone(&fixture.session);
        fixture.locking = Some(thread::spawn(move || {
            let outcome =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| session.lock_now()));
            LOCK_OK.store(matches!(outcome, Ok(Ok(()))), Ordering::Release);
            let state = session.lock_state();
            LOCKED.store(
                state.vault.is_none() && state.credentials.is_none(),
                Ordering::Release,
            );
            drop(state);
            DONE.store(true, Ordering::Release);
        }));
        Ok(())
    })
    .ok()
    .and_then(Result::ok)
    .map_or(-1, |()| 0)
}

/// Bitmask: reached active wait, completed lock, successful lock, vault locked.
#[unsafe(no_mangle)]
pub extern "C" fn hidlins_ios_publication_fixture_status() -> u32 {
    u32::from(WAITING.load(Ordering::Acquire))
        | (u32::from(DONE.load(Ordering::Acquire)) << 1)
        | (u32::from(LOCK_OK.load(Ordering::Acquire)) << 2)
        | (u32::from(LOCKED.load(Ordering::Acquire)) << 3)
}

/// Bounded cleanup after the native callback has returned. Never hang `XCTest`.
#[unsafe(no_mangle)]
pub extern "C" fn hidlins_ios_publication_fixture_finish() -> i32 {
    std::panic::catch_unwind(|| -> Result<(), ()> {
        let current = fixture().lock().map_err(|_| ())?.take();
        if let Some(mut current) = current {
            if let Some(worker) = current.locking.take() {
                let deadline = Instant::now() + Duration::from_secs(5);
                while !worker.is_finished() {
                    if Instant::now() >= deadline {
                        return Err(());
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                worker.join().map_err(|_| ())?;
            }
            current.session.shutdown();
        }
        OWNER.store(0, Ordering::Release);
        Ok(())
    })
    .ok()
    .and_then(Result::ok)
    .map_or(-1, |()| 0)
}

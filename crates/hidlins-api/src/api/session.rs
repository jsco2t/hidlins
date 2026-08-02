use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use hidlins_core::{HidlinsPaths, Keyfile, MasterPassword, Totp, Uuid, Vault, VaultRegistry};
use hidlins_security::{AutoLockConfig, AutoLockController, LockState, OsLockReason};
use zeroize::Zeroize;

pub use crate::dto::{
    AppInitConfig, ClipboardEvent, KeyfileRef, LifecycleStateDto, LockEvent, SyncEvent, VaultTree,
};
pub use crate::error::HidlinsApiError;
pub use crate::event::EventSink;
use crate::event::StreamSinkAdapter;

const LIFECYCLE_GRACE_SECS: u64 = 15;

/// D-12: the lifecycle grace timer is a *mobile* behavior. Desktop locks on
/// the idle timer and OS lock events only, so a window minimize must never
/// start the grace countdown. Enforced here in Rust rather than by Dart
/// discipline — `report_lifecycle_state` is inert on desktop targets.
const LIFECYCLE_ENABLED: bool = cfg!(any(target_os = "ios", target_os = "android"));

static HARDENED: OnceLock<()> = OnceLock::new();

// NOTE: activity is tracked with a monotonic COUNTER, not a timestamp.
//
// The ticker detects "did activity happen since the last tick?" by comparing
// the current value against the one it last saw. A wall-clock timestamp fails
// at that job: two reports landing in the same millisecond compare equal, so
// the tick silently concludes nothing happened and the idle deadline is never
// reset. `unlock()` followed immediately by `report_activity()` hits this
// reliably. A counter cannot collide.

fn lifecycle_from_u8(v: u8) -> LifecycleStateDto {
    match v {
        1 => LifecycleStateDto::Inactive,
        2 => LifecycleStateDto::Hidden,
        3 => LifecycleStateDto::Paused,
        4 => LifecycleStateDto::Detached,
        _ => LifecycleStateDto::Resumed,
    }
}

fn lifecycle_to_u8(state: LifecycleStateDto) -> u8 {
    match state {
        LifecycleStateDto::Resumed => 0,
        LifecycleStateDto::Inactive => 1,
        LifecycleStateDto::Hidden => 2,
        LifecycleStateDto::Paused => 3,
        LifecycleStateDto::Detached => 4,
    }
}

// ---------------------------------------------------------------------------
// SessionCredentials — D-11: retained master password + optional keyfile
// ---------------------------------------------------------------------------

pub(crate) struct SessionCredentials {
    pub(crate) master: MasterPassword,
    pub(crate) keyfile: Option<Keyfile>,
}

// MasterPassword and Keyfile both implement ZeroizeOnDrop individually.
// SessionCredentials owns them; dropping it drops them, which zeroizes.
impl Drop for SessionCredentials {
    fn drop(&mut self) {
        drop(self.keyfile.take());
    }
}

impl std::fmt::Debug for SessionCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionCredentials(***)")
    }
}

// ---------------------------------------------------------------------------
// SessionState — the mutex-protected interior
// ---------------------------------------------------------------------------

// Four bools, each an independent axis of session state — not a config
// blob to be newtyped (the clippy lint's target).
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct SessionState {
    pub(crate) registry: VaultRegistry,
    pub(crate) vault: Option<Vault>,
    pub(crate) credentials: Option<SessionCredentials>,
    pub(crate) controller: AutoLockController,
    /// A lock trigger arrived while the vault was owned elsewhere (an
    /// in-flight unlock KDF or sync worker). The owner consumes it at
    /// its commit point and discards instead of presenting — design
    /// threading rule 5.
    pub(crate) lock_pending: bool,
    /// An `unlock` is between phase 1 (claim) and phase 3 (commit); the KDF
    /// runs without the session mutex. See [`AppSession::unlock_inner`].
    pub(crate) unlocking: bool,
    /// A sync worker owns the vault + registry; queries return
    /// `VaultBusySyncing`.
    pub(crate) syncing: bool,
    pub(crate) lock_sink: Option<Box<dyn EventSink<LockEvent>>>,
    pub(crate) sync_sink: Option<Box<dyn EventSink<SyncEvent>>>,
    pub(crate) dead: bool,
    pub(crate) lifecycle_enabled: bool,
    baseline_lock_config: AutoLockConfig,
    pub(crate) startup_warnings: Vec<String>,
}

impl SessionState {
    pub(crate) fn push_lock_event(&self, event: LockEvent) {
        if let Some(ref sink) = self.lock_sink {
            sink.send(event);
        }
    }

    pub(crate) fn push_sync_event(&self, event: SyncEvent) {
        if let Some(ref sink) = self.sync_sink {
            sink.send(event);
        }
    }

    fn install_lock_sink(&mut self, sink: Box<dyn EventSink<LockEvent>>) {
        // Replacing an existing sink drops the old `StreamSink`, whose
        // `StreamSinkCloser::drop` posts a close to the old Dart port — so a
        // re-subscribing listener terminates cleanly rather than going deaf.
        self.lock_sink = Some(sink);
        // Authoritative snapshot at registration (fail-closed): the generated
        // Dart `lockEvents` returns its stream to the subscriber *before* the
        // registration call reaches Rust, so a transition inside that window
        // would otherwise be lost while the UI keeps rendering cached state.
        // Pushing the current state under the same mutex that installed the
        // sink makes registration + first event atomic.
        let snapshot = if self.is_unlocked() {
            LockEvent::Unlocked
        } else {
            LockEvent::Locked
        };
        self.push_lock_event(snapshot);
        if self.dead {
            // Terminal session: deliver the `Locked` snapshot, then close so
            // the Dart stream ends instead of waiting for events that can
            // never come.
            self.lock_sink = None;
        }
    }

    pub(crate) fn do_lock(&mut self) {
        self.vault = None;
        self.credentials = None;
        self.controller.lock_now(OsLockReason::Manual);
        self.push_lock_event(LockEvent::Locked);
    }

    pub(crate) fn is_unlocked(&self) -> bool {
        self.vault.is_some()
    }

    pub(crate) fn require_vault(&self) -> Result<&Vault, HidlinsApiError> {
        if self.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }
        self.vault.as_ref().ok_or(HidlinsApiError::VaultLocked)
    }

    pub(crate) fn require_vault_mut(&mut self) -> Result<&mut Vault, HidlinsApiError> {
        if self.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }
        self.vault.as_mut().ok_or(HidlinsApiError::VaultLocked)
    }

    fn kill(&mut self) {
        // Idempotent: only the first transition emits `Locked`. Both the
        // poison-recovery path and the ticker's poison branch call this.
        let was_alive = !self.dead;
        self.vault = None;
        self.credentials = None;
        self.dead = true;
        if was_alive {
            self.push_lock_event(LockEvent::Locked);
        }
        // A dead session emits nothing further — drop the sink so Dart
        // observes end-of-stream rather than indefinite silence.
        self.lock_sink = None;
    }
}

// ---------------------------------------------------------------------------
// AppSession — the opaque FFI boundary type
// ---------------------------------------------------------------------------

/// Opaque per design D-1.
///
/// **Normative invariant: every bridged method on this type takes `&self`,
/// never `&mut self`.**
///
/// frb wraps opaque *method receivers* in `RustAutoOpaqueImplicit` regardless
/// of this attribute — that is structural to how it models borrows across FFI,
/// not something an attribute toggles. So every bridged call acquires a guard
/// on frb's per-object `tokio::sync::RwLock`, and `#[frb(sync)]` calls acquire
/// it on the Dart UI thread. With only `&self` methods those are all *read*
/// guards, which never contend, so D-1's requirement ("UI-thread
/// `report_activity` must never wait on a lock") holds.
///
/// A single `&mut self` method would generate a `blocking_write()` and break
/// that: tokio's `RwLock` is write-preferring, so a queued writer stalls every
/// subsequent reader — including `report_activity` on the UI thread. All state
/// lives behind `Arc<Mutex<SessionState>>`, so `&mut self` is never necessary.
/// `tools/dev/boundary-check.sh` fails the build if a write guard ever appears
/// in the generated bindings.
#[flutter_rust_bridge::frb(opaque)]
pub struct AppSession {
    inner: Arc<Mutex<SessionState>>,
    activity_counter: Arc<AtomicU64>,
    lifecycle_state: Arc<AtomicU8>,
    dropped_lock_events: Arc<AtomicU64>,
    ticker_shutdown: Arc<AtomicBool>,
    ticker_join: Mutex<Option<JoinHandle<()>>>,
    pub(crate) totp_cache: Arc<RwLock<HashMap<Uuid, TotpSnapshot>>>,
    pub(crate) paths: HidlinsPaths,
    #[cfg_attr(not(feature = "desktop"), allow(dead_code))]
    pub(crate) clipboard: Option<Arc<dyn crate::clipboard_port::ClipboardPort>>,
    pub(crate) sync_engine: Arc<dyn crate::sync_port::SyncEnginePort>,
    #[cfg(any(test, feature = "test-fixtures"))]
    test_grace_start: Mutex<Option<Instant>>,
    #[cfg(any(test, feature = "test-fixtures"))]
    test_last_seen: Mutex<u64>,
}

pub(crate) struct TotpSnapshot {
    pub(crate) totp: Totp,
    secret_uri: String,
}

impl TotpSnapshot {
    pub(crate) fn new(totp: Totp, secret_uri: String) -> Self {
        Self { totp, secret_uri }
    }

    pub(crate) fn secret_uri(&self) -> &str {
        &self.secret_uri
    }
}

impl Drop for TotpSnapshot {
    fn drop(&mut self) {
        self.secret_uri.zeroize();
    }
}

impl Drop for AppSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl std::fmt::Debug for AppSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppSession")
            .field(
                "has_ticker",
                &self.ticker_join.lock().ok().map(|g| g.is_some()),
            )
            .finish_non_exhaustive()
    }
}

pub fn api_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[allow(clippy::needless_pass_by_value)] // frb FFI passes owned DTOs
pub fn init_app(cfg: AppInitConfig) -> Result<AppSession, HidlinsApiError> {
    HARDENED.get_or_init(|| {
        hidlins_security::harden_process();
    });

    let paths = match (&cfg.state_dir, &cfg.config_dir) {
        (Some(state), Some(config)) => HidlinsPaths::with_state_dir(PathBuf::from(state))
            .with_config_dir(PathBuf::from(config)),
        (Some(state), None) => HidlinsPaths::with_state_dir(PathBuf::from(state)),
        _ => HidlinsPaths::from_env()?,
    };
    paths.ensure_exists()?;

    let session_paths = paths.clone();
    let registry = VaultRegistry::load(paths)?;

    // Mutated only on platform/feature combinations with an OS event source.
    #[allow(unused_mut)]
    let mut controller = AutoLockController::new(AutoLockConfig::default()).map_err(|e| {
        HidlinsApiError::Internal {
            context: format!("auto-lock controller init: {e}"),
        }
    })?;

    // T1.8: Desktop OS lock-event source wiring — FIRST PRODUCTION
    // CONSUMER of LogindSource/IoKitSource/SigstopSource. Startup failure
    // degrades to idle-only locking and records a warning retrievable via
    // `startup_warnings()` (FR-052 best-effort).
    // Populated only by the cfg-gated OS event source blocks below.
    #[allow(unused_mut)]
    let mut startup_warnings: Vec<String> = Vec::new();
    #[cfg(all(target_os = "linux", feature = "desktop"))]
    {
        match hidlins_security::SigstopSource::new() {
            Ok(source) => {
                if let Err(e) = controller.attach_event_source(source) {
                    startup_warnings
                        .push(format!("OS lock detection degraded (SIGTSTP attach): {e}"));
                }
            }
            Err(e) => {
                startup_warnings.push(format!("OS lock detection degraded (SIGTSTP init): {e}"));
            }
        }
    }
    #[cfg(all(target_os = "linux", feature = "logind"))]
    {
        if let Err(e) = controller.attach_event_source(hidlins_security::LogindSource) {
            startup_warnings.push(format!("OS lock detection degraded (logind): {e}"));
        }
    }
    #[cfg(all(target_os = "macos", feature = "iokit"))]
    {
        if let Err(e) = controller.attach_event_source(hidlins_security::IoKitSource) {
            startup_warnings.push(format!("OS lock detection degraded (IOKit): {e}"));
        }
    }

    let state = SessionState {
        registry,
        vault: None,
        credentials: None,
        controller,
        lock_pending: false,
        unlocking: false,
        syncing: false,
        lock_sink: None,
        sync_sink: None,
        dead: false,
        lifecycle_enabled: LIFECYCLE_ENABLED,
        baseline_lock_config: AutoLockConfig::default(),
        startup_warnings,
    };

    let inner = Arc::new(Mutex::new(state));
    let activity_counter = Arc::new(AtomicU64::new(0));
    let lifecycle_state = Arc::new(AtomicU8::new(lifecycle_to_u8(LifecycleStateDto::Resumed)));
    let ticker_shutdown = Arc::new(AtomicBool::new(false));
    let totp_cache = Arc::new(RwLock::new(HashMap::new()));

    let ticker_join = spawn_ticker(
        Arc::clone(&inner),
        Arc::clone(&activity_counter),
        Arc::clone(&lifecycle_state),
        Arc::clone(&ticker_shutdown),
        Arc::clone(&totp_cache),
    );

    #[cfg(feature = "desktop")]
    let clipboard: Option<Arc<dyn crate::clipboard_port::ClipboardPort>> = {
        match crate::clipboard_port::SystemClipboard::new() {
            Ok(cb) => Some(Arc::new(cb)),
            Err(_) => None,
        }
    };
    #[cfg(not(feature = "desktop"))]
    let clipboard: Option<Arc<dyn crate::clipboard_port::ClipboardPort>> = None;

    Ok(AppSession {
        inner,
        activity_counter,
        lifecycle_state,
        dropped_lock_events: Arc::new(AtomicU64::new(0)),
        ticker_shutdown,
        ticker_join: Mutex::new(Some(ticker_join)),
        totp_cache,
        paths: session_paths,
        clipboard,
        sync_engine: Arc::new(crate::sync_port::DefaultSyncEngine),
        #[cfg(any(test, feature = "test-fixtures"))]
        test_grace_start: Mutex::new(None),
        #[cfg(any(test, feature = "test-fixtures"))]
        test_last_seen: Mutex::new(0),
    })
}

impl AppSession {
    /// Subscribe to lock-state events.
    ///
    /// Registration atomically pushes an authoritative snapshot of the
    /// current state (`Locked`/`Unlocked`) as the first event, so a
    /// subscriber never has to guess the state it started from and a
    /// transition racing the subscription cannot be observed as silence. On
    /// a dead (shut-down) session the snapshot is `Locked` and the stream is
    /// closed immediately; [`Self::shutdown`] also closes an active stream.
    /// Client protocol: end-of-stream, or an *increase* in the monotonic
    /// [`Self::dropped_lock_events`] counter since the client last read it,
    /// means cached lock state is stale — treat the vault as locked and
    /// resubscribe for a fresh snapshot (which resets nothing; compare
    /// against the newly read counter value from then on).
    pub fn lock_events(&self, sink: crate::frb_generated::StreamSink<LockEvent>) {
        let adapter = StreamSinkAdapter::new(sink, Arc::clone(&self.dropped_lock_events));
        self.lock_state().install_lock_sink(Box::new(adapter));
    }

    /// Number of lock events frb refused to deliver to Dart, monotonic for
    /// the session's lifetime (it never resets, including on resubscribe).
    ///
    /// An increase since the caller last read it means the UI may be
    /// rendering stale lock state; see [`crate::event::StreamSinkAdapter`].
    /// Exposed because the project has no logging facility and a dropped
    /// `Locked` is otherwise unobservable.
    #[flutter_rust_bridge::frb(sync)]
    pub fn dropped_lock_events(&self) -> u64 {
        self.dropped_lock_events.load(Ordering::Relaxed)
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn report_activity(&self) {
        self.activity_counter.fetch_add(1, Ordering::Relaxed);
    }

    #[flutter_rust_bridge::frb(sync)]
    pub fn report_lifecycle_state(&self, state: LifecycleStateDto) {
        self.lifecycle_state
            .store(lifecycle_to_u8(state), Ordering::Relaxed);
    }

    pub fn unlock(
        &self,
        name: String,
        master_password: String,
        keyfile: Option<KeyfileRef>,
    ) -> Result<VaultTree, HidlinsApiError> {
        self.unlock_inner(name, master_password, keyfile, || {})
    }

    /// The unlock protocol, split around the KDF so the session mutex is
    /// never held across `Vault::open` (Argon2id is tuned to ~1s):
    ///
    ///   phase 1 (short guard)  validate, resolve the vault path, claim the
    ///                          in-flight slot via `unlocking`
    ///   phase 2 (no guard)     KDF + KDBX parse + DTO tree snapshot
    ///   phase 3 (short guard)  re-validate, then commit — or discard
    ///
    /// Holding the mutex across the KDF would queue `lock_now` and
    /// `shutdown` (which the Dart finalizer runs on the UI isolate's thread)
    /// behind a ~1s wait, and would reorder a lock issued mid-unlock to
    /// *after* the commit — Flutter could then receive a successful unlock
    /// response after having observed `Locked`. Instead, a lock that arrives
    /// during phase 2 sets `lock_pending` (a shutdown sets `dead`), and
    /// phase 3 honors it: the freshly opened vault and credentials are
    /// dropped — exactly what `do_lock` drops, with the same scrubbing
    /// (`MasterPassword`/`Keyfile` zeroize on drop; the vault's protected
    /// values scrub via keepass-rs internals) — no `Unlocked` event fires,
    /// and the caller gets `VaultLocked` (a shutdown race instead surfaces
    /// the dead-session `Internal` error). This is design threading rule 5's
    /// `lock_pending` discipline applied to the unlock window.
    #[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
    fn unlock_inner(
        &self,
        name: String,
        master_password: String,
        keyfile: Option<KeyfileRef>,
        between_open_and_commit: impl FnOnce(),
    ) -> Result<VaultTree, HidlinsApiError> {
        let master = MasterPassword::new(master_password);
        // Move the bytes directly into `Keyfile` — no clone, no residual copy.
        // `Keyfile` has a hand-rolled `impl Drop` that zeroizes the `Bytes`
        // arm, so the material is scrubbed on every drop path below,
        // including `Vault::open` failure and the phase-3 discard. The move
        // happens before any early return so no branch can leak an
        // unscrubbed `KeyfileRef`.
        let kf = keyfile.map(|kr| match kr {
            KeyfileRef::Path(p) => Keyfile::Path(PathBuf::from(p)),
            KeyfileRef::Bytes(b) => Keyfile::Bytes(b),
        });

        // Phase 1: claim the unlock under a short guard.
        let vault_path = {
            let mut state = self.lock_state();

            if state.dead {
                return Err(HidlinsApiError::Internal {
                    context: "session is shut down".to_string(),
                });
            }

            if state.is_unlocked() {
                return Err(HidlinsApiError::Internal {
                    context: "a vault is already unlocked".to_string(),
                });
            }

            if state.unlocking {
                return Err(HidlinsApiError::Internal {
                    context: "an unlock is already in progress".to_string(),
                });
            }

            let path = state
                .registry
                .get(&name)
                .ok_or_else(|| HidlinsApiError::FileNotFound { path: name.clone() })?
                .path
                .clone();
            state.unlocking = true;
            path
        };

        // If phase 2 unwinds (frb catches panics on its pool threads), the
        // claim must not stay set or the session would refuse every later
        // unlock. Phase 3 disarms this guard under its own mutex guard — see
        // the type's doc for why an armed drop after phase 3 would be a race.
        let mut clear_unlocking = ClearUnlockingOnDrop {
            session: self,
            armed: true,
        };

        // Phase 2: the expensive work, without the session mutex.
        let opened = Vault::open(&vault_path, &master, kf.as_ref()).map(|vault| {
            let tree = crate::dto::vault_tree_from_database(vault.database(), chrono::Utc::now());
            (vault, tree)
        });

        between_open_and_commit();

        // Phase 3: re-validate and commit under a short guard.
        let mut state = self.lock_state();
        state.unlocking = false;
        // Disarm while still holding the mutex: from the moment this guard
        // is released, the claim slot may legally belong to another unlock,
        // and the drop guard must not touch it.
        clear_unlocking.armed = false;
        // Consume the flag on every exit below — a stale `lock_pending` must
        // never abort a *future* unlock. (T1.6's sync worker will share this
        // flag; revisit the consume point when that lands.)
        let lock_requested = std::mem::take(&mut state.lock_pending);
        // Phase 1's claim makes concurrent unlocks impossible, so nothing can
        // have committed a vault while the KDF ran; keep that provable.
        debug_assert!(
            !state.is_unlocked(),
            "vault committed during an unlock's KDF window"
        );

        // Open failure: `master`/`kf` drop (zeroized) on return.
        let (vault, _pre_sync_tree) = opened?;

        if state.dead {
            return Err(HidlinsApiError::Internal {
                context: "session is shut down".to_string(),
            });
        }
        if lock_requested {
            // Superseded by a lock issued while the KDF ran: discard the
            // opened vault instead of presenting it — the same drops (and
            // the same scrubbing) as `do_lock` on an unlocked session.
            return Err(HidlinsApiError::VaultLocked);
        }

        // Apply per-vault lock config, falling back to the session's
        // init-time baseline so a previous vault's override never leaks.
        let lock_config = state
            .registry
            .get(&name)
            .and_then(
                |entry| match hidlins_security::VaultLockConfig::from_extra(&entry.extra) {
                    Ok(Some(vlc)) => vlc.to_auto_lock_config().ok(),
                    Ok(None) | Err(_) => None,
                },
            )
            .unwrap_or(state.baseline_lock_config);
        if let Err(e) = state.controller.set_config(lock_config) {
            return Err(HidlinsApiError::InvalidInput {
                field: "vault.lock".to_string(),
                reason: format!("{e}"),
            });
        }

        state.controller.unlock(Instant::now());
        self.activity_counter.fetch_add(1, Ordering::Relaxed);

        state.credentials = Some(SessionCredentials {
            master,
            keyfile: kf,
        });
        state.vault = Some(vault);

        // FR-041: merge-before-present. If sync is configured, enter
        // Syncing state BEFORE releasing the guard so no concurrent
        // caller can read the pre-merge vault (threading rule 8: the
        // unlock-merge window never exposes a readable vault).
        let sync_configured = {
            let vault_path = state
                .vault
                .as_ref()
                .expect("just committed")
                .path()
                .to_path_buf();
            state
                .registry
                .list()
                .find(|e| e.path == vault_path)
                .and_then(hidlins_sync::SyncConfig::from_vault_entry)
                .is_some()
        };

        if sync_configured {
            // Move vault + registry out and set syncing under this
            // guard — no concurrent caller can read the pre-merge
            // vault (threading rule 8 / finding 4).
            let creds = state.credentials.as_ref().expect("just committed");
            let master_clone =
                MasterPassword::new(String::from_utf8_lossy(creds.master.as_bytes()).into_owned());
            let keyfile_clone = creds.keyfile.as_ref().map(|kf| match kf {
                Keyfile::Path(p) => Keyfile::Path(p.clone()),
                Keyfile::Bytes(b) => Keyfile::Bytes(b.clone()),
            });
            let vault_path = state
                .vault
                .as_ref()
                .expect("just committed")
                .path()
                .to_path_buf();
            let vault_name = state
                .registry
                .list()
                .find(|e| e.path == vault_path)
                .map(|e| e.name.clone())
                .expect("just committed vault is registered");
            let mut sync_vault = state.vault.take().expect("just committed");
            let placeholder_paths = state.registry.paths().clone();
            let mut sync_registry = std::mem::replace(
                &mut state.registry,
                VaultRegistry::with_paths(placeholder_paths),
            );
            state.syncing = true;
            state.push_sync_event(SyncEvent::Started);
            drop(state);

            // Phase 2: sync without the session mutex.
            let engine = Arc::clone(&self.sync_engine);
            let sync_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.sync_now(
                    &mut sync_vault,
                    &vault_name,
                    &mut sync_registry,
                    &master_clone,
                    keyfile_clone.as_ref(),
                    hidlins_sync::SyncOptions::default(),
                )
            }));

            // Phase 3: commit or discard.
            let mut state = self.lock_state();
            state.syncing = false;
            let lock_requested = std::mem::take(&mut state.lock_pending);
            self.clear_totp_cache();

            if lock_requested || state.dead {
                state.registry = sync_registry;
                state.credentials = None;
                state.controller.lock_now(OsLockReason::Manual);
                state.push_sync_event(SyncEvent::Failed(HidlinsApiError::VaultLocked));
                if !state.dead {
                    state.push_lock_event(LockEvent::Locked);
                }
                return Err(HidlinsApiError::VaultLocked);
            }

            if let Ok(Ok(outcome)) = sync_result {
                let dto = crate::dto::sync_outcome_from_core(&outcome);
                state.push_sync_event(SyncEvent::Done(dto));
                state.vault = Some(sync_vault);
                state.registry = sync_registry;
            } else {
                state.push_sync_event(SyncEvent::Failed(HidlinsApiError::Internal {
                    context: "FR-041 merge failed; using local vault".to_string(),
                }));
                state.vault = Some(sync_vault);
                state.registry = sync_registry;
            }
        } else {
            drop(state);
        }

        // Re-acquire and branch on vault presence — the only safe
        // way to determine whether sync succeeded, failed-and-restored,
        // or dropped the vault (lock_pending / panic / dead).
        let final_state = self.lock_state();
        if let Some(ref v) = final_state.vault {
            final_state.push_lock_event(LockEvent::Unlocked);
            Ok(crate::dto::vault_tree_from_database(
                v.database(),
                chrono::Utc::now(),
            ))
        } else {
            // Vault was dropped by sync (lock_pending, panic, or dead).
            // No Unlocked event — the session is locked.
            Err(HidlinsApiError::VaultLocked)
        }
    }

    pub(crate) fn maybe_sync_after_save(&self) {
        let should_sync = {
            let state = self.lock_state();
            if !state.is_unlocked() || state.syncing {
                return;
            }
            let Some(vault) = state.vault.as_ref() else {
                return;
            };
            let vault_path = vault.path().to_path_buf();
            let found = state.registry.list().any(|e| {
                e.path == vault_path && hidlins_sync::SyncConfig::from_vault_entry(e).is_some()
            });
            found
        };

        if should_sync {
            let _ = self.sync_now();
        }
    }

    pub fn sync_events(&self, sink: crate::frb_generated::StreamSink<SyncEvent>) {
        let adapter = StreamSinkAdapter::new(sink, Arc::clone(&self.dropped_lock_events));
        self.lock_state().sync_sink = Some(Box::new(adapter));
    }

    #[cfg(feature = "desktop")]
    pub fn clipboard_events(&self, _sink: crate::frb_generated::StreamSink<ClipboardEvent>) {
        // Alpha: Dart drives the 30 s countdown from the known constant.
        // The stream is registered for forward-compat; a Cleared event
        // will be wired when ClipboardPort exposes guard-expiry callbacks.
    }

    pub fn startup_warnings(&self) -> Vec<String> {
        let state = self.lock_state();
        state.startup_warnings.clone()
    }

    pub fn lock_now(&self) -> Result<(), HidlinsApiError> {
        self.clear_totp_cache();
        let mut state = self.lock_state();
        if state.is_unlocked() {
            state.do_lock();
        } else if state.unlocking || state.syncing {
            state.lock_pending = true;
        }
        Ok(())
    }

    pub(crate) fn invalidate_totp(&self, uuid: &Uuid) {
        match self.totp_cache.write() {
            Ok(mut cache) => {
                cache.remove(uuid);
            }
            Err(poison) => {
                // Poisoned — clear everything as the conservative fallback.
                // The cache holds only derivable data; entry_detail
                // repopulates on next access.
                poison.into_inner().clear();
            }
        }
    }

    pub(crate) fn clear_totp_cache(&self) {
        match self.totp_cache.write() {
            Ok(mut cache) => cache.clear(),
            Err(poison) => {
                poison.into_inner().clear();
                // The RwLock was poisoned — clear and move on. The cache
                // holds only derivable data; the next entry_detail call
                // will repopulate.
            }
        }
    }

    pub fn shutdown(&self) {
        self.ticker_shutdown.store(true, Ordering::Relaxed);
        self.clear_totp_cache();

        {
            let mut state = self.lock_state();
            state.dead = true;
            if state.is_unlocked() {
                state.do_lock();
            }
            state.lock_sink = None;
            state.controller.shutdown_sources();
        }

        if let Ok(mut guard) = self.ticker_join.lock() {
            if let Some(handle) = guard.take() {
                // Wake the ticker so the join returns promptly instead of
                // waiting out the remainder of its 1 s park. `Drop` calls
                // `shutdown`, and Dart's `dispose`/`NativeFinalizer` run the
                // release inline on the calling isolate thread — a blocking
                // join here would stall the UI thread.
                handle.thread().unpark();
                let _ = handle.join();
            }
        }
    }

    pub(crate) fn lock_state(&self) -> MutexGuard<'_, SessionState> {
        self.inner.lock().unwrap_or_else(|poison| {
            // Threading rule 7 — fail locked. Recover the guard, force the
            // session to the locked state, then clear the poison so the
            // recovery is a one-shot transition; without `clear_poison` every
            // later call would re-run `kill()` and push another `Locked`.
            let mut state = poison.into_inner();
            state.kill();
            self.inner.clear_poison();
            state
        })
    }
}

/// Clears the `unlocking` claim if `unlock_inner`'s phase 2 unwinds. A stale
/// claim would make every later unlock fail with "already in progress"; a
/// stale `lock_pending` is deliberately NOT cleared here — it can at worst
/// abort one subsequent unlock toward the locked state (the safe direction).
///
/// MUST be disarmed by phase 3 while phase 3 still holds the session mutex.
/// This drop runs only *after* the phase-3 guard is released (locals drop in
/// reverse declaration order), and in that gap another unlock can already
/// have claimed the slot — an armed drop firing there would clear the *new*
/// unlock's claim, and a `lock_now` during its KDF would then find neither
/// `is_unlocked()` nor `unlocking` and be silently lost. Disarmed, the drop
/// fires only when phase 3 never ran, in which case the claim is provably
/// still ours: phase 1 set it, and no other unlock can claim while it is set.
struct ClearUnlockingOnDrop<'a> {
    session: &'a AppSession,
    armed: bool,
}

impl Drop for ClearUnlockingOnDrop<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.session.lock_state().unlocking = false;
        }
    }
}

// ---------------------------------------------------------------------------
// Test-only API — manual tick driving without wall-clock sleeps.
// ---------------------------------------------------------------------------

#[cfg(any(test, feature = "test-fixtures"))]
#[allow(clippy::missing_panics_doc)]
impl AppSession {
    pub fn for_test(paths: HidlinsPaths) -> Result<Self, HidlinsApiError> {
        Self::for_test_inner(paths, AutoLockConfig::default(), false)
    }

    pub fn for_test_with_timeout(
        paths: HidlinsPaths,
        idle_timeout: Duration,
    ) -> Result<Self, HidlinsApiError> {
        Self::for_test_inner(paths, AutoLockConfig { idle_timeout }, false)
    }

    pub fn for_test_mobile(paths: HidlinsPaths) -> Result<Self, HidlinsApiError> {
        Self::for_test_inner(paths, AutoLockConfig::default(), true)
    }

    pub fn for_test_mobile_with_timeout(
        paths: HidlinsPaths,
        idle_timeout: Duration,
    ) -> Result<Self, HidlinsApiError> {
        Self::for_test_inner(paths, AutoLockConfig { idle_timeout }, true)
    }

    fn for_test_inner(
        paths: HidlinsPaths,
        config: AutoLockConfig,
        lifecycle_enabled: bool,
    ) -> Result<Self, HidlinsApiError> {
        let session_paths = paths.clone();
        let registry = VaultRegistry::load(paths)?;
        let controller =
            AutoLockController::new(config).map_err(|e| HidlinsApiError::Internal {
                context: format!("controller init: {e}"),
            })?;

        let state = SessionState {
            registry,
            vault: None,
            credentials: None,
            controller,
            lock_pending: false,
            unlocking: false,
            syncing: false,
            lock_sink: None,
            sync_sink: None,
            dead: false,
            lifecycle_enabled,
            baseline_lock_config: config,
            startup_warnings: Vec::new(),
        };

        Ok(Self {
            inner: Arc::new(Mutex::new(state)),
            activity_counter: Arc::new(AtomicU64::new(0)),
            lifecycle_state: Arc::new(AtomicU8::new(0)),
            dropped_lock_events: Arc::new(AtomicU64::new(0)),
            ticker_shutdown: Arc::new(AtomicBool::new(false)),
            ticker_join: Mutex::new(None),
            totp_cache: Arc::new(RwLock::new(HashMap::new())),
            paths: session_paths,
            clipboard: None,
            sync_engine: Arc::new(crate::sync_port::DefaultSyncEngine),
            test_grace_start: Mutex::new(None),
            test_last_seen: Mutex::new(0),
        })
    }

    pub fn with_ports(
        paths: HidlinsPaths,
        clipboard: Option<Arc<dyn crate::clipboard_port::ClipboardPort>>,
        sync_engine: Arc<dyn crate::sync_port::SyncEnginePort>,
    ) -> Result<Self, HidlinsApiError> {
        let session_paths = paths.clone();
        let registry = VaultRegistry::load(paths)?;
        let controller = AutoLockController::new(AutoLockConfig::default()).map_err(|e| {
            HidlinsApiError::Internal {
                context: format!("controller init: {e}"),
            }
        })?;

        let state = SessionState {
            registry,
            vault: None,
            credentials: None,
            controller,
            lock_pending: false,
            unlocking: false,
            syncing: false,
            lock_sink: None,
            sync_sink: None,
            dead: false,
            lifecycle_enabled: false,
            baseline_lock_config: AutoLockConfig::default(),
            startup_warnings: Vec::new(),
        };

        Ok(Self {
            inner: Arc::new(Mutex::new(state)),
            activity_counter: Arc::new(AtomicU64::new(0)),
            lifecycle_state: Arc::new(AtomicU8::new(0)),
            dropped_lock_events: Arc::new(AtomicU64::new(0)),
            ticker_shutdown: Arc::new(AtomicBool::new(false)),
            ticker_join: Mutex::new(None),
            totp_cache: Arc::new(RwLock::new(HashMap::new())),
            paths: session_paths,
            clipboard,
            sync_engine,
            test_grace_start: Mutex::new(None),
            test_last_seen: Mutex::new(0),
        })
    }

    pub fn drive_tick(&self, now: Instant) {
        let activity = self.activity_counter.load(Ordering::Relaxed);
        let lifecycle = lifecycle_from_u8(self.lifecycle_state.load(Ordering::Relaxed));

        let mut grace = self.test_grace_start.lock().expect("grace lock");
        let mut last_seen = self.test_last_seen.lock().expect("last_seen lock");
        let mut state = self.lock_state();
        tick_inner(
            &mut state,
            now,
            activity,
            lifecycle,
            &mut grace,
            &mut last_seen,
        );
    }

    /// Installs a test sink through the same path production uses, so tests
    /// exercise the registration snapshot and dead-session close behavior.
    pub fn lock_events_for_test(&self, sink: Box<dyn EventSink<LockEvent>>) {
        self.lock_state().install_lock_sink(sink);
    }

    /// Runs the production unlock protocol with a hook between the KDF
    /// (phase 2) and the commit (phase 3), for deterministic interleaving of
    /// concurrent `lock_now`/`shutdown`/`unlock`/tick calls.
    pub fn unlock_with_hook(
        &self,
        name: &str,
        password: &str,
        keyfile: Option<KeyfileRef>,
        between_open_and_commit: impl FnOnce(),
    ) -> Result<VaultTree, HidlinsApiError> {
        self.unlock_inner(
            name.to_string(),
            password.to_string(),
            keyfile,
            between_open_and_commit,
        )
    }

    pub fn is_unlocking(&self) -> bool {
        self.lock_state().unlocking
    }

    /// Attaches a real `OsLockEventSource` to this session's controller, so
    /// tests can drive the drained-event path (`take_lock_event_observed`)
    /// through the same machinery T1.8 will wire in production.
    pub fn attach_lock_source_for_test(
        &self,
        source: impl hidlins_security::OsLockEventSource + 'static,
    ) {
        self.lock_state()
            .controller
            .attach_event_source(source)
            .expect("attach test event source");
    }

    pub fn sync_events_for_test(&self, sink: Box<dyn EventSink<SyncEvent>>) {
        self.lock_state().sync_sink = Some(sink);
    }

    pub fn has_vault(&self) -> bool {
        self.lock_state().is_unlocked()
    }

    pub fn has_credentials(&self) -> bool {
        self.lock_state().credentials.is_some()
    }

    pub fn is_dead(&self) -> bool {
        self.lock_state().dead
    }

    pub fn ticker_is_joined(&self) -> bool {
        self.ticker_join.lock().is_ok_and(|g| g.is_none())
    }

    pub fn hold_mutex_for_test(&self) -> impl Drop + '_ {
        self.lock_state()
    }

    /// Poison the TOTP cache `RwLock` for testing recovery paths.
    ///
    /// Spawns a thread that takes a write guard and panics, leaving the
    /// lock poisoned. The next `totp_now` call must recover gracefully
    /// (clear the cache and report a cache miss) rather than panicking.
    pub fn poison_totp_cache_for_test(&self) {
        let cache = Arc::clone(&self.totp_cache);
        let handle = std::thread::spawn(move || {
            let _guard = cache.write().expect("take write guard to poison");
            panic!("intentional poison for test");
        });
        // The thread panicked — join collects it so the panic doesn't
        // propagate. The RwLock is now poisoned.
        let _ = handle.join();
    }
}

// ---------------------------------------------------------------------------
// Ticker thread
// ---------------------------------------------------------------------------

fn spawn_ticker(
    inner: Arc<Mutex<SessionState>>,
    activity_counter: Arc<AtomicU64>,
    lifecycle_state: Arc<AtomicU8>,
    shutdown: Arc<AtomicBool>,
    totp_cache: Arc<RwLock<HashMap<Uuid, TotpSnapshot>>>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("hidlins-api-ticker".to_string())
        .spawn(move || {
            let mut last_seen_activity: u64 = 0;
            let mut grace_start: Option<Instant> = None;

            loop {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }

                // `park_timeout` rather than `sleep` so `shutdown()` can
                // `unpark` us and the join returns immediately. A plain sleep
                // made every drop cost up to a full second on the caller's
                // thread — which, via Dart's finalizer, is the UI thread.
                thread::park_timeout(Duration::from_secs(1));

                if shutdown.load(Ordering::Relaxed) {
                    break;
                }

                let activity = activity_counter.load(Ordering::Relaxed);
                let lifecycle = lifecycle_from_u8(lifecycle_state.load(Ordering::Relaxed));

                let mut state = match inner.try_lock() {
                    Ok(guard) => guard,
                    Err(std::sync::TryLockError::WouldBlock) => continue,
                    Err(std::sync::TryLockError::Poisoned(p)) => {
                        let mut s = p.into_inner();
                        s.kill();
                        break;
                    }
                };

                let locked_before = !state.is_unlocked();
                tick_inner(
                    &mut state,
                    Instant::now(),
                    activity,
                    lifecycle,
                    &mut grace_start,
                    &mut last_seen_activity,
                );
                let locked_after = !state.is_unlocked() || state.lock_pending;
                if !locked_before && locked_after {
                    clear_totp_cache_inner(&totp_cache);
                }
            }
        })
        .expect("failed to spawn ticker thread")
}

fn clear_totp_cache_inner(cache: &RwLock<HashMap<Uuid, TotpSnapshot>>) {
    match cache.write() {
        Ok(mut c) => c.clear(),
        Err(poison) => {
            poison.into_inner().clear();
        }
    }
}

fn tick_inner(
    state: &mut SessionState,
    now: Instant,
    activity: u64,
    lifecycle: LifecycleStateDto,
    grace_start: &mut Option<Instant>,
    last_seen_activity: &mut u64,
) {
    if activity != 0 && activity != *last_seen_activity {
        state.controller.register_activity(now);
    }
    *last_seen_activity = activity;

    let controller_locked = state.controller.tick(now) == LockState::Locked;
    // An affirmative lock event (OS lock / manual) drained by that tick is
    // meaningful even while no vault is open: the controller already idles
    // in `Locked` so the drain changes no state, but an unlock KDF in
    // flight must still be superseded by it. Without this, a Super+L
    // arriving mid-KDF would be consumed and the vault would commit behind
    // the OS lock screen once T1.8 wires the event sources.
    let lock_event_drained = state.controller.take_lock_event_observed();

    // Lifecycle-driven lock triggers (mobile only), tracked separately from
    // the controller verdict: with no vault open the controller idles in
    // `Locked`, so its verdict says nothing about an in-flight unlock — only
    // an *affirmative* trigger (lifecycle, or a drained lock event above)
    // may mark one pending below.
    let mut lifecycle_lock = false;

    if state.lifecycle_enabled {
        match lifecycle {
            LifecycleStateDto::Resumed => {
                if grace_start.take().is_some() {
                    state.controller.register_activity(now);
                }
            }
            LifecycleStateDto::Inactive => {}
            LifecycleStateDto::Hidden | LifecycleStateDto::Paused => {
                if grace_start.is_none() {
                    *grace_start = Some(now);
                } else if let Some(started) = *grace_start {
                    if now.duration_since(started) >= Duration::from_secs(LIFECYCLE_GRACE_SECS) {
                        lifecycle_lock = true;
                        *grace_start = None;
                    }
                }
            }
            LifecycleStateDto::Detached => {
                lifecycle_lock = true;
                *grace_start = None;
            }
        }
    }

    if state.is_unlocked() {
        if controller_locked || lifecycle_lock {
            state.do_lock();
        }
    } else if (state.unlocking || state.syncing) && (lifecycle_lock || lock_event_drained) {
        // An affirmative lock trigger fired while an unlock KDF or sync
        // worker owns the vault: mark pending so the commit phase
        // discards instead of presenting (threading rule 5).
        state.lock_pending = true;
    }
}

//! Hidlins runtime safety behaviours.
//!
//! This crate owns the three runtime safety behaviours every Hidlins
//! frontend depends on:
//!
//! - **Idle auto-lock** (FR-051) — [`auto_lock::AutoLockController`]
//!   tracks user activity and produces an [`auto_lock::LockState`] the
//!   frontend reacts to. Landed in Phase 2.
//! - **Clipboard auto-clear** (FR-053) — [`clipboard::Clipboard`]
//!   places a secret on the system clipboard, then a background timer
//!   compares-and-clears after a configurable TTL. Landed in Phase 3.
//! - **OS lock-event sources** (FR-052) — [`os_events::OsLockEventSource`]
//!   trait + concrete `NoopSource` / `SigstopSource` implementations.
//!   Arrives in Phase 4.
//!
//! ## Crate posture
//!
//! - `#![cfg_attr(not(test), deny(unsafe_code))]` — the crate is
//!   `unsafe`-free except for two locally-`#[allow]`ed, audited sites:
//!   `os_events::macos` (Phase 5b `IoKitSource`, the `IOKit` /
//!   `NSWorkspace` FFI, feature-gated behind `iokit`) and
//!   [`harden::harden_process`] (PMF-2, the always-compiled
//!   `setrlimit(RLIMIT_CORE, 0)` core-dump suppression). This mirrors
//!   the pattern `hidlins-core` reserves for its future `mlock` block:
//!   `deny` crate-wide, `#[allow(unsafe_code)]` on exactly the audited
//!   block. The flip from MVP's `forbid` to `deny` is irreversible
//!   (design §3.12, Phase 5 risk #6); a `forbid` would make the
//!   `#[allow]` a hard error.
//! - No async runtime; the controller is a pure-sync state machine
//!   driven by `tick(now)` from the frontend's event loop.
//! - One public error enum (`SecurityError`). No variant ever carries
//!   secret material; the design's mapping rule is enforced by code
//!   review (see `error.rs`).

#![cfg_attr(not(test), deny(unsafe_code))]
#![deny(missing_docs)]

pub mod auto_lock;
#[cfg(feature = "desktop")]
pub mod clipboard;
pub mod clock;
pub mod error;
pub mod harden;
pub mod os_events;
#[cfg(feature = "desktop")]
pub(crate) mod secret;
pub mod vault_lock;

pub use auto_lock::{
    AutoLockConfig, AutoLockController, LockState, OsLockReason, SecurityEvent,
    DEFAULT_IDLE_TIMEOUT,
};
#[cfg(feature = "desktop")]
pub use clipboard::{AutoClearGuard, Clipboard};
pub use clock::{Clock, SystemClock};
pub use error::SecurityError;
pub use harden::harden_process;
#[cfg(all(target_os = "macos", feature = "iokit"))]
pub use os_events::IoKitSource;
#[cfg(all(target_os = "linux", feature = "logind"))]
pub use os_events::LogindSource;
#[cfg(all(unix, feature = "desktop"))]
pub use os_events::SigstopSource;
pub use os_events::{NoopSource, OsLockEventSource, ShutdownHandle};
pub use vault_lock::{VaultLockConfig, IDLE_TIMEOUT_KEY, LOCK_TABLE_KEY};

//! Per-subcommand dispatch functions.
//!
//! Each module owns the implemented dispatch for one top-level command family.
//! The stable module shape keeps the integration-test surface and external
//! links into `hidlins_cli::commands::*` from churning.
//!
//! Helpers shared by more than one command module live here — they
//! define cross-cutting CLI contracts (the global `--registry` flag,
//! the `--format json` single-document envelope, the `--copy`
//! auto-clear TTL) that must not drift between subcommands.

#[cfg(feature = "desktop")]
use std::io::Write as _;
#[cfg(feature = "desktop")]
use std::time::Duration;

use hidlins_core::HidlinsPaths;
#[cfg(feature = "desktop")]
use hidlins_security::Clipboard;
#[cfg(not(feature = "desktop"))]
use zeroize::Zeroize as _;

use crate::cli::{Cli, OutputFormat};
use crate::exit::CliExit;
use crate::format::OutputFormatter;

pub mod completions;
pub mod entry;
pub mod gen;
pub mod keys;
pub(crate) mod open;
pub mod ssh;
pub mod sync;
pub mod vault;

/// Default auto-clear TTL for the `--copy` flag. Mirrors
/// security-behaviors' default (30s); the CLI exposes no per-invocation
/// override in MVP (open-item OQ-3).
#[cfg(feature = "desktop")]
pub(crate) const CLIPBOARD_TTL_SECONDS: u64 = 30;

/// Resolve [`HidlinsPaths`] from the global `--registry <path>` flag,
/// honoring the exact file path the user supplied; when the flag is
/// omitted, fall back to `$HOME/.local/state/hidlins/vaults.toml` via
/// [`HidlinsPaths::from_env`].
pub(crate) fn resolve_paths(cli: &Cli) -> Result<HidlinsPaths, CliExit> {
    if let Some(registry) = cli.registry.as_deref() {
        return Ok(HidlinsPaths::with_registry_file(registry.to_path_buf()));
    }
    HidlinsPaths::from_env().map_err(CliExit::from)
}

/// Open the system clipboard, place `value` on it under a
/// [`CLIPBOARD_TTL_SECONDS`] auto-clear timer, and return the guard.
/// The caller is responsible for blocking on `guard.wait_for_clear()`
/// before exit — required on Wayland because the clipboard doesn't
/// survive the source process.
///
/// Returns BEFORE any stdout writes happen so a failure (no DISPLAY,
/// spawn failure, etc.) produces a single JSON error envelope rather
/// than "success view + error envelope" on stdout.
#[cfg(feature = "desktop")]
pub(crate) type ClipboardGuard = hidlins_security::AutoClearGuard;

#[cfg(not(feature = "desktop"))]
pub(crate) struct ClipboardGuard;

#[cfg(not(feature = "desktop"))]
impl ClipboardGuard {
    pub(crate) fn wait_for_clear(&mut self) -> Result<(), hidlins_security::SecurityError> {
        Err(hidlins_security::SecurityError::ClipboardUnavailable(
            "clipboard support is unavailable in this build".to_owned(),
        ))
    }
}

#[cfg(feature = "desktop")]
pub(crate) fn arm_clipboard(value: String) -> Result<ClipboardGuard, CliExit> {
    let mut clipboard = Clipboard::new().map_err(CliExit::from)?;
    let ttl = Duration::from_secs(CLIPBOARD_TTL_SECONDS);
    let guard = clipboard
        .copy_with_autoclear(value, ttl)
        .map_err(CliExit::from)?;
    // Informational stderr note: human-mode users see why the CLI is
    // not returning to the prompt immediately. JSON-mode scripts can
    // also read this from stderr without polluting stdout.
    let _ = writeln!(
        std::io::stderr().lock(),
        "copied to clipboard; will auto-clear in {CLIPBOARD_TTL_SECONDS}s"
    );
    Ok(guard)
}

#[cfg(not(feature = "desktop"))]
pub(crate) fn arm_clipboard(mut value: String) -> Result<ClipboardGuard, CliExit> {
    value.zeroize();
    Err(CliExit::UserError(
        "clipboard support is unavailable in this build".to_owned(),
    ))
}

/// Write a success view through the [`OutputFormatter`] — the single
/// chokepoint for the `--format json` output contract.
pub(crate) fn write_success<V>(cli: &Cli, view: &V) -> Result<(), CliExit>
where
    V: serde::Serialize + crate::format::HumanFormat,
{
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let mut formatter = OutputFormatter::new(stdout.lock(), stderr.lock(), cli.format);
    formatter
        .write(view)
        .map_err(|e| CliExit::Internal(format!("failed to write output: {e}")))
}

/// Write a JSON-only success view through the shared [`OutputFormatter`].
///
/// Used when a command's canonical human output depends on runtime context
/// that does not belong in its serializable view.
pub(crate) fn write_json_success<V: serde::Serialize>(view: &V) -> Result<(), CliExit> {
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let mut formatter = OutputFormatter::new(stdout.lock(), stderr.lock(), OutputFormat::Json);
    formatter
        .write_json(view)
        .map_err(|e| CliExit::Internal(format!("failed to write output: {e}")))
}

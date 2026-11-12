//! The command registry — the single source of truth for every user-facing
//! operation, from which dispatch, help/palette, hint bar, which-key, and TOML
//! remapping all derive (design §2.2.1).
//!
//! - [`registry`] — the [`Command`] enum, the [`CommandSpec`] table
//!   ([`COMMANDS`]), context/enablement types, and the derived projections.
//! - [`keymap`] — the [`Keymap`] (trigger → command resolution), presets, and
//!   the tab-motion sequence/count state machine ([`PendingSeq`], [`Resolution`]).
//!
//! Converted from the former `keys.rs` in T1.1; the preset-aware [`Keymap`] and
//! its resolver landed in T1.2, with TOML overrides in T1.3.

pub(crate) mod keymap;
pub(crate) mod registry;

pub(crate) use keymap::{edit_save_key, Keymap, PendingSeq, Preset, Resolution};
pub(crate) use registry::Command;

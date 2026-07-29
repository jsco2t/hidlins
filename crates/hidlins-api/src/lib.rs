#![deny(unsafe_code)]
// NOTE: the `frb_expand` cfg emitted by `#[flutter_rust_bridge::frb(...)]` is
// declared via `check-cfg` in the workspace `[lints.rust]` table, NOT suppressed
// here. A crate-wide `allow(unexpected_cfgs)` would disable typo detection for
// every hand-written `#[cfg]` in the boundary crate.

pub mod api;
pub mod clipboard_port;
pub mod dto;
pub mod error;
pub mod event;
#[cfg(any(test, feature = "test-fixtures"))]
pub mod fixtures;
pub mod sync_port;
// frb-generated FFI glue necessarily contains unsafe (extern "C" fns,
// raw pointer casts, #[no_mangle]) and code patterns that trigger
// clippy lints. This is the documented exception for the boundary
// crate — the generated code is never hand-edited.
#[allow(unsafe_code, clippy::all, clippy::pedantic)]
mod frb_generated;

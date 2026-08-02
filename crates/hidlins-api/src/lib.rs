#![deny(unsafe_code)]
// NOTE: the `frb_expand` cfg emitted by `#[flutter_rust_bridge::frb(...)]` is
// declared via `check-cfg` in the workspace `[lints.rust]` table, NOT suppressed
// here. A crate-wide `allow(unexpected_cfgs)` would disable typo detection for
// every hand-written `#[cfg]` in the boundary crate.

// Android-only JNI export for the one-time rustls-platform-verifier init
// (design A9). The single `#[no_mangle]` item inside carries its own
// scoped `#[allow(unsafe_code)]` (that attribute is what trips the
// workspace lint); the rest of the module stays under the crate-wide
// deny — unlike `frb_generated` below, this is hand-written code, so the
// allowance is kept as narrow as the one item that needs it.
#[cfg(target_os = "android")]
pub mod android;
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

#![deny(unsafe_code)]
// NOTE: the `frb_expand` cfg emitted by `#[flutter_rust_bridge::frb(...)]` is
// declared via `check-cfg` in the workspace `[lints.rust]` table, NOT suppressed
// here. A crate-wide `allow(unexpected_cfgs)` would disable typo detection for
// every hand-written `#[cfg]` in the boundary crate.

// Android-only JNI export for the one-time rustls-platform-verifier init
// (design A9). `#[no_mangle]` trips the `unsafe_code` lint, so the module
// carries a scoped allowance like `frb_generated` below — it is the ONE
// hand-written FFI-export module in the crate; its body contains no
// `unsafe` blocks (safe `jni`-crate wrappers only) and the safety
// reasoning is documented in the module itself.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
pub mod android;
// SPIKE-ONLY (T6.2): TLS probe export for the scratch harness app.
// Committed only on branch spike/android-verifier — never merge.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
pub mod android_spike;
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

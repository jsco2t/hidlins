//! Run the clipboard's existing white-box contract tests against the actual
//! private source before extracting its fallible backend-construction seam.
//! This avoids exposing platform backends in the public security API.
#![cfg(feature = "desktop")]

mod error {
    pub use hidlins_security::SecurityError;
}

#[allow(dead_code)]
#[path = "../src/secret.rs"]
mod secret;

// Public API methods become private in this white-box test crate; retain their
// production signatures and consuming-drop semantics.
#[allow(dead_code, clippy::unused_self, clippy::unnecessary_wraps)]
#[path = "../src/clipboard.rs"]
mod clipboard;

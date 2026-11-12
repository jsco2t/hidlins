//! Exercise the production fallback without requiring a platform clipboard.

#[path = "../../hidlins-core/tests/common/allocation_observer.rs"]
mod allocation_observer;
// The production module also contains real-platform and recording sinks, which
// are not all used by this focused integration executable.
#[allow(dead_code)]
#[path = "../src/clipboard.rs"]
mod clipboard;

use std::time::Duration;

use allocation_observer::DeallocationWatch;
use clipboard::{ClipboardSink, UnavailableClipboard};

#[test]
fn unavailable_sink_erases_owned_password_before_error() {
    let text = String::from("synthetic unavailable clipboard password");
    let watch = DeallocationWatch::new(text.as_bytes());
    let result = UnavailableClipboard.copy(text, Duration::from_secs(30));
    assert!(matches!(
        result,
        Err(hidlins_security::SecurityError::ClipboardUnavailable(_))
    ));
    watch.assert_erased();
}

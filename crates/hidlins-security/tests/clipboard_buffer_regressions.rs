//! Inspect application-owned bytes before their allocator releases them.
//! Platform clipboard storage remains outside this ownership boundary.
#![cfg(feature = "desktop")]

#[path = "../../hidlins-core/tests/common/allocation_observer.rs"]
mod allocation_observer;
mod error {
    pub use hidlins_security::SecurityError;
}
#[allow(dead_code, clippy::unused_self, clippy::unnecessary_wraps)]
#[path = "../src/clipboard.rs"]
mod clipboard;
#[allow(dead_code)]
#[path = "../src/secret.rs"]
mod secret;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use allocation_observer::DeallocationWatch;
use clipboard::{copy_with_autoclear_factory, copy_with_autoclear_inner, ClipboardBackend};
use hidlins_security::SecurityError;

struct Backend {
    readback: Option<String>,
    cleared: Arc<AtomicBool>,
}

impl ClipboardBackend for Backend {
    fn set_text(&mut self, text: &str) -> Result<(), arboard::Error> {
        self.cleared.store(text.is_empty(), Ordering::Release);
        Ok(())
    }

    fn get_text(&mut self) -> Result<String, arboard::Error> {
        Ok(self.readback.take().expect("one timer read"))
    }
}

#[test]
fn owned_input_is_erased_when_timer_backend_initialization_fails() {
    let text = String::from("synthetic clipboard initialization fixture");
    let watch = DeallocationWatch::new(text.as_bytes());
    let mut arming = Backend {
        readback: None,
        cleared: Arc::default(),
    };
    let result = copy_with_autoclear_factory(&mut arming, text, Duration::ZERO, || {
        Err(SecurityError::ClipboardUnavailable(
            "fixture unavailable".into(),
        ))
    });
    assert!(matches!(
        result,
        Err(SecurityError::ClipboardUnavailable(_))
    ));
    watch.assert_erased();
}

fn check_readback_erasure(readback: String, should_clear: bool) {
    let watch = DeallocationWatch::new(readback.as_bytes());
    let cleared = Arc::new(AtomicBool::new(false));
    let mut arming = Backend {
        readback: None,
        cleared: Arc::default(),
    };
    let timer = Backend {
        readback: Some(readback),
        cleared: Arc::clone(&cleared),
    };
    let mut guard = copy_with_autoclear_inner(
        &mut arming,
        Box::new(timer),
        "synthetic copied fixture".into(),
        Duration::ZERO,
    )
    .unwrap();
    guard.wait_for_clear().unwrap();
    assert_eq!(cleared.load(Ordering::Acquire), should_clear);
    watch.assert_erased();
}

#[test]
fn matching_owned_readback_is_erased_after_clearing() {
    check_readback_erasure("synthetic copied fixture".into(), true);
}

#[test]
fn replaced_owned_readback_is_erased_without_clearing() {
    check_readback_erasure("synthetic other application fixture".into(), false);
}

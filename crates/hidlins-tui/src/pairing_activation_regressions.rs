//! Deterministic observation at the private authority lock boundary.
#[path = "../../hidlins-sync/tests/common/authorization_capture.rs"]
mod common;
use super::*;

pub(super) fn before_lock() {
    common::before_lock();
}

fn owner(fixture: &common::Fixture) -> common::AuthorityOwner {
    let (sender, _receiver) = mpsc::channel();
    common::AuthorityOwner {
        authority: Arc::new(TuiPairingAuthority {
            sender,
            paths: fixture.paths.clone(),
            vault_name: "vault".into(),
            pending: Mutex::new(None),
        }),
        keepalive: Box::new(()),
    }
}

#[test]
fn tui_activation_waiting_before_registry_lock_rejects_closed_window() {
    common::run_case(owner, Some(false));
}
#[test]
fn tui_activation_waiting_before_registry_lock_rejects_replaced_window() {
    common::run_case(owner, Some(true));
}
#[test]
fn tui_activation_in_live_window_remains_active_after_close() {
    common::run_case(owner, None);
}

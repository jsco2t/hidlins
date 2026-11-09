//! Deterministic observation at the private authority lock boundary.
#[path = "../../../hidlins-sync/tests/common/authorization_capture.rs"]
mod common;
use super::*;

pub(super) fn before_lock() {
    common::before_lock();
}

fn owner(fixture: &common::Fixture) -> common::AuthorityOwner {
    common::AuthorityOwner {
        authority: Arc::new(CliPairingAuthority::new(
            fixture.paths.clone(),
            "vault".into(),
        )),
        keepalive: Box::new(()),
    }
}

#[test]
fn cli_activation_waiting_before_registry_lock_rejects_closed_window() {
    common::run_case(owner, Some(false));
}
#[test]
fn cli_activation_waiting_before_registry_lock_rejects_replaced_window() {
    common::run_case(owner, Some(true));
}
#[test]
fn cli_activation_in_live_window_remains_active_after_close() {
    common::run_case(owner, None);
}

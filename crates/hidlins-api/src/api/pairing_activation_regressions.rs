//! Deterministic observation at the private authority lock boundary.
#[path = "../../../hidlins-sync/tests/common/authorization_capture.rs"]
mod common;
use super::*;

pub(super) fn before_lock() {
    common::before_lock();
}

fn owner(fixture: &common::Fixture) -> common::AuthorityOwner {
    let session = AppSession::for_test(fixture.paths.clone()).unwrap();
    session
        .unlock("vault".into(), "authority regression master".into(), None)
        .unwrap();
    let authority = Arc::new(ApiPairingAuthority::new(
        Arc::downgrade(&session.inner),
        session.pairing_api.clone(),
    ));
    common::AuthorityOwner {
        authority,
        keepalive: Box::new(session),
    }
}

#[test]
fn api_activation_waiting_before_application_lock_rejects_closed_window() {
    common::run_case(owner, Some(false));
}
#[test]
fn api_activation_waiting_before_application_lock_rejects_replaced_window() {
    common::run_case(owner, Some(true));
}
#[test]
fn api_activation_in_live_window_remains_active_after_close() {
    common::run_case(owner, None);
}

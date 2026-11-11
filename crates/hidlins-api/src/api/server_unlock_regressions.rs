//! A host ownership loan must exclude a competing vault unlock.
use super::*;
use crate::dto::LockEvent;
use crate::event::MpscEventSink;
use crate::fixtures::{create_test_vault, TestEnv};
use std::cell::{Cell, RefCell};
use std::net::TcpListener;
use std::rc::Rc;

const MASTER: &str = "original vault password";
const OTHER_MASTER: &str = "different vault password";
thread_local! {
    static CLAIM_HOOK: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}
pub(super) fn after_claim() {
    if let Some(hook) = CLAIM_HOOK.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
}
fn during_claim(session: &AppSession, action: impl FnOnce() + 'static) {
    let reached = Rc::new(Cell::new(false));
    let seen = Rc::clone(&reached);
    CLAIM_HOOK.with(|slot| {
        assert!(slot
            .borrow_mut()
            .replace(Box::new(move || {
                seen.set(true);
                action();
            }))
            .is_none());
    });
    session.pump_sync_server_for_test();
    assert!(reached.get(), "actual host ownership loan reached");
}
struct Fixture {
    session: Arc<AppSession>,
    _env: TestEnv,
}
impl Fixture {
    fn new() -> Self {
        let env = TestEnv::new();
        create_test_vault(&env.paths, "original", MASTER);
        create_test_vault(&env.paths, "other", OTHER_MASTER);
        let session = Arc::new(AppSession::for_test(env.paths.clone()).unwrap());
        session
            .unlock("original".into(), MASTER.into(), None)
            .unwrap();
        session
            .configure_local_sync(LocalSyncRoleDto::Server)
            .unwrap();
        (0..8)
            .find_map(|_| {
                let probe = TcpListener::bind("127.0.0.1:0").unwrap();
                let port = probe.local_addr().unwrap().port();
                drop(probe);
                session
                    .start_sync_server(LocalEndpointDto {
                        address: "127.0.0.1".into(),
                        port,
                        scope_id: 0,
                    })
                    .ok()
            })
            .expect("actual local server started");
        Self { session, _env: env }
    }
}
#[test]
fn competing_unlock_is_busy_and_preserves_original_vault_credentials() {
    let fixture = Fixture::new();
    let session = Arc::clone(&fixture.session);
    let rejected = Rc::new(Cell::new(false));
    let seen = Rc::clone(&rejected);
    during_claim(&fixture.session, move || {
        seen.set(matches!(
            session.unlock("other".into(), OTHER_MASTER.into(), None),
            Err(HidlinsApiError::VaultBusySyncing)
        ));
    });
    let state = fixture.session.inner.lock().unwrap();
    assert!(
        rejected.get(),
        "unlock must reject the active ownership loan"
    );
    assert_eq!(
        state.vault.as_ref().unwrap().path().file_name().unwrap(),
        "original.kdbx"
    );
    assert_eq!(
        state.credentials.as_ref().unwrap().master.as_bytes(),
        MASTER.as_bytes()
    );
    assert!(!state.syncing && !state.unlocking);
}
#[test]
fn competing_unlock_then_lock_cannot_resurrect_a_vault_on_handback() {
    let fixture = Fixture::new();
    let (tx, events) = mpsc::channel();
    fixture
        .session
        .lock_events_for_test(Box::new(MpscEventSink::new(tx)));
    assert!(matches!(events.recv().unwrap(), LockEvent::Unlocked));
    let session = Arc::clone(&fixture.session);
    during_claim(&fixture.session, move || {
        let _ = session.unlock("other".into(), OTHER_MASTER.into(), None);
        session.lock_now().unwrap();
    });
    assert!(
        !fixture.session.has_vault(),
        "host handback must not restore a vault after Locked"
    );
    assert!(!fixture.session.has_credentials());
    let events: Vec<_> = events.try_iter().collect();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], LockEvent::Locked));
}
#[test]
fn completed_loan_then_explicit_lock_allows_other_vault_unlock() {
    let fixture = Fixture::new();
    during_claim(&fixture.session, || {});
    fixture.session.lock_now().unwrap();
    fixture
        .session
        .unlock("other".into(), OTHER_MASTER.into(), None)
        .unwrap();
    let state = fixture.session.inner.lock().unwrap();
    assert_eq!(
        state.vault.as_ref().unwrap().path().file_name().unwrap(),
        "other.kdbx"
    );
    assert_eq!(
        state.credentials.as_ref().unwrap().master.as_bytes(),
        OTHER_MASTER.as_bytes()
    );
}

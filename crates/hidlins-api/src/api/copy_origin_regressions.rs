//! Clipboard values belong to the exact revocation origin that resolved them.
use std::cell::RefCell;
#[cfg(feature = "desktop")]
use std::rc::Rc;
use std::sync::Arc;
#[cfg(feature = "desktop")]
use std::time::Duration;

#[cfg(feature = "desktop")]
#[path = "../../../hidlins-core/tests/common/allocation_observer.rs"]
mod allocation_observer;

use super::*;
use crate::clipboard_port::{ClipboardPort, RecordingClipboard};
use crate::fixtures::{create_test_vault, TestEnv};
use crate::sync_port::SucceedingSyncEngine;
use hidlins_core::{EntryBuilder, MasterPassword, Vault};

const MASTER: &str = "clipboard origin master";
const OTHER_MASTER: &str = "other clipboard vault master";
const SECRET: &str = "clipboard origin password regression secret";
const URI: &str = "otpauth://totp/Example:copy?secret=JBSWY3DPEHPK3PXP&issuer=Example";
type ResolveHook = Box<dyn FnOnce(&str)>;
thread_local! {
    static RESOLVE_HOOK: RefCell<Option<ResolveHook>> = const { RefCell::new(None) };
}
pub(super) fn after_resolve(value: &str) {
    if let Some(hook) = RESOLVE_HOOK.with(|slot| slot.borrow_mut().take()) {
        hook(value);
    }
}
fn hook(action: impl FnOnce(&str) + 'static) {
    RESOLVE_HOOK.with(|slot| assert!(slot.borrow_mut().replace(Box::new(action)).is_none()));
}
struct Fixture {
    session: Arc<AppSession>,
    clipboard: Arc<RecordingClipboard>,
    credential: String,
    otp: String,
    _env: TestEnv,
}
impl Fixture {
    fn new(available: bool) -> Self {
        let env = TestEnv::new();
        let path = create_test_vault(&env.paths, "original", MASTER);
        create_test_vault(&env.paths, "other", OTHER_MASTER);
        let mut vault = Vault::open(&path, &MasterPassword::new(MASTER.into()), None).unwrap();
        let credential = vault
            .add_entry(
                vault.root_group_uuid(),
                EntryBuilder::credential("copy fixture")
                    .username("fixture-user")
                    .password(SECRET)
                    .build(),
            )
            .unwrap();
        vault
            .update_entry(credential, |entry| {
                entry.set_custom_field("protected", SECRET, true);
                Ok(())
            })
            .unwrap();
        let otp = vault
            .add_entry(
                vault.root_group_uuid(),
                EntryBuilder::totp("copy OTP", URI).unwrap().build(),
            )
            .unwrap();
        vault.save().unwrap();
        drop(vault);
        let clipboard = Arc::new(RecordingClipboard::new());
        let port: Arc<dyn ClipboardPort> = clipboard.clone();
        let session = Arc::new(
            AppSession::with_ports(
                env.paths.clone(),
                available.then_some(port),
                Arc::new(SucceedingSyncEngine(
                    hidlins_sync::SyncOutcome::AlreadyInSync,
                )),
            )
            .unwrap(),
        );
        session
            .unlock("original".into(), MASTER.into(), None)
            .unwrap();
        session.entry_detail(otp.to_string()).unwrap();
        Self {
            session,
            clipboard,
            credential: credential.to_string(),
            otp: otp.to_string(),
            _env: env,
        }
    }
    fn rejected(&self, uuid: &str, field: CopyField) {
        assert!(
            copy(&self.session, uuid, field).is_err(),
            "a resolved value must not publish across lock or vault origin change"
        );
        assert!(self.clipboard.last_copied().is_none());
    }
}
#[cfg(feature = "desktop")]
fn copy(
    session: &AppSession,
    uuid: &str,
    field: CopyField,
) -> Result<Option<ClipboardTransferTicket>, HidlinsApiError> {
    session.copy_entry_field(uuid.into(), field).map(|()| None)
}
#[cfg(not(feature = "desktop"))]
fn copy(
    session: &AppSession,
    uuid: &str,
    field: CopyField,
) -> Result<Option<ClipboardTransferTicket>, HidlinsApiError> {
    session
        .prepare_clipboard_transfer(uuid.into(), field)
        .map(Some)
}
#[test]
fn password_resolved_before_lock_cannot_publish_after_lock() {
    let fixture = Fixture::new(true);
    let session = Arc::clone(&fixture.session);
    hook(move |_| session.lock_now().unwrap());
    fixture.rejected(&fixture.credential, CopyField::Password);
}
#[test]
fn username_resolved_before_same_vault_aba_cannot_publish() {
    let fixture = Fixture::new(true);
    let session = Arc::clone(&fixture.session);
    hook(move |_| {
        session.lock_now().unwrap();
        session
            .unlock("original".into(), MASTER.into(), None)
            .unwrap();
    });
    fixture.rejected(&fixture.credential, CopyField::Username);
    assert!(fixture.session.has_vault());
}
#[test]
fn protected_custom_resolved_before_vault_switch_cannot_publish() {
    let fixture = Fixture::new(true);
    let session = Arc::clone(&fixture.session);
    hook(move |_| {
        session.lock_now().unwrap();
        session
            .unlock("other".into(), OTHER_MASTER.into(), None)
            .unwrap();
    });
    fixture.rejected(
        &fixture.credential,
        CopyField::CustomField("protected".into()),
    );
    assert!(fixture.session.has_vault());
}
#[test]
fn totp_resolved_before_same_vault_aba_cannot_publish() {
    let fixture = Fixture::new(true);
    let session = Arc::clone(&fixture.session);
    hook(move |_| {
        session.lock_now().unwrap();
        session
            .unlock("original".into(), MASTER.into(), None)
            .unwrap();
    });
    fixture.rejected(&fixture.otp, CopyField::TotpCode);
}
#[test]
fn resolved_password_cannot_publish_after_shutdown() {
    let fixture = Fixture::new(true);
    let session = Arc::clone(&fixture.session);
    hook(move |_| session.shutdown());
    fixture.rejected(&fixture.credential, CopyField::Password);
}
#[test]
fn current_password_copy_preserves_value_and_thirty_second_ttl() {
    let fixture = Fixture::new(true);
    let ticket = copy(&fixture.session, &fixture.credential, CopyField::Password).unwrap();
    #[cfg(feature = "desktop")]
    {
        assert!(ticket.is_none());
        assert!(fixture.clipboard.last_copied().as_deref() == Some(SECRET));
        assert_eq!(fixture.clipboard.last_ttl(), Some(Duration::from_secs(30)));
    }
    #[cfg(not(feature = "desktop"))]
    {
        let ticket = ticket.unwrap();
        assert_eq!(ticket.expires_after_secs, 30);
        assert!(consume_clipboard_transfer(&ticket.id).unwrap().as_str() == SECRET);
    }
}
#[cfg(not(feature = "desktop"))]
#[test]
fn native_current_ticket_is_single_use() {
    let fixture = Fixture::new(true);
    let ticket = copy(&fixture.session, &fixture.credential, CopyField::Username)
        .unwrap()
        .unwrap();
    assert!(consume_clipboard_transfer(&ticket.id).unwrap().as_str() == "fixture-user");
    assert!(consume_clipboard_transfer(&ticket.id).is_err());
}
#[cfg(feature = "desktop")]
#[test]
fn unavailable_clipboard_erases_resolved_password_before_deallocation() {
    let fixture = Fixture::new(false);
    let observed = Rc::new(RefCell::new(None));
    let saved = Rc::clone(&observed);
    hook(move |value| {
        *saved.borrow_mut() = Some(allocation_observer::DeallocationWatch::new(
            value.as_bytes(),
        ));
    });
    assert!(matches!(
        copy(&fixture.session, &fixture.credential, CopyField::Password),
        Err(HidlinsApiError::Io { .. })
    ));
    observed
        .borrow_mut()
        .take()
        .expect("actual resolved allocation observed")
        .assert_erased();
}
#[cfg(feature = "desktop")]
#[test]
fn lock_waits_for_an_already_started_clipboard_publication() {
    use std::sync::{mpsc, Mutex};
    struct BlockingClipboard {
        entered: mpsc::Sender<()>,
        release: Mutex<mpsc::Receiver<()>>,
        recorded: RecordingClipboard,
    }
    impl ClipboardPort for BlockingClipboard {
        fn copy_with_autoclear(&self, text: String, ttl: Duration) -> Result<(), HidlinsApiError> {
            let mut text = Zeroizing::new(text);
            self.entered.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            self.recorded
                .copy_with_autoclear(std::mem::take(&mut *text), ttl)
        }
    }
    let mut fixture = Fixture::new(true);
    let (entered, received) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let port = Arc::new(BlockingClipboard {
        entered,
        release: Mutex::new(released),
        recorded: RecordingClipboard::new(),
    });
    Arc::get_mut(&mut fixture.session).unwrap().clipboard = Some(port.clone());
    let session = Arc::clone(&fixture.session);
    let uuid = fixture.credential.clone();
    let copying = std::thread::spawn(move || session.copy_entry_field(uuid, CopyField::Password));
    received.recv_timeout(Duration::from_secs(2)).unwrap();
    let (starting, started) = mpsc::channel();
    let (locked, observed_lock) = mpsc::channel();
    let session = Arc::clone(&fixture.session);
    let locking = std::thread::spawn(move || {
        starting.send(()).unwrap();
        session.lock_now().unwrap();
        locked.send(()).unwrap();
    });
    started.recv_timeout(Duration::from_secs(2)).unwrap();
    let locked_before_publication = observed_lock
        .recv_timeout(Duration::from_millis(400))
        .is_ok();
    release.send(()).unwrap();
    copying.join().unwrap().unwrap();
    locking.join().unwrap();
    assert!(port.recorded.last_copied().as_deref() == Some(SECRET));
    assert!(!fixture.session.has_vault());
    assert!(
        !locked_before_publication,
        "Lock returned while a clipboard publication could still write afterward"
    );
}

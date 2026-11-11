//! Exact generated output must use its own origin, never an entry UUID or
//! a mutable last-generated slot. Legacy fallback mirrors pre-fix dispatch;
//! the dedicated inherent API takes precedence once implemented.
use super::*;
use crate::clipboard_port::{ClipboardPort, RecordingClipboard};
use crate::dto::{GeneratedSecret, PassphraseOptionsDto, PasswordOptionsDto};
use crate::fixtures::{create_test_vault, TestEnv};
use crate::sync_port::SucceedingSyncEngine;
use std::sync::Arc;

trait LegacyGeneratedCopy {
    #[cfg(feature = "desktop")]
    fn copy_generated_secret(&self, result: GeneratedSecret) -> Result<(), HidlinsApiError>;
    #[cfg(not(feature = "desktop"))]
    fn prepare_generated_clipboard_transfer(
        &self,
        result: GeneratedSecret,
    ) -> Result<ClipboardTransferTicket, HidlinsApiError>;
}
impl LegacyGeneratedCopy for AppSession {
    #[cfg(feature = "desktop")]
    fn copy_generated_secret(&self, _result: GeneratedSecret) -> Result<(), HidlinsApiError> {
        self.copy_entry_field("generator".into(), CopyField::Password)
    }
    #[cfg(not(feature = "desktop"))]
    fn prepare_generated_clipboard_transfer(
        &self,
        _result: GeneratedSecret,
    ) -> Result<ClipboardTransferTicket, HidlinsApiError> {
        self.prepare_clipboard_transfer("generator".into(), CopyField::Password)
    }
}

const MASTER: &str = "generated copy master fixture";
struct Fixture {
    session: AppSession,
    clipboard: Arc<RecordingClipboard>,
    _env: TestEnv,
}
impl Fixture {
    fn new() -> Self {
        let env = TestEnv::new();
        create_test_vault(&env.paths, "first", MASTER);
        create_test_vault(&env.paths, "second", MASTER);
        let clipboard = Arc::new(RecordingClipboard::new());
        let port: Arc<dyn ClipboardPort> = clipboard.clone();
        let session = AppSession::with_ports(
            env.paths.clone(),
            Some(port),
            Arc::new(SucceedingSyncEngine(
                hidlins_sync::SyncOutcome::AlreadyInSync,
            )),
        )
        .unwrap();
        session.unlock("first".into(), MASTER.into(), None).unwrap();
        Self {
            session,
            clipboard,
            _env: env,
        }
    }
    fn random(&self) -> GeneratedSecret {
        self.session
            .generate_password(PasswordOptionsDto {
                length: 32,
                lowercase: true,
                uppercase: true,
                digits: true,
                symbols: true,
                exclude_ambiguous: false,
            })
            .unwrap()
    }
    fn copy(
        &self,
        result: GeneratedSecret,
    ) -> Result<Option<ClipboardTransferTicket>, HidlinsApiError> {
        #[cfg(feature = "desktop")]
        {
            self.session.copy_generated_secret(result).map(|()| None)
        }
        #[cfg(not(feature = "desktop"))]
        {
            self.session
                .prepare_generated_clipboard_transfer(result)
                .map(Some)
        }
    }
    fn assert_copy(&self, result: GeneratedSecret) {
        let expected = Zeroizing::new(result.value.clone());
        let ticket = self
            .copy(result)
            .expect("the actual displayed generated result must copy");
        #[cfg(feature = "desktop")]
        {
            assert!(ticket.is_none());
            assert!(self.clipboard.last_copied().as_deref() == Some(expected.as_str()));
            assert_eq!(self.clipboard.last_ttl(), Some(Duration::from_secs(30)));
        }
        #[cfg(not(feature = "desktop"))]
        {
            let ticket = ticket.unwrap();
            assert_eq!(ticket.expires_after_secs, 30);
            assert!(consume_clipboard_transfer(&ticket.id).unwrap().as_str() == expected.as_str());
            assert!(self.clipboard.last_copied().is_none());
        }
    }
}

#[test]
fn generated_random_copies_exact_bytes_and_thirty_second_ttl() {
    let fixture = Fixture::new();
    fixture.assert_copy(fixture.random());
}
#[test]
fn generated_diceware_copies_exact_displayed_bytes() {
    let fixture = Fixture::new();
    let result = fixture
        .session
        .generate_passphrase(PassphraseOptionsDto {
            words: 6,
            separator: "-".into(),
        })
        .unwrap();
    fixture.assert_copy(result);
}
#[test]
fn subsequent_generations_cannot_evict_the_displayed_result() {
    let fixture = Fixture::new();
    let displayed = fixture.random();
    for _ in 0..40 {
        drop(fixture.random());
    }
    fixture.assert_copy(displayed);
}
#[test]
fn generated_result_is_revoked_by_same_vault_lock_reopen() {
    let fixture = Fixture::new();
    let displayed = fixture.random();
    fixture.session.lock_now().unwrap();
    fixture
        .session
        .unlock("first".into(), MASTER.into(), None)
        .unwrap();
    assert!(fixture.copy(displayed).is_err());
    assert!(fixture.clipboard.last_copied().is_none());
    fixture.assert_copy(fixture.random());
}
#[test]
fn generated_result_is_revoked_by_vault_switch() {
    let fixture = Fixture::new();
    let displayed = fixture.random();
    fixture.session.lock_now().unwrap();
    fixture
        .session
        .unlock("second".into(), MASTER.into(), None)
        .unwrap();
    assert!(fixture.copy(displayed).is_err());
    assert!(fixture.clipboard.last_copied().is_none());
    fixture.assert_copy(fixture.random());
}
#[test]
fn generated_result_cannot_publish_after_shutdown() {
    let fixture = Fixture::new();
    let displayed = fixture.random();
    fixture.session.shutdown();
    assert!(fixture.copy(displayed).is_err());
    assert!(fixture.clipboard.last_copied().is_none());
}
#[test]
fn stateless_generation_while_locked_is_preserved_but_copy_requires_vault() {
    let fixture = Fixture::new();
    fixture.session.lock_now().unwrap();
    let generated = fixture.random();
    assert_eq!(generated.value.len(), 32);
    assert!(fixture.copy(generated).is_err());
}
#[test]
fn legacy_invalid_entry_dispatch_remains_a_negative_control() {
    let fixture = Fixture::new();
    #[cfg(feature = "desktop")]
    assert!(
        LegacyGeneratedCopy::copy_generated_secret(&fixture.session, fixture.random()).is_err()
    );
    #[cfg(not(feature = "desktop"))]
    assert!(LegacyGeneratedCopy::prepare_generated_clipboard_transfer(
        &fixture.session,
        fixture.random()
    )
    .is_err());
}
#[cfg(not(feature = "desktop"))]
#[test]
fn native_generated_ticket_is_single_use() {
    let fixture = Fixture::new();
    let result = fixture.random();
    let expected = Zeroizing::new(result.value.clone());
    let ticket = fixture.copy(result).unwrap().unwrap();
    assert!(consume_clipboard_transfer(&ticket.id).unwrap().as_str() == expected.as_str());
    assert!(consume_clipboard_transfer(&ticket.id).is_err());
}
#[cfg(not(feature = "desktop"))]
#[test]
fn native_generated_ticket_is_purged_on_lock() {
    let fixture = Fixture::new();
    let ticket = fixture.copy(fixture.random()).unwrap().unwrap();
    fixture.session.lock_now().unwrap();
    assert!(consume_clipboard_transfer(&ticket.id).is_err());
}

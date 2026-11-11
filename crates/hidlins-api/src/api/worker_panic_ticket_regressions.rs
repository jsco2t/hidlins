//! Caught worker panic must revoke native transfers before publishing Locked.
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use hidlins_core::{EntryBuilder, MasterPassword, Vault, VaultRegistry};
use hidlins_sync::SyncOutcome;

use super::AppSession;
use crate::dto::{CopyField, DiscoveryPermissionDto, LockEvent};
use crate::error::HidlinsApiError;
use crate::event::EventSink;
use crate::fixtures::{create_test_vault, TestEnv};
use crate::sync_port::{PanickingSyncEngine, SucceedingSyncEngine, SyncEnginePort};

const MASTER: &str = "panic-ticket regression master";

fn active_client_config() -> hidlins_sync::config::local::LocalSyncConfig {
    use hidlins_sync::config::local::LocalSyncConfig;
    use hidlins_sync::identity::SyncRole;
    use hidlins_sync::pairing::PairingWindow;
    let master = MasterPassword::new(MASTER.into());
    let mut client = LocalSyncConfig::create("vault", SyncRole::Client, &master).unwrap();
    let mut server = LocalSyncConfig::create("vault", SyncRole::Server, &master).unwrap();
    let now = Instant::now();
    let hash = [37; 32];
    let mut client_window = PairingWindow::open_at(now);
    let sas = client_window
        .begin_candidate(server.identity().public_key(), hash, now)
        .unwrap();
    let client_confirmation = client_window.confirm(sas, now).unwrap();
    let mut server_window = PairingWindow::open_at(now);
    let sas = server_window
        .begin_candidate(client.identity().public_key(), hash, now)
        .unwrap();
    let server_confirmation = server_window.confirm(sas, now).unwrap();
    let transaction = client_confirmation
        .transaction(
            server.identity().public_key(),
            client.identity().public_key(),
        )
        .unwrap();
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    server
        .prepare_client(
            &transaction,
            &server_confirmation,
            "client".into(),
            epoch,
            epoch + 180,
        )
        .unwrap();
    client
        .prepare_server(
            &transaction,
            &client_confirmation,
            "server".into(),
            epoch,
            epoch + 180,
        )
        .unwrap();
    server.activate_client(&transaction, epoch).unwrap();
    client.activate_server(&transaction, epoch).unwrap();
    client
}

fn session(engine: Arc<dyn SyncEnginePort>) -> (TestEnv, AppSession, String) {
    let env = TestEnv::new();
    let path = create_test_vault(&env.paths, "vault", MASTER);
    let master = MasterPassword::new(MASTER.into());
    let mut vault = Vault::open(&path, &master, None).unwrap();
    let uuid = vault
        .add_entry(
            vault.root_group_uuid(),
            EntryBuilder::totp(
                "OTP",
                "otpauth://totp/Example:test?secret=JBSWY3DPEHPK3PXP&issuer=Example",
            )
            .unwrap()
            .build(),
        )
        .unwrap()
        .to_string();
    vault.save().unwrap();
    drop(vault);
    let mut registry = VaultRegistry::load(env.paths.clone()).unwrap();
    active_client_config()
        .persist(&mut registry, "vault")
        .unwrap();
    let mut session = AppSession::for_test_mobile(env.paths.clone()).unwrap();
    session.sync_engine = engine;
    session
        .set_discovery_candidates(DiscoveryPermissionDto::Granted, Vec::new())
        .unwrap();
    session.unlock("vault".into(), MASTER.into(), None).unwrap();
    session.entry_detail(uuid.clone()).unwrap();
    assert!(session.totp_now(uuid.clone()).is_ok());
    (env, session, uuid)
}

struct ObserveLocked {
    ticket: String,
    tx: mpsc::Sender<bool>,
}

impl EventSink<LockEvent> for ObserveLocked {
    fn send(&self, event: LockEvent) {
        if matches!(event, LockEvent::Locked) {
            // Observe the actual native consumption boundary during notification,
            // rather than consuming the ticket after the lock event has returned.
            let revoked = crate::api::secrets::consume_clipboard_transfer(&self.ticket).is_err();
            let _ = self.tx.send(revoked);
        }
    }
}

#[test]
fn caught_panic_revokes_ticket_before_locked_event() {
    let (_env, session, uuid) = session(Arc::new(PanickingSyncEngine));
    let ticket = session
        .prepare_clipboard_transfer(uuid.clone(), CopyField::TotpCode)
        .unwrap();
    let (tx, rx) = mpsc::channel();
    session.lock_events_for_test(Box::new(ObserveLocked {
        ticket: ticket.id,
        tx,
    }));
    assert!(matches!(
        session.sync_now(),
        Err(HidlinsApiError::Internal { .. })
    ));
    let revoked = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(rx.try_iter().count(), 0, "exactly one Locked event");
    assert!(
        !session.is_dead(),
        "caught worker panic remains recoverable"
    );
    assert!(matches!(
        session.entry_detail(uuid.clone()),
        Err(HidlinsApiError::VaultLocked)
    ));
    assert!(session.lock_state().credentials.is_none());
    session.unlock("vault".into(), MASTER.into(), None).unwrap();
    assert!(session.entry_detail(uuid).is_ok());
    assert!(
        revoked,
        "native ticket must be revoked before Locked is delivered"
    );
}

#[test]
fn successful_sync_preserves_prepared_native_ticket() {
    let (_env, session, uuid) = session(Arc::new(SucceedingSyncEngine(SyncOutcome::AlreadyInSync)));
    let ticket = session
        .prepare_clipboard_transfer(uuid, CopyField::TotpCode)
        .unwrap();
    session.sync_now().unwrap();
    let value = crate::api::secrets::consume_clipboard_transfer(&ticket.id).unwrap();
    assert_eq!(value.len(), 6);
    assert!(value.bytes().all(|byte| byte.is_ascii_digit()));
    assert!(crate::api::secrets::consume_clipboard_transfer(&ticket.id).is_err());
}

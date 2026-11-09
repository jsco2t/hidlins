//! Regression tests for original-window pairing authorization.

use super::*;
use crate::{
    client::{recover_pairing, ClientPairingSession},
    config::local::LocalSyncConfig,
    identity::{PublicIdentity, SyncRole},
    trust::TrustError,
};
use hidlins_core::MasterPassword;
use std::{
    sync::atomic::AtomicUsize,
    time::{SystemTime, UNIX_EPOCH},
};

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

struct InterruptedAuthority {
    config: Mutex<LocalSyncConfig>,
    activations: AtomicUsize,
}

impl PairingAuthority for InterruptedAuthority {
    fn confirm(
        &self,
        _peer: PublicIdentity,
        _sas: SasCode,
        _timeout: Duration,
    ) -> Result<String, PairingAuthorityError> {
        Ok("confirmed client".into())
    }

    fn prepare(
        &self,
        transaction: &PairingTransaction,
        confirmation: &ConfirmedPairing,
        display_name: String,
    ) -> Result<(), PairingAuthorityError> {
        let now = epoch_seconds();
        self.config
            .lock()
            .unwrap()
            .prepare_client(transaction, confirmation, display_name, now, now + 180)
            .map_err(|_| PairingAuthorityError::Persistence)
    }

    fn activate(&self, transaction: &PairingTransaction) -> Result<(), PairingAuthorityError> {
        if self.activations.fetch_add(1, Ordering::AcqRel) == 0 {
            return Err(PairingAuthorityError::Persistence);
        }
        self.config
            .lock()
            .unwrap()
            .activate_client(transaction, epoch_seconds())
            .map_err(|_| PairingAuthorityError::Persistence)
    }

    fn recover(&self, peer: PublicIdentity) -> Result<PairingTransaction, PairingAuthorityError> {
        self.config
            .lock()
            .unwrap()
            .recover_pairing_transaction(peer, epoch_seconds())
            .map_err(|_| PairingAuthorityError::Persistence)
    }

    fn shutdown(&self) {}
}

struct InterruptedPairing {
    server: ServerController,
    client: LocalSyncConfig,
    authority: Arc<InterruptedAuthority>,
    master: MasterPassword,
    processor: super::super::HostProcessor,
}

impl InterruptedPairing {
    fn new() -> Self {
        let master = MasterPassword::new("window regression master".into());
        let server_config = LocalSyncConfig::create("vault", SyncRole::Server, &master).unwrap();
        let mut client = LocalSyncConfig::create("vault", SyncRole::Client, &master).unwrap();
        let identity = server_config
            .identity()
            .unlock("vault", SyncRole::Server, &master)
            .unwrap();
        let authority = Arc::new(InterruptedAuthority {
            config: Mutex::new(server_config),
            activations: AtomicUsize::new(0),
        });
        let (queue, processor) = HostQueue::new();
        let server = ServerController::start_on_bind_endpoint_with_pairing_authority(
            LocalBindEndpoint::ephemeral("127.0.0.1".parse().unwrap(), 0).unwrap(),
            Arc::new(identity),
            [],
            queue,
            Some(authority.clone()),
        )
        .unwrap();
        server.open_pairing().unwrap();
        let identity = client
            .identity()
            .unlock("vault", SyncRole::Client, &master)
            .unwrap();
        let session = ClientPairingSession::begin(&identity, [server.endpoint()]).unwrap();
        assert!(session
            .confirm(&mut client, "server".into(), epoch_seconds(), |_| Ok(()))
            .is_err());
        assert!(client.provisional().is_some());
        assert!(!client.is_active_client());
        assert!(!authority
            .config
            .lock()
            .unwrap()
            .authorizes(client.identity().public_key()));
        Self {
            server,
            client,
            authority,
            master,
            processor,
        }
    }

    fn recover(&mut self) -> Result<PairingTransaction, crate::client::LanError> {
        recover_pairing(
            &mut self.client,
            "vault",
            &self.master,
            [self.server.endpoint()],
            epoch_seconds(),
            |_| Ok(()),
        )
    }

    fn assert_inactive(&self) {
        assert!(!self.client.is_active_client());
        assert!(!self
            .authority
            .config
            .lock()
            .unwrap()
            .authorizes(self.client.identity().public_key()));
        assert!(self.server.trusted.permits.read().unwrap().is_empty());
    }
}

#[test]
fn recovery_succeeds_during_its_original_window_and_close_preserves_active_trust() {
    let mut pairing = InterruptedPairing::new();
    pairing.recover().expect("original-window recovery");
    assert!(pairing.client.is_active_client());
    let peer = pairing.client.identity().public_key();
    pairing.server.close_pairing();
    assert!(pairing.authority.config.lock().unwrap().authorizes(peer));
    assert!(matches!(
        pairing.server.trusted.admit(peer.into_bytes()),
        Some(TrustedAdmission::Active(_))
    ));
}

#[test]
fn closed_window_cannot_activate_an_unexpired_provisional_receipt() {
    let mut pairing = InterruptedPairing::new();
    pairing.server.close_pairing();
    assert!(
        pairing.recover().is_err(),
        "closing pairing must end recovery authority"
    );
    pairing.assert_inactive();
}

#[test]
fn automatic_monotonic_expiry_cannot_activate_an_unexpired_receipt() {
    let mut pairing = InterruptedPairing::new();
    // Inject only the existing window clock, then execute the actual expiry path.
    *pairing.server.pairing.window.lock().unwrap() = Some(PairingWindow::open_at(
        Instant::now().checked_sub(PAIRING_WINDOW).unwrap(),
    ));
    pairing.server.pairing.expire(Instant::now());
    assert!(
        pairing.recover().is_err(),
        "expired window must end recovery authority"
    );
    pairing.assert_inactive();
}

#[test]
fn replacement_window_cannot_reauthorize_an_older_receipt() {
    let mut pairing = InterruptedPairing::new();
    pairing.server.open_pairing().unwrap();
    assert!(
        pairing.recover().is_err(),
        "new window cannot extend the old transaction"
    );
    pairing.assert_inactive();
}

#[test]
fn restored_provisional_key_cannot_recreate_a_closed_server_window() {
    let mut pairing = InterruptedPairing::new();
    pairing.server.stop();
    let config = pairing.authority.config.lock().unwrap();
    let identity = config
        .identity()
        .unlock("vault", SyncRole::Server, &pairing.master)
        .unwrap();
    drop(config);
    let (queue, processor) = HostQueue::new();
    pairing.server = ServerController::start_on_bind_endpoint_with_pairing_authority(
        LocalBindEndpoint::ephemeral("127.0.0.1".parse().unwrap(), 0).unwrap(),
        Arc::new(identity),
        [],
        queue,
        Some(pairing.authority.clone()),
    )
    .unwrap();
    pairing.processor = processor;
    pairing
        .server
        .replace_pairing_recovery([pairing.client.identity().public_key().into_bytes()]);
    assert!(
        pairing.recover().is_err(),
        "restarted server has no original live window"
    );
    pairing.assert_inactive();
}

fn proof_from_window_opened_at(
    opened: Instant,
) -> (LocalSyncConfig, ConfirmedPairing, PairingTransaction) {
    let master = MasterPassword::new("proof regression master".into());
    let config = LocalSyncConfig::create("vault", SyncRole::Server, &master).unwrap();
    let peer = PublicIdentity::new([9; 32]);
    let mut window = PairingWindow::open_at(opened);
    let sas = window.begin_candidate(peer, [8; 32], opened).unwrap();
    let proof = window.confirm(sas, opened).unwrap();
    let transaction = proof
        .transaction(config.identity().public_key(), peer)
        .unwrap();
    (config, proof, transaction)
}

#[test]
fn late_confirmation_cannot_extend_the_original_durable_window() {
    let (mut config, proof, transaction) = proof_from_window_opened_at(
        Instant::now()
            .checked_sub(Duration::from_secs(170))
            .unwrap(),
    );
    config
        .prepare_client(&transaction, &proof, "client".into(), 1_000, 1_180)
        .unwrap();
    assert_eq!(
        config.recover_pairing_transaction(transaction.client_key(), 1_011),
        Err(TrustError::Expired)
    );
}

#[test]
fn expired_confirmation_cannot_prepare_a_fresh_durable_receipt() {
    let (mut config, proof, transaction) =
        proof_from_window_opened_at(Instant::now().checked_sub(PAIRING_WINDOW).unwrap());
    assert_eq!(
        config.prepare_client(&transaction, &proof, "client".into(), 1_000, 1_180),
        Err(TrustError::Expired)
    );
    assert!(config.provisional().is_none());
}

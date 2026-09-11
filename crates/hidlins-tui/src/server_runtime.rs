//! Process-local authoritative sync-server ownership for the TUI.

use std::{
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

use hidlins_core::{HidlinsPaths, MasterPassword, VaultRegistry};
use hidlins_sync::{
    config::local::LocalSyncConfig,
    discovery::desktop,
    identity::{PublicIdentity, SyncRole},
    noise::NoiseKeypair,
    pairing::{ConfirmedPairing, PairingTransaction, SasCode},
    server::{
        HostProcessor, HostQueue, PairingAuthority, PairingAuthorityError, PendingHostOperation,
        ServerController,
    },
    trust::PeerStatus,
};

pub(crate) enum AuthorityRequest {
    Confirm {
        sas: SasCode,
        reply: mpsc::SyncSender<Option<String>>,
    },
    ReloadTrust,
}

struct TuiPairingAuthority {
    sender: mpsc::Sender<AuthorityRequest>,
    paths: HidlinsPaths,
    vault_name: String,
    pending: Mutex<Option<mpsc::SyncSender<Option<String>>>>,
}

impl PairingAuthority for TuiPairingAuthority {
    fn confirm(
        &self,
        _peer: PublicIdentity,
        sas: SasCode,
        timeout: Duration,
    ) -> Result<String, PairingAuthorityError> {
        let (reply, answer) = mpsc::sync_channel(1);
        if let Ok(mut pending) = self.pending.lock() {
            *pending = Some(reply.clone());
        }
        self.sender
            .send(AuthorityRequest::Confirm { sas, reply })
            .map_err(|_| PairingAuthorityError::Canceled)?;
        let result = answer
            .recv_timeout(timeout)
            .map_err(|_| PairingAuthorityError::Canceled)?
            .ok_or(PairingAuthorityError::Canceled);
        if let Ok(mut pending) = self.pending.lock() {
            *pending = None;
        }
        result
    }

    fn prepare(
        &self,
        transaction: &PairingTransaction,
        confirmation: &ConfirmedPairing,
        display_name: String,
    ) -> Result<(), PairingAuthorityError> {
        let mut registry = VaultRegistry::load(self.paths.clone())
            .map_err(|_| PairingAuthorityError::Persistence)?;
        let now = epoch_seconds().ok_or(PairingAuthorityError::Persistence)?;
        LocalSyncConfig::transactional_update(&mut registry, &self.vault_name, |config| {
            config.prepare_client(
                transaction,
                confirmation,
                display_name,
                now,
                now.saturating_add(hidlins_sync::protocol::PAIRING_WINDOW.as_secs()),
            )
        })
        .map_err(|_| PairingAuthorityError::Persistence)
    }

    fn activate(&self, transaction: &PairingTransaction) -> Result<(), PairingAuthorityError> {
        let mut registry = VaultRegistry::load(self.paths.clone())
            .map_err(|_| PairingAuthorityError::Persistence)?;
        let now = epoch_seconds().ok_or(PairingAuthorityError::Persistence)?;
        LocalSyncConfig::transactional_update(&mut registry, &self.vault_name, |config| {
            config.activate_client(transaction, now)
        })
        .map_err(|_| PairingAuthorityError::Persistence)?;
        let _ = self.sender.send(AuthorityRequest::ReloadTrust);
        Ok(())
    }

    fn recover(&self, peer: PublicIdentity) -> Result<PairingTransaction, PairingAuthorityError> {
        let registry = VaultRegistry::load(self.paths.clone())
            .map_err(|_| PairingAuthorityError::Persistence)?;
        let config = registry
            .get(&self.vault_name)
            .and_then(LocalSyncConfig::from_vault_entry)
            .ok_or(PairingAuthorityError::Persistence)?;
        let now = epoch_seconds().ok_or(PairingAuthorityError::Persistence)?;
        config
            .recover_pairing_transaction(peer, now)
            .map_err(|_| PairingAuthorityError::Persistence)
    }

    fn shutdown(&self) {
        self.cancel_pending();
    }

    fn cancel_pending(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            if let Some(reply) = pending.take() {
                let _ = reply.try_send(None);
            }
        }
    }
}

pub(crate) struct ServerRuntime {
    controller: ServerController,
    processor: HostProcessor,
    master: Option<MasterPassword>,
    authority: mpsc::Receiver<AuthorityRequest>,
}

impl ServerRuntime {
    pub(crate) fn start(
        config: &LocalSyncConfig,
        vault_name: &str,
        master: MasterPassword,
        paths: HidlinsPaths,
    ) -> Result<Self, String> {
        let identity: NoiseKeypair = config
            .identity()
            .unlock(vault_name, SyncRole::Server, &master)
            .map_err(|_| "Master password did not unlock the sync identity.".to_string())?;
        let endpoint = desktop::allowed_interface_bind_endpoints()
            .map_err(|_| "No private or link-local network interface is available.".to_string())?
            .into_iter()
            .next()
            .ok_or_else(|| {
                "No private or link-local network interface is available.".to_string()
            })?;
        let trusted = config
            .trusted_peers()
            .iter()
            .filter(|peer| peer.status() == PeerStatus::Active)
            .map(|peer| peer.public_key().into_bytes())
            .collect::<Vec<_>>();
        let (queue, processor) = HostQueue::new();
        let (sender, authority) = mpsc::channel();
        let pairing_authority: Arc<dyn PairingAuthority> = Arc::new(TuiPairingAuthority {
            sender,
            paths,
            vault_name: vault_name.to_string(),
            pending: Mutex::new(None),
        });
        let controller = ServerController::start_on_bind_endpoint_with_pairing_authority(
            endpoint,
            Arc::new(identity),
            trusted,
            queue,
            Some(pairing_authority),
        )
        .map_err(|_| "Could not start the local sync listener.".to_string())?;
        controller.replace_pairing_recovery(
            config
                .provisional()
                .map(|record| record.peer_key().into_bytes()),
        );
        Ok(Self {
            controller,
            processor,
            master: Some(master),
            authority,
        })
    }

    pub(crate) fn endpoint(&self) -> String {
        self.controller.endpoint().to_string()
    }

    pub(crate) fn open_pairing(&self) -> Result<(), String> {
        self.controller
            .open_pairing()
            .map_err(|_| "Could not open the pairing window.".to_string())
    }

    pub(crate) fn close_pairing(&self) {
        self.controller.close_pairing();
    }

    pub(crate) fn drain_authority(&self) -> Vec<AuthorityRequest> {
        std::iter::from_fn(|| self.authority.try_recv().ok()).collect()
    }

    pub(crate) fn take_work(
        &mut self,
    ) -> Result<Option<(PendingHostOperation, MasterPassword)>, String> {
        let Some(operation) = self
            .processor
            .take_one()
            .map_err(|_| "The local sync server stopped unexpectedly.".to_string())?
        else {
            return Ok(None);
        };
        let master = self
            .master
            .take()
            .ok_or_else(|| "The local sync server is busy.".to_string())?;
        Ok(Some((operation, master)))
    }

    pub(crate) fn restore_master(&mut self, master: MasterPassword) {
        debug_assert!(self.master.is_none());
        self.master = Some(master);
    }

    pub(crate) fn replace_trusted(&self, config: &LocalSyncConfig) {
        self.controller.replace_trusted(
            config
                .trusted_peers()
                .iter()
                .filter(|peer| peer.status() == PeerStatus::Active)
                .map(|peer| peer.public_key().into_bytes()),
        );
    }

    pub(crate) fn stop(&mut self) {
        self.processor.shutdown();
        self.controller.stop();
    }
}

fn epoch_seconds() -> Option<u64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

impl Drop for ServerRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpStream, time::Duration};

    // LNS-REVIEW-004
    #[test]
    fn explicit_tui_server_binds_a_real_ephemeral_port_and_stops() {
        let directory = tempfile::tempdir().expect("tempdir");
        let paths = HidlinsPaths::with_state_dir(directory.path().join("state"));
        let master = MasterPassword::new("server-runtime-test".to_string());
        let config = LocalSyncConfig::create("authority", SyncRole::Server, &master)
            .expect("server identity");

        let mut runtime =
            ServerRuntime::start(&config, "authority", master, paths).expect("start server");
        let endpoint = runtime
            .endpoint()
            .parse::<hidlins_sync::address::LocalEndpoint>()
            .expect("published endpoint is connectable");
        assert_ne!(endpoint.port(), 0);
        TcpStream::connect_timeout(&endpoint.socket_addr(), Duration::from_secs(1))
            .expect("real listener accepts connections");

        runtime.stop();
        assert!(
            TcpStream::connect_timeout(&endpoint.socket_addr(), Duration::from_millis(200))
                .is_err(),
            "explicit stop closes the listener"
        );
    }
}

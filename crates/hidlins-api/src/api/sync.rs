//! Local-network sync API and application-owned lifecycle integration.

use std::{
    collections::HashMap,
    net::IpAddr,
    panic::AssertUnwindSafe,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, RwLock, TryLockError, Weak,
    },
    thread,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

use hidlins_core::{Keyfile, MasterPassword, Uuid};
use hidlins_sync::{
    address::LocalEndpoint,
    config::local::LocalSyncConfig,
    discovery::{DiscoveryPermission, ServiceKind},
    identity::{PublicIdentity, SyncRole},
    server::{AuthoritativeVault, HostQueue, PairingAuthorityError, ServerController},
    trust::PeerStatus,
    SyncOptions,
};

use super::session::{AppSession, SessionServerRuntime, SessionState, TotpSnapshot};
use crate::dto::{
    DiscoveryPermissionDto, DiscoveryStatusDto, KeyfileRef, LocalEndpointDto, LocalSyncRoleDto,
    PairingPromptDto, SyncEvent, SyncOutcomeDto, SyncPeerDto, SyncStatusDto, VaultSummary,
};
use crate::error::HidlinsApiError;

pub(crate) struct PairingApiState {
    clients: Mutex<HashMap<String, PendingClientPairing>>,
    server_decisions: Mutex<HashMap<String, mpsc::SyncSender<ServerPairingDecision>>>,
}

impl PairingApiState {
    pub(crate) fn new() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            server_decisions: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn cancel_all(&self) {
        if let Ok(mut clients) = self.clients.lock() {
            clients.clear();
        }
        if let Ok(mut decisions) = self.server_decisions.lock() {
            for (_, sender) in decisions.drain() {
                let _ = sender.try_send(ServerPairingDecision::Reject);
            }
        }
    }
}

enum PendingPairingTarget {
    Existing {
        vault_name: String,
    },
    Import {
        vault_name: String,
        master: MasterPassword,
        keyfile: Option<Keyfile>,
        target: PathBuf,
    },
}

struct PendingClientPairing {
    session: hidlins_sync::client::ClientPairingSession,
    config: LocalSyncConfig,
    target: PendingPairingTarget,
    candidates: Vec<LocalEndpoint>,
}

enum ServerPairingDecision {
    Accept(String),
    Reject,
}

pub(crate) struct ApiPairingAuthority {
    session: Weak<Mutex<SessionState>>,
    pairing: Arc<PairingApiState>,
    cancelled: AtomicBool,
}

impl ApiPairingAuthority {
    pub(crate) fn new(session: Weak<Mutex<SessionState>>, pairing: Arc<PairingApiState>) -> Self {
        Self {
            session,
            pairing,
            cancelled: AtomicBool::new(false),
        }
    }

    fn with_ready_state<T>(
        &self,
        action: impl FnOnce(&mut SessionState) -> Result<T, ()>,
    ) -> Result<T, ()> {
        let session = self.session.upgrade().ok_or(())?;
        let mut action = Some(action);
        loop {
            if self.cancelled.load(Ordering::Acquire) {
                return Err(());
            }
            match session.try_lock() {
                Ok(state) if state.dead => return Err(()),
                Ok(state) if state.syncing => drop(state),
                Ok(mut state) => {
                    return action.take().ok_or(())?(&mut state);
                }
                Err(TryLockError::Poisoned(_)) => return Err(()),
                Err(TryLockError::WouldBlock) => {}
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl hidlins_sync::server::PairingAuthority for ApiPairingAuthority {
    fn confirm(
        &self,
        _peer: PublicIdentity,
        sas: hidlins_sync::pairing::SasCode,
        timeout: Duration,
    ) -> Result<String, PairingAuthorityError> {
        let handle = pairing_handle().map_err(|_| PairingAuthorityError::Canceled)?;
        let (sender, receiver) = mpsc::sync_channel(1);
        self.pairing
            .server_decisions
            .lock()
            .map_err(|_| PairingAuthorityError::Canceled)?
            .insert(handle.clone(), sender);
        self.with_ready_state(|state| {
            state.push_sync_event(SyncEvent::PairingRequested(PairingPromptDto {
                transaction_handle: handle.clone(),
                sas: sas.to_string(),
            }));
            Ok(())
        })
        .map_err(|()| PairingAuthorityError::Canceled)?;
        let decision = receiver
            .recv_timeout(timeout)
            .map_err(|_| PairingAuthorityError::Canceled)?;
        if let Ok(mut decisions) = self.pairing.server_decisions.lock() {
            decisions.remove(&handle);
        }
        match decision {
            ServerPairingDecision::Accept(name) => Ok(name),
            ServerPairingDecision::Reject => Err(PairingAuthorityError::Canceled),
        }
    }

    fn prepare(
        &self,
        transaction: &hidlins_sync::pairing::PairingTransaction,
        confirmation: &hidlins_sync::pairing::ConfirmedPairing,
        display_name: String,
    ) -> Result<(), PairingAuthorityError> {
        self.with_ready_state(|state| {
            let (name, _) = current_vault_name_and_master(state).map_err(|_| ())?;
            let now = epoch_seconds()?;
            LocalSyncConfig::transactional_update(&mut state.registry, &name, |config| {
                config.prepare_client(
                    transaction,
                    confirmation,
                    display_name,
                    now,
                    now.saturating_add(hidlins_sync::protocol::PAIRING_WINDOW.as_secs()),
                )
            })
            .map_err(|_| ())
        })
        .map_err(|()| PairingAuthorityError::Persistence)
    }

    fn activate(
        &self,
        transaction: &hidlins_sync::pairing::PairingTransaction,
    ) -> Result<(), PairingAuthorityError> {
        self.with_ready_state(|state| {
            let (name, _) = current_vault_name_and_master(state).map_err(|_| ())?;
            let now = epoch_seconds()?;
            LocalSyncConfig::transactional_update(&mut state.registry, &name, |config| {
                config.activate_client(transaction, now)
            })
            .map_err(|_| ())
        })
        .map_err(|()| PairingAuthorityError::Persistence)
    }

    fn recover(
        &self,
        peer: PublicIdentity,
    ) -> Result<hidlins_sync::pairing::PairingTransaction, PairingAuthorityError> {
        self.with_ready_state(|state| {
            let now = epoch_seconds()?;
            let config = current_local_config(state).map_err(|_| ())?;
            config
                .recover_pairing_transaction(peer, now)
                .map_err(|_| ())
        })
        .map_err(|()| PairingAuthorityError::Persistence)
    }

    fn shutdown(&self) {
        self.cancelled.store(true, Ordering::Release);
        self.cancel_pending();
    }

    fn cancel_pending(&self) {
        self.pairing.cancel_all();
    }
}

impl From<LocalSyncRoleDto> for SyncRole {
    fn from(value: LocalSyncRoleDto) -> Self {
        match value {
            LocalSyncRoleDto::Server => Self::Server,
            LocalSyncRoleDto::Client => Self::Client,
        }
    }
}

impl From<SyncRole> for LocalSyncRoleDto {
    fn from(value: SyncRole) -> Self {
        match value {
            SyncRole::Server => Self::Server,
            SyncRole::Client => Self::Client,
        }
    }
}

impl From<DiscoveryPermission> for DiscoveryPermissionDto {
    fn from(value: DiscoveryPermission) -> Self {
        match value {
            DiscoveryPermission::NotDetermined => Self::NotDetermined,
            DiscoveryPermission::Granted => Self::Granted,
            DiscoveryPermission::Denied => Self::Denied,
            DiscoveryPermission::Restricted => Self::Restricted,
            DiscoveryPermission::Unavailable => Self::Unavailable,
        }
    }
}

impl From<DiscoveryPermissionDto> for DiscoveryPermission {
    fn from(value: DiscoveryPermissionDto) -> Self {
        match value {
            DiscoveryPermissionDto::NotDetermined => Self::NotDetermined,
            DiscoveryPermissionDto::Granted => Self::Granted,
            DiscoveryPermissionDto::Denied => Self::Denied,
            DiscoveryPermissionDto::Restricted => Self::Restricted,
            DiscoveryPermissionDto::Unavailable => Self::Unavailable,
        }
    }
}

impl LocalEndpointDto {
    fn checked(&self) -> Result<LocalEndpoint, HidlinsApiError> {
        let ip = self
            .address
            .parse::<IpAddr>()
            .map_err(|_| HidlinsApiError::InvalidInput {
                field: "address".to_string(),
                reason: "an allowed IP literal is required".to_string(),
            })?;
        LocalEndpoint::new(ip, self.port, self.scope_id).map_err(|_| {
            HidlinsApiError::InvalidInput {
                field: "address".to_string(),
                reason: "endpoint is outside the local-only policy".to_string(),
            }
        })
    }
}

fn endpoint_dto(endpoint: LocalEndpoint) -> LocalEndpointDto {
    LocalEndpointDto {
        address: endpoint.ip().to_string(),
        port: endpoint.port(),
        scope_id: endpoint.scope_id(),
    }
}

#[allow(clippy::needless_pass_by_value)]
impl AppSession {
    pub fn configure_local_sync(&self, role: LocalSyncRoleDto) -> Result<(), HidlinsApiError> {
        let mut state = self.lock_state();
        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }
        let (vault_name, master) = current_vault_name_and_master(&state)?;
        LocalSyncConfig::configure(&mut state.registry, &vault_name, role.into(), &master)
            .map_err(|_| HidlinsApiError::LocalSyncConfiguration)?;
        Ok(())
    }

    pub fn set_discovery_candidates(
        &self,
        permission: DiscoveryPermissionDto,
        candidates: Vec<LocalEndpointDto>,
    ) -> Result<(), HidlinsApiError> {
        if candidates.len() > hidlins_sync::protocol::MAX_DISCOVERY_ENDPOINTS {
            return Err(HidlinsApiError::SyncBusy);
        }
        let checked = candidates
            .iter()
            .map(LocalEndpointDto::checked)
            .collect::<Result<Vec<_>, _>>()?;
        let mut state = self.lock_state();
        state.discovery_permission = permission.into();
        state.injected_candidates = checked;
        Ok(())
    }

    pub fn local_discovery_status(&self) -> DiscoveryStatusDto {
        let state = self.lock_state();
        DiscoveryStatusDto {
            permission: state.discovery_permission.into(),
            candidates: operation_candidates(&state)
                .into_iter()
                .map(endpoint_dto)
                .collect(),
        }
    }

    /// Poll the platform discovery adapter and apply only Rust-validated routes.
    pub fn poll_local_discovery(&self) -> Result<DiscoveryStatusDto, HidlinsApiError> {
        #[cfg(feature = "desktop")]
        {
            let mut port = self
                .discovery_port
                .lock()
                .map_err(|_| HidlinsApiError::Internal {
                    context: "discovery state unavailable".to_string(),
                })?;
            if port.is_none() {
                *port = Some(Box::new(
                    hidlins_sync::discovery::desktop::DesktopBrowser::start()
                        .map_err(map_discovery_error)?,
                ));
            }
            let browser = port.as_mut().ok_or_else(|| HidlinsApiError::Internal {
                context: "discovery browser did not initialize".to_string(),
            })?;
            let mut cache = self.lock_state().discovery_cache.clone();
            hidlins_sync::discovery::collect_candidates(
                browser.as_mut(),
                &mut cache,
                ServiceKind::Trusted,
                std::time::Duration::from_millis(1_200),
                std::time::Duration::from_millis(25),
                || false,
            )
            .map_err(map_discovery_error)?;
            let mut state = self.lock_state();
            state.discovery_cache = cache;
            state.discovery_permission = DiscoveryPermission::Granted;
        }
        Ok(self.local_discovery_status())
    }

    pub fn local_sync_status(&self) -> Result<SyncStatusDto, HidlinsApiError> {
        let state = self.lock_state();
        sync_status(&state)
    }

    /// Return policy-approved local bind candidates for a desktop server.
    /// Mobile targets reject this before interface enumeration.
    pub fn local_server_endpoints(&self) -> Result<Vec<LocalEndpointDto>, HidlinsApiError> {
        {
            let state = self.lock_state();
            if !state.server_capable {
                return Err(HidlinsApiError::UnsupportedPlatform {
                    capability: "local sync server".to_string(),
                });
            }
        }
        #[cfg(feature = "desktop")]
        {
            let interfaces = hidlins_sync::discovery::desktop::allowed_interface_bind_endpoints()
                .map_err(map_discovery_error)?;
            let endpoints = interfaces
                .into_iter()
                .filter_map(|interface| {
                    let listener = std::net::TcpListener::bind(interface.socket_addr()).ok()?;
                    let actual = listener.local_addr().ok()?;
                    let scope_id = match actual {
                        std::net::SocketAddr::V4(_) => 0,
                        std::net::SocketAddr::V6(address) => address.scope_id(),
                    };
                    LocalEndpoint::new(actual.ip(), actual.port(), scope_id)
                        .ok()
                        .map(endpoint_dto)
                })
                .collect::<Vec<_>>();
            if endpoints.is_empty() {
                return Err(HidlinsApiError::SyncOffline);
            }
            Ok(endpoints)
        }
        #[cfg(not(feature = "desktop"))]
        Err(HidlinsApiError::UnsupportedPlatform {
            capability: "local sync server".to_string(),
        })
    }

    pub fn sync_status(&self) -> Result<SyncStatusDto, HidlinsApiError> {
        self.local_sync_status()
    }

    pub fn sync_now(&self) -> Result<SyncOutcomeDto, HidlinsApiError> {
        run_sync(&self.inner, &self.sync_engine, &self.totp_cache, false)
    }

    /// Consume the once-per-process startup attempt after the application has
    /// completed its platform discovery pass.
    ///
    /// The work remains nonblocking and best effort: the already-open local
    /// vault stays available, and any network failure is reported through the
    /// sync event stream.
    pub fn start_startup_sync(&self) -> Result<(), HidlinsApiError> {
        let vault_name = {
            let state = self.lock_state();
            let vault = state.require_vault()?;
            let name = state
                .registry
                .list()
                .find(|entry| entry.path == vault.path())
                .map(|entry| entry.name.clone())
                .ok_or_else(|| HidlinsApiError::Internal {
                    context: "unlocked vault is not registered".to_string(),
                })?;
            name
        };
        schedule_startup_sync(self, &vault_name);
        Ok(())
    }

    /// Cancel foreground client transfer/pairing work without changing trust.
    pub fn cancel_sync(&self) {
        let state = self.lock_state();
        if let Some(cancellation) = state.sync_cancellation.as_ref() {
            cancellation.cancel();
        }
        drop(state);
        self.pairing_api.cancel_all();
    }

    pub fn start_sync_server(
        &self,
        endpoint: LocalEndpointDto,
    ) -> Result<LocalEndpointDto, HidlinsApiError> {
        {
            let state = self.lock_state();
            if !state.server_capable {
                return Err(HidlinsApiError::UnsupportedPlatform {
                    capability: "local sync server".to_string(),
                });
            }
        }
        let endpoint = endpoint.checked()?;
        let mut state = self.lock_state();
        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }
        if state
            .server_runtime
            .as_ref()
            .is_some_and(|runtime| runtime.controller.endpoint() == endpoint)
        {
            state.server_requested = Some(endpoint);
            return Ok(endpoint_dto(endpoint));
        }
        state.stop_server_runtime();
        state.server_requested = Some(endpoint);
        let pairing_authority: Arc<dyn hidlins_sync::server::PairingAuthority> = Arc::new(
            ApiPairingAuthority::new(Arc::downgrade(&self.inner), Arc::clone(&self.pairing_api)),
        );
        if let Err(error) = start_server_runtime(&mut state, endpoint, Some(pairing_authority)) {
            state.server_requested = None;
            return Err(error);
        }
        let actual = state
            .server_runtime
            .as_ref()
            .map(|runtime| runtime.controller.endpoint())
            .ok_or_else(|| HidlinsApiError::Internal {
                context: "server runtime did not start".to_string(),
            })?;
        let dto = endpoint_dto(actual);
        state.push_sync_event(SyncEvent::ServerStarted(dto.clone()));
        Ok(dto)
    }

    pub fn stop_sync_server(&self) {
        let mut state = self.lock_state();
        state.server_requested = None;
        state.stop_server_runtime();
    }

    pub fn open_pairing_window(&self) -> Result<(), HidlinsApiError> {
        let mut state = self.lock_state();
        let runtime = state
            .server_runtime
            .as_ref()
            .ok_or(HidlinsApiError::SyncOffline)?;
        runtime
            .controller
            .open_pairing()
            .map_err(|_| HidlinsApiError::SyncOffline)?;
        state.pairing_open = true;
        Ok(())
    }

    pub fn close_pairing_window(&self) {
        let mut state = self.lock_state();
        if let Some(runtime) = state.server_runtime.as_ref() {
            runtime.controller.close_pairing();
        }
        state.pairing_open = false;
    }

    /// Begin XX pairing for the currently unlocked client vault.
    pub fn begin_pairing(&self) -> Result<PairingPromptDto, HidlinsApiError> {
        let (config, name, master, candidates) = {
            let state = self.lock_state();
            let config = current_local_config(&state)?;
            if config.role() != SyncRole::Client || config.is_active_client() {
                return Err(HidlinsApiError::SyncAuthFailed);
            }
            let (name, master) = current_vault_name_and_master(&state)?;
            (config, name, master, pairing_candidates(&state))
        };
        let identity = config
            .identity()
            .unlock(&name, SyncRole::Client, &master)
            .map_err(|_| HidlinsApiError::AuthenticationFailed)?;
        let pairing =
            hidlins_sync::client::ClientPairingSession::begin(&identity, candidates.clone())
                .map_err(|error| HidlinsApiError::from(hidlins_sync::SyncError::from(error)))?;
        self.store_client_pairing(
            pairing,
            config,
            candidates,
            PendingPairingTarget::Existing { vault_name: name },
        )
    }

    /// Begin pair-and-import without creating a local vault or registration.
    pub fn begin_pair_import(
        &self,
        name: String,
        master_password: String,
        keyfile: Option<KeyfileRef>,
        candidates: Vec<LocalEndpointDto>,
    ) -> Result<PairingPromptDto, HidlinsApiError> {
        super::vaults::validate_vault_name(&name)?;
        let checked = candidates
            .iter()
            .map(LocalEndpointDto::checked)
            .collect::<Result<Vec<_>, _>>()?;
        if checked.is_empty() || checked.len() > hidlins_sync::protocol::MAX_CANDIDATE_ATTEMPTS {
            return Err(HidlinsApiError::SyncNotFound);
        }
        let master = MasterPassword::new(master_password);
        let keyfile = keyfile.map(|value| match value {
            KeyfileRef::Path(path) => Keyfile::Path(PathBuf::from(path)),
            KeyfileRef::Bytes(bytes) => Keyfile::Bytes(bytes),
        });
        let target = {
            let state = self.lock_state();
            if state.is_unlocked() || state.registry.get(&name).is_some() {
                return Err(HidlinsApiError::PathExists { path: name });
            }
            let target = state
                .registry
                .paths()
                .state_dir()
                .join(format!("{name}.kdbx"));
            if target.exists() {
                return Err(HidlinsApiError::PathExists {
                    path: target.display().to_string(),
                });
            }
            target
        };
        let config = LocalSyncConfig::create(&name, SyncRole::Client, &master)
            .map_err(|_| HidlinsApiError::LocalSyncConfiguration)?;
        let identity = config
            .identity()
            .unlock(&name, SyncRole::Client, &master)
            .map_err(|_| HidlinsApiError::AuthenticationFailed)?;
        let pairing = hidlins_sync::client::ClientPairingSession::begin(&identity, checked.clone())
            .map_err(|error| HidlinsApiError::from(hidlins_sync::SyncError::from(error)))?;
        self.store_client_pairing(
            pairing,
            config,
            checked.clone(),
            PendingPairingTarget::Import {
                vault_name: name,
                master,
                keyfile,
                target,
            },
        )
    }

    fn store_client_pairing(
        &self,
        session: hidlins_sync::client::ClientPairingSession,
        config: LocalSyncConfig,
        candidates: Vec<LocalEndpoint>,
        target: PendingPairingTarget,
    ) -> Result<PairingPromptDto, HidlinsApiError> {
        let sas = session.sas().to_string();
        let handle = pairing_handle()?;
        self.pairing_api
            .clients
            .lock()
            .map_err(|_| HidlinsApiError::Internal {
                context: "pairing state unavailable".to_string(),
            })?
            .insert(
                handle.clone(),
                PendingClientPairing {
                    session,
                    config,
                    target,
                    candidates,
                },
            );
        Ok(PairingPromptDto {
            transaction_handle: handle,
            sas,
        })
    }

    /// Confirm either a local client prompt or an inbound server prompt.
    /// Key material and transcript hashes never cross this boundary.
    pub fn confirm_pairing(
        &self,
        transaction_handle: String,
        accepted: bool,
        peer_display_name: String,
    ) -> Result<Option<VaultSummary>, HidlinsApiError> {
        if let Some(sender) = self
            .pairing_api
            .server_decisions
            .lock()
            .map_err(|_| HidlinsApiError::Internal {
                context: "pairing state unavailable".to_string(),
            })?
            .remove(&transaction_handle)
        {
            let decision = if accepted {
                ServerPairingDecision::Accept(peer_display_name)
            } else {
                ServerPairingDecision::Reject
            };
            sender
                .try_send(decision)
                .map_err(|_| HidlinsApiError::SyncCanceled)?;
            return Ok(None);
        }

        let pending = self
            .pairing_api
            .clients
            .lock()
            .map_err(|_| HidlinsApiError::Internal {
                context: "pairing state unavailable".to_string(),
            })?
            .remove(&transaction_handle)
            .ok_or(HidlinsApiError::SyncCanceled)?;
        if !accepted {
            return Ok(None);
        }
        self.confirm_client_pairing(pending, peer_display_name)
    }

    fn confirm_client_pairing(
        &self,
        mut pending: PendingClientPairing,
        peer_display_name: String,
    ) -> Result<Option<VaultSummary>, HidlinsApiError> {
        let now = epoch_seconds().map_err(|()| HidlinsApiError::Internal {
            context: "system clock unavailable".to_string(),
        })?;
        let _transaction = match &pending.target {
            PendingPairingTarget::Existing { vault_name } => pending
                .session
                .confirm(&mut pending.config, peer_display_name, now, |prepared| {
                    let mut state = self.lock_state();
                    if state.dead || !state.is_unlocked() {
                        return Err(hidlins_sync::config::local::LocalConfigError::Malformed);
                    }
                    prepared.persist(&mut state.registry, vault_name)
                })
                .map_err(|error| HidlinsApiError::from(hidlins_sync::SyncError::from(error)))?,
            PendingPairingTarget::Import { vault_name, .. } => {
                let state_dir = self.lock_state().registry.paths().state_dir().to_path_buf();
                let store = hidlins_sync::client::PendingPairingStore::new(&state_dir, vault_name);
                pending
                    .session
                    .confirm(&mut pending.config, peer_display_name, now, |prepared| {
                        store.save(prepared)
                    })
                    .map_err(|error| HidlinsApiError::from(hidlins_sync::SyncError::from(error)))?
            }
        };

        match pending.target {
            PendingPairingTarget::Existing { vault_name } => {
                let mut state = self.lock_state();
                if state.dead || !state.is_unlocked() {
                    return Err(HidlinsApiError::VaultLocked);
                }
                pending
                    .config
                    .persist(&mut state.registry, &vault_name)
                    .map_err(|_| HidlinsApiError::LocalSyncConfiguration)?;
                Ok(None)
            }
            PendingPairingTarget::Import {
                vault_name,
                master,
                keyfile,
                target,
            } => {
                let (bytes, version) = hidlins_sync::client::fetch_paired_vault(
                    &vault_name,
                    &master,
                    &pending.config,
                    pending.candidates,
                )
                .map_err(|error| HidlinsApiError::from(hidlins_sync::SyncError::from(error)))?;
                pending
                    .config
                    .set_sync_versions(Some(version), Some(version));
                let mut state = self.lock_state();
                hidlins_sync::client::import_paired_vault(
                    &bytes,
                    &target,
                    &vault_name,
                    &master,
                    keyfile.as_ref(),
                    &pending.config,
                    &mut state.registry,
                )
                .map_err(|_| HidlinsApiError::InvalidFormat)?;
                hidlins_sync::client::PendingPairingStore::new(
                    state.registry.paths().state_dir(),
                    &vault_name,
                )
                .clear()
                .map_err(|_| HidlinsApiError::LocalSyncConfiguration)?;
                Ok(Some(VaultSummary {
                    name: vault_name,
                    path: target.display().to_string(),
                    has_keyfile: keyfile.is_some(),
                    has_sync: true,
                }))
            }
        }
    }

    pub fn list_sync_peers(&self) -> Result<Vec<SyncPeerDto>, HidlinsApiError> {
        let state = self.lock_state();
        let config = current_local_config(&state)?;
        let peers: Vec<_> = match config.role() {
            SyncRole::Server => config.trusted_peers().iter().collect(),
            SyncRole::Client => config.pinned_server().into_iter().collect(),
        };
        Ok(peers
            .into_iter()
            .enumerate()
            .map(|(index, peer)| SyncPeerDto {
                peer_id: format!("peer-{index}"),
                display_name: peer.display_name().to_string(),
                revoked: peer.status() == PeerStatus::Revoked,
            })
            .collect())
    }

    pub fn rename_sync_peer(
        &self,
        peer_id: String,
        display_name: String,
    ) -> Result<(), HidlinsApiError> {
        self.update_peer(peer_id, |config, key| config.rename_peer(key, display_name))
    }

    pub fn revoke_sync_peer(&self, peer_id: String) -> Result<(), HidlinsApiError> {
        self.update_peer(
            peer_id,
            hidlins_sync::config::local::LocalSyncConfig::revoke_peer,
        )?;
        let state = self.lock_state();
        if let (Some(runtime), Ok(config)) = (&state.server_runtime, current_local_config(&state)) {
            runtime
                .controller
                .replace_trusted(active_peer_keys(&config));
        }
        Ok(())
    }

    fn update_peer(
        &self,
        peer_id: String,
        update: impl FnOnce(
            &mut LocalSyncConfig,
            PublicIdentity,
        ) -> Result<(), hidlins_sync::trust::TrustError>,
    ) -> Result<(), HidlinsApiError> {
        let index = parse_peer_id(&peer_id)?;
        let mut state = self.lock_state();
        let (name, _) = current_vault_name_and_master(&state)?;
        LocalSyncConfig::transactional_update(&mut state.registry, &name, |config| {
            let key = match config.role() {
                SyncRole::Server => config
                    .trusted_peers()
                    .get(index)
                    .map(hidlins_sync::trust::PeerRecord::public_key),
                SyncRole::Client if index == 0 => config
                    .pinned_server()
                    .map(hidlins_sync::trust::PeerRecord::public_key),
                SyncRole::Client => None,
            }
            .ok_or(hidlins_sync::trust::TrustError::NotActive)?;
            update(config, key)
        })
        .map_err(|_| HidlinsApiError::SyncRevoked)
    }

    pub fn clear_sync_config(&self, name: String) -> Result<(), HidlinsApiError> {
        let mut state = self.lock_state();
        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }
        state.server_requested = None;
        state.stop_server_runtime();
        state.registry.update_registered_extra(&name, |extra| {
            extra.remove("sync");
        })?;
        Ok(())
    }
}

fn sync_status(state: &SessionState) -> Result<SyncStatusDto, HidlinsApiError> {
    let pairing_open = state.server_runtime.as_ref().is_some_and(|runtime| {
        runtime
            .controller
            .advertisements()
            .iter()
            .any(|record| record.kind() == ServiceKind::Pairing)
    });
    if state.syncing {
        return Ok(SyncStatusDto {
            configured: true,
            in_flight: true,
            last_outcome: None,
            role: None,
            paired: false,
            active_peer_count: 0,
            server_enabled: state.server_requested.is_some(),
            server_running: state.server_runtime.is_some(),
            pairing_open,
        });
    }
    state.require_vault()?;
    let config = current_local_config(state).ok();
    let summary = config.as_ref().map(LocalSyncConfig::status);
    Ok(SyncStatusDto {
        configured: config.is_some(),
        in_flight: false,
        last_outcome: None,
        role: summary.map(|value| value.role.into()),
        paired: summary.is_some_and(|value| value.paired),
        active_peer_count: summary.map_or(0, |value| value.active_clients),
        server_enabled: state.server_requested.is_some(),
        server_running: state.server_runtime.is_some(),
        pairing_open,
    })
}

fn current_local_config(state: &SessionState) -> Result<LocalSyncConfig, HidlinsApiError> {
    let vault = state.require_vault()?;
    let entry = state
        .registry
        .list()
        .find(|entry| entry.path == vault.path())
        .ok_or_else(|| HidlinsApiError::Internal {
            context: "unlocked vault is not registered".to_string(),
        })?;
    LocalSyncConfig::from_vault_entry(entry).ok_or(HidlinsApiError::SyncNotConfigured)
}

fn current_vault_name_and_master(
    state: &SessionState,
) -> Result<(String, MasterPassword), HidlinsApiError> {
    let vault = state.require_vault()?;
    let name = state
        .registry
        .list()
        .find(|entry| entry.path == vault.path())
        .map(|entry| entry.name.clone())
        .ok_or_else(|| HidlinsApiError::Internal {
            context: "unlocked vault is not registered".to_string(),
        })?;
    let credentials = state
        .credentials
        .as_ref()
        .ok_or(HidlinsApiError::VaultLocked)?;
    Ok((
        name,
        MasterPassword::new(String::from_utf8_lossy(credentials.master.as_bytes()).into_owned()),
    ))
}

fn operation_candidates(state: &SessionState) -> Vec<LocalEndpoint> {
    let mut candidates = state.injected_candidates.clone();
    for endpoint in state.discovery_cache.candidates(ServiceKind::Trusted) {
        if !candidates.contains(&endpoint) {
            candidates.push(endpoint);
        }
    }
    candidates.truncate(hidlins_sync::protocol::MAX_CANDIDATE_ATTEMPTS);
    candidates
}

fn pairing_candidates(state: &SessionState) -> Vec<LocalEndpoint> {
    let mut candidates = state.injected_candidates.clone();
    for endpoint in state.discovery_cache.candidates(ServiceKind::Pairing) {
        if !candidates.contains(&endpoint) {
            candidates.push(endpoint);
        }
    }
    candidates.truncate(hidlins_sync::protocol::MAX_CANDIDATE_ATTEMPTS);
    candidates
}

fn clone_keyfile(keyfile: Option<&Keyfile>) -> Option<Keyfile> {
    keyfile.map(|value| match value {
        Keyfile::Path(path) => Keyfile::Path(path.clone()),
        Keyfile::Bytes(bytes) => Keyfile::Bytes(bytes.clone()),
    })
}

#[allow(clippy::too_many_lines)] // Claim/run/commit cleanup remains one auditable lifecycle boundary.
fn run_sync(
    inner: &Arc<Mutex<SessionState>>,
    engine: &Arc<dyn crate::sync_port::SyncEnginePort>,
    totp_cache: &Arc<RwLock<std::collections::HashMap<Uuid, TotpSnapshot>>>,
    automatic: bool,
) -> Result<SyncOutcomeDto, HidlinsApiError> {
    let (mut vault, mut registry, master, keyfile, name, candidates, cancellation) = {
        let mut state = inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }
        let (name, master) = current_vault_name_and_master(&state)?;
        let config = current_local_config(&state)?;
        if config.role() != SyncRole::Client || !config.is_active_client() {
            return Err(HidlinsApiError::SyncAuthFailed);
        }
        match state.discovery_permission {
            DiscoveryPermission::Denied
            | DiscoveryPermission::Restricted
            | DiscoveryPermission::NotDetermined => {
                return Err(HidlinsApiError::SyncPermissionDenied);
            }
            DiscoveryPermission::Granted | DiscoveryPermission::Unavailable => {}
        }
        let keyfile = clone_keyfile(state.credentials.as_ref().and_then(|c| c.keyfile.as_ref()));
        let candidates = operation_candidates(&state);
        let vault = state.vault.take().ok_or(HidlinsApiError::VaultLocked)?;
        let paths = state.registry.paths().clone();
        let registry = std::mem::replace(
            &mut state.registry,
            hidlins_core::VaultRegistry::with_paths(paths),
        );
        state.syncing = true;
        let cancellation = hidlins_sync::client::LanCancellation::default();
        state.sync_cancellation = Some(cancellation.clone());
        state.push_sync_event(SyncEvent::Started { automatic });
        (
            vault,
            registry,
            master,
            keyfile,
            name,
            candidates,
            cancellation,
        )
    };

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        engine.sync_now(
            &mut vault,
            &name,
            &mut registry,
            &master,
            keyfile.as_ref(),
            candidates,
            SyncOptions::default(),
            cancellation,
        )
    }));

    let mut state = inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.syncing = false;
    state.sync_cancellation = None;
    let lock_requested = std::mem::take(&mut state.lock_pending);
    super::session::clear_totp_cache_inner(totp_cache);
    if lock_requested || state.dead {
        state.registry = registry;
        state.stop_server_runtime();
        state.credentials = None;
        state
            .controller
            .lock_now(hidlins_security::OsLockReason::Manual);
        state.push_sync_event(SyncEvent::Failed(HidlinsApiError::VaultLocked));
        if !state.dead {
            state.push_lock_event(crate::dto::LockEvent::Locked);
        }
        return Err(HidlinsApiError::VaultLocked);
    }
    match result {
        Ok(Ok(outcome)) => {
            let dto = crate::dto::sync_outcome_from_core(&outcome);
            state.vault = Some(vault);
            state.registry = registry;
            state.push_sync_event(SyncEvent::Done(dto.clone()));
            Ok(dto)
        }
        Ok(Err(error)) => {
            let error = HidlinsApiError::from(error);
            state.vault = Some(vault);
            state.registry = registry;
            state.push_sync_event(SyncEvent::Failed(error.clone()));
            Err(error)
        }
        Err(_) => {
            if let Ok(reloaded) = hidlins_core::VaultRegistry::load(state.registry.paths().clone())
            {
                state.registry = reloaded;
            }
            state.stop_server_runtime();
            state.credentials = None;
            state
                .controller
                .lock_now(hidlins_security::OsLockReason::Manual);
            let error = HidlinsApiError::Internal {
                context: "local sync worker panicked".to_string(),
            };
            state.push_sync_event(SyncEvent::Failed(error.clone()));
            state.push_lock_event(crate::dto::LockEvent::Locked);
            Err(error)
        }
    }
}

pub(crate) fn schedule_startup_sync(session: &AppSession, vault_name: &str) {
    let configured = {
        let mut state = session.lock_state();
        let configured = state
            .registry
            .get(vault_name)
            .and_then(LocalSyncConfig::from_vault_entry)
            .is_some_and(|config| config.is_active_client());
        state.startup_sync_tracker.should_attempt(
            vault_name,
            configured,
            hidlins_sync::client::SyncTrigger::StartupUnlock,
        )
    };
    if !configured {
        return;
    }
    let inner = Arc::clone(&session.inner);
    let engine = Arc::clone(&session.sync_engine);
    let cache = Arc::clone(&session.totp_cache);
    let worker = std::thread::Builder::new()
        .name("hidlins-startup-sync".to_string())
        .spawn(move || {
            let _ = run_sync(&inner, &engine, &cache, true);
        });
    if let Ok(worker) = worker {
        if let Ok(mut workers) = session.background_workers.lock() {
            workers.retain(|handle| !handle.is_finished());
            workers.push(worker);
        }
    }
}

fn start_server_runtime(
    state: &mut SessionState,
    endpoint: LocalEndpoint,
    pairing_authority: Option<Arc<dyn hidlins_sync::server::PairingAuthority>>,
) -> Result<(), HidlinsApiError> {
    if state.server_runtime.is_some() {
        return Ok(());
    }
    let (name, master) = current_vault_name_and_master(state)?;
    let config = current_local_config(state)?;
    if config.role() != SyncRole::Server {
        return Err(HidlinsApiError::InvalidInput {
            field: "role".to_string(),
            reason: "only an authoritative vault can serve".to_string(),
        });
    }
    let identity = config
        .identity()
        .unlock(&name, SyncRole::Server, &master)
        .map_err(|_| HidlinsApiError::AuthenticationFailed)?;
    let (queue, processor) = HostQueue::new();
    let controller = ServerController::start_with_pairing_authority(
        endpoint,
        Arc::new(identity),
        active_peer_keys(&config),
        queue,
        pairing_authority,
    )
    .map_err(|_| HidlinsApiError::SyncOffline)?;
    controller.replace_pairing_recovery(
        config
            .provisional()
            .map(|record| record.peer_key().into_bytes()),
    );
    state.server_runtime = Some(SessionServerRuntime {
        controller,
        processor,
    });
    Ok(())
}

fn active_peer_keys(config: &LocalSyncConfig) -> Vec<[u8; 32]> {
    config
        .trusted_peers()
        .iter()
        .filter(|peer| peer.status() == PeerStatus::Active)
        .map(|peer| peer.public_key().into_bytes())
        .collect()
}

pub(crate) fn restart_requested_server(session: &AppSession, state: &mut SessionState) {
    if let Some(endpoint) = state.server_requested {
        let authority: Arc<dyn hidlins_sync::server::PairingAuthority> =
            Arc::new(ApiPairingAuthority::new(
                Arc::downgrade(&session.inner),
                Arc::clone(&session.pairing_api),
            ));
        if let Err(error) = start_server_runtime(state, endpoint, Some(authority)) {
            state.push_sync_event(SyncEvent::Failed(error));
        } else if let Some(runtime) = state.server_runtime.as_ref() {
            state.push_sync_event(SyncEvent::ServerStarted(endpoint_dto(
                runtime.controller.endpoint(),
            )));
        }
    }
}

pub(crate) fn pump_server_once(inner: &Arc<Mutex<SessionState>>) {
    let (mut runtime, mut vault, master, keyfile) = {
        let Ok(mut state) = inner.try_lock() else {
            return;
        };
        if state.syncing || state.dead || state.server_runtime.is_none() || state.vault.is_none() {
            return;
        }
        let Some(credentials) = state.credentials.as_ref() else {
            return;
        };
        let master = MasterPassword::new(
            String::from_utf8_lossy(credentials.master.as_bytes()).into_owned(),
        );
        let keyfile = clone_keyfile(credentials.keyfile.as_ref());
        let runtime = state.server_runtime.take().expect("checked above");
        let vault = state.vault.take().expect("checked above");
        state.syncing = true;
        (runtime, vault, master, keyfile)
    };
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut host = AuthoritativeVault::new(&mut vault, &master, keyfile.as_ref());
        runtime.processor.process_one(&mut host)
    }));
    let mut state = inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.syncing = false;
    let lock_requested = std::mem::take(&mut state.lock_pending);
    if state.dead || lock_requested || result.is_err() {
        runtime.processor.shutdown();
        runtime.controller.stop();
        if state.dead || result.is_err() {
            state.server_requested = None;
        }
        state.credentials = None;
        state.push_sync_event(SyncEvent::ServerStopped);
        state.push_lock_event(crate::dto::LockEvent::Locked);
        return;
    }
    state.vault = Some(vault);
    state.server_runtime = Some(runtime);
}

fn parse_peer_id(value: &str) -> Result<usize, HidlinsApiError> {
    if let Some(index) = value
        .strip_prefix("peer-")
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .filter(|index| value == format!("peer-{index}"))
    {
        return Ok(index);
    }
    Err(HidlinsApiError::InvalidInput {
        field: "peer_id".to_string(),
        reason: "invalid opaque peer handle".to_string(),
    })
}

fn pairing_handle() -> Result<String, HidlinsApiError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = hidlins_sync::pairing::TransactionId::generate()
        .map_err(|_| HidlinsApiError::Internal {
            context: "pairing handle generation failed".to_string(),
        })?
        .into_bytes();
    let mut encoded = String::with_capacity(32);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}

fn epoch_seconds() -> Result<u64, ()> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| ())
}

#[cfg(feature = "desktop")]
fn map_discovery_error(error: hidlins_sync::discovery::DiscoveryError) -> HidlinsApiError {
    use hidlins_sync::discovery::DiscoveryError;
    match error {
        DiscoveryError::PermissionRequired | DiscoveryError::PermissionDenied => {
            HidlinsApiError::SyncPermissionDenied
        }
        DiscoveryError::NoAllowedEndpoint => HidlinsApiError::SyncNotFound,
        DiscoveryError::Capacity => HidlinsApiError::SyncBusy,
        DiscoveryError::Stopped | DiscoveryError::Unavailable | DiscoveryError::Backend => {
            HidlinsApiError::SyncOffline
        }
        DiscoveryError::InvalidRecord | DiscoveryError::EntropyUnavailable => {
            HidlinsApiError::Internal {
                context: "local discovery failed".to_string(),
            }
        }
    }
}

#[cfg(all(test, feature = "desktop"))]
mod tests {
    use std::collections::VecDeque;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;

    use hidlins_core::HidlinsPaths;
    use hidlins_sync::discovery::{
        DiscoveryBatch, DiscoveryError, DiscoveryEvent, DiscoveryPermission, DiscoveryPort,
        RawDiscoveryRecord, RawEndpoint, ServiceKind,
    };

    use super::*;

    struct BatchPort(VecDeque<DiscoveryBatch>);

    impl DiscoveryPort for BatchPort {
        fn poll(&mut self) -> Result<DiscoveryBatch, DiscoveryError> {
            self.0.pop_front().map_or_else(
                || DiscoveryBatch::new(DiscoveryPermission::Granted, Vec::new()),
                Ok,
            )
        }

        fn shutdown(&mut self) -> Result<(), DiscoveryError> {
            self.0.clear();
            Ok(())
        }
    }

    fn batch(label: &str, last_octet: u8) -> DiscoveryBatch {
        let kind = ServiceKind::Trusted;
        let record = RawDiscoveryRecord::try_from_untrusted(
            kind.service_type().to_string(),
            format!("{label}.{}", kind.service_type()),
            48_101,
            vec![("v".to_string(), b"1".to_vec())],
            vec![RawEndpoint::new(
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, last_octet)),
                0,
            )],
            Duration::from_secs(120),
        )
        .unwrap();
        DiscoveryBatch::new(
            DiscoveryPermission::Granted,
            vec![DiscoveryEvent::Resolved(record)],
        )
        .unwrap()
    }

    #[test]
    fn api_discovery_collects_later_batch_after_first_candidate() {
        // LNS-DISCOVERY-006: exercise temporal accumulation through the API's
        // production discovery poll rather than presenting both routes at once.
        let directory = tempfile::tempdir().unwrap();
        let session =
            AppSession::for_test(HidlinsPaths::with_state_dir(directory.path().join("state")))
                .unwrap();
        *session.discovery_port.lock().unwrap() = Some(Box::new(BatchPort(VecDeque::from([
            batch("00000000000000000000000001", 10),
            batch("00000000000000000000000002", 20),
        ]))));

        let status = session.poll_local_discovery().unwrap();

        assert_eq!(
            status.candidates.len(),
            2,
            "a first untrusted route must not end the temporal discovery attempt"
        );
    }
}

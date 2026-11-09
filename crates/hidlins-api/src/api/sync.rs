//! Local-network sync API and application-owned lifecycle integration.

use std::{
    net::IpAddr,
    panic::AssertUnwindSafe,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, Weak,
    },
    thread,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

use hidlins_core::{Keyfile, MasterPassword};
use hidlins_sync::{
    address::LocalEndpoint,
    config::local::LocalSyncConfig,
    discovery::{DiscoveryPermission, ServiceKind},
    identity::{PublicIdentity, SyncRole},
    server::{AuthoritativeVault, HostQueue, PairingAuthorityError, ServerController},
    trust::PeerStatus,
    SyncOptions,
};

use super::session::{
    lock_session_state, try_lock_session_state, AppSession, SessionServerRuntime, SessionState,
};
use crate::dto::{
    DiscoveryPermissionDto, DiscoveryServiceDto, DiscoveryStatusDto, KeyfileRef, LocalEndpointDto,
    LocalSyncRoleDto, PairingPromptDto, SyncEvent, SyncOutcomeDto, SyncPeerDto, SyncStatusDto,
    VaultSummary,
};
use crate::error::HidlinsApiError;

#[path = "pairing_state.rs"]
mod pairing_state;
pub(crate) use pairing_state::PairingApiState;
use pairing_state::PairingRegistration;

enum PendingPairingTarget {
    Existing {
        vault_name: String,
        backing_path: PathBuf,
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
    registration: PairingRegistration,
}

enum ServerPairingDecision {
    Accept(String),
    Reject,
}

fn pairing_origin_matches(
    state: &SessionState,
    target: &PendingPairingTarget,
    expected: &LocalSyncConfig,
) -> bool {
    if state.dead || state.syncing || state.unlocking || state.lock_pending {
        return false;
    }
    match target {
        PendingPairingTarget::Existing {
            vault_name,
            backing_path,
        } => {
            let (Some(vault), Some(entry)) = (state.vault.as_ref(), state.registry.get(vault_name))
            else {
                return false;
            };
            vault.backing_path() == backing_path
                && vault.path() == entry.path
                && LocalSyncConfig::from_vault_entry(entry).as_ref() == Some(expected)
        }
        PendingPairingTarget::Import {
            vault_name, target, ..
        } => {
            !state.is_logically_unlocked()
                && state.registry.get(vault_name).is_none()
                && *target
                    == state
                        .registry
                        .paths()
                        .state_dir()
                        .join(format!("{vault_name}.kdbx"))
                && !target.exists()
        }
    }
}

fn pairing_failure(
    registration: &PairingRegistration,
    error: hidlins_sync::client::LanError,
) -> HidlinsApiError {
    if registration.is_current() {
        HidlinsApiError::from(hidlins_sync::SyncError::from(error))
    } else {
        HidlinsApiError::SyncCanceled
    }
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
            match try_lock_session_state(&session) {
                Some(state) if state.dead => return Err(()),
                Some(state) if state.syncing => drop(state),
                Some(mut state) => {
                    return action.take().ok_or(())?(&mut state);
                }
                None => {}
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
        #[cfg(all(test, feature = "desktop"))]
        pairing_activation_regressions::before_lock();
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

    fn activate_authorized(
        &self,
        transaction: &hidlins_sync::pairing::PairingTransaction,
        authorization: &hidlins_sync::server::PairingAuthorization,
    ) -> Result<(), PairingAuthorityError> {
        #[cfg(all(test, feature = "desktop"))]
        pairing_activation_regressions::before_lock();
        self.with_ready_state(|state| {
            let (name, _) = current_vault_name_and_master(state).map_err(|_| ())?;
            let now = epoch_seconds()?;
            LocalSyncConfig::transactional_update(&mut state.registry, &name, |config| {
                authorization
                    .check()
                    .map_err(|_| hidlins_sync::trust::TrustError::Expired)?;
                config.activate_client(transaction, now)
            })
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

impl From<DiscoveryServiceDto> for ServiceKind {
    fn from(value: DiscoveryServiceDto) -> Self {
        match value {
            DiscoveryServiceDto::Trusted => Self::Trusted,
            DiscoveryServiceDto::Pairing => Self::Pairing,
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

fn discovery_status(state: &SessionState, service: DiscoveryServiceDto) -> DiscoveryStatusDto {
    let candidates = match service {
        DiscoveryServiceDto::Trusted => operation_candidates(state),
        DiscoveryServiceDto::Pairing => pairing_candidates(state),
    };
    DiscoveryStatusDto {
        permission: state.discovery_permission.into(),
        candidates: candidates.into_iter().map(endpoint_dto).collect(),
    }
}

fn owns_discovery(state: &SessionState, cancellation: &Arc<AtomicBool>) -> bool {
    state
        .discovery_cancel
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, cancellation))
}

fn require_current_discovery(
    state: &SessionState,
    cancellation: &Arc<AtomicBool>,
) -> Result<(), HidlinsApiError> {
    if state.dead || cancellation.load(Ordering::Acquire) || !owns_discovery(state, cancellation) {
        return Err(HidlinsApiError::SyncCanceled);
    }
    Ok(())
}

pub(super) fn cancel_discovery_locked(state: &mut SessionState) -> Arc<AtomicBool> {
    // Keep this canceled identity installed until its owner finishes cleanup.
    // Already queued polls retain it; new calls cannot start a replacement yet.
    let cancellation = state
        .discovery_cancel
        .get_or_insert_with(|| Arc::new(AtomicBool::new(false)));
    cancellation.store(true, Ordering::Release);
    Arc::clone(cancellation)
}

fn discovery_unavailable() -> HidlinsApiError {
    HidlinsApiError::Internal {
        context: "discovery state unavailable".into(),
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
        let previous = state
            .registry
            .get(&vault_name)
            .and_then(LocalSyncConfig::from_vault_entry)
            .map(|config| config.identity().public_key());
        let configured =
            LocalSyncConfig::configure(&mut state.registry, &vault_name, role.into(), &master);
        let current = state
            .registry
            .get(&vault_name)
            .and_then(LocalSyncConfig::from_vault_entry)
            .map(|config| config.identity().public_key());
        // A durability error may still leave the new registry committed.
        // Retire the old authority whenever its identity actually changed.
        if previous != current {
            state.pairing_api.cancel_all();
            state.server_requested = None;
            state.stop_server_runtime();
        }
        configured.map_err(|_| HidlinsApiError::LocalSyncConfiguration)?;
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
        self.local_discovery_status_for(DiscoveryServiceDto::Trusted)
    }

    /// Return untrusted routes for the requested service, retaining explicit routes first.
    pub fn local_discovery_status_for(&self, service: DiscoveryServiceDto) -> DiscoveryStatusDto {
        let state = self.lock_state();
        discovery_status(&state, service)
    }

    /// Poll the platform discovery adapter and apply only Rust-validated routes.
    pub fn poll_local_discovery(&self) -> Result<DiscoveryStatusDto, HidlinsApiError> {
        self.poll_local_discovery_for(DiscoveryServiceDto::Trusted)
    }

    /// Collect routes for normal sync or an explicitly open pairing service.
    pub fn poll_local_discovery_for(
        &self,
        service: DiscoveryServiceDto,
    ) -> Result<DiscoveryStatusDto, HidlinsApiError> {
        let cancellation = {
            let mut state = self.lock_state();
            if state.dead {
                return Err(HidlinsApiError::SyncCanceled);
            }
            let cancellation = state
                .discovery_cancel
                .get_or_insert_with(|| Arc::new(AtomicBool::new(false)));
            if cancellation.load(Ordering::Acquire) {
                return Err(HidlinsApiError::SyncCanceled);
            }
            Arc::clone(cancellation)
        };
        // Capture the identity before queuing, so stop also cancels polls that
        // have not acquired this mutex. Valid polls serialize through publish.
        #[cfg(all(test, feature = "desktop"))]
        discovery_stop_controls::after_claim();
        let _operation = self
            .discovery_operation
            .lock()
            .map_err(|_| discovery_unavailable())?;
        let state = self.lock_state();
        require_current_discovery(&state, &cancellation)?;
        #[cfg(feature = "desktop")]
        let mut cache = state.discovery_cache.clone();
        drop(state);
        #[cfg(feature = "desktop")]
        let collected = {
            let mut port = self
                .discovery_port
                .lock()
                .map_err(|_| discovery_unavailable())?;
            if cancellation.load(Ordering::Acquire) {
                return Err(HidlinsApiError::SyncCanceled);
            }
            if port.is_none() {
                *port = Some(Box::new(
                    hidlins_sync::discovery::desktop::DesktopBrowser::start()
                        .map_err(map_discovery_error)?,
                ));
            }
            let browser = port.as_mut().ok_or_else(|| HidlinsApiError::Internal {
                context: "discovery browser did not initialize".to_string(),
            })?;
            hidlins_sync::discovery::collect_candidates(
                browser.as_mut(),
                &mut cache,
                service.into(),
                std::time::Duration::from_millis(1_200),
                std::time::Duration::from_millis(25),
                || cancellation.load(Ordering::Acquire),
            )
            .map_err(map_discovery_error)
        };
        // The browser guard is gone before State is acquired, including on
        // backend errors. Stop never waits for an operation while owning State.
        #[cfg(all(test, feature = "desktop"))]
        discovery_stop_controls::before_publish();
        let state = self.lock_state();
        require_current_discovery(&state, &cancellation)?;
        #[cfg(feature = "desktop")]
        let state = {
            collected?;
            let mut state = state;
            state.discovery_cache = cache;
            state.discovery_permission = DiscoveryPermission::Granted;
            state
        };
        Ok(discovery_status(&state, service))
    }

    /// Stop browsing and cancel active or queued discovery without clearing routes.
    pub fn stop_local_discovery(&self) -> Result<(), HidlinsApiError> {
        let cancellation = cancel_discovery_locked(&mut self.lock_state());
        #[cfg(all(test, feature = "desktop"))]
        discovery_stop_controls::after_stop_request();
        self.finish_discovery_stop(&cancellation)
    }

    pub(super) fn finish_discovery_stop(
        &self,
        cancellation: &Arc<AtomicBool>,
    ) -> Result<(), HidlinsApiError> {
        let operation = self
            .discovery_operation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !owns_discovery(&self.lock_state(), cancellation) {
            return Ok(());
        }
        let browser = {
            let mut port = self
                .discovery_port
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let browser = port.take();
            self.discovery_port.clear_poison();
            browser
        };
        // Backend shutdown and Drop run outside both State and browser guards.
        let result = browser.map_or(Ok(()), |mut browser| {
            browser.shutdown().map_err(map_discovery_error)
        });
        {
            let mut state = self.lock_state();
            if owns_discovery(&state, cancellation) {
                state.discovery_cancel = None;
            }
        }
        self.discovery_operation.clear_poison();
        drop(operation);
        result
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
        run_sync(&self.inner, &self.sync_engine, false)
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
        let (config, name, master, candidates, backing_path, registration) = {
            let state = self.lock_state();
            let config = current_local_config(&state)?;
            if config.role() != SyncRole::Client || config.is_active_client() {
                return Err(HidlinsApiError::SyncAuthFailed);
            }
            let (name, master) = current_vault_name_and_master(&state)?;
            let backing_path = state.require_vault()?.backing_path().to_path_buf();
            let registration = self.pairing_api.reserve(None)?;
            (
                config,
                name,
                master,
                pairing_candidates(&state),
                backing_path,
                registration,
            )
        };
        let identity = config
            .identity()
            .unlock(&name, SyncRole::Client, &master)
            .map_err(|_| {
                if registration.is_current() {
                    HidlinsApiError::AuthenticationFailed
                } else {
                    HidlinsApiError::SyncCanceled
                }
            })?;
        let pairing = hidlins_sync::client::ClientPairingSession::begin_with_cancellation(
            &identity,
            candidates.clone(),
            registration.cancellation(),
        )
        .map_err(|error| pairing_failure(&registration, error))?;
        #[cfg(test)]
        pairing_lifetime_regressions::after_begin();
        self.store_client_pairing(
            pairing,
            config,
            candidates,
            PendingPairingTarget::Existing {
                vault_name: name,
                backing_path,
            },
            registration,
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
        let master = MasterPassword::new(master_password);
        let keyfile = super::vaults::protect_keyfile(keyfile);
        super::vaults::validate_vault_name(&name)?;
        let checked = candidates
            .iter()
            .map(LocalEndpointDto::checked)
            .collect::<Result<Vec<_>, _>>()?;
        if checked.is_empty() || checked.len() > hidlins_sync::protocol::MAX_CANDIDATE_ATTEMPTS {
            return Err(HidlinsApiError::SyncNotFound);
        }
        let (target, registration) = {
            let state = self.lock_state();
            if state.dead {
                return Err(HidlinsApiError::SyncCanceled);
            }
            if state.syncing || state.unlocking || state.lock_pending {
                return Err(HidlinsApiError::VaultBusySyncing);
            }
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
            let registration = self
                .pairing_api
                .reserve(Some((name.clone(), target.clone())))?;
            (target, registration)
        };
        let config = LocalSyncConfig::create(&name, SyncRole::Client, &master)
            .map_err(|_| HidlinsApiError::LocalSyncConfiguration)?;
        let identity = config
            .identity()
            .unlock(&name, SyncRole::Client, &master)
            .map_err(|_| {
                if registration.is_current() {
                    HidlinsApiError::AuthenticationFailed
                } else {
                    HidlinsApiError::SyncCanceled
                }
            })?;
        let pairing = hidlins_sync::client::ClientPairingSession::begin_with_cancellation(
            &identity,
            checked.clone(),
            registration.cancellation(),
        )
        .map_err(|error| pairing_failure(&registration, error))?;
        #[cfg(test)]
        pairing_lifetime_regressions::after_begin();
        self.store_client_pairing(
            pairing,
            config,
            checked,
            PendingPairingTarget::Import {
                vault_name: name,
                master,
                keyfile,
                target,
            },
            registration,
        )
    }

    fn store_client_pairing(
        &self,
        session: hidlins_sync::client::ClientPairingSession,
        config: LocalSyncConfig,
        candidates: Vec<LocalEndpoint>,
        target: PendingPairingTarget,
        registration: PairingRegistration,
    ) -> Result<PairingPromptDto, HidlinsApiError> {
        let state = self.lock_state();
        if !pairing_origin_matches(&state, &target, &config) {
            registration.cancel();
            return Err(HidlinsApiError::SyncCanceled);
        }
        let sas = session.sas().to_string();
        let handle = registration.handle().to_string();
        self.pairing_api.publish(PendingClientPairing {
            session,
            config,
            target,
            candidates,
            registration,
        })?;
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
                context: "pairing state unavailable".into(),
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
        let pending = self.pairing_api.take_pending(&transaction_handle)?;
        #[cfg(test)]
        pairing_lifetime_regressions::after_remove();
        if !accepted {
            return Ok(None);
        }
        self.confirm_client_pairing(pending, peer_display_name)
    }

    fn persist_existing_pairing(
        &self,
        registration: &PairingRegistration,
        target: &PendingPairingTarget,
        expected: &mut LocalSyncConfig,
        prepared: &LocalSyncConfig,
    ) -> Result<(), hidlins_sync::config::local::LocalConfigError> {
        use hidlins_sync::config::local::LocalConfigError;
        let mut state = self.lock_state();
        if !pairing_origin_matches(&state, target, expected) {
            registration.cancel();
            return Err(LocalConfigError::Malformed);
        }
        let PendingPairingTarget::Existing { vault_name, .. } = target else {
            return Err(LocalConfigError::Malformed);
        };
        let mut origin_changed = false;
        let result = registration
            .with_current(|| {
                #[cfg(test)]
                pairing_lifetime_regressions::before_persist();
                // Compare inside the registry's locked reload: another process or
                // a parallel prompt must not be overwritten by this old config.
                LocalSyncConfig::transactional_update(&mut state.registry, vault_name, |actual| {
                    if *actual != *expected {
                        origin_changed = true;
                        return Err(hidlins_sync::trust::TrustError::TransactionMismatch);
                    }
                    *actual = prepared.clone();
                    Ok(())
                })
            })
            .map_err(|_| LocalConfigError::Malformed)?;
        if origin_changed
            || matches!(
                &result,
                Err(LocalConfigError::Trust(_)
                    | LocalConfigError::Registry(hidlins_core::VaultError::NotRegistered { .. }))
            )
        {
            registration.cancel();
        }
        if result.is_ok() {
            *expected = prepared.clone();
        }
        result
    }

    fn confirm_client_pairing(
        &self,
        pending: PendingClientPairing,
        peer_display_name: String,
    ) -> Result<Option<VaultSummary>, HidlinsApiError> {
        // Registration outlives every later State/clients guard and the core
        // confirmation consumes only its own session, never this registration.
        let PendingClientPairing {
            session,
            mut config,
            target,
            candidates,
            registration,
        } = pending;
        {
            let state = self.lock_state();
            if !pairing_origin_matches(&state, &target, &config) {
                registration.cancel();
                return Err(HidlinsApiError::SyncCanceled);
            }
            registration.with_current(|| ())?;
        }
        let now = epoch_seconds().map_err(|()| HidlinsApiError::Internal {
            context: "system clock unavailable".into(),
        })?;
        let mut expected = config.clone();
        let result = match &target {
            PendingPairingTarget::Existing { .. } => {
                session.confirm(&mut config, peer_display_name, now, |prepared| {
                    self.persist_existing_pairing(&registration, &target, &mut expected, prepared)
                })
            }
            PendingPairingTarget::Import { vault_name, .. } => {
                let store = hidlins_sync::client::PendingPairingStore::new(
                    self.paths.state_dir(),
                    vault_name,
                );
                session.confirm(&mut config, peer_display_name, now, |prepared| {
                    let state = self.lock_state();
                    if !pairing_origin_matches(&state, &target, prepared) {
                        registration.cancel();
                        return Err(hidlins_sync::config::local::LocalConfigError::Malformed);
                    }
                    registration
                        .with_current(|| store.save(prepared))
                        .map_err(|_| hidlins_sync::config::local::LocalConfigError::Malformed)?
                })
            }
        };
        result.map_err(|error| pairing_failure(&registration, error))?;
        match &target {
            // Core confirm already persisted the active config under the
            // guarded compare-and-replace. Do not add a third stale write.
            PendingPairingTarget::Existing { .. } => Ok(None),
            PendingPairingTarget::Import {
                vault_name,
                master,
                keyfile,
                target: path,
            } => {
                let (bytes, version) = hidlins_sync::client::fetch_paired_vault_with_cancellation(
                    vault_name,
                    master,
                    &config,
                    candidates,
                    registration.cancellation(),
                )
                .map_err(|error| pairing_failure(&registration, error))?;
                #[cfg(test)]
                pairing_lifetime_regressions::after_fetch();
                config.set_sync_versions(Some(version), Some(version));
                let mut state = self.lock_state();
                if !pairing_origin_matches(&state, &target, &config) {
                    registration.cancel();
                    return Err(HidlinsApiError::SyncCanceled);
                }
                registration.with_current(|| {
                    hidlins_sync::client::import_paired_vault(
                        &bytes,
                        path,
                        vault_name,
                        master,
                        keyfile.as_ref(),
                        &config,
                        &mut state.registry,
                    )
                    .map_err(|_| HidlinsApiError::InvalidFormat)?;
                    hidlins_sync::client::PendingPairingStore::new(
                        state.registry.paths().state_dir(),
                        vault_name,
                    )
                    .clear()
                    .map_err(|_| HidlinsApiError::LocalSyncConfiguration)?;
                    Ok(Some(VaultSummary {
                        name: vault_name.clone(),
                        path: path.display().to_string(),
                        has_keyfile: keyfile.is_some(),
                        has_sync: true,
                    }))
                })?
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
        let result = self.update_peer(
            peer_id,
            hidlins_sync::config::local::LocalSyncConfig::revoke_peer,
        );
        #[cfg(all(test, debug_assertions))]
        revoke_reconciliation_regressions::after_update();
        result
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
        let result = LocalSyncConfig::transactional_update(&mut state.registry, &name, |config| {
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
        .map_err(|_| HidlinsApiError::SyncRevoked);
        // Registry memory follows a committed rename even if durability
        // reports an error. Reconcile installed trust before releasing this
        // guard, so a host ownership loan cannot skip revocation afterward.
        if let (Some(runtime), Ok(config)) = (&state.server_runtime, current_local_config(&state)) {
            runtime
                .controller
                .replace_trusted(active_peer_keys(&config));
        }
        result
    }

    pub fn clear_sync_config(&self, name: String) -> Result<(), HidlinsApiError> {
        let mut state = self.lock_state();
        if state.syncing {
            return Err(HidlinsApiError::VaultBusySyncing);
        }
        state.pairing_api.cancel_all();
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
    automatic: bool,
) -> Result<SyncOutcomeDto, HidlinsApiError> {
    let (mut vault, mut registry, master, keyfile, name, candidates, cancellation) = {
        let mut state = lock_session_state(inner);
        if state.dead || state.lock_pending {
            return Err(HidlinsApiError::VaultLocked);
        }
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

    let mut state = lock_session_state(inner);
    state.syncing = false;
    state.sync_cancellation = None;
    let lock_requested = std::mem::take(&mut state.lock_pending);
    super::session::clear_totp_cache_inner(&state.totp_cache);
    if lock_requested || state.dead {
        state.registry = registry;
        state.stop_server_runtime();
        state.credentials = None;
        state
            .controller
            .lock_now(hidlins_security::OsLockReason::Manual);
        state.push_sync_event(SyncEvent::Failed(HidlinsApiError::VaultLocked));
        // The pending-lock or poison transition already published Locked.
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
            state.revoke_cached_secrets();
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
    let worker = std::thread::Builder::new()
        .name("hidlins-startup-sync".to_string())
        .spawn(move || {
            let _ = run_sync(&inner, &engine, true);
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
        processor: Some(processor),
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
    let (mut processor, mut vault, master, keyfile) = {
        let Some(mut state) = try_lock_session_state(inner) else {
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
        // The listener/controller stays reachable by stop, close and lock
        // while the worker exclusively owns its processor and vault.
        let processor = state
            .server_runtime
            .as_mut()
            .expect("checked above")
            .processor
            .take()
            .expect("host processor is present outside an ownership loan");
        let vault = state.vault.take().expect("checked above");
        state.syncing = true;
        (processor, vault, master, keyfile)
    };
    #[cfg(test)]
    server_lifecycle_regressions::after_claim();
    #[cfg(test)]
    server_unlock_regressions::after_claim();
    #[cfg(all(test, debug_assertions))]
    revoke_reconciliation_regressions::after_claim();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut host = AuthoritativeVault::new(&mut vault, &master, keyfile.as_ref());
        let result = processor.process_one(&mut host);
        (result, host.has_committed())
    }));
    let mut state = lock_session_state(inner);
    state.syncing = false;
    let lock_requested = std::mem::take(&mut state.lock_pending);
    if state.dead || lock_requested || result.is_err() {
        state.revoke_cached_secrets();
        processor.shutdown();
        state.stop_server_runtime();
        if state.dead || result.is_err() {
            state.server_requested = None;
        }
        state.credentials = None;
        if !state.dead && !lock_requested {
            state
                .controller
                .lock_now(hidlins_security::OsLockReason::Manual);
            state.push_lock_event(crate::dto::LockEvent::Locked);
        }
        return;
    }
    state.vault = Some(vault);
    if let Some(runtime) = state.server_runtime.as_mut() {
        runtime.processor = Some(processor);
    } else {
        // An explicit stop already closed admission and published its event.
        // The vault remains available for local operations after handback.
        processor.shutdown();
    }
    if result.as_ref().is_ok_and(|(_, committed)| *committed) {
        // A fresh host adapter reports only an actually installed replacement,
        // including rename followed by a durability error. Restore the vault
        // and revoke old snapshots before the existing provider change event.
        state.revoke_cached_secrets();
        state.push_sync_event(SyncEvent::Done(SyncOutcomeDto::FastReplaced));
    }
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

#[cfg(test)]
#[path = "server_lifecycle_regressions.rs"]
mod server_lifecycle_regressions;

#[cfg(test)]
#[path = "server_unlock_regressions.rs"]
mod server_unlock_regressions;

#[cfg(all(test, debug_assertions))]
#[path = "server_config_commit_regressions.rs"]
mod server_config_commit_regressions;

#[cfg(all(test, debug_assertions))]
#[path = "revoke_reconciliation_regressions.rs"]
mod revoke_reconciliation_regressions;

#[cfg(all(test, debug_assertions))]
#[path = "incoming_commit_regressions.rs"]
mod incoming_commit_regressions;

#[cfg(test)]
#[path = "pairing_lifetime_regressions.rs"]
mod pairing_lifetime_regressions;

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
#[path = "pairing_activation_regressions.rs"]
mod pairing_activation_regressions;

#[cfg(all(test, feature = "desktop"))]
#[path = "discovery_kind_regressions.rs"]
mod discovery_kind_regressions;

#[cfg(all(test, feature = "desktop"))]
#[path = "discovery_cancel_regressions.rs"]
pub(super) mod discovery_cancel_regressions;

#[cfg(all(test, feature = "desktop"))]
#[path = "discovery_stop_controls.rs"]
mod discovery_stop_controls;

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

//! Non-blocking discovery, pairing confirmation, and pair-and-import work.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hidlins_core::{HidlinsPaths, MasterPassword, VaultRegistry};
use hidlins_sync::{
    address::LocalEndpoint,
    client::{self, ClientPairingSession},
    config::local::LocalSyncConfig,
    discovery::{self, CandidateCache, ServiceKind},
    identity::SyncRole,
};

pub(crate) enum PairingEvent {
    Prompt { sas: String },
    Complete { imported: Option<String> },
    Failed(String),
}

enum PairingTarget {
    Existing {
        vault_name: String,
    },
    Import {
        vault_name: String,
        target: PathBuf,
        master: MasterPassword,
    },
}

struct Pending {
    session: ClientPairingSession,
    config: LocalSyncConfig,
    candidates: Vec<LocalEndpoint>,
    peer_name: String,
    target: PairingTarget,
}

enum WorkerResult {
    Begun(Box<Pending>),
    Complete(Option<String>),
    Failed(String),
}

pub(crate) struct PairingRuntime {
    receiver: Option<Receiver<WorkerResult>>,
    pending: Option<Pending>,
    paths: HidlinsPaths,
}

impl PairingRuntime {
    pub(crate) fn new(paths: HidlinsPaths) -> Self {
        Self {
            receiver: None,
            pending: None,
            paths,
        }
    }

    pub(crate) fn begin_existing(
        &mut self,
        vault_name: String,
        config: LocalSyncConfig,
        master: &MasterPassword,
        peer_name: String,
    ) -> Result<(), String> {
        if self.receiver.is_some() || self.pending.is_some() {
            return Err("A pairing attempt is already active.".to_string());
        }
        let identity = config
            .identity()
            .unlock(&vault_name, SyncRole::Client, master)
            .map_err(|_| "Master password did not unlock the sync identity.".to_string())?;
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("hidlins-tui-pairing".to_string())
            .spawn(move || {
                let candidates = discover(ServiceKind::Pairing, config.routing_hint());
                let result = ClientPairingSession::begin(&identity, candidates.clone())
                    .map(|session| {
                        Box::new(Pending {
                            session,
                            config,
                            candidates,
                            peer_name,
                            target: PairingTarget::Existing { vault_name },
                        })
                    })
                    .map_err(|_| {
                        "No pairable Hidlins server was found on the local network.".to_string()
                    });
                let _ = sender.send(result.map_or_else(WorkerResult::Failed, WorkerResult::Begun));
            })
            .map_err(|_| "Could not start the pairing worker.".to_string())?;
        self.receiver = Some(receiver);
        Ok(())
    }

    pub(crate) fn begin_import(
        &mut self,
        vault_name: String,
        master: MasterPassword,
        peer_name: String,
    ) -> Result<(), String> {
        if self.receiver.is_some() || self.pending.is_some() {
            return Err("A pairing attempt is already active.".to_string());
        }
        if vault_name.is_empty()
            || vault_name.len() > 64
            || !vault_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || self
                .paths
                .state_dir()
                .join(format!("{vault_name}.kdbx"))
                .exists()
        {
            return Err(
                "Choose a new vault name using only letters, numbers, '-' and '_'.".to_string(),
            );
        }
        let config = LocalSyncConfig::create(&vault_name, SyncRole::Client, &master)
            .map_err(|_| "Could not create the local sync identity.".to_string())?;
        let identity = config
            .identity()
            .unlock(&vault_name, SyncRole::Client, &master)
            .map_err(|_| "Could not unlock the local sync identity.".to_string())?;
        let target = self.paths.state_dir().join(format!("{vault_name}.kdbx"));
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("hidlins-tui-import-pairing".to_string())
            .spawn(move || {
                let candidates = discover(ServiceKind::Pairing, None);
                let result = ClientPairingSession::begin(&identity, candidates.clone())
                    .map(|session| {
                        Box::new(Pending {
                            session,
                            config,
                            candidates,
                            peer_name,
                            target: PairingTarget::Import {
                                vault_name,
                                target,
                                master,
                            },
                        })
                    })
                    .map_err(|_| {
                        "No pairable Hidlins server was found on the local network.".to_string()
                    });
                let _ = sender.send(result.map_or_else(WorkerResult::Failed, WorkerResult::Begun));
            })
            .map_err(|_| "Could not start the pairing worker.".to_string())?;
        self.receiver = Some(receiver);
        Ok(())
    }

    pub(crate) fn confirm(&mut self, accepted: bool) -> Result<(), String> {
        let Some(mut pending) = self.pending.take() else {
            return Err("The pairing request is no longer active.".to_string());
        };
        if !accepted {
            return Ok(());
        }
        let paths = self.paths.clone();
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("hidlins-tui-pairing-confirm".to_string())
            .spawn(move || {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs());
                let result = pending
                    .session
                    .confirm(&mut pending.config, pending.peer_name, now)
                    .map_err(|_| "Pairing failed safely; trust was not activated.".to_string())
                    .and_then(|_| match pending.target {
                        PairingTarget::Existing { vault_name } => {
                            let mut registry = VaultRegistry::load(paths.clone())
                                .unwrap_or_else(|_| VaultRegistry::with_paths(paths));
                            pending
                                .config
                                .persist(&mut registry, &vault_name)
                                .map_err(|_| "Could not persist paired trust.".to_string())?;
                            Ok(None)
                        }
                        PairingTarget::Import {
                            vault_name,
                            target,
                            master,
                        } => {
                            let (bytes, version) = client::fetch_paired_vault(
                                &vault_name,
                                &master,
                                &pending.config,
                                pending.candidates,
                            )
                            .map_err(|_| "The paired vault could not be fetched.".to_string())?;
                            pending
                                .config
                                .set_sync_versions(Some(version), Some(version));
                            let mut registry = VaultRegistry::load(paths.clone())
                                .unwrap_or_else(|_| VaultRegistry::with_paths(paths));
                            client::import_paired_vault(
                                &bytes,
                                &target,
                                &vault_name,
                                &master,
                                None,
                                &pending.config,
                                &mut registry,
                            )
                            .map_err(|_| {
                                "The imported KDBX vault failed validation.".to_string()
                            })?;
                            Ok(Some(vault_name))
                        }
                    });
                let _ =
                    sender.send(result.map_or_else(WorkerResult::Failed, WorkerResult::Complete));
            })
            .map_err(|_| "Could not start the confirmation worker.".to_string())?;
        self.receiver = Some(receiver);
        Ok(())
    }

    pub(crate) fn poll(&mut self) -> Option<PairingEvent> {
        let result = self.receiver.as_ref()?.try_recv().ok()?;
        self.receiver = None;
        match result {
            WorkerResult::Begun(pending) => {
                let sas = pending.session.sas().to_string();
                self.pending = Some(*pending);
                Some(PairingEvent::Prompt { sas })
            }
            WorkerResult::Complete(imported) => Some(PairingEvent::Complete { imported }),
            WorkerResult::Failed(message) => Some(PairingEvent::Failed(message)),
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.receiver = None;
        self.pending = None;
    }
}

fn discover(kind: ServiceKind, hint: Option<LocalEndpoint>) -> Vec<LocalEndpoint> {
    let mut candidates = hint.into_iter().collect::<Vec<_>>();
    let Ok(mut browser) = discovery::desktop::DesktopBrowser::start() else {
        return candidates;
    };
    let mut cache = CandidateCache::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && candidates.is_empty() {
        if discovery::poll_into(&mut browser, &mut cache, Instant::now()).is_err() {
            break;
        }
        candidates.extend(cache.candidates(kind));
        if candidates.is_empty() {
            thread::sleep(Duration::from_millis(20));
        }
    }
    candidates
}

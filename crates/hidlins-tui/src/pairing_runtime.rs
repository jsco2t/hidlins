//! Non-blocking discovery, pairing confirmation, and pair-and-import work.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hidlins_core::{HidlinsPaths, MasterPassword, VaultRegistry};
use hidlins_sync::{
    address::LocalEndpoint,
    client::{self, ClientPairingSession, LanCancellation, PendingPairingStore},
    config::local::{LocalConfigError, LocalSyncConfig},
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
    operation: Option<PairingOperation>,
    pending: Option<Pending>,
    paths: HidlinsPaths,
}

struct PairingOperation {
    receiver: Receiver<WorkerResult>,
    cancellation: LanCancellation,
    handle: JoinHandle<()>,
}

impl PairingRuntime {
    pub(crate) fn new(paths: HidlinsPaths) -> Self {
        Self {
            operation: None,
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
        self.cancel();
        let identity = config
            .identity()
            .unlock(&vault_name, SyncRole::Client, master)
            .map_err(|_| "Master password did not unlock the sync identity.".to_string())?;
        self.spawn_operation("hidlins-tui-pairing", move |cancellation| {
            let candidates = discover(ServiceKind::Pairing, config.routing_hint(), &cancellation);
            ClientPairingSession::begin_with_cancellation(
                &identity,
                candidates.clone(),
                cancellation,
            )
            .map(|session| {
                Box::new(Pending {
                    session,
                    config,
                    candidates,
                    peer_name,
                    target: PairingTarget::Existing { vault_name },
                })
            })
            .map_err(|_| "No pairable Hidlins server was found on the local network.".to_string())
            .map_or_else(WorkerResult::Failed, WorkerResult::Begun)
        })
    }

    pub(crate) fn begin_import(
        &mut self,
        vault_name: String,
        master: MasterPassword,
        peer_name: String,
    ) -> Result<(), String> {
        self.cancel();
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
        self.spawn_operation("hidlins-tui-import-pairing", move |cancellation| {
            let candidates = discover(ServiceKind::Pairing, None, &cancellation);
            ClientPairingSession::begin_with_cancellation(
                &identity,
                candidates.clone(),
                cancellation,
            )
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
            .map_err(|_| "No pairable Hidlins server was found on the local network.".to_string())
            .map_or_else(WorkerResult::Failed, WorkerResult::Begun)
        })
    }

    pub(crate) fn confirm(&mut self, accepted: bool) -> Result<(), String> {
        let Some(mut pending) = self.pending.take() else {
            return Err("The pairing request is no longer active.".to_string());
        };
        if !accepted {
            return Ok(());
        }
        let paths = self.paths.clone();
        self.spawn_operation("hidlins-tui-pairing-confirm", move |cancellation| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            let persistence = match &pending.target {
                PairingTarget::Existing { vault_name } => {
                    let paths = paths.clone();
                    let vault_name = vault_name.clone();
                    let cancellation = cancellation.clone();
                    Box::new(move |prepared: &LocalSyncConfig| {
                        if cancellation.is_cancelled() {
                            return Err(LocalConfigError::Malformed);
                        }
                        let mut registry = VaultRegistry::load(paths.clone())
                            .unwrap_or_else(|_| VaultRegistry::with_paths(paths.clone()));
                        prepared.persist(&mut registry, &vault_name)
                    })
                        as Box<dyn FnMut(&LocalSyncConfig) -> Result<(), LocalConfigError>>
                }
                PairingTarget::Import { vault_name, .. } => {
                    let store = PendingPairingStore::new(paths.state_dir(), vault_name);
                    let cancellation = cancellation.clone();
                    Box::new(move |prepared: &LocalSyncConfig| {
                        if cancellation.is_cancelled() {
                            return Err(LocalConfigError::Malformed);
                        }
                        store.save(prepared)
                    })
                }
            };
            let mut persistence = persistence;
            let result = pending
                .session
                .confirm(
                    &mut pending.config,
                    pending.peer_name,
                    now,
                    &mut persistence,
                )
                .map_err(|_| "Pairing failed safely; trust was not activated.".to_string())
                .and_then(|_| match pending.target {
                    PairingTarget::Existing { .. } => Ok(None),
                    PairingTarget::Import {
                        vault_name,
                        target,
                        master,
                    } => {
                        let (bytes, version) = client::fetch_paired_vault_with_cancellation(
                            &vault_name,
                            &master,
                            &pending.config,
                            pending.candidates,
                            cancellation.clone(),
                        )
                        .map_err(|_| "The paired vault could not be fetched.".to_string())?;
                        if cancellation.is_cancelled() {
                            return Err("Pairing was cancelled.".to_string());
                        }
                        pending
                            .config
                            .set_sync_versions(Some(version), Some(version));
                        let mut registry = VaultRegistry::load(paths.clone())
                            .unwrap_or_else(|_| VaultRegistry::with_paths(paths.clone()));
                        client::import_paired_vault(
                            &bytes,
                            &target,
                            &vault_name,
                            &master,
                            None,
                            &pending.config,
                            &mut registry,
                        )
                        .map_err(|_| "The imported KDBX vault failed validation.".to_string())?;
                        PendingPairingStore::new(paths.state_dir(), &vault_name)
                            .clear()
                            .map_err(|_| "Could not clear completed pairing state.".to_string())?;
                        Ok(Some(vault_name))
                    }
                });
            result.map_or_else(WorkerResult::Failed, WorkerResult::Complete)
        })
        .map_err(|_| "Could not start the confirmation worker.".to_string())
    }

    pub(crate) fn poll(&mut self) -> Option<PairingEvent> {
        let result = match self.operation.as_ref()?.receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                WorkerResult::Failed("The pairing worker stopped unexpectedly.".to_string())
            }
        };
        let operation = self.operation.take().expect("polled operation exists");
        let _ = operation.handle.join();
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

    /// Cancel and join any secret-bearing worker before returning.
    ///
    /// TCP connection setup is bounded by the two-second per-candidate timeout;
    /// once a socket exists, cancellation shuts it down to interrupt I/O.
    pub(crate) fn cancel(&mut self) {
        if let Some(operation) = self.operation.take() {
            operation.cancellation.cancel();
            let _ = operation.handle.join();
        }
        self.pending = None;
    }

    fn spawn_operation(
        &mut self,
        name: &str,
        work: impl FnOnce(LanCancellation) -> WorkerResult + Send + 'static,
    ) -> Result<(), String> {
        let cancellation = LanCancellation::default();
        let worker_cancellation = cancellation.clone();
        let (sender, receiver) = mpsc::channel();
        let handle = thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                let result = work(worker_cancellation);
                let _ = sender.send(result);
            })
            .map_err(|_| "Could not start the pairing worker.".to_string())?;
        self.operation = Some(PairingOperation {
            receiver,
            cancellation,
            handle,
        });
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn install_cancellation_probe(
        &mut self,
        entered: std::sync::Arc<std::sync::Barrier>,
        dropped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) {
        self.cancel();
        self.spawn_operation("pairing-cancellation-probe", move |cancellation| {
            struct DropProbe(std::sync::Arc<std::sync::atomic::AtomicBool>);
            impl Drop for DropProbe {
                fn drop(&mut self) {
                    self.0.store(true, std::sync::atomic::Ordering::Release);
                }
            }

            let _secret = DropProbe(dropped);
            entered.wait();
            while !cancellation.is_cancelled() {
                thread::yield_now();
            }
            WorkerResult::Complete(None)
        })
        .expect("test pairing operation starts");
    }
}

impl Drop for PairingRuntime {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn discover(
    kind: ServiceKind,
    hint: Option<LocalEndpoint>,
    cancellation: &LanCancellation,
) -> Vec<LocalEndpoint> {
    let mut candidates = hint.into_iter().collect::<Vec<_>>();
    if cancellation.is_cancelled() {
        return candidates;
    }
    let Ok(mut browser) = discovery::desktop::DesktopBrowser::start() else {
        return candidates;
    };
    let mut cache = CandidateCache::new();
    if let Ok(discovered) = discovery::collect_candidates(
        &mut browser,
        &mut cache,
        kind,
        Duration::from_secs(2),
        Duration::from_millis(20),
        || cancellation.is_cancelled(),
    ) {
        for endpoint in discovered {
            if !candidates.contains(&endpoint) {
                candidates.push(endpoint);
            }
            if candidates.len() == hidlins_sync::protocol::MAX_CANDIDATE_ATTEMPTS {
                break;
            }
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};

    use super::*;

    #[test]
    fn pairing_cancel_joins_worker_before_modeled_mutation_and_releases_capture() {
        // LNS-TUI-012: explicit cancellation joins the owned worker before a
        // persistence-like mutation and releases its worker capture.
        let mut runtime = PairingRuntime::new(HidlinsPaths::with_state_dir(PathBuf::from(
            "/unused/pairing-runtime-test",
        )));
        let entered = Arc::new(Barrier::new(2));
        let mutations = Arc::new(AtomicUsize::new(0));
        let secret_dropped = Arc::new(AtomicBool::new(false));

        let worker_entered = Arc::clone(&entered);
        let worker_mutations = Arc::clone(&mutations);
        let worker_secret_dropped = Arc::clone(&secret_dropped);
        runtime
            .spawn_operation("pairing-cancellation-test", move |cancellation| {
                struct DropProbe(Arc<AtomicBool>);
                impl Drop for DropProbe {
                    fn drop(&mut self) {
                        self.0.store(true, Ordering::Release);
                    }
                }
                let _secret = DropProbe(worker_secret_dropped);
                worker_entered.wait();
                while !cancellation.is_cancelled() {
                    thread::yield_now();
                }
                if !cancellation.is_cancelled() {
                    worker_mutations.fetch_add(1, Ordering::AcqRel);
                }
                WorkerResult::Complete(None)
            })
            .unwrap();

        entered.wait();
        runtime.cancel();

        assert_eq!(
            mutations.load(Ordering::Acquire),
            0,
            "pairing cancellation must prevent worker mutation before returning"
        );
        assert!(
            secret_dropped.load(Ordering::Acquire),
            "joining cancellation must release secret-bearing worker captures"
        );
    }

    #[test]
    fn pairing_replacement_cancels_and_joins_previous_worker() {
        // LNS-TUI-013: replacement cannot detach the previous generation.
        let mut runtime = PairingRuntime::new(HidlinsPaths::with_state_dir(PathBuf::from(
            "/unused/pairing-runtime-replacement-test",
        )));
        let first_entered = Arc::new(Barrier::new(2));
        let first_dropped = Arc::new(AtomicBool::new(false));
        runtime.install_cancellation_probe(Arc::clone(&first_entered), Arc::clone(&first_dropped));
        first_entered.wait();

        let second_entered = Arc::new(Barrier::new(2));
        let second_dropped = Arc::new(AtomicBool::new(false));
        runtime
            .install_cancellation_probe(Arc::clone(&second_entered), Arc::clone(&second_dropped));
        assert!(first_dropped.load(Ordering::Acquire));

        second_entered.wait();
        runtime.cancel();
        assert!(second_dropped.load(Ordering::Acquire));
    }
}

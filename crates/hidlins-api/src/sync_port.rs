use hidlins_core::{Keyfile, MasterPassword, Vault, VaultRegistry};
use hidlins_sync::{address::LocalEndpoint, client::LanCancellation, SyncOptions, SyncOutcome};

pub trait SyncEnginePort: Send + Sync {
    #[allow(clippy::too_many_arguments)] // Port mirrors the complete core sync ownership claim.
    fn sync_now(
        &self,
        vault: &mut Vault,
        vault_name: &str,
        registry: &mut VaultRegistry,
        master_password: &MasterPassword,
        keyfile: Option<&Keyfile>,
        candidates: Vec<LocalEndpoint>,
        opts: SyncOptions,
        cancellation: LanCancellation,
    ) -> Result<SyncOutcome, hidlins_sync::SyncError>;
}

pub(crate) struct DefaultSyncEngine;

impl SyncEnginePort for DefaultSyncEngine {
    fn sync_now(
        &self,
        vault: &mut Vault,
        vault_name: &str,
        registry: &mut VaultRegistry,
        master_password: &MasterPassword,
        keyfile: Option<&Keyfile>,
        candidates: Vec<LocalEndpoint>,
        opts: SyncOptions,
        cancellation: LanCancellation,
    ) -> Result<SyncOutcome, hidlins_sync::SyncError> {
        hidlins_sync::client::sync_vault_with_cancellation(
            vault,
            vault_name,
            registry,
            master_password,
            keyfile,
            candidates,
            opts,
            cancellation,
        )
    }
}

#[cfg(any(test, feature = "test-fixtures"))]
pub use test_doubles::*;

#[cfg(any(test, feature = "test-fixtures"))]
mod test_doubles {
    use super::{
        Keyfile, MasterPassword, SyncEnginePort, SyncOptions, SyncOutcome, Vault, VaultRegistry,
    };
    use std::sync::{Arc, Barrier};

    pub struct SucceedingSyncEngine(pub SyncOutcome);

    impl SyncEnginePort for SucceedingSyncEngine {
        fn sync_now(
            &self,
            _vault: &mut Vault,
            _vault_name: &str,
            _registry: &mut VaultRegistry,
            _master_password: &MasterPassword,
            _keyfile: Option<&Keyfile>,
            _candidates: Vec<hidlins_sync::address::LocalEndpoint>,
            _opts: SyncOptions,
            _cancellation: hidlins_sync::client::LanCancellation,
        ) -> Result<SyncOutcome, hidlins_sync::SyncError> {
            Ok(self.0.clone())
        }
    }

    pub struct FailingSyncEngine<F: Fn() -> hidlins_sync::SyncError + Send + Sync>(pub F);

    impl<F: Fn() -> hidlins_sync::SyncError + Send + Sync> SyncEnginePort for FailingSyncEngine<F> {
        fn sync_now(
            &self,
            _vault: &mut Vault,
            _vault_name: &str,
            _registry: &mut VaultRegistry,
            _master_password: &MasterPassword,
            _keyfile: Option<&Keyfile>,
            _candidates: Vec<hidlins_sync::address::LocalEndpoint>,
            _opts: SyncOptions,
            _cancellation: hidlins_sync::client::LanCancellation,
        ) -> Result<SyncOutcome, hidlins_sync::SyncError> {
            Err(self.0())
        }
    }

    pub struct PanickingSyncEngine;

    impl SyncEnginePort for PanickingSyncEngine {
        fn sync_now(
            &self,
            _vault: &mut Vault,
            _vault_name: &str,
            _registry: &mut VaultRegistry,
            _master_password: &MasterPassword,
            _keyfile: Option<&Keyfile>,
            _candidates: Vec<hidlins_sync::address::LocalEndpoint>,
            _opts: SyncOptions,
            _cancellation: hidlins_sync::client::LanCancellation,
        ) -> Result<SyncOutcome, hidlins_sync::SyncError> {
            panic!("PanickingSyncEngine: deliberate test panic");
        }
    }

    pub struct BlockingSyncEngine {
        pub barrier: Arc<Barrier>,
        pub outcome: SyncOutcome,
        pub entered_tx: Option<std::sync::mpsc::Sender<()>>,
    }

    impl BlockingSyncEngine {
        pub fn new(barrier: Arc<Barrier>, outcome: SyncOutcome) -> Self {
            Self {
                barrier,
                outcome,
                entered_tx: None,
            }
        }

        pub fn with_entry_signal(
            barrier: Arc<Barrier>,
            outcome: SyncOutcome,
        ) -> (Self, std::sync::mpsc::Receiver<()>) {
            let (tx, rx) = std::sync::mpsc::channel();
            (
                Self {
                    barrier,
                    outcome,
                    entered_tx: Some(tx),
                },
                rx,
            )
        }
    }

    impl SyncEnginePort for BlockingSyncEngine {
        fn sync_now(
            &self,
            _vault: &mut Vault,
            _vault_name: &str,
            _registry: &mut VaultRegistry,
            _master_password: &MasterPassword,
            _keyfile: Option<&Keyfile>,
            _candidates: Vec<hidlins_sync::address::LocalEndpoint>,
            _opts: SyncOptions,
            _cancellation: hidlins_sync::client::LanCancellation,
        ) -> Result<SyncOutcome, hidlins_sync::SyncError> {
            if let Some(ref tx) = self.entered_tx {
                let _ = tx.send(());
            }
            self.barrier.wait();
            Ok(self.outcome.clone())
        }
    }
}

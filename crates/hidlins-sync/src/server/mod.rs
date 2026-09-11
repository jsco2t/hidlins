//! Host-owned authoritative vault operations and bounded network queue.

mod runtime;

pub use runtime::{PairingAuthority, PairingAuthorityError, ServerController, ServerRuntimeError};

use std::{
    collections::BTreeSet,
    fmt,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
        Arc, RwLock,
    },
    time::Duration,
};

/// Deterministic storage boundaries used by the security fault suite.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ServerFaultPoint {
    /// Before creating a sibling stage.
    StageCreate,
    /// Before appending an encrypted chunk.
    StageWrite,
    /// Before syncing and finalizing the stage.
    StageFinalize,
    /// Before parsing and authenticating the staged KDBX.
    Validation,
    /// Before producing the pre-commit backup.
    Backup,
    /// Before replacing the process-owned database.
    DatabaseReplacement,
    /// Immediately before the atomic encrypted save operation.
    Save,
}

/// Cloneable deterministic fault plan. Production constructors use an empty plan.
#[derive(Clone, Default)]
pub struct ServerFaultPlan(Arc<BTreeSet<ServerFaultPoint>>);

impl ServerFaultPlan {
    /// Fail at exactly the supplied boundaries.
    #[must_use]
    pub fn at(points: impl IntoIterator<Item = ServerFaultPoint>) -> Self {
        Self(Arc::new(points.into_iter().collect()))
    }

    fn check(&self, point: ServerFaultPoint) -> Result<(), ServerError> {
        if self.0.contains(&point) {
            Err(ServerError::Internal)
        } else {
            Ok(())
        }
    }
}

impl fmt::Debug for ServerFaultPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServerFaultPlan([REDACTED])")
    }
}

use hidlins_core::{Keyfile, MasterPassword, Vault};
use sha2::{Digest, Sha256};
use tempfile::{Builder as TempFileBuilder, NamedTempFile};

use crate::{
    backup,
    protocol::{
        RemoteVersion, MAX_APPLICATION_CHUNK, MAX_ENCRYPTED_VAULT, MAX_PENDING_HOST_OPERATIONS,
    },
};

/// Secret-free server failure suitable for fixed wire-code mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ServerError {
    /// The authenticated identity is not active for this vault.
    #[error("server peer is not authorized")]
    NotAuthorized,
    /// A bounded queue, connection set, or operation is occupied.
    #[error("server is busy")]
    Busy,
    /// The conditional version no longer matches authoritative bytes.
    #[error("server version changed")]
    StaleVersion,
    /// A declared or streamed object exceeds the fixed V1 maximum.
    #[error("server object is too large")]
    TooLarge,
    /// Length, digest, KDBX, credential, identity, or KDF validation failed.
    #[error("server rejected invalid encrypted vault data")]
    InvalidUpload,
    /// The request was cancelled before its commit point.
    #[error("server request was cancelled")]
    Cancelled,
    /// A fixed operation deadline elapsed.
    #[error("server request timed out")]
    TimedOut,
    /// The server or host queue has stopped.
    #[error("server has stopped")]
    Stopped,
    /// A local I/O, backup, save, or invariant failed.
    #[error("server operation failed")]
    Internal,
}

/// Canonical encrypted bytes and the SHA-256 version of those exact bytes.
pub struct CanonicalSnapshot {
    version: RemoteVersion,
    file: File,
    length: u64,
}

impl CanonicalSnapshot {
    /// Return the canonical byte version.
    #[must_use]
    pub const fn version(&self) -> RemoteVersion {
        self.version
    }

    /// Return the coherent encrypted snapshot length.
    #[must_use]
    pub const fn length(&self) -> u64 {
        self.length
    }

    /// Read the next bounded chunk from the open snapshot inode.
    pub fn read_chunk(&mut self, bytes: &mut [u8]) -> Result<usize, ServerError> {
        self.file.read(bytes).map_err(|_| ServerError::Internal)
    }
}

impl fmt::Debug for CanonicalSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CanonicalSnapshot([ENCRYPTED BYTES REDACTED])")
    }
}

/// Mode-0600 sibling staging file for one bounded encrypted upload.
pub struct StagedUpload {
    file: NamedTempFile,
    expected: Option<RemoteVersion>,
    total_length: u64,
    expected_digest: [u8; 32],
    hasher: Sha256,
    written: u64,
    chunks: u32,
    finalized: bool,
    faults: ServerFaultPlan,
}

impl StagedUpload {
    /// Create a sibling stage only after validating the declared size.
    pub fn new(
        vault_path: &Path,
        expected: Option<RemoteVersion>,
        total_length: u64,
        expected_digest: [u8; 32],
    ) -> Result<Self, ServerError> {
        Self::with_faults(
            vault_path,
            expected,
            total_length,
            expected_digest,
            ServerFaultPlan::default(),
        )
    }

    /// Create a stage with deterministic security-test fault boundaries.
    pub fn with_faults(
        vault_path: &Path,
        expected: Option<RemoteVersion>,
        total_length: u64,
        expected_digest: [u8; 32],
        faults: ServerFaultPlan,
    ) -> Result<Self, ServerError> {
        if total_length == 0 || total_length > MAX_ENCRYPTED_VAULT {
            return Err(ServerError::TooLarge);
        }
        faults.check(ServerFaultPoint::StageCreate)?;
        let parent = vault_path.parent().ok_or(ServerError::Internal)?;
        let file = TempFileBuilder::new()
            .prefix(".hidlins-upload-")
            .tempfile_in(parent)
            .map_err(|_| ServerError::Internal)?;
        Ok(Self {
            file,
            expected,
            total_length,
            expected_digest,
            hasher: Sha256::new(),
            written: 0,
            chunks: 0,
            finalized: false,
            faults,
        })
    }

    /// Append the next contiguous bounded encrypted chunk.
    pub fn write_chunk(&mut self, sequence: u32, bytes: &[u8]) -> Result<(), ServerError> {
        self.faults.check(ServerFaultPoint::StageWrite)?;
        if self.finalized || sequence != self.chunks || bytes.is_empty() {
            return Err(ServerError::InvalidUpload);
        }
        if bytes.len() > MAX_APPLICATION_CHUNK {
            return Err(ServerError::TooLarge);
        }
        let length = u64::try_from(bytes.len()).map_err(|_| ServerError::TooLarge)?;
        let next = self
            .written
            .checked_add(length)
            .ok_or(ServerError::TooLarge)?;
        if next > self.total_length || next > MAX_ENCRYPTED_VAULT {
            return Err(ServerError::TooLarge);
        }
        self.file
            .as_file_mut()
            .write_all(bytes)
            .map_err(|_| ServerError::Internal)?;
        self.hasher.update(bytes);
        self.written = next;
        self.chunks = self.chunks.checked_add(1).ok_or(ServerError::TooLarge)?;
        Ok(())
    }

    /// Verify exact count, length, digest, and durable staging bytes.
    pub fn finish(mut self, chunk_count: u32) -> Result<Self, ServerError> {
        self.faults.check(ServerFaultPoint::StageFinalize)?;
        if self.finalized || chunk_count != self.chunks || self.written != self.total_length {
            return Err(ServerError::InvalidUpload);
        }
        let digest: [u8; 32] = self.hasher.clone().finalize().into();
        if digest != self.expected_digest {
            return Err(ServerError::InvalidUpload);
        }
        self.file
            .as_file_mut()
            .sync_all()
            .map_err(|_| ServerError::Internal)?;
        self.finalized = true;
        Ok(self)
    }

    fn read_bytes(&self) -> Result<Vec<u8>, ServerError> {
        if !self.finalized {
            return Err(ServerError::InvalidUpload);
        }
        let mut file = File::open(self.file.path()).map_err(|_| ServerError::Internal)?;
        let capacity = usize::try_from(self.total_length).map_err(|_| ServerError::TooLarge)?;
        let mut bytes = Vec::with_capacity(capacity);
        file.read_to_end(&mut bytes)
            .map_err(|_| ServerError::Internal)?;
        if bytes.len() != capacity {
            return Err(ServerError::InvalidUpload);
        }
        Ok(bytes)
    }
}

impl fmt::Debug for StagedUpload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("StagedUpload([ENCRYPTED BYTES REDACTED])")
    }
}

/// The sole adapter allowed to read or mutate the process-owned unlocked vault.
pub struct AuthoritativeVault<'a> {
    vault: &'a mut Vault,
    master: &'a MasterPassword,
    keyfile: Option<&'a Keyfile>,
    faults: ServerFaultPlan,
}

impl<'a> AuthoritativeVault<'a> {
    /// Bind server operations to an already-open exclusively locked vault.
    #[must_use]
    pub fn new(
        vault: &'a mut Vault,
        master: &'a MasterPassword,
        keyfile: Option<&'a Keyfile>,
    ) -> Self {
        Self {
            vault,
            master,
            keyfile,
            faults: ServerFaultPlan::default(),
        }
    }

    /// Bind a deterministic fault plan for exhaustive crash-boundary tests.
    #[must_use]
    pub fn with_faults(
        vault: &'a mut Vault,
        master: &'a MasterPassword,
        keyfile: Option<&'a Keyfile>,
        faults: ServerFaultPlan,
    ) -> Self {
        Self {
            vault,
            master,
            keyfile,
            faults,
        }
    }

    /// Return the path only so the host can create a sibling encrypted stage.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.vault.path()
    }

    /// Hash the currently saved canonical encrypted KDBX bytes.
    pub fn head(&self) -> Result<RemoteVersion, ServerError> {
        let bytes = std::fs::read(self.vault.path()).map_err(|_| ServerError::Internal)?;
        Ok(version_of(&bytes))
    }

    /// Return coherent canonical bytes unless the caller already has them.
    pub fn fetch(
        &self,
        known: Option<RemoteVersion>,
    ) -> Result<Option<CanonicalSnapshot>, ServerError> {
        let mut file = File::open(self.vault.path()).map_err(|_| ServerError::Internal)?;
        let length = file.metadata().map_err(|_| ServerError::Internal)?.len();
        if length == 0 || length > MAX_ENCRYPTED_VAULT {
            return Err(ServerError::TooLarge);
        }
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
        loop {
            let read = file.read(&mut buffer).map_err(|_| ServerError::Internal)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|_| ServerError::Internal)?;
        let version = RemoteVersion::new(hasher.finalize().into());
        if known == Some(version) {
            return Ok(None);
        }
        Ok(Some(CanonicalSnapshot {
            version,
            file,
            length,
        }))
    }

    /// Validate and conditionally install one finalized staged KDBX object.
    pub fn commit(&mut self, upload: &StagedUpload) -> Result<RemoteVersion, ServerError> {
        let current = self.head()?;
        if upload.expected != Some(current) {
            return Err(ServerError::StaleVersion);
        }
        let bytes = upload.read_bytes()?;
        self.faults.check(ServerFaultPoint::Validation)?;
        let database = Vault::open_from_bytes(&bytes, self.master, self.keyfile)
            .map_err(|_| ServerError::InvalidUpload)?;
        if self.vault.database().root().id().uuid() != database.root().id().uuid()
            || self.vault.database().config.kdf_config != database.config.kdf_config
        {
            return Err(ServerError::InvalidUpload);
        }

        self.faults.check(ServerFaultPoint::Backup)?;
        backup::snapshot_pre_merge(self.vault.path()).map_err(|_| ServerError::Internal)?;
        self.faults.check(ServerFaultPoint::DatabaseReplacement)?;
        self.faults.check(ServerFaultPoint::Save)?;
        self.vault
            .replace_database_and_save(database)
            .map_err(|_| ServerError::Internal)?;
        self.head()
    }
}

/// One operation that a network worker may enqueue for the vault owner.
pub enum HostOperation {
    /// Read the canonical version.
    Head,
    /// Read canonical bytes only when different.
    Fetch {
        /// Client's previously known canonical version.
        known: Option<RemoteVersion>,
    },
    /// Create a bounded sibling staging file.
    BeginUpload {
        /// Conditional version required at commit.
        expected: Option<RemoteVersion>,
        /// Declared complete encrypted byte length.
        total_length: u64,
        /// Declared SHA-256 of streamed encrypted bytes.
        digest: [u8; 32],
    },
    /// Validate and commit a finalized stage.
    Commit(StagedUpload),
}

impl fmt::Debug for HostOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HostOperation([REDACTED])")
    }
}

/// Successful host response. Encrypted contents stay redacted in diagnostics.
pub enum HostResponse {
    /// Current canonical version.
    Head(RemoteVersion),
    /// Conditional canonical snapshot.
    Fetch(Option<CanonicalSnapshot>),
    /// Empty staging file returned to the authenticated worker.
    UploadStage(StagedUpload),
    /// Newly saved canonical version.
    Committed(RemoteVersion),
}

impl fmt::Debug for HostResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HostResponse([REDACTED])")
    }
}

struct HostRequest {
    operation: HostOperation,
    cancelled: Arc<AtomicBool>,
    authorization: Option<AuthorizationPermit>,
    response: SyncSender<Result<HostResponse, ServerError>>,
}

/// Revocable, linearizable authorization carried by network-originated work.
///
/// Revocation takes the write side of `commit_gate`; host execution holds the
/// read side through the operation's commit point. Once revocation returns, no
/// operation carrying this permit can newly reach persistent state.
#[derive(Clone)]
pub(crate) struct AuthorizationPermit {
    revoked: Arc<AtomicBool>,
    commit_gate: Arc<RwLock<()>>,
}

impl AuthorizationPermit {
    pub(crate) fn active() -> Self {
        Self {
            revoked: Arc::new(AtomicBool::new(false)),
            commit_gate: Arc::new(RwLock::new(())),
        }
    }

    pub(crate) fn is_authorized(&self) -> bool {
        !self.revoked.load(Ordering::Acquire)
    }

    pub(crate) fn revoke(&self) {
        let _gate = self
            .commit_gate
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.revoked.store(true, Ordering::Release);
    }

    fn execute(
        &self,
        operation: impl FnOnce() -> Result<HostResponse, ServerError>,
    ) -> Result<HostResponse, ServerError> {
        let _gate = self
            .commit_gate
            .read()
            .map_err(|_| ServerError::NotAuthorized)?;
        if !self.is_authorized() {
            return Err(ServerError::NotAuthorized);
        }
        operation()
    }
}

/// Bounded cloneable producer used by authenticated network workers.
#[derive(Clone)]
pub struct HostQueue {
    sender: SyncSender<HostRequest>,
    stopped: Arc<AtomicBool>,
}

impl HostQueue {
    /// Create the fixed-capacity queue and its single host-side processor.
    #[must_use]
    pub fn new() -> (Self, HostProcessor) {
        let (sender, receiver) = mpsc::sync_channel(MAX_PENDING_HOST_OPERATIONS);
        let stopped = Arc::new(AtomicBool::new(false));
        (
            Self {
                sender,
                stopped: Arc::clone(&stopped),
            },
            HostProcessor { receiver, stopped },
        )
    }

    /// Attempt to enqueue without allowing network workers to block the host.
    pub fn submit(&self, operation: HostOperation) -> Result<HostTicket, ServerError> {
        self.submit_inner(operation, None)
    }

    pub(crate) fn submit_authorized(
        &self,
        operation: HostOperation,
        authorization: &AuthorizationPermit,
    ) -> Result<HostTicket, ServerError> {
        if !authorization.is_authorized() {
            return Err(ServerError::NotAuthorized);
        }
        self.submit_inner(operation, Some(authorization.clone()))
    }

    fn submit_inner(
        &self,
        operation: HostOperation,
        authorization: Option<AuthorizationPermit>,
    ) -> Result<HostTicket, ServerError> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(ServerError::Stopped);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let (response, receiver) = mpsc::sync_channel(1);
        let request = HostRequest {
            operation,
            cancelled: Arc::clone(&cancelled),
            authorization,
            response,
        };
        match self.sender.try_send(request) {
            Ok(()) => Ok(HostTicket {
                receiver,
                cancelled,
            }),
            Err(TrySendError::Full(_)) => Err(ServerError::Busy),
            Err(TrySendError::Disconnected(_)) => Err(ServerError::Stopped),
        }
    }
}

/// Request handle supporting cancellation and bounded response waits.
pub struct HostTicket {
    receiver: Receiver<Result<HostResponse, ServerError>>,
    cancelled: Arc<AtomicBool>,
}

impl HostTicket {
    /// Request cancellation before the host commit point.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Wait only for the caller's remaining idle/session budget.
    pub fn wait_timeout(&self, timeout: Duration) -> Result<HostResponse, ServerError> {
        self.receiver
            .recv_timeout(timeout)
            .unwrap_or_else(|error| match error {
                mpsc::RecvTimeoutError::Timeout => Err(ServerError::TimedOut),
                mpsc::RecvTimeoutError::Disconnected => Err(ServerError::Stopped),
            })
    }
}

impl Drop for HostTicket {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

/// Single consumer driven only while the process owns the unlocked vault.
pub struct HostProcessor {
    receiver: Receiver<HostRequest>,
    stopped: Arc<AtomicBool>,
}

/// One dequeued authenticated host operation. Applications may move this
/// opaque value to a worker together with exclusive vault ownership so KDBX
/// validation and commits never block an event loop.
pub struct PendingHostOperation(HostRequest);

impl PendingHostOperation {
    /// Execute and answer the waiting network worker exactly once.
    pub fn execute(self, host: &mut AuthoritativeVault<'_>) -> Result<(), ServerError> {
        let request = self.0;
        let result = if request.cancelled.load(Ordering::Acquire) {
            Err(ServerError::Cancelled)
        } else if let Some(authorization) = request.authorization {
            authorization.execute(|| execute_operation(host, request.operation))
        } else {
            execute_operation(host, request.operation)
        };
        let _ = request.response.try_send(result);
        Ok(())
    }
}

impl HostProcessor {
    /// Dequeue at most one request without performing vault work.
    pub fn take_one(&mut self) -> Result<Option<PendingHostOperation>, ServerError> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(ServerError::Stopped);
        }
        match self.receiver.try_recv() {
            Ok(request) => Ok(Some(PendingHostOperation(request))),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(ServerError::Stopped),
        }
    }

    /// Process at most one queued operation. Returns whether work was found.
    pub fn process_one(&mut self, host: &mut AuthoritativeVault<'_>) -> Result<bool, ServerError> {
        let Some(request) = self.take_one()? else {
            return Ok(false);
        };
        request.execute(host)?;
        Ok(true)
    }

    /// Reject queued/future work during lock, stop, signal, or host death.
    pub fn shutdown(&mut self) {
        self.stopped.store(true, Ordering::Release);
        while let Ok(request) = self.receiver.try_recv() {
            let _ = request.response.try_send(Err(ServerError::Stopped));
        }
    }
}

impl Drop for HostProcessor {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn execute_operation(
    host: &mut AuthoritativeVault<'_>,
    operation: HostOperation,
) -> Result<HostResponse, ServerError> {
    match operation {
        HostOperation::Head => host.head().map(HostResponse::Head),
        HostOperation::Fetch { known } => host.fetch(known).map(HostResponse::Fetch),
        HostOperation::BeginUpload {
            expected,
            total_length,
            digest,
        } => StagedUpload::new(host.path(), expected, total_length, digest)
            .map(HostResponse::UploadStage),
        HostOperation::Commit(upload) => host.commit(&upload).map(HostResponse::Committed),
    }
}

fn version_of(bytes: &[u8]) -> RemoteVersion {
    RemoteVersion::new(Sha256::digest(bytes).into())
}

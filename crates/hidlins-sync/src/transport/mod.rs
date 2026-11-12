//! Transport-neutral synchronization contract and content-addressed values.

#[cfg(any(test, feature = "test-helpers"))]
pub mod memory;

/// Classify a transport error as a failed compare-and-swap precondition.
pub trait IsPreconditionFailed {
    /// Return true when the authority advanced since the supplied version.
    fn is_precondition_failed(&self) -> bool;
}

/// Opaque, transport-defined version identifier for an authoritative vault.
///
/// For local-network sync this is the canonical digest returned by the
/// authoritative server. The trait makes no assumption beyond:
///
/// - `==` defines version equality, and
/// - the transport is the sole interpreter of the inner bytes.
///
/// The orchestrator never inspects the inner string; it only compares
/// `SyncVersion` values against the bookmark it persists in
/// `vaults.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyncVersion(
    /// The transport-defined version string.
    pub String,
);

/// A snapshot of the authoritative encrypted vault and its exact version.
///
/// Returned by [`SyncTransport::fetch_if_changed`] when the remote has
/// advanced relative to the caller's `prev_version`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncSnapshot {
    /// The transport's version identifier for these bytes.
    pub version: SyncVersion,
    /// The raw remote bytes (an encrypted KDBX blob in the production path).
    pub bytes: Vec<u8>,
}

/// The transport-agnostic sync contract (FR-046; design.md §2.2.1).
///
/// **Implementor's promise:**
///
/// - [`head`](Self::head) is idempotent within a single instance and is
///   the cheapest way to learn the remote's current version.
/// - [`fetch_if_changed`](Self::fetch_if_changed) given `Some(prev)` MUST use
///   the protocol's version-aware fetch so that, when `prev` matches the
///   authority's current version, the method returns `Ok(None)` *without*
///   transferring the vault. Given `None`, it MUST return the current snapshot
///   unconditionally—used by the orchestrator's merge-retry loop after a
///   failed precondition to fold in the new authoritative state.
/// - [`commit_conditional`](Self::commit_conditional) given `Some(prev)` MUST
///   be atomic compare-and-swap: succeed only if the remote's current
///   version equals `prev`; otherwise return an error for which
///   [`IsPreconditionFailed::is_precondition_failed`] returns `true`.
///   Given `None`, the commit seeds an authority that has no vault yet.
/// - All methods are blocking. The caller manages threading.
///
/// **Caller's promise:** single-threaded use of any given transport
/// instance. Two threads wishing to sync the same configured target must
/// either coordinate above this trait or hold separate instances.
///
/// The trait is deliberately *not* object-safe (the associated [`Error`](Self::Error)
/// type with its `IsPreconditionFailed` bound would force `Box<dyn …>`
/// gymnastics that aren't worth it). The orchestrator is generic over
/// `T: SyncTransport`; each impl picks its own error.
pub trait SyncTransport {
    /// Transport-specific error type. Mapped into [`crate::SyncError`] by
    /// the orchestrator via an `Into` bound. The
    /// [`IsPreconditionFailed`] bound lets the orchestrator branch on
    /// the retry-the-merge case without naming the concrete type.
    type Error: std::error::Error + Send + Sync + IsPreconditionFailed + 'static;

    /// Read the configured authority's current version: returns its current version, or
    /// `Ok(None)` when the authority has no vault yet.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] on network, auth, or protocol failure
    ///.
    fn head(&mut self) -> Result<Option<SyncVersion>, Self::Error>;

    /// Fetch the authoritative vault iff its version differs from `prev_version`.
    ///
    /// When `prev_version` is `Some(v)` and the remote's current version
    /// equals `v`, returns `Ok(None)` — implementations MUST use a
    /// the protocol's version-aware fetch so no vault bytes are transferred.
    ///
    /// When `prev_version` is `None`, returns the current snapshot
    /// unconditionally.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] on network, auth, or protocol failure.
    fn fetch_if_changed(
        &mut self,
        prev_version: Option<&SyncVersion>,
    ) -> Result<Option<SyncSnapshot>, Self::Error>;

    /// Commit `bytes` to the configured authority.
    ///
    /// When `if_match` is `Some(v)`, succeeds only if the remote's current
    /// version equals `v`; otherwise returns an error for which
    /// [`IsPreconditionFailed::is_precondition_failed`] returns `true`.
    ///
    /// When `if_match` is `None`, the commit seeds an authority that has no
    /// vault yet.
    ///
    /// Returns the new [`SyncVersion`] reported by the server.
    ///
    /// # Errors
    ///
    /// Returns [`Self::Error`] on network, auth, or protocol failure,
    /// including a precondition-failed variant
    /// (`is_precondition_failed() == true`) when `if_match` was supplied
    /// and the remote did not match.
    fn commit_conditional(
        &mut self,
        bytes: &[u8],
        if_match: Option<&SyncVersion>,
    ) -> Result<SyncVersion, Self::Error>;
}

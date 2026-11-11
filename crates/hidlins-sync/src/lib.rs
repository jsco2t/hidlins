//! Local-network synchronization for encrypted Hidlins vaults.
//!
//! The crate owns local-address policy, authenticated sessions, device
//! identity and trust, secure discovery, the authoritative server, and the
//! transport-neutral no-data-loss merge state machine.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod address;
pub mod backup;
pub mod client;
pub mod config;
pub mod discovery;
mod encoding;
pub mod error;
pub mod framing;
pub mod identity;
pub mod merge;
pub mod noise;
pub mod pairing;
pub mod protocol;
pub mod sealed;
pub mod server;
pub mod sync;
pub mod sync_log;
pub mod transport;
pub mod trust;

pub use error::SyncError;
pub use merge::{reconcile, EntryDelta, MergeError, MergeSummary};
pub use sync::{SyncOptions, SyncOutcome};
pub use sync_log::format as format_sync_log;
pub use transport::{IsPreconditionFailed, SyncSnapshot, SyncTransport, SyncVersion};

#[cfg(any(test, feature = "test-helpers"))]
pub use transport::memory::{FaultBoundary, MemoryTransport, MemoryTransportError};

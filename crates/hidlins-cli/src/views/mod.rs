//! Per-subcommand serializable view structs — JSON schema contract.
//!
//! The shipped modules are:
//!
//! - [`vault`]: `VaultCreateView`, `VaultListView`, `VaultOpenView`, `VaultSetLockView`, `VaultSetSyncView`.
//! - [`entry`]: `EntryGetView`, `EntryListView`, `EntrySearchView`, `EntryAddView`, `EntryEditView`, `EntryRmView`.
//! - [`gen`]: `PasswordGenView`, `PassphraseGenView`.
//! - [`sync`]: the `hidlins sync` outcome view `SyncView`.

pub mod entry;
pub mod gen;
pub mod sync;
pub mod vault;

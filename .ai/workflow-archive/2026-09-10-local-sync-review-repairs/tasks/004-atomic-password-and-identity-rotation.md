# Task 004: Atomic Password and Identity Rotation

Delegation: main-only

## Goal

Make master-password changes and sync-identity rewrapping one crash-recoverable transaction, with process-level evidence across every durable commit boundary.

## Context

The KDBX replacement and registry identity rewrap are individually atomic but sequential. A crash after the new KDBX is committed and before the registry is rewritten leaves the new master password unable to unwrap the old registry identity. Existing fault injection covers failure before KDBX rename, not this cross-file boundary.

## Scope

### In scope

- Coordinated staging, durable marker, commit, cleanup, and recovery for KDBX password plus registry identity rewrap.
- Existing-password, new-password, sync identity, key-file, unrelated registry-entry, and retry/idempotence invariants.
- Process-level fault injection at each multi-file transaction boundary.
- Active atomic-write and coverage documentation directly affected.

### Out of scope

- Changing KDBX format, KDF policy, or sync identity keys.
- A general-purpose filesystem transaction framework.
- Password recovery or escrow.

## Implementation requirements

- Before repair, add a process-level regression that terminates after KDBX replacement and before registry replacement, then fails because the reopened vault/registry pair is inconsistent. Record red and identical green commands in `evidence/004.md`.
- Stage encrypted KDBX and registry outputs as sibling files and use a durable, non-secret transaction marker with explicit phases sufficient for deterministic restart recovery.
- Validate internally generated/fixed paths; do not trust arbitrary filesystem paths from marker contents.
- Preserve advisory locking and prevent recovery from overwriting unrelated concurrent registry changes. Acquire locks in a documented stable order.
- Fsync files and containing directories where required by the repository's atomic-write durability contract.
- Recovery must be idempotent after interruption at every marker/stage/rename/cleanup boundary and must leave either the old consistent pair or the new consistent pair.
- No plaintext vault or secret identity material may be written to the marker, logs, or error text.
- Cover the supported key-file path and prove the sync public identity is unchanged after successful rewrap.
- Inspect adjacent cases: wrong old password, stage-write failure, each rename boundary, repeated recovery, missing/corrupt transaction artifacts, and unrelated registry records. Add distinct high-value tests only.

## Acceptance criteria

- [ ] The cross-file crash regression fails on the baseline and passes after repair.
- [ ] Process-level fault injection covers every durable transaction boundary and recovery always yields an openable vault with a matching unwrap-able sync identity.
- [ ] Recovery is idempotent and fail-closed for missing, corrupt, or inconsistent artifacts without truncating the live vault.
- [ ] Successful rotation preserves the sync public key, KDBX interoperability, key-file behavior, and unrelated registry entries.
- [ ] Transaction artifacts and errors contain no plaintext secrets; cleanup leaves no stale staged files after successful recovery.
- [ ] Adjacent durability coverage decisions and corrected atomicity claims are recorded in task evidence.

## Validation

- `cargo test -p hidlins-core --offline --locked --test us_006_change_master_password -- --test-threads=1`
- `cargo test -p hidlins-api --offline --locked --test us_095_bootstrap_change_password -- --test-threads=1`
- `cargo test -p hidlins-sync --offline --locked --test identity_pairing_trust -- --test-threads=1`

## Dependencies

- Task 003

## Expected areas of change

- `crates/hidlins-core/src/vault.rs`
- `crates/hidlins-core/src/registry.rs`
- `crates/hidlins-core/tests/us_006_change_master_password.rs`
- `crates/hidlins-core/tests/bin/`
- `crates/hidlins-api/src/api/vaults.rs`
- `crates/hidlins-api/tests/us_095_bootstrap_change_password.rs`
- Sync identity/config integration under `crates/hidlins-sync/src/`
- Active durability/security documentation

## Risks / notes

Rename ordering alone is not a transaction. The marker and recovery protocol must be designed before implementation and must account for crash durability, lock ordering, rollback/roll-forward choice, and old/new credential availability without ever persisting credentials.


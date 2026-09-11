# Task 006: Implement the Authoritative Vault Server

Delegation: main-only

## Goal

Deliver the bounded authenticated TCP server, host-owned vault-operation queue, serialized version/fetch/CAS behavior, and crash-safe canonical KDBX commit path.

## Context

An unlocked `Vault` holds an exclusive file lock and is owned by the host session. Network workers therefore cannot open or mutate it independently. This task establishes the concurrency/data-integrity boundary shared by CLI, TUI, and desktop hosts.

## Scope

### In scope

- Allowed-interface listener binding, accepted-peer revalidation, connection/time/resource caps, shutdown, and discovery advertisement lifecycle.
- XX pairing admission only while pairing mode is open and IK authorization for trusted clients.
- Bounded network worker and host-operation queues with request/response cancellation.
- Host operations for version/head, conditional fetch, streamed conditional upload, validation, commit, and generic pre-auth/authenticated errors.
- SHA-256 canonical encrypted-KDBX versions and serialized compare-and-swap.
- Upload staging with mode 0600, size/digest checks, KDBX/password/keyfile/root-identity/KDF validation, `.kdbx.bak`, live database replacement, atomic save, and cleanup.
- A reusable server runtime/controller API and deterministic/real-loopback integration harness.
- `test-local-sync-integration` Makefile target.

### Out of scope

- CLI/TUI/Flutter controls, client merge orchestration, pair-and-import, or native mobile discovery.
- Background daemon/service operation.

## Implementation requirements

- Network threads receive only authenticated identity and encrypted KDBX bytes; they never borrow or concurrently open the host's mutable vault.
- The host adapter must follow the existing single-owner claim/run/commit and lock-pending invariants and remain responsive/cancellable during KDF validation.
- Only one commit per vault may be active; stale expected versions return a transport-neutral precondition error without modifying disk or live state.
- Recompute the returned version from the actual canonical bytes after save because KDBX save re-encrypts.
- Reject unauthenticated/provisional/revoked peers before revealing version/existence/size.
- Fault inject every stage-file, validation, backup, database replacement, save, response, disconnect, cancellation, and shutdown boundary.

## Acceptance criteria

- [ ] Allowed paired clients can head/fetch/commit through real loopback Noise sessions; unauthenticated, wrong-key, provisional, and revoked clients learn no vault metadata.
- [ ] Concurrent clients and local host work are serialized with no lost update; stale commits return precondition failure and retain old canonical/live state.
- [ ] Invalid size/digest/KDBX/password/keyfile/root UUID/KDF input never replaces the vault and leaves no staging plaintext or partial file.
- [ ] Every injected crash/disconnect/cancel boundary leaves either the complete old or complete new KDBX plus required backup, never corruption.
- [ ] Stop, lock, `Ctrl+C` signal request, or host death closes listeners/advertisements, resolves queued work, and drops session/identity secrets.
- [ ] Connection, queue, frame, idle, handshake, and total-session caps release resources under exhaustion.

## Validation

- `cargo test -p hidlins-sync --offline --locked server`
- `make test-local-sync-integration`
- `make test-local-sync-security`

## Dependencies

- Task 005

## Expected areas of change

- `crates/hidlins-sync/src/server/`
- `crates/hidlins-sync/src/transport/`
- `crates/hidlins-sync/src/backup.rs`
- `crates/hidlins-sync/tests/`
- `tools/local-sync-tests/`
- `Makefile`

## Risks / notes

Validation can be CPU-heavy because KDBX uses Argon2id. Expensive work must occur outside UI/session mutexes while the vault remains under exclusive logical ownership, mirroring the current sync worker pattern.

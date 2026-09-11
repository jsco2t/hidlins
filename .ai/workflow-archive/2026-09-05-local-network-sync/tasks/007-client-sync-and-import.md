# Task 007: Implement LAN Client Sync, Generic Orchestration, and Pair-and-Import

Delegation: main-only

## Goal

Connect discovery and pinned IK sessions to the existing no-data-loss sync engine, make its concepts transport-neutral, and deliver safe first-client import plus startup scheduling policy.

## Context

The server now exposes a single encrypted object with compare-and-swap. The current sync truth table can be retained after removing ETag/S3 assumptions and adapting the production transport.

## Scope

### In scope

- Production `LanTransport` over allowed discovery/manual candidates and pinned IK.
- Candidate ordering/deduplication, total attempt budgets, actual connected-peer revalidation, key mismatch handling, cancellation, and generic errors.
- Rename public orchestration types/fields/errors from object/ETag/S3 terms to local transport-neutral versions while preserving the memory transport.
- Change the merge CAS policy to one retry after the initial failed commit.
- V1 local-sync configure/status operations for server/client roles.
- Existing-vault client pairing and clean-device pair-and-import through normal KDBX validation/atomic registration paths.
- Process-local “first configured unlock only” startup tracker and explicit/manual scheduling primitives.
- Nonfatal offline/auth/discovery/permission startup results and secret-free status events.

### Out of scope

- Application API/CLI/TUI/Flutter presentation wiring.
- Removing still-referenced S3 implementation files before surfaces move.

## Implementation requirements

- A discovered endpoint is usable only after core allowlist checks, socket peer recheck, and successful pinned IK; key failure never falls back to XX.
- Import must collect master password/keyfile through the caller's secure mechanism, validate fetched encrypted bytes before install, write atomically, register atomically, and roll back every partial artifact on failure.
- Import may continue the successfully committed XX channel or reconnect with IK, but cannot fetch while trust is provisional.
- Sync failure returns the local vault/session intact. Fast replacement/merge updates the in-memory and on-disk vault consistently.
- No post-save scheduler exists in the core. Startup tracking is per process and vault, and an attempted failure still counts as the one automatic attempt.
- Preserve all existing merge/property/interop invariants and record red/green evidence for naming/policy changes.

## Acceptance criteria

- [ ] Real client/server tests pass for already-current, first push, fast replacement, disjoint merge, collision history, one CAS retry, and retry exhaustion.
- [ ] DHCP candidate change succeeds with the same pinned identity; spoofed key, public endpoint, DNS name, or silent XX fallback fails.
- [ ] Pair-and-import creates only a complete validated registered KDBX; every interruption leaves no registration, partial vault, or plaintext residue.
- [ ] Startup tracker attempts once after first configured unlock and never after mutation/re-unlock in the same process; manual sync remains repeatable.
- [ ] Network failure is nonfatal to offline CRUD and produces stable secret-free status/error categories.
- [ ] Public reusable sync fields and errors contain no S3/ETag semantic names.

## Validation

- `cargo test -p hidlins-sync --offline --locked client`
- `cargo test -p hidlins-sync --offline --locked --test merge_property_tests --test merge_semantics --test fault_injection`
- `make test-local-sync-integration`
- `make test-local-sync-security`
- `make interop-sync`

## Dependencies

- Task 006

## Expected areas of change

- `crates/hidlins-sync/src/sync.rs`
- `crates/hidlins-sync/src/config.rs`
- `crates/hidlins-sync/src/client/`
- `crates/hidlins-sync/src/transport/`
- `crates/hidlins-sync/src/import.rs`
- `crates/hidlins-sync/tests/`
- `tools/interop-tests/`

## Risks / notes

S3 may remain temporarily compiled solely for old callers until Tasks 008–011 switch them. That is an intermediate buildability measure, not migration or final dual-transport support; Task 012 deletes it completely.

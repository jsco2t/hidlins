# Task 002: TUI Pairing Cancellation

Delegation: main-only

## Goal

Make TUI pairing an owned, cancellable operation so locking, cancelling, quitting, or replacing the operation cannot leave secret-bearing work running or allow late persistent mutation.

## Context

The TUI currently detaches pairing work and drops its visible state on lock. The worker can continue through networking and persistence after the vault is locked, and its closure can retain the master password until it eventually exits. Existing TUI tests do not block a worker at a deterministic transition and then exercise lifecycle cancellation.

## Scope

### In scope

- Ownership and cancellation of TUI pairing work.
- Cooperative cancellation through any pairing/network layer needed for prompt termination.
- Lock, explicit cancel, quit, and operation-replacement behavior.
- User-visible cancellation state only where required to keep current feedback accurate.
- Deterministic lifecycle regression tests and adjacent transition audit.

### Out of scope

- Pairing durability/protocol semantics beyond Task 001's completed interfaces.
- Broad TUI navigation or visual redesign.
- Cancelling unrelated sync-server ownership already covered by existing lifecycle tests unless the audit finds the same concrete defect.

## Implementation requirements

- Before product changes, add a deterministic regression using a production-facing pairing seam that blocks an in-flight operation, locks the TUI, and fails because the worker can still complete or mutate persistence.
- Record the red result in `evidence/002.md`, then record the same command green after repair.
- Lock, cancel, quit, and replacement must signal cancellation and join the operation before lifecycle cleanup returns. Blocking network work must observe cancellation within a documented bound.
- A cancelled worker must not persist trust, import a vault, register a vault, or deliver a late success/error into a later UI generation.
- Secret-bearing inputs captured by the worker must be dropped when cancellation completes; test with an observable drop/lifecycle seam rather than attempting to inspect zeroized bytes.
- Avoid timing-only tests. Use barriers/channels or an equivalent deterministic scheduler to prove the ordering.
- Inspect adjacent states before network start, during handshake, after remote activation but before local finalization, and after completion; add only distinct high-value cases.

## Acceptance criteria

- [ ] The baseline regression deterministically demonstrates late work or mutation after TUI lock and passes after the repair.
- [ ] Lock, explicit cancel, quit, and operation replacement all cancel and join pairing work within the defined bound.
- [ ] No cancelled operation can persist/import/register or mutate a newer operation's UI state.
- [ ] Captured master-password/key material is released when cancellation completes.
- [ ] Existing successful pairing and server lifecycle behavior remains covered and passing.
- [ ] Adjacent cancellation coverage decisions and corrected test claims are recorded in task evidence.

## Validation

- `cargo test -p hidlins-tui --offline --locked --lib pairing -- --test-threads=1`
- `cargo test -p hidlins-tui --offline --locked --test local_sync_surface -- --test-threads=1`

## Dependencies

- Task 001

## Expected areas of change

- `crates/hidlins-tui/src/pairing_runtime.rs`
- `crates/hidlins-tui/src/overlay/local_sync.rs`
- `crates/hidlins-tui/src/`
- `crates/hidlins-tui/tests/local_sync_surface.rs`
- Pairing/network cancellation seams under `crates/hidlins-sync/src/` if required

## Risks / notes

Joining from the UI thread must not hang on an uninterruptible socket deadline. Cancellation ownership must also prevent stale result delivery without creating a second detached cleanup thread.


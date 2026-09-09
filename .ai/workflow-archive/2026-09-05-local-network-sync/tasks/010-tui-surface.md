# Task 010: Replace the TUI Sync Surface

Delegation: main-only

## Goal

Deliver keyboard-complete local discovery, pairing/import, manual sync, status, peer management, and explicit in-process server operation in the reference TUI.

## Context

The TUI directly owns the unlocked vault and already moves it into a background sync worker. It also has S3 forms and optional unlock/lock-quit sync preferences that conflict with the approved startup/manual-only model.

## Scope

### In scope

- Replace S3 configuration overlay/settings with discovery, existing-vault pairing, pair-and-import entry point, status, manual fallback, peers, revocation, SAS confirmation, and actionable errors.
- Replace `sync_on_unlock`/`sync_on_lock_quit` preferences with fixed once-per-process startup behavior and an in-memory explicit server toggle.
- Add a separate “allow pairing for three minutes” action and visible bounded countdown/state.
- Extend background ownership/runtime to process server vault operations and client sync without blocking rendering/input.
- Stop listeners/advertisements and zeroize live secrets on lock/quit/server-off; restart after re-unlock only if the in-process toggle remains enabled.
- Keyboard registry, palette, hints, settings rows, snapshots, deterministic journeys, and accessibility semantics.

### Out of scope

- Flutter/native mobile UI or persistent service/daemon behavior.
- Final core S3 deletion.

## Implementation requirements

- Server toggle defaults off on every process start and is not persisted to disk; it may remain set only across lock/re-unlock in the same process.
- Startup client sync is automatic once for each configured vault after successful unlock and is never user-disableable or repeated on mutation/lock/quit.
- All mouse-reachable actions map to registered keyboard commands and pairing/error overlays are non-bypassable and accessible.
- The event loop remains responsive during discovery, handshake, KDF validation, transfer, and server commit; deferred lock wins safely.
- SAS/key/error/status rendering is secret-safe and clears on dismissal/lock.
- Record red-before/green-after journeys for each lifecycle and negative path.

## Acceptance criteria

- [ ] Discovery/pair/import/manual-sync/status/peer/server operations are fully usable from the keyboard and covered by deterministic journeys.
- [ ] First unlock attempts one client sync; later unlock/mutation/lock/quit does not, and offline failure preserves the ready workspace.
- [ ] Server enable/disable, pairing-window expiry/failure, lock/re-unlock, incoming commit, quit, and deferred-lock ownership transitions are tested without dual vault ownership.
- [ ] All S3 credential/configuration text and old auto-sync toggles are absent from active TUI state and snapshots.
- [ ] Accessibility contracts describe SAS, countdown, server state, errors, and confirmation actions without exposing sensitive material.

## Validation

- `make test-tui-contracts`
- `make snapshots-check`
- `cargo test -p hidlins-tui --offline --locked`
- `make test-local-sync-integration`

## Dependencies

- Task 009

## Expected areas of change

- `crates/hidlins-tui/src/app.rs`
- `crates/hidlins-tui/src/sync_runtime.rs`
- `crates/hidlins-tui/src/session.rs`
- `crates/hidlins-tui/src/overlay/`
- `crates/hidlins-tui/src/screens/`
- `crates/hidlins-tui/src/command/`
- `crates/hidlins-tui/src/user_config.rs`
- `crates/hidlins-tui/tests/snapshots/`

## Risks / notes

Incoming server work adds another temporary vault owner. Extend the existing ownership enum/state machine rather than adding booleans or independent handles that can race with client sync or lock.

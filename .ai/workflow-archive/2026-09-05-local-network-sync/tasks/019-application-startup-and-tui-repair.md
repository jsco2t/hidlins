# Task 019: Repair Startup Discovery Orchestration and TUI Server Binding

Delegation: main-only

## Goal

Make the default Flutter startup experience perform discover-then-sync after restart and make explicit TUI server mode start a real, safely allocated listener.

## Context

The Rust API currently launches startup sync immediately after unlock using only existing candidate cache state. The mobile acceptance scenario injects a literal endpoint before unlock, so it does not exercise default startup discovery. Separately, the TUI enumerates interfaces with port zero through `LocalEndpoint`, which rejects zero.

## Scope

### In scope

- Explicit cross-platform startup discover-then-sync orchestration across Flutter desktop and native mobile discovery ports.
- Once-per-process, cancellation, foreground-mobile, and nonfatal-offline behavior.
- Removal of pre-unlock endpoint injection from the acceptance startup path.
- A checked bind-interface/ephemeral-port boundary distinct from connectable `LocalEndpoint`.
- Real TUI listener startup/shutdown regression tests.

### Out of scope

- Background/persistent mobile services.
- Automatic sync after mutation, save, lock, or quit.
- New UI flows unrelated to startup/server status.

## Implementation requirements

- Unlock must remain responsive and expose the local vault even when discovery or startup sync fails.
- On mobile, native discovery executes only while the app is foreground/resumed and is canceled on pause/lock/shutdown.
- The startup tracker is consumed exactly once per configured vault/process without duplicate sync from controller rebuilds.
- Desktop can use the Rust discovery adapter; mobile must use the existing Dart/native platform channel and return candidates through Rust validation.
- Ephemeral port zero is permitted only as a bind request. Every advertised, returned, stored, or connected endpoint remains a validated nonzero `LocalEndpoint`.

## Acceptance criteria

- [ ] A fresh Flutter process with a paired vault and empty injected/cache state discovers before its first startup sync and reaches the pinned authority.
- [ ] Startup discovery/sync failure emits a secret-free nonfatal result and local vault use continues.
- [ ] Lifecycle pause/lock cancels mobile discovery/sync and no mobile listener or background service is introduced.
- [ ] Simulator scenarios no longer pre-inject a literal endpoint before the startup-unlock assertion.
- [ ] TUI explicit server mode starts a real listener on an allowed interface with a nonzero port and shuts it down on toggle, lock, and process exit.
- [ ] Startup remains once-per-process and no post-mutation automatic sync is added.

## Validation

- `cargo test -p hidlins-api --offline --locked --test us_094_sync_api -- --test-threads=1`
- `cargo test -p hidlins-tui --offline --locked --test local_sync_surface`
- `cargo test -p hidlins-tui --offline --locked --lib -- server`
- `make app-test`
- `make test-local-sync-integration`

## Dependencies

Task 018

## Expected areas of change

- `crates/hidlins-api/src/api/session.rs`
- `crates/hidlins-api/src/api/sync.rs`
- Rust bridge DTO/generated bindings when the API contract changes
- `app/lib/src/data/bridge_repositories.dart`
- `app/lib/src/features/sync/sync_controller.dart`
- `app/integration_test/mobile_local_sync_scenario_test.dart`
- `crates/hidlins-sync/src/address.rs`
- `crates/hidlins-sync/src/discovery/desktop.rs`
- `crates/hidlins-tui/src/server_runtime.rs`
- API, Flutter, TUI, and scenario tests

## Risks / notes

Do not let framework provider reconstruction schedule duplicate startup work. The bind-address repair must preserve the nonzero invariant relied on by every network destination and serialized endpoint.

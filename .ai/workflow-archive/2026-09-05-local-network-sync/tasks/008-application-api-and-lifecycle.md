# Task 008: Replace the Application API and Lifecycle Integration

Delegation: main-only

## Goal

Expose the complete local-sync capability through the Rust application session/FFI boundary and enforce startup, server, locking, pairing, and mobile capability rules without UI-owned security logic.

## Context

Flutter consumes `AppSession` through generated bindings. The current API is S3-shaped, auto-syncs after mutations, and already has a robust claim/run/commit worker and lock-pending model that must be extended rather than bypassed.

## Scope

### In scope

- Replace S3 DTOs/configure/bootstrap methods with typed discovery, endpoint, pair/import, status, sync, peer, pairing-event, and server-control APIs.
- Extend session state and ports for background startup/manual sync, pairing confirmation, server host operations, events, cancellation, and shutdown.
- Remove every `maybe_sync_after_save` call and the helper itself.
- Trigger exactly one nonblocking automatic sync after first configured unlock per vault/process.
- Integrate server stop/secret destruction with manual lock, idle/OS lock, lifecycle shutdown, panic containment, and master-password identity rewrap.
- Enforce mobile server unavailability before any listener/advertisement creation.
- Regenerate and review Flutter Rust bridge artifacts and boundary manifests.

### Out of scope

- Dart widgets/controllers, CLI, TUI, or native Swift/Android discovery mechanisms.
- Final removal of S3 files still used by other surfaces.

## Implementation requirements

- All bridged `AppSession` receivers remain `&self`; do not introduce FRB write guards or hold the session mutex across network/KDF work.
- Pairing SAS and confirmation use opaque transaction handles; Dart never receives key material or handshake hashes.
- Candidate inputs from mobile are bounded and revalidated in Rust.
- Server requests reuse the claim/run/commit ownership protocol; lock-pending/dead/panic paths discard unsafe state, close server runtime, and emit exactly one terminal event.
- Event/debug/error DTOs redact sensitive material and distinguish permission denied/restricted, not found, auth/key mismatch, revoked, conflict, busy, canceled, and offline cases.
- Capture red-before/green-after API lifecycle and no-post-save tests.

## Acceptance criteria

- [ ] Generated API exposes all required local client operations and desktop server/peer operations with no S3 DTO or credential field.
- [ ] Entry mutations save locally without invoking sync; first configured unlock schedules exactly one sync and leaves local use available on failure.
- [ ] Desktop server operations serialize safely with CRUD/sync and stop on every lock/shutdown/panic path.
- [ ] Mobile-target tests prove server start is rejected before bind/advertise and that only bounded candidate injection/client operations are usable.
- [ ] Password change rewraps local identity without changing its public key and failures do not leave mismatched session/registry credentials.
- [ ] Bridge generation/boundary checks show no UI-layer cryptography, trust, range, merge, or vault-storage policy.

## Validation

- `cargo test -p hidlins-api --offline --locked --test us_091_session_lifecycle --test us_094_sync_api --test us_095_bootstrap_change_password`
- `make app-test-bridge`
- `make api-gen-check`
- `make boundary-check`
- `make check-feature-gates`

## Dependencies

- Task 007

## Expected areas of change

- `crates/hidlins-api/src/api/`
- `crates/hidlins-api/src/dto.rs`
- `crates/hidlins-api/src/sync_port.rs`
- `crates/hidlins-api/src/error.rs`
- `crates/hidlins-api/src/frb_generated.rs`
- `crates/hidlins-api/tests/`
- `app/lib/src/bridge/`
- `tools/dev/frb-api-manifest.txt`

## Risks / notes

Server operations, client sync, and lock requests compete for the one vault owner. State enums/claims must make illegal simultaneous ownership unrepresentable and preserve the current “lock wins at commit” rule.

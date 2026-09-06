# Task 008: Stabilize the mobile FFI and platform-service boundary

Delegation: main-only

## Goal

Provide one generated bridge API and small testable Dart/native service contracts
for mobile lifecycle, protected clipboard, paths, vault import, and keyfiles.

## Context

Bindings generated on desktop cannot contain APIs removed by mobile-only Rust
`cfg` gates. Mobile platform behavior also needs explicit seams so widget tests do
not require devices and Dart does not absorb core business logic.

## Scope

### In scope

- Make the one-shot secret/clipboard API stable across all generated bindings,
  returning a typed unsupported-platform result where inapplicable.
- Add narrow platform-service interfaces for lifecycle events, secure clipboard,
  application-support paths, file import, and keyfile references.
- Add production dispatch stubs only where a later platform task owns the native
  implementation; all approved methods and failure modes must be complete and
  testable, not silent no-ops.
- Regenerate bindings and enforce the boundary/feature-gate contract.
- Replace the current blanket `MethodChannel` rejection with a fail-closed manifest
  that permits only fixed capability adapters under `app/lib/src/platform/`, fixed
  channel names, and fixed method sets; keep all other native bypasses prohibited.

### Out of scope

- iOS or Android native implementations (Tasks 009 and 012).
- Vault parsing, sync, merge, crypto, or secret persistence in Dart/native UI code.
- A second platform-specific generated binding set.

## Implementation requirements

- Unsupported operations return an explicit typed error; they must never pretend
  success.
- Keep interfaces capability-specific; do not turn `AppSession` or one Dart
  platform object into a service locator for unrelated native behaviors.
- Rust owns lock policy and session state. Swift/Kotlin own snapshot shielding,
  OS clipboard mechanics, pickers, and sandbox path acquisition. Dart translates
  typed results and renders state but owns neither policy.
- Clipboard transfer remains one-shot and zeroizing, with no plaintext return to
  Dart on mobile.
- Service fakes must model success, cancellation, denial, stale access, lifecycle
  races, and platform errors.
- Prefer small in-repository method-channel code over a dependency unless the
  dependency closes a substantial, audited gap.
- Preserve existing FRB code-generation determinism and checked API manifests.
- Plant and remove negative cases proving an undeclared channel, a declared channel
  outside the platform directory, and an unlisted method all fail `boundary-check`.

## Acceptance criteria

- [ ] One committed generated API compiles for desktop, iOS, and Android and has
  typed unsupported behavior for inapplicable methods.
- [ ] Mobile clipboard calls transfer secrets without returning plaintext to Dart
  and have zeroization/error tests.
- [ ] Lifecycle, path, import, keyfile, and clipboard interfaces have deterministic
  fake-based tests covering all documented result states.
- [ ] Boundary/feature checks reject accidental core FFI expansion or duplicate
  platform business logic.
- [ ] The platform-channel manifest admits only lifecycle, clipboard, path, import,
  and keyfile capability adapters; planted undeclared channel/method/location
  violations fail the gate.
- [ ] Tests prove each capability has exactly one owner and that unsupported or
  unavailable native implementations fail explicitly rather than no-op.

## Validation

- `make check-feature-gates`
- `make boundary-check`
- `make api-gen-check`
- `make app-analyze`
- `make app-test`

## Dependencies

- Task 007

## Expected areas of change

- `crates/hidlins-api/` and generated FRB bindings
- `app/lib/platform/` or equivalent service boundary
- Boundary manifests, feature checks, and focused tests

## Risks / notes

A stable cross-platform API weakens the original compile-time exclusion claim.
Compensate with typed errors, narrow call sites, boundary checks, and device tests;
do not expose a general secret-return API.

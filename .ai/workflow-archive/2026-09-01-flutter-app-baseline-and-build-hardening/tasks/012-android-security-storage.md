# Task 012: Implement Android security, lifecycle, and storage

Delegation: main-only

## Goal

Implement Android-native lifecycle locking, screen protection, owner-checked
clipboard clearing, application storage, vault import, and keyfile access without
moving business logic outside Rust.

## Context

The production artifact from Task 011 can load Rust but does not yet satisfy the
mobile security and storage requirements from FR-050–054 and the original design.

## Scope

### In scope

- Initialize JNI/TLS safely from the Android application lifecycle.
- Report activity/process lifecycle transitions to Rust, apply `FLAG_SECURE`, and
  display a non-secret cover before background snapshots can occur.
- Transfer clipboard secrets through the native one-shot API and clear only if
  Hidlins still owns the clipboard value it wrote.
- Use app-owned state storage, SAF import with atomic copy, and persisted non-secret
  keyfile document permissions with revoked/stale handling.
- Configure backup exclusion and tests for process/activity recreation, denial,
  cancellation, and lifecycle races.

### Out of scope

- Final Android responsive UX, TalkBack, sync, and emulator acceptance (Task 013).
- Background sync, biometrics, secret backup, or broad native business logic.

## Implementation requirements

- Rust remains the sole lock-policy owner; Kotlin reports lifecycle events only.
- Clipboard comparison/clear must avoid logging or retaining extra plaintext and
  tolerate third-party clipboard replacement.
- `FLAG_SECURE` and the cover must be active for all secret-bearing screens and
  process/activity transitions.
- API 33+ marks clips sensitive; API 29–32 degrades honestly while retaining the
  ownership-checked clear. A second app replacing the clip must never lose data.
- Imported vaults are copied atomically to app state; keyfiles may retain SAF
  access but master passwords and secret values may not.
- Add failing instrumentation/unit tests before repairing every discovered race
  or platform error path.
- Run lifecycle, clipboard ownership, import/keyfile, recreation, and verifier
  assertions through automated API 29 and current-API emulator targets, supported
  by JVM/native units and release-artifact inspection. Physical hardware is not an
  acceptance requirement.

## Acceptance criteria

- [ ] Background/process events fail-lock the Rust session and prevent readable
  recents/screenshots before sensitive content can be captured.
- [ ] Clipboard clearing occurs after the configured delay only when Hidlins still
  owns the value; replacement content is never erased.
- [ ] Vault create/import and keyfile flows survive recreation and correctly report
  cancellation, denial, revoked permission, and stale references.
- [ ] App backups exclude vault, keyfile reference, credential, and secret state as
  documented.
- [ ] Release/R8 TLS initialization succeeds; intentionally missing initialization
  produces a clear bounded error rather than a hang.
- [ ] Platform channels contain no vault parsing, merge, crypto, or lock policy.

## Validation

- `make app-test-android-integration`
- `make app-build-android`
- `make app-test`

## Dependencies

- Task 011

## Expected areas of change

- Android Kotlin application/activity and platform-channel code
- Android manifest, backup/security resources, and instrumentation tests
- Shared platform-service adapters

## Risks / notes

Lifecycle callbacks are reordered during rotation, multi-window, and process
recreation. Tests must assert fail-locked outcomes rather than depending on one
ideal callback sequence.

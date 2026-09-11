# Task 005: iOS Discovery Generation Safety

Delegation: main-only

## Goal

Prevent delayed callbacks from a stopped iOS discovery attempt from stopping, completing, or mutating a newer attempt.

## Context

iOS discovery schedules delayed completion without associating it with the attempt that created it. A stop/restart sequence can therefore let an old timer act on the new browser. Existing native tests validate discovery data conversion but not overlapping lifecycle generations.

## Scope

### In scope

- iOS discovery attempt identity, timer cancellation, delegate callback scoping, and exactly-once completion.
- Deterministic native tests for stop/restart and stale callback behavior.
- Audit of the analogous macOS and Android lifecycle implementations for the same concrete generation defect.

### Out of scope

- Changes to mDNS payload, address admission, or Rust candidate authentication.
- General platform-channel redesign.
- Android/macOS changes when inspection proves their existing lifecycle already invalidates stale callbacks.

## Implementation requirements

- Before product changes, add a deterministic native regression that starts attempt A, stops it, starts attempt B, fires A's delayed completion, and fails because B is stopped or completed. Record red and identical green commands in `evidence/005.md`.
- Use an injectable scheduler/attempt coordinator or equivalent production seam; do not rely on wall-clock sleeps or live Bonjour.
- Every timer and delegate mutation must validate the active attempt generation. Stop invalidates the generation and is idempotent.
- Completion is exactly once for the active generation, and stale success, error, and timeout callbacks are no-ops.
- Audit macOS and Android for the same sequence. Add a test or repair only if the same reachable defect exists; record the evidence either way.
- Correct native test names/comments and active coverage claims that imply restart safety without executing it.

## Acceptance criteria

- [ ] The stale-attempt regression fails on the baseline and passes after repair without timing sleeps.
- [ ] A callback from attempt A cannot stop, complete, or mutate attempt B after stop/restart.
- [ ] Active completion is exactly once across success, error, timeout, and explicit stop paths.
- [ ] Repeated stop and rapid repeated start remain deterministic and leak no browser/timer ownership.
- [ ] macOS and Android analogous behavior is inspected and any confirmed same-class gap is covered.
- [ ] Adjacent native lifecycle coverage decisions and corrected claims are recorded in task evidence.

## Validation

- `make check-ios`
- `make app-test-ios-simulator`
- `make app-check`

## Dependencies

- Task 004

## Expected areas of change

- `app/ios/Runner/AppDelegate.swift`
- `app/ios/RunnerTests/RunnerTests.swift`
- `app/macos/Runner/AppDelegate.swift` and `app/macos/RunnerTests/RunnerTests.swift` only if the same defect is confirmed
- Android discovery plugin code/tests only if the same defect is confirmed
- `app/lib/src/platform/local_discovery.dart` only if channel lifecycle semantics require it
- Active mobile/local-sync coverage documentation

## Risks / notes

Timer cancellation by itself is insufficient because an already-enqueued callback can still run. Generation validation must happen at mutation time, and the test must exercise that stale callback explicitly.


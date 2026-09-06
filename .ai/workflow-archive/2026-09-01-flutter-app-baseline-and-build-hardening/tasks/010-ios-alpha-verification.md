# Task 010: Complete and verify the iOS alpha experience

Delegation: main-only

## Goal

Deliver the documented Hidlins alpha flows for iPhone/iPad layouts with accessible
compact and expanded behavior plus real foreground sync/interoperability evidence
executed through iOS simulators.

## Context

Task 009 establishes platform safety and storage. This task applies the shared UI
to iOS idioms and proves simulator-observable platform behavior, synchronization,
and disclosure requirements without a physical-device dependency.

## Scope

### In scope

- Complete compact and expanded navigation, keyboard/external-input behavior,
  scaling, rotation, safe areas, and native back/dismiss conventions.
- Audit labels, roles, values, focus order, touch targets, and secret visibility
  with automated semantics/focus/scaling, accessibility-guideline, keyboard, and
  simulator interaction tests.
- Add the required privacy manifest and document network/clipboard/file behavior.
- Prove foreground sync through automated two-session simulator tests against
  managed MinIO, including merge, same-field
  conflict history, offline edits, network loss, retry, and app suspend/resume.
  Retain the secure real-S3 target as an optional non-gating confidence run.

### Out of scope

- Android work, store metadata/submission, background sync, or biometrics.
- Adding platform-specific business logic to Dart or Swift.

## Implementation requirements

- Preserve shared responsive widgets unless an iOS-native adapter is needed for a
  platform capability.
- Tests must verify the iOS semantics heading changes introduced by current
  Flutter and ensure secrets are never announced unless explicitly revealed.
- Sync credentials use the established secure entry/storage flow and never appear
  in fixtures or logs.
- All deterministic simulator and MinIO checks must be driven by
  repository-owned Makefile targets using secure prompts or protected temporary
  configuration rather than credentials in arguments, environment variables, or
  logs.
- Preserve and test the secure credentialed-S3 harness, but record its live run as
  `SKIPPED — user decision / credentials not supplied` when no configuration is
  provided.
- VoiceOver spoken-navigation and launcher/app-switcher visual observations are
  `SKIPPED — user decision`. Maximize automated semantics, focus, concealment,
  touch-target, resource, lifecycle-cover, and artifact assertions instead.

## Acceptance criteria

- [ ] All alpha vault, entry, generator, settings, sync, lock, and error flows are
  reachable on compact and expanded iOS layouts.
- [ ] Automated semantics, focus, scaling, accessibility-guideline, keyboard, and
  simulator interaction suites pass across compact and expanded layouts; the
  skipped VoiceOver residual is recorded without being represented as a pass.
- [ ] The privacy manifest accurately declares used APIs and the binary contains
  no telemetry/crash/update-check integration.
- [ ] Automated simulator MinIO targets prove merge,
  conflict history, offline recovery, and foreground-only sync without data loss.
- [ ] Automated suites on at least two simulator sizes and supported iOS runtimes pass the
  July first-run/core/security/sync cases; resource, lifecycle-cover, artifact,
  and controlled-capture assertions maximize coverage of the skipped
  launcher/app-switcher residual.
- [ ] The documented iOS artifact builds through `make` and CI.

## Validation

- `make app-test-ios-integration`
- `make app-test-ios-simulator`
- `make app-build-ios`
- `make app-test`

## Dependencies

- Task 009

## Expected areas of change

- Shared responsive UI and iOS-specific adapters
- `app/ios/` privacy/configuration files
- iOS automated integration/simulator harness, skip/limitation documentation, and
  CI jobs

## Risks / notes

Physical hardware and manual observations are explicitly not required. Live real
S3, VoiceOver speech/gesture, and OS-shell rendering remain documented residuals;
the plan's automated precursors and skip register define the honest acceptance
boundary.

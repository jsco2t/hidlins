# Task 013: Complete and verify the Android alpha experience

Delegation: main-only

## Goal

Deliver the documented Hidlins alpha flows on phone/tablet with accessible
responsive UI and real foreground synchronization/interoperability evidence.

## Context

Task 012 establishes Android-native security and storage. This task completes the
shared experience for Android conventions and proves behavior across the required
emulator matrix.

## Scope

### In scope

- Complete compact/expanded navigation, predictive/system back behavior, IME and
  keyboard handling, rotation, scaling, touch targets, and platform conventions.
- Audit labels/roles/values/focus order and concealed-secret behavior through
  automated semantics/focus/scaling, accessibility-guideline, keyboard, and
  emulator interaction suites.
- Prove foreground-only sync through automated two-session emulator tests against
  managed MinIO, including merge, conflict history, airplane/network loss, retry,
  background/resume, and process recreation. Retain the secure real-S3 target as
  an optional non-gating confidence run.
- Record machine-readable emulator result manifests with artifact, API, ABI, and
  virtual-device identity plus redacted logs.

### Out of scope

- Store metadata/submission, production signing, background sync, biometrics, or
  Android versions below API 29.
- Platform-specific copies of Rust business logic.

## Implementation requirements

- Shared UI remains the default; Android adapters are limited to platform
  interaction and presentation conventions.
- Back navigation must never bypass destructive confirmation or leave a secret
  visible during lifecycle transitions.
- Sync credentials and secrets must be redacted from test output and emulator logs.
- All deterministic emulator and MinIO checks must be driven by
  repository-owned Makefile targets using secure prompts or protected temporary
  configuration rather than credentials in arguments, environment variables, or
  logs.
- Preserve and test the secure credentialed-S3 harness, but record its live run as
  `SKIPPED — user decision / credentials not supplied` when no configuration is
  provided.
- TalkBack spoken-navigation and launcher/app-switcher visual observations are
  `SKIPPED — user decision`. Maximize automated semantics, focus, concealment,
  touch-target, resource, lifecycle-cover, and artifact assertions instead.

## Acceptance criteria

- [ ] All alpha vault, entry, generator, settings, sync, lock, and error flows are
  reachable on compact and expanded Android layouts.
- [ ] Back/IME/rotation/scaling tests pass and no flow traps keyboard, touch, or
  assistive-technology users.
- [ ] Automated semantics, focus, scaling, and emulator interaction suites
  pass; the skipped TalkBack residual is recorded without being represented as a
  pass.
- [ ] Automated emulator MinIO targets prove merge,
  history-preserving conflict, airplane/network recovery, and foreground-only
  sync without data loss.
- [ ] Automated suites on API 29 and a current API emulator, covering both supported ABIs,
  pass the July first-run/core/security/sync cases; resource, lifecycle-cover,
  artifact, and controlled-capture assertions maximize coverage of the skipped
  launcher/app-switcher residual.
- [ ] The documented two-ABI Android release artifact builds through `make` and CI.

## Validation

- `make app-test-android-integration`
- `make app-test-android-emulator`
- `make app-build-android`
- `make app-test`

## Dependencies

- Task 012

## Expected areas of change

- Shared responsive UI and Android adapters
- Android automated integration/emulator harness and skip/limitation documentation
- Android emulator CI or harness configuration

## Risks / notes

Physical hardware and manual observations are explicitly not required. Live real
S3, TalkBack speech/gesture, and OS-shell rendering remain documented residuals;
the plan's automated precursors and skip register define the honest acceptance
boundary.

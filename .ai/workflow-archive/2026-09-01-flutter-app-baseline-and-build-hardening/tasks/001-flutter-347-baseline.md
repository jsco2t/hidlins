# Task 001: Update the local Flutter toolchain and rebaseline the repository

Delegation: main-only

## Goal

Walk the user through updating `/Users/jason/Developer/flutter` to exact Flutter
3.47.2, verify the local macOS/iOS/Android toolchain is usable and telemetry-free,
and only then move the repository to the matching Dart 3.13.2 baseline without
regressing reviewed behavior.

## Context

The installed SDK is a clean `stable` checkout at tag 3.44.8, and the branch also
pins 3.44.8. Updating the SDK writes outside the repository and therefore requires
the user to perform or explicitly authorize it. The repository's standard app gate
cannot pass between the machine update and pin migration, so both steps belong to
one task, with the human walkthrough completed before repository edits begin.

## Scope

### In scope

- Give the user exact, reversible commands to fetch Flutter tags, switch the clean
  SDK checkout to detached tag 3.47.2, precache macOS/iOS/Android artifacts,
  disable Dart/Flutter telemetry, and run `flutter doctor -v`.
- Stop at the authorization boundary if the user has not already completed the
  machine-level update; do not install or mutate `/Users/jason/Developer/flutter`
  silently.
- Verify Flutter 3.47.2, Dart 3.13.2, Xcode/CocoaPods, Android SDK/NDK, accepted
  Android licenses, and a clean telemetry configuration before source work.
- Update the exact Flutter pin, Dart SDK constraint, CI/tooling references, and
  checked-in Flutter metadata that must change for 3.47.2.
- Generate clean 3.47.2 comparison projects outside tracked source and review the
  platform-template deltas relevant to Hidlins.
- Capture fail-before evidence for each actual analyzer/build/test incompatibility,
  then make the smallest compatible repair.
- Record template changes deliberately retained, adopted, or postponed to their
  already-approved platform task.

### Out of scope

- Standalone Material-package migration (Task 002).
- Android production Gradle/JNI integration (Tasks 007 and 011).
- New product behavior or broad regeneration of platform directories.
- Installing unrelated workstation packages or changing the user's global PATH.

## Implementation requirements

- Pin exactly 3.47.2; do not use a channel or floating version.
- Use the SDK-bundled Dart 3.13.2 as the minimum application baseline.
- The walkthrough uses `git fetch --tags origin` followed by
  `git switch --detach 3.47.2`; do not use an unbounded `flutter upgrade`.
- Capture the pre-update clean SDK state and the post-update `flutter doctor -v`
  result. Any blocking doctor issue for macOS, iOS, or Android must be resolved or
  the workflow enters `BLOCKED` before repository edits.
- Preserve bundle identifiers, entitlements, deployment floors, native plugins,
  and Cargokit integration unless a recorded 3.47.2 incompatibility requires a
  scoped change.
- Keep Flutter analytics disabled and dependency resolution vendored/offline.
- Do not accept generated changes without reviewing them against current source.
- Re-run the high-value July regression invariants already present on `main`:
  secret-free errors/DTOs, session credential drop paths, lock-during-sync matrix,
  bootstrap rollback/password rewrap, activity capture, two-layer lock guarding,
  localization, boundary checks, and bridge-generation drift.

## Acceptance criteria

- [ ] The user has completed or explicitly authorized the exact SDK update; the
  installed checkout reports Flutter 3.47.2 and Dart 3.13.2, telemetry is disabled,
  and `flutter doctor -v` has no blocking macOS/iOS/Android issue.
- [ ] `.flutter-version`, application constraints, and developer/CI tooling agree
  on exact Flutter 3.47.2 and Dart 3.13.2.
- [ ] The application resolves only from the committed pub vendor and passes its
  analyzer, tests, code-generation checks, and macOS build on the new SDK.
- [ ] Each compatibility repair has fail-before/pass-after evidence; unchanged
  platform customizations have a recorded comparison rationale.
- [ ] The retained July invariant suites pass on 3.47.2; no historical task is
  replayed solely because notebook completion state differs from current source.
- [ ] No current functional test or reviewed security invariant is weakened.

## Validation

- `git -C /Users/jason/Developer/flutter describe --tags --exact-match HEAD`
- `/Users/jason/Developer/flutter/bin/flutter --version --machine`
- `/Users/jason/Developer/flutter/bin/flutter doctor -v`
- `tools/dev/telemetry-check.sh`
- `make flutter-version-check`
- `make app-deps`
- `make app-analyze`
- `make app-test`
- `make app-build-macos`

## Dependencies

None

## Expected areas of change

- `.flutter-version`
- `/Users/jason/Developer/flutter` (user-performed or separately authorized
  machine-level update; never a repository edit)
- `app/pubspec.yaml`, `app/pubspec.lock`, `app/.metadata`
- Flutter-related Makefile/tooling and CI configuration
- Platform project files only where the 3.47.2 comparison proves a need

## Risks / notes

The task pauses at the normal permission boundary if the user has not updated the
external SDK. Flutter template upgrades can erase native security or bridge
customization, so apply reviewed deltas selectively and keep future-platform work in
its assigned task.

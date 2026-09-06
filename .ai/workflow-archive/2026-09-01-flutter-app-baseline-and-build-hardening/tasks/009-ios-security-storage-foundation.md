# Task 009: Build the iOS security and storage foundation

Delegation: main-only

## Goal

Produce a working iOS 16+ application that loads the Rust bridge and correctly
implements lifecycle locking, snapshot protection, protected clipboard transfer,
state paths, vault import, and keyfile access.

## Context

The current iOS project is largely a Flutter scaffold. The Rust bridge is pod-only,
so CocoaPods must remain explicit even though newer Flutter templates default to
Swift Package Manager.

## Scope

### In scope

- Reconcile the iOS 3.47.2 project with the existing CocoaPods/Cargokit bridge,
  correct bundle/deployment settings, and no-codesign/device build targets.
- Send report-only inactive/background lifecycle signals to Rust, immediately
  hide sensitive UI snapshots, and restore only after lock state is known.
- Implement one-shot pasteboard transfer with OS expiration and clear semantics.
- Use Application Support for state; implement UIDocumentPicker import by atomic
  copy and persisted non-secret keyfile references with stale-access recovery.
- Preserve the July three-path first run: local create with optional keyfile,
  sync-first bootstrap, and imported KDBX; never edit provider files in place.
- Prove create/import/unlock/lock and representative native bridge operations with
  automated simulator, native-hosted, and built-artifact runners.

### Out of scope

- Final iOS adaptive polish, sync acceptance, VoiceOver, and privacy manifest
  closure (Task 010).
- Background sync, biometric unlock, production signing, or store submission.
- Replacing Cargokit with a speculative SPM bridge.

## Implementation requirements

- Native lifecycle code reports events; Rust owns lock decisions and timers.
- Snapshot shielding must be installed before sensitive UI can be captured.
- Pasteboard APIs must use local-only/expiration behavior and never echo secret
  text through Dart, logs, or exceptions.
- Imported vaults are atomically copied to Hidlins-owned storage; security-scoped
  references are retained only for non-secret keyfiles.
- Add deterministic Swift/Dart/Rust boundary tests before repairing discovered
  failures.
- Expose simulator suites through Makefile targets using Flutter SDK and
  XCTest/XCUITest facilities already supplied by the toolchain. Pair simulator
  coverage with deterministic native units, Dart/Rust boundary tests, and
  no-codesign device-artifact inspection for behavior the simulator cannot expose.
  Tests must be bounded, self-cleaning, and identify the simulator/OS/artifact
  without logging secret material.

## Acceptance criteria

- [ ] Simulator and no-codesign device artifacts load or contain the real Rust
  bridge on iOS 16+; no physical-device execution or signing identity is required.
- [ ] Background/inactive events cause fail-locked behavior and snapshot shielding
  without allowing Dart to decide whether a vault remains unlocked.
- [ ] Automated simulator XCTest plus Dart/Rust boundary tests prove clipboard
  transfer never returns plaintext to Dart and uses iOS expiry/local semantics.
- [ ] Create/import/keyfile flows use correct state ownership, atomic copy, and
  stale-reference handling.
- [ ] Automated catalog/resource and simulator capture checks prove the
  supplied Hidlins icon is complete, opaque, installed, and free of template
  assets; vendor launcher masking is reserved for Task 010's registered visual
  observation.
- [ ] No telemetry, crash reporting, update checks, or unintended networking is
  introduced.

## Validation

- `make check-ios`
- `make app-test-ios-simulator`
- `make app-build-ios`
- `make app-test`

## Dependencies

- Task 008

## Expected areas of change

- `app/ios/`
- iOS platform-service implementations and integration tests
- `Makefile`, CI, and iOS verification evidence

## Risks / notes

Simulator success cannot prove every hardware-specific pasteboard, lifecycle, or
bridge property. Physical hardware is unavailable and is therefore neither an
acceptance requirement nor a blocker; residual hardware-only uncertainty must be
documented honestly and may not weaken the simulator/native/artifact assertions.

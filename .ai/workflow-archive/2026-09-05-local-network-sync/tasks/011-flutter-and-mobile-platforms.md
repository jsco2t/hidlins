# Task 011: Replace Flutter Sync and Add Mobile Discovery Permissions

Delegation: main-only

## Goal

Deliver the complete local-sync experience in Flutter desktop, iOS, and Android, with native mobile discovery/permission adapters and no usable mobile server/background path.

## Context

Flutter currently exposes S3 forms and bootstrap over generated bridge DTOs. Existing native platform-channel dispatchers already enforce a narrow mechanism/policy boundary and can implement Bonjour/NSD without another Dart plugin.

## Scope

### In scope

- Replace Dart S3 models/repositories/controllers/forms/bootstrap with discovery, pair/import, SAS confirmation, manual sync, status, peers/revocation, manual endpoint, permission, and error models.
- Desktop-only visible server toggle and pairing-window control wired to the Rust API.
- iOS Bonjour discovery and permission adapter, required Info.plist service/privacy declarations, retry/settings states, and simulator/native tests.
- Android Network Service Discovery and local-network permission adapter across supported API levels, manifest declarations, retry/settings states, and instrumentation tests.
- Candidate/result bounds and platform-channel manifest/boundary updates.
- First-unlock startup sync, foreground lifecycle cancellation/resume behavior, no post-save sync, and offline continuation.
- Widget, controller, repository, bridge, native, integration, accessibility-semantic, narrow/wide, iPhone/iPad, and Android phone/tablet coverage.
- Remove Flutter/mobile MinIO and live-S3 harnesses as their flows are replaced.

### Out of scope

- Persistent/background mobile sync/server, a Flutter discovery plugin, a service/daemon, or public-network access.
- Final deletion of remaining Rust S3 internals.

## Implementation requirements

- Native adapters return only permission state and bounded endpoint candidates; Rust owns normalization, allowlisting, trust, pairing, Noise, and vault operations.
- Explain local-network access before the OS prompt and distinguish denied, restricted, unsupported, not-found, and protocol/auth failure.
- Mobile lifecycle must cancel/close foreground operations safely when the app is no longer eligible, without registering background modes/services or opening a listener.
- Server controls render only on desktop. Rust target guards must reject server start before bind/advertise on iOS/Android even if a stale/generated caller invokes it.
- Clear SAS and any sensitive input controllers on completion, dismissal, lifecycle lock, and dispose; `Debug`/`toString` remain redacted.
- Avoid new Dart dependencies. Any native API/wrapper dependency requires the full license/footprint review before use.
- Capture red-before/green-after widget/native/bridge behavior.

## Acceptance criteria

- [ ] Desktop Flutter completes discovery, pair/import, startup/manual sync, status, peers/revocation, server toggle, and pairing-window journeys through the real bridge.
- [ ] iOS and Android complete discovery, pair/import, startup/manual foreground sync, permission denial/retry, DHCP candidate change, and offline-use journeys.
- [ ] No mobile UI, manifest/background declaration, platform method, or successful Rust path can start a sync listener/server.
- [ ] No S3 credential/config/bootstrap text, DTO, controller, or MinIO/live-S3 application harness remains.
- [ ] Accessibility semantics and responsive layouts cover pairing SAS/confirmation, permission rationale, progress/cancel, status, conflict, and peer/server controls.
- [ ] Native and Dart channel boundaries reject malformed/oversized candidates and never log vault/key/SAS transcript data.

## Validation

- `make app-test`
- `make app-test-bridge`
- `make app-test-integration`
- `make check-ios`
- `make app-test-ios-simulator`
- `make check-android`
- `make android-harness-check`
- `make boundary-check`

## Dependencies

- Task 010

## Expected areas of change

- `app/lib/src/features/sync/`
- `app/lib/src/features/vaults/`
- `app/lib/src/data/`
- `app/lib/src/platform/`
- `app/lib/src/bridge/`
- `app/l10n/`
- `app/test/`
- `app/integration_test/`
- `app/ios/Runner/`
- `app/android/app/src/`
- `tools/ios-native/`
- `tools/android-native/`

## Risks / notes

Apple and Android permission behavior is version- and device-dependent. Simulated/native unit tests own deterministic policy coverage; physical presentation remains in Task 014's explicit manual residue.

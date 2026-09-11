# Android alpha verification

Android verification is automation-first and runs through top-level `make`
targets. The supported application floor is API 29. Shipping APKs contain only
`arm64-v8a` and `x86_64` Rust libraries.

## Local commands

- `make app-build-android` builds and inspects debug and unsigned release APKs.
- `make app-test-android-integration` runs the native Android lifecycle,
  clipboard, storage, SAF, backup, and sync-permission assertions on an already
  selected emulator.
- `make app-test-android-emulator` runs native, responsive UI, IME/rotation,
  real Rust bridge, local discovery permission, foreground local-network sync,
  background/resume, and process-recreation checks.
- `make test-local-sync-mobile-scenarios-android HIDLINS_ANDROID_STRICT=1`
  runs the expensive API 29 phone/API 36 tablet application scenarios against a
  separate release-mode CLI authority through shipping Android NSD discovery.

The emulator matrix is declared in
`tools/android-native/emulator-matrix.json`: API 29 phone and API 36 tablet for
both supported ABIs. A host executes its native ABI pair—Apple Silicon runs the
two `arm64-v8a` entries and Linux x86_64 CI runs the two `x86_64` entries. The
matrix validator rejects missing API, ABI, or form-factor coverage.

The local-sync scenario adds API/ABI-matched authority and replacement AVDs.
The real `hidlins sync serve` process runs inside the authority guest. An
`androidTest`-only `NsdManager` registrar on that guest receives only the CLI
port and service kind; Android selects the network address. The shipping client
resolves that service through `LocalDiscoveryController`, Flutter, and Rust
private-address validation, then connects directly to the CLI for Noise XX/IK
and sync traffic. No endpoint, `10.0.2.2` data route, ADB forwarding, proxy, or
synthetic discovery result is supplied to the client. API 29 uses the official
emulator Wi-Fi peer link with synthetic cellular data disabled so Android sees
the same single active LAN path as a Wi-Fi-only device; API 36 uses shared
emulator Wi-Fi with its platform-default network state. Neither setup injects a
route or selects an address for NSD.

Passing runs emit redacted logs and machine-readable manifests under
`build/verification/android/`. Each manifest records the AVD, API, ABI, form
factor, device model, host architecture, artifact hashes, log hashes, and
explicit residuals. Verification commands have a repository-owned process-group
timeout and treat Flutter device failure markers as failures even if the host
driver exits successfully.

## Automated coverage

The deterministic suite covers:

- compact phone and expanded tablet navigation across Entries, Search,
  Generator, Sync, and Settings;
- compact system Back, expanded master/detail behavior, IME resize, keyboard
  input, scaling, touch-target guidelines, focus, and concealed-secret
  semantics;
- `FLAG_SECURE`, the opaque lifecycle cover, backup exclusion, clipboard
  ownership/clearing, SAF cancellation/denial, app-owned imports, and missing or
  invalid keyfiles;
- real KDBX create/unlock/CRUD/history/search/TOTP/password rotation/import and
  idle auto-lock through the bundled Rust bridge;
- shipping APK background/resume and cold process recreation;
- two-session foreground local-network sync, peer trust and revocation,
  offline/airplane recovery, retry, and paired-vault import;
- platform-discovered pairing and trusted sync against the real CLI, including
  client process restart, encrypted-state transfer to a replacement authority,
  distinct private DHCP routes, rediscovery without a hint, and pinned-identity
  authentication;
- exact release ABIs, API floor, JNI/R8 classes, Internet permission,
  launcher metadata, and 16 KiB alignment.

Mobile attachment mutation is intentionally outside the approved alpha scope.
The real bridge suite asserts its typed unsupported-platform response; attachment
metadata remains readable and desktop retains full mutation coverage.

## Optional manual observations

Physical Android local-network permission presentation/recovery,
OS scheduling, human SAS perception, representative-router discovery/DHCP
churn, and TalkBack speech can add confidence but do not block acceptance. Follow
[`../../docs/local-network-sync-manual-verification.md`](../../docs/local-network-sync-manual-verification.md).
Programmatic foreground cancellation/resume, pairing, discovery, restart,
revocation, and client-only behavior are covered by the automated emulator and
process scenarios.

The emulator scenario proves Android-platform NSD and the complete direct CLI
Noise/data path on emulator Wi-Fi. The Rust desktop `mdns-sd` publisher/browser
has a separate real-daemon automated test. Only an optional physical
desktop-to-Android observation can establish interoperability across a specific
access point or router firmware; no simulator result is reported as proof of
that boundary.

Launcher and app-switcher OS-shell visual observation remains a separate
optional confidence check. Automated semantics, focus, concealment, touch
targets, resource inspection, lifecycle cover, artifact inspection, emulator
interaction, and local-network cases remain the mandatory acceptance authority.

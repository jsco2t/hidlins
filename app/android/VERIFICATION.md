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
  real Rust bridge, background/resume, and process-recreation checks.
- `make app-test-android-emulator-minio` additionally runs the two-session real
  bridge sync, merge/history preservation, network-loss recovery, retry, and
  bootstrap scenario against repository-managed MinIO.
- `make app-test-android-emulator-s3` is an optional, non-gating confidence run
  and requires a protected `HIDLINS_ANDROID_S3_CONFIG` file.

The emulator matrix is declared in
`tools/android-native/emulator-matrix.json`: API 29 phone and API 36 tablet for
both supported ABIs. A host executes its native ABI pair—Apple Silicon runs the
two `arm64-v8a` entries and Linux x86_64 CI runs the two `x86_64` entries. The
matrix validator rejects missing API, ABI, or form-factor coverage.

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
- managed-MinIO two-session foreground sync, merge with conflict history,
  offline/airplane recovery, retry, and remote bootstrap;
- exact release ABIs, API floor, JNI/R8/TLS classes, Internet permission,
  launcher metadata, and 16 KiB alignment.

Mobile attachment mutation is intentionally outside the approved alpha scope.
The real bridge suite asserts its typed unsupported-platform response; attachment
metadata remains readable and desktop retains full mutation coverage.

## Explicit residuals

- `SKIPPED — user decision`: TalkBack spoken output and gesture traversal.
- `SKIPPED — user decision`: launcher and app-switcher OS-shell visual
  observation.
- `SKIPPED — hardware unavailable and not required`: physical-device execution.
- `SKIPPED — user decision / credentials not supplied`: live credentialed S3.

These skips are not passes. Automated semantics, focus, concealment, touch
targets, resource inspection, lifecycle cover, artifact inspection, emulator
interaction, and managed-MinIO cases are the gating precursors.

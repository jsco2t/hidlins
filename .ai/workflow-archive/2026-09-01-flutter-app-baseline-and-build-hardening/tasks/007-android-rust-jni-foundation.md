# Task 007: Establish the Android Rust and JNI foundation

Delegation: main-only

## Goal

Create a reproducible, audited Android Rust/JNI build on the Flutter 3.47.2
baseline for both supported ABIs and both supported developer hosts.

## Context

The historical verifier spike proved a prebuilt-library fallback under an older
toolchain, but its production changes are absent and it did not prove 16 KiB page
alignment. The existing Cargokit Gradle plugin is tied to legacy Gradle/AGP
assumptions and should not block built-in Kotlin adoption.

## Scope

### In scope

- Build `hidlins-api` for `aarch64-linux-android` and
  `x86_64-linux-android` with the supported NDK through Makefile targets.
- Implement the minimal audited JNI initialization/platform-verifier hook needed
  by Rust TLS on Android.
- Vendor and verify the exact allowed native verifier artifact and its Gradle
  dependency metadata.
- Stage prebuilt `.so` files into a verifier/application-consumable layout without
  using the legacy Cargokit Gradle plugin.
- Define one checked native-artifact contract (library name, target triple, ABI,
  profile, destination, hashes/metadata) shared by Make staging and Flutter/Gradle
  packaging so Android does not become a second implicit bridge build system.
- Validate macOS and Linux host cross-builds, shrinker behavior, ABI contents, and
  16 KiB ELF page alignment.

### Out of scope

- Full Flutter Android app packaging (Task 011).
- Android UI, lifecycle, storage, or clipboard behavior.
- General-purpose JNI abstractions or new crypto/TLS implementations.

## Implementation requirements

- JNI unsafe code must be minimal, locally documented, panic-contained, and
  tested at its safe boundary.
- Use only `arm64-v8a` and `x86_64`; fail if an unexpected ABI is packaged or an
  expected ABI is absent.
- Dependency work must satisfy the full license/transitive/maintenance/vendor
  checklist before newly obtained code executes.
- Verify release/R8 TLS initialization, not only debug loading.
- The Makefile is the only developer/CI entry point for Android Rust builds.
- Retain the July no-silent-fallback rule: inability to build TLS-capable sync on
  either required ABI/host is a plan blocker, not permission to ship sync-degraded.

## Acceptance criteria

- [ ] Strict Makefile targets cross-build the release Rust library for both ABIs
  on macOS and Linux hosts.
- [ ] JNI verifier initialization succeeds in a release/shrunk Android verifier
  without secret logging or process crashes.
- [ ] Automated inspection proves the staged/package ABI set and 16 KiB page
  alignment.
- [ ] The exact verifier dependency is licensed, vendored/verified, locked, and
  documented under repository supply-chain rules.
- [ ] No production Android build depends on the legacy Cargokit Gradle plugin.
- [ ] The checked artifact manifest makes a stale, wrong-profile, wrong-ABI, or
  wrong-hash staged library fail before Gradle packaging.

## Validation

- `make check-android`
- `make build-android HIDLINS_ANDROID_STRICT=1`
- `make deny`

## Dependencies

- Task 006

## Expected areas of change

- `crates/hidlins-api/src/` Android/JNI boundary
- Android build scripts/fixtures and checked dependency metadata
- `Makefile`, CI workflows, and supply-chain evidence

## Risks / notes

JNI and TLS initialization are security-sensitive. Do not generalize the unsafe
surface, and do not accept success on only one ABI, host, or debug build variant.

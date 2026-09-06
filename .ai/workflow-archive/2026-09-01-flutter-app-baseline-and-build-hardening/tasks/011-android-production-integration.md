# Task 011: Integrate the production Android build

Delegation: main-only

## Goal

Produce reproducible Flutter 3.47.2 Android debug and unsigned release artifacts
that package the audited Rust/JNI foundation for exactly the supported ABIs.

## Context

The original Android project opted out of built-in Kotlin and applied a separate
Kotlin plugin. The first production migration attempt established two important
Flutter 3.47.2 constraints: enabling AGP's new DSL crashes Flutter's Gradle
plugin at its remaining `AbstractAppExtension` cast, and removing the
settings-level Kotlin version declaration exposes AGP's Kotlin 2.2.10, which is
below Flutter's 2.2.20 minimum. Flutter's supported migration is therefore
built-in Kotlin plus the old-DSL compatibility flag, with a settings-only
compiler version declaration that is never applied as a module plugin.

## Scope

### In scope

- Migrate the application to Flutter's built-in Kotlin support and exact
  Flutter 3.47.2 Gradle/AGP/Kotlin toolchain, removing the applied Kotlin plugin
  while retaining Flutter's required old-DSL compatibility flag and
  settings-only compiler version declaration.
- Stage Task 007's Makefile-built Rust libraries into Android packaging without
  invoking the legacy Cargokit Gradle plugin.
- Configure API 29 floor, release shrinker/TLS rules, exact ABI filters, Gradle
  dependency locking/verification, and reproducible offline-friendly builds.
- Add Makefile and CI targets for debug APK and unsigned release APK/artifact
  inspection.

### Out of scope

- Android lifecycle, clipboard, storage, or UX behavior (Tasks 012–013).
- Production signing, Play Store bundles, publishing, or Windows-host builds.
- Replacing Gradle/Flutter tooling with a custom packager.

## Implementation requirements

- Follow the official Flutter 3.47.2 built-in Kotlin migration exactly:
  `android.builtInKotlin=true`, `android.newDsl=false`, AGP 9.1.0, Gradle 9.3.1,
  and Kotlin 2.4.0 declared in settings with `apply false`.
- Do not apply `org.jetbrains.kotlin.android` in the application or plugin
  modules, do not use imperative `kotlin-android` application, and do not pass
  `--android-skip-build-dependency-validation`. The settings declaration is a
  compiler/toolchain pin, not an applied legacy Kotlin build mode.
- Add a repository-owned `make app-android-config-check` with negative controls
  that rejects built-in Kotlin disabled, new DSL incorrectly enabled for this
  Flutter pin, missing/drifted toolchain pins, KGP applied to a module, or a
  dependency-validation bypass. Record the current invalid partial configuration
  failing before the repair and the corrected configuration passing afterward.
- Run Rust cross-build/staging before Flutter/Gradle packaging through one Makefile
  dependency graph.
- Package exactly `arm64-v8a` and `x86_64` and inspect the release artifact for
  missing/extra libraries and 16 KiB alignment.
- Add both adaptive and legacy Hidlins launcher resources, checking safe-zone,
  monochrome/foreground/background behavior, and absence of template icons.
- Enable fatal warnings for first-party Kotlin/Java only and preserve release TLS
  initialization under R8.
- No build command may live only in CI YAML.

## Acceptance criteria

- [ ] The Android project uses Flutter 3.47.2 built-in Kotlin with AGP 9.1.0,
  Gradle 9.3.1, and Kotlin 2.4.0; `android.newDsl=false` is retained as the
  documented Flutter compatibility flag, and no Hidlins module applies KGP.
- [ ] `make app-android-config-check` proves the exact supported configuration
  and rejects each legacy-KGP, unsupported-new-DSL, version-drift, and validation-
  bypass negative control.
- [ ] `make app-build-android` produces debug and unsigned release artifacts that
  load the real Rust bridge.
- [ ] Release inspection proves the exact two-ABI set, correct minimum SDK, 16 KiB
  alignment, and successful R8/TLS verifier operation.
- [ ] Gradle dependencies are locked/verified under repository supply-chain rules.
- [ ] macOS and Linux CI invoke only Makefile targets for the supported cross-build
  and Android packaging matrix.
- [ ] Installed debug and unsigned release artifacts show the approved launcher
  mark across tested launcher masks and contain no Flutter template icon.

## Validation

- `make app-android-config-check`
- `make build-android HIDLINS_ANDROID_STRICT=1`
- `make app-build-android`
- `make pub-vendor-check`

## Dependencies

- Task 010

## Expected areas of change

- `app/android/`
- Rust library staging/build scripts
- `Makefile`, dependency verification, and CI workflows

## Risks / notes

The settings-level `apply false` Kotlin declaration must not be mistaken for an
applied Kotlin plugin, and `android.newDsl=false` must not be mistaken for
disabling built-in Kotlin. The static contract distinguishes those independent
controls; real debug/release builds prove the resulting configuration. A future
Flutter upgrade may remove the old-DSL requirement, but changing it under the
3.47.2 pin is not cleanup.

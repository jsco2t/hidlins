# Flutter App Baseline and Build Hardening

## Objective

Ship the existing Hidlins Flutter application as an alpha-quality desktop and
mobile client on an exact Flutter 3.47.2 / Dart 3.13.2 baseline, while preserving
the reviewed Rust core and current application behavior. Close the remaining
desktop integration gaps, establish viable iOS and Android delivery paths, add a
canonical release-binary build, and make warnings fatal in every first-party
compilation path exposed through `make`. Begin by walking the user through the
required local Flutter SDK update, and replace all Flutter template branding with
the user-supplied Hidlins logo package.

This plan supersedes the July 2026 Flutter implementation plan and design for
execution. Those documents remain useful design history, but their greenfield
assumptions and older Flutter/Gradle baseline do not describe the current branch.

## Sources reviewed

- Product requirements: `notebook/projects/hidlins/prd.md`, especially FR-040–048,
  FR-050–054, FR-090–092, NFR-013–015, and the offline/no-telemetry constraints.
- Original feature documents: `features/flutter-app/plans/index.md`,
  `implementation-plan.md`, `design.md`, all 45 July task descriptions, both July
  engineering reviews, follow-ups/open items, the Android verifier spike report,
  and all desktop/iOS/Android verification matrices.
- User-supplied brand archive: `.ai/workflow/hidlins-logo-package.zip` — three
  canonical 1024×1024 SVG/PDF treatments plus 16–1024 px raster exports. The navy
  treatment is the package's primary favicon/launcher candidate.
- Current branch at `ae2b2e5f1e5f23619613d56389c576f98dc361a1`, which is the
  post-review `main` baseline and is treated as sound unless a new baseline or
  platform test demonstrates otherwise.
- Official Flutter guidance:
  - Flutter 3.47 release notes:
    <https://docs.flutter.dev/release/release-notes/release-notes-3.47.0>
  - Flutter 3.47.2 stable hotfix tracker:
    <https://github.com/flutter/flutter/issues/191758>
  - Dart 3.13.2 tag matching the Flutter hotfix's pinned Dart revision:
    <https://dart.googlesource.com/sdk/+/refs/tags/3.13.2>
  - Standalone Material/Cupertino migration:
    <https://docs.flutter.dev/release/breaking-changes/material-ui-and-cupertino-ui>
  - Built-in Kotlin migration:
    <https://docs.flutter.dev/release/breaking-changes/migrate-to-built-in-kotlin>
  - Built-in Kotlin application migration, including Flutter's retained old-DSL
    compatibility flag:
    <https://docs.flutter.dev/release/breaking-changes/migrate-to-built-in-kotlin/for-app-developers>
- Stable SDK archive:
    <https://docs.flutter.dev/install/archive>

## Current behavior

The post-review branch already has a substantial, passing Flutter application:
the Rust session/API layer and generated bridge; adaptive desktop shell; vault,
entry, search, generator, and settings flows; native desktop scaffolding; and unit,
widget, boundary, and bridge-generation tests. The sync repository exists, but the
visible sync route is a placeholder. The full desktop golden/integration/interop
suite is absent. iOS remains mostly scaffolded, and Android does not contain the
production Rust/JNI integration described by the historical verifier spike.

The repository currently pins Flutter 3.44.8, opts Android out of built-in Kotlin,
uses CocoaPods/Cargokit for Apple native builds, and has no `make release` target.
Clippy and Linux C++ reject warnings, while ordinary Cargo and other platform
build paths are not uniformly warning-fatal.

The installed SDK at `/Users/jason/Developer/flutter` is also a clean Flutter
3.44.8 checkout. It cannot be changed by repository planning, and the user must
perform or explicitly authorize the exact 3.47.2 checkout and platform precache.
Task 001 places this walkthrough and verification before any repository mutation.

The July notebook task rollup is therefore not live execution state: some work it
describes as future is already on `main`, while some work it describes as landed
is absent. Current source and tests control scope.

Revision 6 follows an implementation-time compatibility finding after Tasks
001–010 completed. A partial Task 011 migration proved that Flutter 3.47.2 still
uses AGP's legacy `AbstractAppExtension` API and fails before configuration when
`android.newDsl=true`. Removing the settings-level, `apply false` Kotlin version
declaration also exposed AGP's Kotlin 2.2.10 to Flutter's dependency validator,
below Flutter's 2.2.20 minimum. The incomplete Android tasks are therefore
replanned around Flutter 3.47.2's documented supported split: built-in Kotlin is
enabled, the old AGP DSL remains temporarily selected, and the Kotlin compiler
version is declared but its Android plugin is never applied.

### July task audit

| July phase | Current evidence | Ideas retained in this package |
| --- | --- | --- |
| P1 API boundary (T1.1–T1.9) | `hidlins-api` and its US-090–095 suites are present; the recent review repaired secret argv/output, merge-before-present, per-vault lock config, event registration, zeroization ordering, shutdown, and deterministic concurrency gaps. | Re-run all invariants on 3.47.2; retain stable DTO/error shapes, no-password DTOs, session credential drop/poison/panic rules, complete lock-during-sync mutation/trigger matrix, one-time OS-source warnings, atomic bootstrap rollback, and RST-CRED-1 password rewrap. |
| P2 toolchain/bridge (T2.1–T2.6) | Exact FRB pins, vendored pub, boundary/codegen checks, Linux/macOS scaffolds, and telemetry gates exist. | Exact SDK walkthrough; cross-host identical binding check; offline resolution; macOS real-bridge/coexistence; no raw CI build commands; retain CocoaPods exception. |
| P3 design/data layer (T3.1–T3.5) | Theme/l10n, adaptive scaffold, lock guard, activity capture, repositories/fakes/providers, widgets, and a seed golden harness exist. | Preserve platform fonts, all strings through localization, two-layer lock defense, pointer/keyboard/text activity, transient reveal state, reduced motion, deterministic goldens, and capability-oriented fakes. |
| P4 feature verticals (T4.1–T4.8) | Vault/entry/search/generator/settings and attachment/TOTP flows plus focused widget tests exist. | Revalidate staged unlock/sync, keyfile create/import, bootstrap/change-password error states, UTF-16 search highlights, attachment path-only boundary, history, no-dead-row Settings rule, and secret concealment. |
| P5 desktop closure (T5.1–T5.5) | Notebook automation claimed completion, but current source has a placeholder sync route, placeholder goldens, and no current app integration/interop harness. | Implement the actual sync UI; exact shortcut/context-menu/keyboard/reduced-motion contract; 24-golden matrix; 5k perf sanity; real unresolvable conflict; bootstrap rollback; app/CLI/TUI coexistence; MinIO LAN fixture; KeePassXC round-trip. |
| P6 Android spike (T6.1–T6.2) | Historical GO memo only; production JNI/build files are absent. | Re-prove both ABIs and both hosts on 3.47.2; narrow JNI; release/R8 TLS; dependency verification; no sync-degraded fallback; add 16 KiB alignment and checked native-artifact manifest. |
| P7 iOS (T7.1–T7.5) | Mostly Flutter scaffold. | Retain iOS 16, CocoaPods bridge, report-only lifecycle, snapshot shield, pasteboard expiry, app-support storage, native keyfile bookmarks, three-path onboarding, privacy manifest, VoiceOver, and simulator-driven sync. |
| P8 Android (T8.1–T8.5) | Scaffold uses legacy Kotlin opt-outs and template icons. | Retain API 29, two ABIs, FLAG_SECURE, API-aware sensitive clips with owner check, SAF keyfiles, backup exclusion, TalkBack, airplane-mode recovery, release/R8 verification, and emulator-driven sync. |

Completed July work is not reimplemented. It is either reused as existing coverage,
re-run as a migration invariant, or represented by a remaining task above. The
final task must account for every historical task using one of those dispositions.

## Architectural decisions

### Preserve from the original architecture

- Rust remains the sole owner of vault, secret, sync, merge, lock policy, session
  state, and cryptographic behavior. Swift/Kotlin own OS-mediated snapshot,
  clipboard, picker, sandbox-path, and launcher integration. Dart adapts typed
  results and renders state; it owns neither policy nor secrets.
- `AppSession`, the explicit lock state, fail-locked sync behavior, panic/poison
  handling, merge-before-present, and the small audited secret-egress doors remain
  the correct security boundary.
- One committed `flutter_rust_bridge` binding set, vendored Dart packages, exact
  toolchain pins, zero telemetry, offline operation, platform-native clipboard
  behavior, and foreground-only mobile sync remain requirements.
- The target floors remain iOS 16, Android API 29, macOS 13, and Ubuntu 22.04.
- CocoaPods remains an explicit exception for the current Cargokit Apple bridge.
  Flutter's newer Swift Package Manager default is not sufficient reason to
  rewrite a functioning pod-only native bridge.

### Corrections to the original plan

1. **Rebaseline instead of rebuilding.** The repository already contains the
   Rust API, generated bridge, adaptive shell, vault and entry flows, settings,
   generator, search, tests, and desktop native scaffolding. Work begins from
   those implementations and does not replay completed July tasks.
2. **Use Flutter 3.47.2 exactly.** Update the repository pin and Dart floor to
   the SDK's Dart 3.13.2 baseline, compare generated platform templates before
   adopting changes, and repair only demonstrated compatibility breaks.
3. **Move first-party UI code to the official standalone Material package.**
   Flutter 3.47 makes this migration available while the framework copy is
   frozen. Because Hidlins is still pre-alpha, performing the migration now is
   less risky than accumulating another compatibility boundary. Pin the exact
   package version, vendor it, audit its license/transitives, and use the official
   compatibility bridge only where an existing third-party API still exposes
   framework Material types. Do not add Cupertino UI unless code directly needs
   it.
4. **Adopt Android's built-in Kotlin support without enabling the unsupported
   AGP DSL.** Set `android.builtInKotlin=true`, retain
   `android.newDsl=false` as required by Flutter 3.47.2, remove
   `org.jetbrains.kotlin.android` from every module's applied plugins, and retain
   only Flutter's settings-level Kotlin 2.4.0 declaration with `apply false` to
   resolve the supported compiler toolchain. Pin AGP 9.1.0 and Gradle 9.3.1 to
   the installed Flutter 3.47.2 template constants. This is built-in Kotlin:
   AGP owns Kotlin compilation and no Hidlins module applies KGP. Do not bypass
   Flutter dependency validation, and do not force the legacy Cargokit Gradle
   plugin through this migration; stage `make`-built Rust shared libraries into
   the Android app instead, using the fallback proven by the prior verifier
   spike.
5. **Keep one stable generated API surface.** Conditional mobile-only Rust
   exports cannot appear in a binding file generated on desktop. Expose stable
   clipboard/lifecycle boundary methods on all builds; an inapplicable platform
   returns a typed unsupported-platform error. Platform adapters and tests, not
   divergent generated files, enforce which calls are reachable.
6. **Treat the Android spike as evidence, not production code.** Revalidate Rust
   Android targets, JNI, rustls platform verification, shrinker behavior, both
   required ABIs, and both host build paths against Flutter 3.47.2. Explicitly
   assert 16 KiB page alignment and package only `arm64-v8a` and `x86_64`.
7. **Define release scope precisely.** `make release` builds optimized, locked,
   offline Rust workspace binaries (CLI, TUI, and agent) for the current host.
   Flutter platform artifacts retain explicit per-platform targets because a
   single host cannot build every platform. Signing and store publication remain
   outside this alpha work package.
8. **Use a capability-oriented platform boundary.** Lifecycle reporting,
   clipboard, paths, import, and keyfile access remain separate small interfaces.
   `AppSession` does not become a cross-platform service locator, and missing
   platform capabilities return typed errors rather than silent no-ops.
9. **Define one Android native-artifact contract.** Make-built libraries and
   Gradle packaging share checked metadata for target, ABI, profile, filename,
   hash, and destination. This prevents the deliberate Android/Cargokit split from
   becoming two untracked bridge build systems.
10. **Treat branding as source-controlled product input.** The supplied SVGs are
    canonical. Ship only required rasters, record provenance/license treatment,
    validate hashes/dimensions/alpha/safe zones, and do not add an icon-generator
    or runtime-SVG dependency for an already-complete asset set.
11. **Make virtual mobile platforms the acceptance baseline.** Physical iOS and
    Android hardware is unavailable and is not required. Exercise all reliable
    platform behavior through multi-size/runtime iOS simulators, API 29/current
    Android emulators across both supported ABIs, native-hosted tests, static
    boundary checks, network-controlled integration, and built-artifact inspection.
    Record hardware-only uncertainty as a limitation rather than a blocker or an
    unverified acceptance claim.

## Architecture review

**Scope and mode:** Planning-architecture review of this work package against the
current Rust/Flutter repository. The `arch-reviewer` separation, testability,
dependency-direction, hidden-control-flow, state-ownership, simplicity, API-shape,
and extension-point dimensions were applied to proposed changes rather than a code
diff. Generated bindings, vendored sources, fixtures, and goldens were excluded.

### Critical

1. **External SDK transition could make Task 001 impossible to complete — confidence
   96.** Dimension: hidden temporal coupling / testability. Evidence:
   `gate.json` requires `make app-check` for every task, while
   `tasks/001-flutter-347-baseline.md:14` records that the machine and repository
   both begin at 3.44.8. A standalone machine-update task would leave the SDK at
   3.47.2 while `.flutter-version` still demanded 3.44.8, so its mandatory gate
   could never pass. Concrete cost: the approved workflow would block before the
   repository migration task. Resolution incorporated: Task 001 performs the
   human walkthrough and verifies the external SDK first, then updates the
   repository pin within the same task before running the standard gate.
2. **“Rust owns lifecycle” blurred policy and OS mechanism — confidence 92.**
   Dimension: separation of concerns / state ownership. Evidence: the original
   plan wording assigned lifecycle behavior wholesale to Rust while the iOS and
   Android tasks necessarily implement snapshot shielding, clipboard expiry, and
   platform callbacks. Concrete cost: Dart/native code could duplicate lock policy
   or Rust could be made responsible for OS presentation details it cannot own.
   Resolution incorporated at `plan.md:91` and Task 008: Rust owns lock policy and
   session state; native code owns OS safety mechanisms; Dart only reports/adapts
   and renders typed state.
3. **The current boundary gate rejects every planned platform channel — confidence
   98.** Dimension: dependency direction / enforceability. Evidence:
   `tools/dev/boundary-check.sh:105` classifies any `MethodChannel(` outside the
   generated bridge as a bypass, while iOS/Android clipboard, picker, lifecycle,
   and sandbox-path requirements need native channels. Concrete cost: either
   mobile implementation cannot pass `make app-check`, or the gate is broadly
   weakened and arbitrary Dart-to-native bypasses become possible. Resolution
   incorporated in Task 008: add a checked capability manifest and allow only
   fixed adapters under `app/lib/src/platform/`; keep Process, `dart:ffi`, arbitrary
   channels, core I/O, and network access banned, with planted-violation tests.

### Important

1. **Android staging risked becoming an implicit second bridge build system —
   confidence 89.** Dimension: dependency direction / durability. Evidence: the
   plan intentionally retains Cargokit for Apple/Linux but uses Make-built `.so`
   staging for Android. Without a shared contract, library name, ABI, profile, or
   stale artifacts could drift independently. Concrete cost: debug libraries or
   wrong-ABI output can be packaged successfully and fail only at runtime.
   Resolution incorporated at `plan.md:146` and Task 007: a checked artifact
   manifest is the sole handoff to Gradle and rejects stale/hash/profile/ABI drift.
2. **Standalone Material can create a permanent dual type system — confidence
   88.** Dimension: API shape / coupling. Evidence: the official compatibility
   bridge transfers inherited theme/localization state but cannot reconcile public
   API types from `package:flutter/material.dart` with `package:material_ui`.
   Concrete cost: legacy types would leak across features and make every future UI
   dependency upgrade harder. Resolution incorporated in Task 002: run a minimal
   router/state/localization feasibility compile before the bulk rewrite, isolate
   any legacy use to one adapter, and require plan revision if public API coupling
   cannot be contained.
3. **Platform icon copies lacked a canonical ownership model — confidence 85.**
   Dimension: state ownership / maintainability. Evidence: four platform projects
   require different icon catalogs while the supplied archive contains 42 logo
   variants. Concrete cost: future changes could update one platform or use
   inconsistent crops without detection. Resolution incorporated at `plan.md:150`
   and Task 004: SVG masters are authoritative, a manifest maps every shipped
   raster to source/hash/dimensions/use, and deterministic checks reject drift.

**Finding count:** 3 critical, 3 important; all six are incorporated into the
revised plan and tasks. No additional ≥80-confidence architecture finding remains.
Out of scope observation: platform-specific correctness and test sufficiency are
handled by the test plan below rather than this architecture review.

## Work included

- Human-assisted exact local Flutter/Dart toolchain update, repository baseline
  migration, and standalone Material-package adoption.
- Deterministic integration of the supplied Hidlins logo across launcher/window
  icons and relevant in-app surfaces.
- Cross-language warning hardening and the optimized Rust workspace release target.
- Remaining desktop UI, golden, accessibility, real-bridge, MinIO, and KeePassXC
  interoperability closure.
- Android Rust/JNI foundation plus complete iOS and Android alpha packaging,
  security, storage, responsive UI, accessibility, and foreground sync.
- Cross-host CI, supply-chain evidence, simulator/emulator acceptance, and release /
  contributor documentation.

All behavior changes and compatibility repairs are test-forward and automation
first. Every deterministic criterion must have repository-owned automation when
the platform exposes a reliable control or observation point. Running on another
host, a simulator/emulator, or a credentialed service does not make a test manual:
those cases use scripts or native/integration runners exposed through `make`.
Physical hardware is not required for this package and its absence is never a
workflow blocker.
The user has explicitly declined manual validation for this package. Residual
perceptual and OS-shell observations are recorded as skipped, with automated
precursor results and the remaining uncertainty stated honestly. Live
credentialed-S3 targets remain implemented and documented but are non-gating
when the user supplies no external credentials; managed MinIO and transport/state
machine automation remain mandatory.

### Warning-as-error policy

The policy applies to first-party source compiled by repository build and test
targets; it must not turn warnings inside vendored dependencies or generated
third-party code into an unmaintainable gate.

- Export/append `-D warnings` for all Cargo commands invoked through the
  top-level Makefile, including builds, tests, checks, docs, integration helpers,
  and Cargokit/native Rust invocations. Preserve caller-supplied `RUSTFLAGS`.
- Make every Flutter build target depend on the existing fatal Dart analyzer
  gate. Flutter has no general build switch equivalent to Rust's `-D warnings`;
  `dart analyze --fatal-infos` is therefore the Dart compiler/static-analysis
  warning gate.
- Keep Linux runner `-Werror`, add warnings-as-errors to first-party Apple
  Swift/Clang build settings and Android Kotlin/Java compilation, and scope those
  settings away from Pods, vendored packages, and generated bridge sources.
- Add a deterministic build-policy contract check that compiles an intentional
  first-party warning and requires the build to fail. Record its fail-before and
  pass-after evidence before accepting the enforcement task.
- Add the contract check to `make check`; every task subsequently exercises the
  policy through the standard gate.

## Proposed implementation

The work is deliberately ordered from global baseline changes to platform
delivery. Each task must retain the current reviewed behavior and use the
smallest test that demonstrates any compatibility defect before repairing it.

1. Walk the user through updating the clean local Flutter checkout to exact
   3.47.2, verify every required platform tool, then rebase the repository.
2. Migrate and vendor the official standalone Material package.
3. Harden all build paths and add `make release`.
4. Integrate the approved brand masters and launcher/window/in-app assets.
5. Close deterministic desktop UI, sync, golden, accessibility, and performance
   gaps using the retained July interaction contract.
6. Exercise the real desktop bridge and KeePass/MinIO/bootstrap/coexistence paths.
7. Establish the Android Rust/JNI build foundation against the new baseline.
8. Stabilize the shared mobile FFI and capability-specific platform contracts.
9. Build the iOS security, storage, lifecycle, and branded artifact foundation.
10. Complete and verify the iOS alpha experience and sync.
11. Integrate production Android packaging on Flutter 3.47.
12. Complete Android security, storage, and lifecycle behavior.
13. Complete and verify the Android alpha experience and sync.
14. Finish the release/CI matrix, July audit, documentation, and acceptance.

### Architecture boundaries

- New platform channels may transport lifecycle signals, file-picker results,
  protected clipboard requests, and non-secret configuration. They may not parse
  vaults, merge data, retain master passwords, or implement crypto.
- Master passwords and entry secrets must use the existing secure-buffer and
  one-shot secret-transfer patterns. They must never enter logs, command lines,
  environment variables, restoration state, analytics, or crash reporting.
- Mobile file import must copy into the Hidlins state directory atomically;
  external provider URLs are not long-term vault storage. Persisted access is
  permitted only for non-secret keyfile references and must have stale-access
  handling.
- Mobile synchronization remains opt-in and foreground-only. A second session
  must prove merge/conflict behavior; UI-only mocks do not satisfy sync
  acceptance.
- Platform code should be small enough to audit directly. No new native/Dart
  dependency is accepted without the repository's license, maintenance,
  transitive-footprint, feature-reduction, and in-repository-alternative review.
- Platform-service interfaces are split by capability and injected into Dart tests;
  no global mutable service registry or hidden channel lookup is permitted.
- `boundary-check` continues to ban arbitrary `MethodChannel` use. It permits only
  a manifest-listed set of OS-capability adapters under `app/lib/src/platform/`,
  with fixed channel names and method sets; Rust/business FFI still flows only
  through the generated `hidlins-api` bridge.
- The Android native library handoff is an explicit checked artifact contract;
  Apple/Linux Cargokit and Android Make staging may differ mechanically but must
  consume the same `hidlins-api` API/version identity.
- Brand masters are build inputs, not mutable runtime state. Platform catalogs
  contain only derived packaging assets and never become independent sources of
  truth.

### Test and verification strategy

- Compatibility work is test-forward: capture the failing analyzer/build/test
  result on 3.47.2 before the minimal repair, then record the green result.
- Automation is the default acceptance mechanism. Each task adds or reuses a
  Makefile target for every repeatable unit, widget, golden, native, integration,
  artifact, cross-host, simulator/emulator, and MinIO check it owns. Optional
  credentialed-S3 targets remain available for later confidence runs. Automation
  must be self-cleaning, bounded by
  timeouts, secret-redacting, and able to report artifact/host/virtual-device identity.
- Unit/widget tests cover adaptive navigation, stable FFI error mapping,
  lifecycle ordering, platform-service fakes, sync state, secret visibility,
  semantics, and warning-policy contracts.
- Golden coverage spans all documented alpha screens in light/dark and compact/
  expanded layouts using the repository's deterministic Linux golden policy.
- Real-bridge integration covers vault lifecycle, lock/auto-lock, KeePassXC
  round-trip, two-session MinIO merge/conflict, and platform network-loss
  recovery without exposing secret material.
- Android CI cross-builds both supported Rust triples and inspects ELF/package
  ABIs and 16 KiB page alignment. iOS CI performs simulator/no-codesign builds;
  macOS and Linux build their native artifacts.
- Lifecycle, clipboard, bridge, storage, and MinIO cases run
  through automated iOS simulator XCTest/Flutter integration and Android emulator
  instrumentation/Flutter integration wherever the virtual OS exposes a stable
  assertion surface. Native-hosted units, boundary checks, and artifact inspection
  cover the remaining deterministic contracts. Credentialed real-S3 harnesses
  receive deterministic configuration, redaction, cleanup, and negative tests;
  the live execution is non-gating without user-supplied credentials.
- No manual acceptance result is required in this revision. Where the OS exposes
  no stable assertion surface, capture the strongest available semantics,
  focus-order, resource, screenshot, lifecycle-state, and artifact assertions,
  then record the unobserved residual as skipped rather than claiming it passed.

### User-skipped validation register

| Skipped case | Mandatory automated coverage | Residual uncertainty |
| --- | --- | --- |
| `SKIPPED — user decision`: VoiceOver spoken-navigation on iOS and TalkBack spoken-navigation on Android | Widget semantics, focus-order, labels/roles/values, secret-concealment, scaling, touch-target guidelines, keyboard traversal, and native integration suites | Public test APIs do not reliably prove the exact speech or gesture traversal of the shipping assistive technology. |
| `SKIPPED — user decision`: representative mobile launcher and app-switcher visual observation | Asset hash/dimension/alpha/safe-zone checks, catalog/package inspection, controlled app screenshots, lifecycle state tests, and automated proof that the opaque non-secret cover is installed before backgrounding | Launcher masks and compositor-rendered app-switcher output remain outside a stable app-process assertion API. |
| `SKIPPED — user decision / credentials not supplied`: live credentialed S3 runs | Required managed-MinIO two-session sync, offline/network failure and retry, merge/conflict/history coverage, transport state-machine tests, plus tested secure live-S3 harness/configuration/redaction/cleanup | Provider-specific behavior of a live public S3-compatible service is not executed in this package. |

No manual acceptance gate remains. Keyboard flows, clipboard ownership/expiry,
lifecycle locking, bridge loading, import/keyfile flows, network loss, MinIO,
artifact inspection, and privacy/network checks remain automated and gating.

### Release artifacts

- `make release`: optimized Rust CLI, TUI, and agent binaries for the current
  host under Cargo's release output directory.
- Existing/new explicit targets build unsigned or no-codesign Flutter
  alpha artifacts for Linux, macOS, iOS, and Android. Platform signing identities,
  notarization, App Store/Play Store metadata, and publishing are out of scope.
- CI and contributor documentation invoke `make` targets only; no release or
  platform build command may exist solely in workflow YAML or shell history.

## Task sequence

1. [Task 001: Update the local Flutter toolchain and rebaseline the repository](tasks/001-flutter-347-baseline.md)
2. [Task 002: Migrate to the standalone Material package](tasks/002-standalone-material-migration.md)
3. [Task 003: Enforce fatal warnings and add release builds](tasks/003-build-hardening-and-release.md)
4. [Task 004: Integrate Hidlins brand assets and launcher icons](tasks/004-brand-assets-and-launcher-icons.md)
5. [Task 005: Close deterministic desktop alpha gaps](tasks/005-desktop-alpha-closure.md)
6. [Task 006: Verify the real desktop bridge and interoperability](tasks/006-desktop-real-bridge-interop.md)
7. [Task 007: Establish the Android Rust and JNI foundation](tasks/007-android-rust-jni-foundation.md)
8. [Task 008: Stabilize the mobile FFI and platform-service boundary](tasks/008-stable-mobile-boundary.md)
9. [Task 009: Build the iOS security and storage foundation](tasks/009-ios-security-storage-foundation.md)
10. [Task 010: Complete and verify the iOS alpha experience](tasks/010-ios-alpha-verification.md)
11. [Task 011: Integrate the production Android build](tasks/011-android-production-integration.md)
12. [Task 012: Implement Android security, lifecycle, and storage](tasks/012-android-security-storage.md)
13. [Task 013: Complete and verify the Android alpha experience](tasks/013-android-alpha-verification.md)
14. [Task 014: Finish release, CI, and cross-platform acceptance](tasks/014-release-ci-and-acceptance.md)

Tasks execute sequentially. A later task cannot start until its predecessor has
passed its own validations plus both standard gates and has complete evidence.

## Quality gate

Every task runs `make check` for the Rust workspace and `make app-check` for the
Flutter application. These are the repository's existing normal completion gates
and cover formatting, lint/static analysis, compilation, unit tests, boundary and
bridge-generation checks. From Task 003 onward, `make check` also includes the
warning-policy contract.

Task 001 deliberately combines the external SDK walkthrough with the repository
pin migration: after the SDK moves from 3.44.8 to 3.47.2, `make app-check` cannot
pass until `.flutter-version` moves too. Keeping them in one task preserves the
mandatory standard gate without starting repository changes before toolchain
readiness is proven.

Final acceptance runs `make verify` and `make app-build-macos`. `make verify` is
the existing complete local gate, including ignored/integration, documentation,
supply-chain, and interoperability checks. The macOS artifact build validates the
host-native Flutter release path. Linux, iOS, and Android artifact/virtual-device commands
cannot honestly be run by this single macOS final command set, so their exact
mandatory commands and automated CI/simulator/emulator evidence are owned by Tasks 006 and
008–013. Task 014 mechanically audits that every non-exempt acceptance criterion
maps to an executed automated target and that every user-skipped residual is
present in the skip register without being represented as a pass.

## Risks

- Standalone Material types may conflict with third-party APIs; the official
  compatibility bridge must remain narrow and could expose an upstream blocker.
- Flutter/AGP template regeneration can erase native security and Cargokit
  customization if applied wholesale.
- Flutter 3.47.2 intentionally combines built-in Kotlin with
  `android.newDsl=false`; a future cleanup could mistake this for an obsolete
  Kotlin opt-out. A checked configuration contract must distinguish the required
  old-DSL flag and settings-only compiler pin from applying legacy KGP, and the
  exact Flutter pin makes a future migration an explicit toolchain change.
- Android JNI, TLS verifier, R8, cross-host ABI, and 16 KiB alignment failures may
  appear only in release artifacts or emulator integration.
- Lifecycle ordering and clipboard ownership can differ on physical hardware.
  This package records that residual limitation but requires the fullest reliable
  simulator/emulator, native-unit, boundary, and artifact coverage instead of
  blocking on unavailable hardware.
- Warning flags scoped too broadly can fail on vendored/generated code, while
  flags scoped too narrowly can leave a first-party build escape hatch.
- Credentialed S3 automation depends on external credentials. The secure targets
  remain available, but missing credentials are explicitly non-gating under
  Revision 5. Physical hardware and manual VoiceOver/TalkBack or OS-shell
  observations are not prerequisites; the residual uncertainty is recorded.
- The supplied launcher PNGs advertise RGBA channels and Android launchers apply
  arbitrary masks; opacity and adaptive-icon safe zones require deterministic
  validation rather than visual assumption.

## Out of scope

- Windows, web, store submission, production signing/notarization, biometrics,
  background sync, telemetry, SSH-agent integration, and an SSH-specific entry
  type.
- Re-architecting or broad refactoring of the reviewed Rust core without a
  failing compatibility/security invariant tied to this work package.
- Replacing Cargokit or `flutter_rust_bridge` wholesale merely to follow a newer
  template.

## Final acceptance criteria

The work package is complete only when all fourteen tasks are complete, every
standard and task-specific validation passes, the final `make verify` gate passes,
the whole-package review has no unresolved high-confidence in-scope findings,
the July audit has no unclassified task, approved branding appears in every
artifact, every automatable requirement has executed automated evidence, the
user-skipped validation register is complete and no skipped case is represented
as passing, and
`.ai/workflow/evidence/final.md` records the final Git revision and artifact
matrix. Completion does not authorize archival.

---

## 1. Test Plan

**Test Planning Date:** 2026-09-01  
**Principles:** Unit-first · Self-cleaning · Value-justified · Reliable · Production-quality

### 1.1 Existing Test Patterns

Rust integration tests live under `crates/hidlins-api/tests/`, use standard
assertions with behavior-named functions, `common::TestEnv`/`fast_kdf()` for
self-cleaning vault state, explicit port injection through `AppSession::with_ports`,
and channels with bounded `recv_timeout` for concurrency. Representative files:
`us_091_session_lifecycle.rs`, `us_094_sync_api.rs`, and
`us_095_bootstrap_change_password.rs`.

Dart uses `flutter_test` and ordinary `expect` matchers. Widget tests inject
hand-written repository fakes through Riverpod `ProviderScope` overrides using
`app/test/helpers/feature_test_helpers.dart`; providers use `ProviderContainer`
with `addTearDown`; semantics tests use Flutter's semantics matchers. The Linux-only
golden helper in `app/test/helpers/golden.dart` fixes logical sizes and theme.

Shell interoperability tests use repository Makefile targets, `mktemp -d`, an EXIT
trap, secure stdin, payload-free failures, and external KeePassXC/MinIO processes.
The iOS/macOS RunnerTests use XCTest; Android will use the Flutter/Gradle-native
unit/instrumentation facilities already supplied by the platform scaffold rather
than adding a test framework.

**Assertion Library:** Rust standard assertions; Dart/Flutter `expect`; XCTest and
Android platform assertions for native code; shell exit status plus explicit
payload-free comparisons.

**Dependency-Control Strategy:** Existing Rust traits/ports and deterministic
engines, Dart repository/platform-service fakes, local ephemeral files, managed
MinIO, pinned KeePassXC, simulator/emulator, automated native/integration runners,
and artifact inspection where the OS boundary is the behavior under test.
Prefer Flutter SDK, XCTest/XCUITest, Android SDK/instrumentation, `xcrun`, and `adb`
capabilities already in the toolchain. A new automation dependency must pass the
normal supply-chain review; if no compliant and proportionate approach exists,
record the exact residual in the user-skipped validation register and maximize
the reliable in-repository precursor coverage.

**Test Helpers:** Reuse `TestEnv`, `fast_kdf`, `RecordingLockSink`, blocking/panic
sync engines, `TestHarness`, `pumpFeature`, `expectGolden`, corpus builders,
`api-test-driver`, managed MinIO scripts, and interop cleanup conventions. Extend a
helper only when at least two tests consume it.

### 1.2 Requirement Coverage

| Requirement / criterion | Observable behavior | Planned test or existing coverage | Gap |
| --- | --- | --- | --- |
| Exact local toolchain | SDK is Flutter 3.47.2 / Dart 3.13.2, telemetry off, platform tools ready | Task 001 version/doctor/telemetry commands plus `make flutter-version-check` | External SDK update requires user action or authorization |
| Preserve reviewed July baseline | Existing API/UI/security invariants remain green after migration | Existing US-090–095, Flutter widget/provider/boundary/codegen suites; July disposition audit | None |
| Standalone Material | One Material type system, correct theme/localization/semantics | Focused compatibility compile, theme/widget/golden suites, import boundary scan | Upstream public-type coupling triggers plan revision |
| Fatal warnings | Warning-only first-party code fails every supported compiler path | `make build-policy-check` fixtures for Rust/Dart/Linux/Apple/Android | Native cases run on their platform hosts |
| `make release` | Optimized locked/offline CLI/TUI/agent binaries are produced | Target contract test, artifact existence/type checks, final release build | Signing/installers excluded |
| Approved branding | Correct, untampered icons and in-app marks on all platforms | Asset manifest/hash/dimension/alpha/safe-zone checks; brand widget/goldens; automated artifact and simulator/emulator screenshot inspection | OS-shell launcher rendering is `SKIPPED — user decision`; mask/compositor residual is documented |
| Desktop sync/polish | All sync states and July interaction contracts render and are operable | Repository fake widget tests, shortcut intents, focus/semantics, reduced-motion tests, and automated real-bridge keyboard flow | None; optional desktop screen-reader exploration is not acceptance evidence |
| FR-001–005 vault lifecycle | Create/import/register/unlock/lock/change-password/bootstrap remain correct | Existing Rust and widget tests; real-bridge lifecycle/bootstrap integration | None |
| FR-010–017 entries/search/TOTP/attachments | CRUD/history/search ranges/TOTP/path-only attachments remain correct | Existing US-090/092/093 and feature widget suites; KeePassXC app interop | Mobile attachment mutation intentionally excluded by alpha scope |
| FR-040–048 sync | Merge-before-present, outcomes, conflict history, offline recovery | Existing Rust state-machine tests; automated two-session MinIO; tested optional credentialed-S3 simulator/emulator harnesses | Live credentialed S3 is `SKIPPED — user decision / credentials not supplied`; provider-specific residual is documented |
| FR-050–054 security | Fail-lock, OS lifecycle, screenshots, clipboard ownership/expiry | Existing Rust tests; platform fake/native tests; automated simulator/emulator lifecycle/clipboard suites plus artifact checks | OS-shell app-switcher observation is `SKIPPED — user decision`; process-death clipboard clear is an OS limitation |
| Stable mobile FFI | One binding set, typed unsupported paths, no secret-return API | Rust boundary tests, `api-gen-check`, Dart adapter tests, feature-target checks | None |
| Platform-channel boundary | Only manifest-listed OS capabilities can use fixed channels | `boundary-check` positive/negative planted cases | Static gate cannot prevent a malicious same-commit rewrite; review remains required |
| iOS 16 alpha | Bridge, three-path onboarding, storage, lifecycle, UX, sync | XCTest/Dart tests plus automated multi-size/runtime simulator integration and no-codesign artifact inspection | Physical-device, VoiceOver speech, and OS-shell visual observations are skipped/non-gating with residuals documented |
| Android API 29 alpha | Two-ABI bridge, storage, lifecycle, UX, sync | Rust cross-build, Gradle unit/instrumentation plus automated API 29/current emulator suites across supported ABIs | Linux-host CI remains required; physical-device, TalkBack speech, and OS-shell visual observations are skipped/non-gating with residuals documented |
| Flutter 3.47 Android build model | Built-in Kotlin is enabled; old AGP DSL compatibility remains; KGP is version-pinned but not applied | `make app-android-config-check` static/negative controls plus real debug/release Gradle builds without dependency-validation bypass | `android.newDsl=false` remains required until a future pinned Flutter baseline removes its legacy API use |
| Android supply chain | Verifier/Gradle artifacts are locked and tamper-evident | Dependency verification negative test, artifact manifest hash checks | None |
| Android 16 KiB / ABI | Exactly two aligned release libraries are packaged | ELF/APK inspection target on both host builds | None |
| NFR-013 privacy | No telemetry or unintended network capability | Telemetry check, resolved-dependency/network boundary gate, and automated clean-run network observation | None |
| NFR-014 localization | User-facing strings use localization after UI migration | Analyzer/review scan and widget tests with delegates | English remains the only translated locale |
| NFR-015 accessibility | Accessible semantics, focus, scaling, and concealment contracts | Widget semantics/guideline tests plus automated focus/scaling/keyboard/native suites | Shipping VoiceOver/TalkBack speech and gesture traversal are `SKIPPED — user decision` and remain an explicit residual |
| NFR-001/002 performance | Unlock/search/list remain within documented budgets | Existing Rust benchmarks plus controlled 5k real-bridge sanity | Host results are recorded, not flaky CI timing gates |
| Cross-platform CI | Developer and CI commands are identical Makefile targets | CI static audit plus actual matrix and simulator/emulator result manifests | Physical devices and attached runners are not required |

### 1.3 Test Cases - Unit and Widget Tests

#### 1.3.1 Toolchain, build policy, and brand contracts

**Files:** `tools/dev/*`, a small warning fixture, brand manifest/check script,
platform resource catalogs, and focused Makefile-target tests.

| Test name / target | Verifies | Value justification | Approach |
| --- | --- | --- | --- |
| `flutter-version-check rejects mismatch` | Installed/pinned SDK drift fails closed | Prevents irreproducible template/codegen output | Run parser against controlled banners or injectable command seam |
| `build-policy-check rejects rust warning` | Make-invoked Rust compilation cannot succeed with a warning | Direct regression for the user's requirement | Intentional temporary/fixture warning; success is a failing test |
| `build-policy-check rejects native warning` | First-party C++/Swift/Kotlin/Java settings are fatal | Prevents platform escape hatches | Small host/platform fixture compiled by native targets |
| `app-android-config-check rejects invalid Kotlin/DSL combinations` | Exact Flutter 3.47.2 AGP/Gradle/Kotlin pins, built-in Kotlin enabled, required old DSL selected, and KGP not applied | Prevents a seemingly tidy flag/plugin edit from restoring legacy Kotlin or breaking Flutter configuration | Parse controlled negative fixtures plus the production Gradle files; a real build remains the integration proof |
| `release artifact manifest is complete` | CLI/TUI/agent optimized binaries exist and are release outputs | Protects `make release` contract | Inspect explicit workspace binary list and artifact metadata |
| `app-brand-check validates supplied assets` | Hashes, dimensions, opacity, safe zones, catalogs, and references agree | Prevents silent icon drift or invalid store assets | Table over manifest rows and platform catalog entries |
| `brand surfaces are semantics-safe` | Informative mark labeled once; decorative mark excluded | Avoids screen-reader noise | Widget semantics tests in lock/first-run/About |

**Controlled dependencies:** Tool output is captured through existing scripts or a
small injectable command/path seam; brand checks read committed files only.

**Setup/teardown:** Warning probes build in a unique temporary target directory and
remove it. They must not edit tracked source or share Cargo/Gradle output with
parallel tests.

#### 1.3.2 Rust API and bridge invariants

**Files:** existing `crates/hidlins-api/tests/us_090_*.rs` through `us_095_*.rs`,
plus focused platform-boundary cases where needed.

| Test name | Verifies | Value justification | Approach |
| --- | --- | --- | --- |
| Existing DTO/error redaction tables | No secret field/message crosses the bridge | Migration/codegen must not reopen reviewed leaks | Reuse unchanged suites on 3.47.2 |
| Existing lifecycle and lock matrix | Credentials/vault drop on every lock, sync outcome, poison, and panic path | Highest-value session safety regression | Deterministic clock/ports and bounded channels |
| Existing bootstrap/password-change rollback tables | Atomic cleanup and RST-CRED-1 behavior | Prevents data loss/unusable sync after credential change | Fault-injection table |
| `stable mobile clipboard API returns typed unsupported on desktop` | One generated surface without a general plaintext door | Protects corrected FFI architecture | Feature-target table and generated API assertions |
| `generated API identity matches native artifact manifest` | Android staged library implements the binding/API version packaged by Dart | Prevents stale native library packaging | Compare committed/generated identity to staged metadata |

**Controlled dependencies:** Existing `with_ports`, fake clipboard/sync engines,
manual clocks, temporary state directories, and feature-gated targets.

**Setup/teardown:** `TempDir` owns files; event sinks/threads are shut down and
joined; channel waits are bounded and never use scheduler sleeps as readiness.

#### 1.3.3 Dart UI, state, and platform adapters

**Files:** existing `app/test/`, new sync/brand/platform tests, and the completed
golden matrix.

| Test name | Verifies | Value justification | Approach |
| --- | --- | --- | --- |
| `sync page renders each status and outcome` | Idle/in-flight/error/conflict/backup states and mutation disabling | Current route is a placeholder; errors must be actionable | `TestHarness` fake sync stream table |
| `lock completion cannot restore workspace` | Late sync completion cannot defeat fail-lock | Preserves July two-layer lock invariant | Emit lock then sync done through fakes |
| `shortcut intents perform documented actions` | Search/new/lock/copy/generator/Escape work keyboard-only | Protects desktop reachability | Table of intent/key combinations |
| `reduced motion disables transitions` | Accessibility setting removes nonessential animation | Retains July UX contract | Controlled `MediaQuery.disableAnimations` |
| `standalone Material bridge preserves localization/theme` | Migrated and legacy dependency subtree see coherent state | Detects split design-system context | Focused widget tree around compatibility adapter |
| `platform adapters map every native result` | success/cancel/deny/stale/unsupported/error are typed | Prevents silent platform no-ops | Capability-specific fakes, table-driven Dart tests |
| `lifecycle adapter reports only` | Dart never starts lock timers or decides lock | Protects state ownership | Fake session records raw state sequence |
| `clipboard adapter drops one-shot secret` | Mobile Dart does not retain/return plaintext | Protects secret boundary | Fake native channel and disposal/error assertions |
| `24+ screen goldens` | Light/dark × compact/medium/expanded layouts plus brand/sync | Detects visual layout/theme/icon regression | Exact Linux canonical goldens |

**Controlled dependencies:** Extend the existing repository `TestHarness` with
separate lifecycle/clipboard/path/picker fakes. Do not create one god platform fake.

**Setup/teardown:** Close stream controllers, dispose ProviderContainers/notifiers,
reset test view size/pixel ratio, cancel fake timers, and delete temp assets.

#### 1.3.4 Native platform units

**Files:** `app/ios/RunnerTests/`, Android JVM/instrumentation test source sets, and
small native bridge fixtures.

| Test name | Verifies | Value justification | Approach |
| --- | --- | --- | --- |
| `SnapshotShieldTests` | Overlay installs before background snapshot and is idempotent | Widget tests cannot observe UIKit ordering | XCTest with controlled window callbacks |
| `ClipboardShimTests` | iOS uses local/expiry options and Android leaves foreign clips intact | Direct FR-053 OS contract | Native fake pasteboard/clipboard where possible; instrumentation for system service |
| `KeyfileReferenceTests` | bookmark/SAF success, cancel, stale/revoked recovery | Prevents inaccessible or persisted secret bytes | Temp document + controlled resolver/provider |
| `LifecycleForwardingTests` | Native callback mapping is complete and policy-free | Prevents platform-specific lock semantics | Callback table to fake channel sink |
| `VerifierInitTests` | JNI init is once-only, bounded, panic-contained, and R8-safe | TLS failures otherwise appear only on sync | JVM/native verifier plus release instrumentation |

**Setup/teardown:** Clear pasteboards/clipboard only when test-owned, release
security-scoped resources/URI grants, remove temp documents, and terminate test
activities/sessions.

### 1.4 Test Cases - Integration and End-to-End

| Scenario / test | Why unit coverage is insufficient | Real and controlled scope | Cleanup |
| --- | --- | --- | --- |
| `full_vault_lifecycle_over_real_bridge` | Proves FRB/native loading, KDF, atomic KDBX writes | Real Hidlins bridge/core; temp state and production KDF | Session shutdown + temp directory removal |
| `auto_lock_fires_through_real_bridge` | Proves event stream/ticker integration | Real bridge with controlled time hook | Close streams/session; no wall-clock sleep for policy |
| `sync_roundtrip_two_sessions_merge` | Merge and locking require two real sessions and object transport | Real core/bridge + managed MinIO | Unique bucket/prefix, delete objects, stop managed service |
| `unresolvable_conflict_surfaces_dialog_with_backup_path` | UI must consume a real irreconcilable result | Real conflict fixture and Flutter integration app | Remove backup/temp roots after assertions |
| `bootstrap_and_password_change_failure_matrix` | Atomic installation/credential rewrap span storage and sync | Real bridge + deterministic faulting transport/registry | Assert no temp/partial target, then remove fixture |
| `app_us_09x` KeePassXC round-trip | Only KeePassXC proves external KDBX interoperability | Real app driver + pinned `keepassxc-cli` | Secure stdin, `mktemp`, EXIT trap, payload-free logs |
| Desktop CLI/TUI coexistence | Advisory OS locks and external writes cross processes | Built app plus real CLI/TUI processes | Bounded processes, shutdown, temp state cleanup |
| Android cross-build/artifact inspection | Host toolchain + linker + Gradle packaging cannot be unit simulated | Both Rust targets on macOS/Linux, release APK inspection | Dedicated target dirs and staged-artifact cleanup |
| iOS simulator core pass | Xcode/CocoaPods/static bridge and sandbox paths are real | Simulator/no-codesign app with temp/test state | Uninstall app/reset test container |
| Android API 29/current core pass | Embedding/Gradle/JNI/SAF differ by OS level | Two controlled emulators | Revoke grants, clear app data, stop emulator if managed |
| Automated iOS/Android virtual-platform security and sync | Bridge, clipboard, lifecycle, import/keyfile, and LAN/MinIO behavior require real platform services even without hardware | Repository-owned iOS simulator XCTest/Flutter integration and Android emulator instrumentation/Flutter integration targets, with a desktop second session; native units and artifact inspection close deterministic gaps | Delete remote prefixes, clear owned clipboard data, remove test vault/keyfile, redact evidence |
| Automated precursor coverage for skipped assistive-technology and OS-shell observations | Maximize reliable assertions before recording the user-directed skip | Semantics/focus/guideline/keyboard suites plus asset, screenshot, lifecycle-cover, and artifact inspection | Record the unobserved shipping speech/compositor residual without claiming success |

Integration tests must use repository Makefile targets. No live public network is
used in routine gates; real-S3 targets remain repository-owned optional confidence
runs enabled only with scoped credentials.

### 1.5 Test Infrastructure

#### New mock or fake implementations

| Interface | Package | Used by | Exists today? |
| --- | --- | --- | --- |
| Lifecycle capability | `app/test/fakes/` | lifecycle adapters/widgets | No — add focused fake |
| Secure clipboard capability | `app/test/fakes/` | copy/expiry/error mapping | No — add focused fake |
| Paths/import/keyfile capabilities | `app/test/fakes/` | onboarding/storage widgets | No — add separate focused fakes |
| Native artifact inspector | tooling script, not product interface | Android staging/packaging tests | No — add deterministic checker |
| Sync/clipboard/session ports | Rust `hidlins-api` test support | API state machines | Yes — reuse/extend only for missing observable state |

#### New test helpers

| Helper | Purpose | Used by | Why shared |
| --- | --- | --- | --- |
| Brand manifest checker | Validate hashes/dimensions/alpha/catalog mapping | brand task and final artifacts | One source of truth across four platforms |
| Platform result table helper | Exercise typed adapter outcomes | five capability adapter suites | Same exhaustive result categories |
| App real-bridge fixture | Initialize compiled library and temp AppSession | lifecycle/bootstrap/sync integration | Avoid repeated unsafe setup/cleanup |
| Artifact inspection helper | Inspect ABI/profile/alignment/native API identity | Android foundation and APK tasks | Same contract before and after Gradle packaging |

#### Test fixtures

| Fixture | Description | Used by | Cleanup |
| --- | --- | --- | --- |
| Supplied logo archive | Immutable approved source inputs | brand manifest/goldens | Read-only; extracted temp copies deleted |
| Existing KDBX corpus | All entry kinds/history/attachments/keyfiles | bridge/interop virtual-platform core passes | Per-test temp copies |
| Irreconcilable sync fixture | Known real unresolvable merge | desktop/mobile conflict UI | Unique temp/bucket prefix deleted |
| 5,000-entry corpus | Deterministic performance/list fixture | desktop sanity | Temp vault removed |
| Managed MinIO bucket/prefix | Two-session sync transport | integration/simulator/emulator LAN checks | Unique names, object deletion, managed service teardown |

### 1.6 Tests Explicitly Not Included

| Behavior / component | Reason not tested |
| --- | --- |
| Flutter/Dart/Gradle/CocoaPods internals | Third-party implementation; Hidlins tests only its configuration, outputs, and boundary contracts |
| Pixel-perfect OS-shell launcher/compositor rendering | Launchers and app switchers apply vendor masks/effects outside the app process; maximize asset, artifact, lifecycle-cover, and controlled-capture assertions, then record `SKIPPED — user decision` for the residual observation |
| Subjective platform-font raster aesthetics | Not a functional acceptance criterion; deterministic goldens plus platform text-scale, overflow, and screenshot-generation tests cover layout correctness without a flaky human or pixel-fidelity gate |
| Live public-S3 execution | Credentials/network create cost and flakiness; managed MinIO plus transport/state-machine coverage are gating, secure credentialed Make targets remain optional, and the live run is `SKIPPED — user decision / credentials not supplied` |
| Shipping VoiceOver/TalkBack spoken output and gesture traversal | Public tests assert semantics/focus/concealment/guidelines but cannot reliably assert exact shipping speech/gesture traversal; residual observation is `SKIPPED — user decision` |
| Timing performance as a shared-runner hard gate | Host contention makes UI timing flaky; retain Rust performance gates and record controlled app sanity measurements |
| Android clipboard clear after process death | OS prevents the app from executing its delayed clear; document limitation and test while process lives |
| Signing, notarization, store validation | Explicitly outside alpha scope |
| Mobile attachment mutation | Approved alpha narrowing is metadata list-only; desktop retains full FR-016 mutation coverage |

### 1.7 Test Implementation Sequence

1. **Toolchain and baseline characterization** — verify 3.47.2 first, capture real
   failures, then run all retained July invariants.
2. **Material compatibility compile and UI unit tests** — fail fast before broad
   import churn.
3. **Warning/release and brand contract tests** — establish gates before new
   platform code and before canonical goldens.
4. **Desktop fake-based widget/semantics/golden tests** — cheapest coverage for
   UI states and interaction contracts.
5. **Desktop real-bridge/MinIO/KeePassXC integration** — prove boundaries only
   after unit/widget behavior is stable.
6. **Android Rust/JNI artifact tests** — retire the highest build risk before
   shared mobile APIs and production packaging.
7. **Capability-specific Dart/native units** — pin platform boundary behavior
   before simulator/emulator flows.
8. **iOS then Android simulator/emulator integration** — prove build/sandbox/OS
   integration across the supported virtual-runtime matrices.
9. **Automated virtual-platform LAN acceptance** — run the strongest reliable
   simulator/emulator and MinIO assertions through repository-owned Make targets,
   pair them with native units and artifact inspection, test optional live-S3
   harness safety, and record the user-directed residual skips honestly.
10. **Whole-matrix regression** — all standard/final commands, artifact inspection,
    July disposition audit, and cumulative review.

### 1.8 Open Questions and Testability Gaps

1. **Material ecosystem compatibility** — Affects Task 002. Resolution: the
   minimal compile gate decides before migration; irreducible public-type coupling
   requires `PLAN_CHANGE_REQUIRED`.
2. **Physical-hardware coverage gap** — Hardware is unavailable and is not an
   acceptance prerequisite. Resolution: Tasks 009–013 maximize multi-runtime
   simulator/emulator, native-unit, boundary, and artifact coverage and record any
   residual hardware-only uncertainty without blocking completion.
3. **Real S3-compatible credentials** — Affects provider-specific confidence but
   not final acceptance under Revision 5. Resolution: keep the scoped, redacted,
   self-cleaning Make targets available; when credentials are not supplied,
   record the live runs as skipped and rely on mandatory managed-MinIO plus
   transport/state-machine coverage.
4. **Hosted Linux/macOS CI availability** — Affects two-host Android builds and
   canonical Linux goldens. Resolution: required CI evidence is collected in the
   owning task; local emulation is not silently substituted.

**Test-planning status:** complete, automation-first Revision 5 applied. Every
deterministic package criterion maps to existing or new automated unit/widget/
native/integration/artifact/simulator/emulator coverage. Manual acceptance and
live credentialed-S3 execution are explicitly skipped/non-gating by user decision;
their automated precursors and residual uncertainty remain auditable.

## Acceptance follow-up — Round 2: Existing-vault CLI registration

### Observed acceptance failure

The new application-running guide had to tell CLI users to create a second
throwaway vault because the CLI cannot register an existing KDBX file. This is a
real product capability gap, not a KDBX, security, or architectural constraint.
`hidlins vault open` only accepts an existing registry ID and performs a one-shot
authentication probe; every entry and sync command also addresses vaults by
registry ID. Consequently, accepting an unregistered path in `vault open` alone
would not make that vault usable by the rest of the CLI.

Repository evidence supports authenticated registration directly:

- `crates/hidlins-tui/src/app.rs` already resolves an existing file, opens it
  with the supplied master password, and persists the registration only after
  authentication succeeds.
- `hidlins_core::VaultRegistry::register_and_save` already performs an atomic,
  advisory-locked, latest-state registration and rechecks duplicate names under
  the lock.
- `crates/hidlins-cli/src/commands/vault.rs` already has the secure no-echo
  password path, keyfile handling, KDBX open behavior, registry resolution, and
  exit-code mappings needed by the new command.

The malformed workflow field that initially prevented this round from opening
was repaired before planning: round 1 incorrectly had `completed_rounds: [1]`.
For a first completed round that collection must be empty; the workflow helper
then preserved round 1 as the required metadata object and moved its final
evidence to `evidence/final-round-001.md`.

### Additive fix strategy

Add an explicit command:

```text
hidlins vault register --id <name> --path <existing.kdbx> [--keyfile <path>]
```

`register` will resolve the selected registry, reject an already-used ID before
prompting, securely collect the master password, open the existing vault with
the optional keyfile, and write the registration only after successful
authentication. It will canonicalize the successfully opened vault and keyfile
paths before persistence so later commands do not depend on the invoking working
directory. The opened handle will be dropped before the registry transaction.
`register_and_save` will perform the final concurrency-safe duplicate check and
atomic write. The command will never create, rewrite, migrate, or otherwise
modify the selected KDBX file.

Keep `vault open --id` as the probe for an already-registered vault. This avoids
an ambiguous `open --path` mode that cannot support subsequent ID-based CLI
operations, while giving users the complete path from arbitrary existing KDBX
to every CLI operation: register once, then open/entry/sync by ID.

Add a dedicated, secret-free human/JSON registration result, update CLI help
and generated bash/zsh/fish completions, and update the CLI surface documentation.
Correct `docs/running-and-testing.md` to register and exercise the generated
kitchen-sink fixture through the CLI instead of creating an unrelated vault.
The already-drafted safe `make demo-vault` workflow and guide are included in
this round's review and validation because they were added after round 1
completed and are the acceptance surface that exposed the gap.

No external dependency is required.

### Architecture and security decisions

- CLI remains a thin presentation layer. KDBX authentication stays in
  `Vault::open`; durable registry concurrency and atomicity stay in
  `VaultRegistry::register_and_save`.
- Authentication must precede persistence. Wrong passwords, missing/invalid
  KDBX files, and missing/incorrect keyfiles leave the registry absent or
  byte-for-byte unchanged.
- Master passwords remain confined to the existing secure stdin and
  zeroize-on-drop path. They never enter arguments, environment variables,
  output views, logs, registry data, or error messages.
- The selected KDBX bytes must be identical before and after both successful
  and failed registration attempts.
- A preflight duplicate-ID check avoids unnecessary password prompts; the core
  transaction remains authoritative against concurrent registration races.
- Registration aliases by path are not newly prohibited. The existing registry
  identity contract is name-based, and advisory file locking already protects
  the shared underlying file. Adding a path-uniqueness migration is unrelated
  to this acceptance failure.

### Test strategy

Follow the repository's test-before-fix rule. First add an end-to-end CLI
regression demonstrating that an existing unregistered fixture cannot yet be
registered. Record the red result, then implement the minimum command and record
the green result.

Automated coverage will prove:

- a valid existing KDBX registers and immediately works with `vault open` and
  an ID-based entry command;
- the persisted vault path is canonical and the original KDBX bytes are
  unchanged;
- optional keyfile registration succeeds and stores a canonical keyfile path;
- wrong passwords, missing/invalid files, bad keyfiles, and duplicate IDs fail
  with the established exit-code/error mapping and do not change the registry;
- human and JSON output are secret-free and have a pinned schema;
- command help and all committed shell completions include `vault register`;
- `make demo-vault` creates a KeePassXC-readable kitchen-sink fixture and
  refuses missing, relative, and pre-existing output paths; and
- the running/testing guide uses only commands supported by the resulting CLI.

The standard and final quality gates remain unchanged. `make check` covers the
Rust CLI integration and view tests; `make app-check` protects the shared app
surface. Task-specific `make completions-check` covers generated CLI artifacts.
The existing final `make verify` and `make app-build-macos` remain the full
cross-package acceptance gate.

### Task sequence

15. `tasks/015-existing-vault-cli-registration.md` — implement authenticated,
    atomic existing-vault registration; regenerate completions; and correct and
    validate the application-running guide and demo-vault workflow.

### Risks and out of scope

- Production KDF authentication makes happy-path process tests slower; reuse a
  fast-KDF test fixture while retaining real `Vault::open` behavior.
- Canonicalization resolves symlinks. This matches TUI onboarding and prevents
  registrations that later depend on a shell working directory, but the output
  path may differ textually from user input.
- Direct unregistered-path support for every entry/sync command, registry
  deregistration UX, path-alias prohibition, agent caching, and desktop native
  file-picker work are outside this focused correction.

### Round 2 acceptance criteria

- `hidlins vault register --id NAME --path FILE [--keyfile FILE]` authenticates
  and atomically registers an existing supported KDBX vault without modifying
  it.
- A failed registration never creates or changes a registry entry and never
  leaks secret material.
- The registered ID works with the existing `vault open`, entry, and sync
  command architecture.
- Human/JSON help and output contracts plus committed shell completions expose
  the new command accurately.
- `docs/running-and-testing.md` uses the generated kitchen-sink vault directly
  in its isolated CLI walkthrough and accurately distinguishes registration
  from the one-shot open probe.
- The demo-vault target and all round-specific, standard, and final automated
  validations pass without new dependencies.

## Acceptance follow-up — Round 3: identity, startup UX, and attachment export

### Observed acceptance failures

The latest hands-on acceptance pass found four independent product gaps:

1. `crates/hidlins-cli/src/cli.rs::BANNER` is the old Falach wordmark even
   though the clap command name and surrounding comments say Hidlins. The unit
   test only checks for underscores and its comment explicitly describes an
   `F`; the integration test pins a fragment of the incorrect logo. The tests
   therefore preserve the defect instead of identifying it.
2. `crates/hidlins-tui/src/screens/startup_modal.rs` renders the onboarding and
   password inputs as plain adjacent text rows headed by `Focused field:`.
   There is no bordered control, active marker, or cursor affordance separating
   input from instructions. Existing snapshot, journey, and accessibility
   tests pin this confusing presentation.
3. `App::from_registry` sends a one-vault registry directly to a `Direct`
   password prompt, and cancelling that prompt exits. The picker appears only
   when two or more vaults were already registered. The picker itself has no
   action for registering another existing vault. After a lock, the screen says
   it will return to the vault list, but a one-vault registry instead returns
   directly to the password prompt. These state transitions make selecting a
   different vault undiscoverable or impossible from the TUI in the common
   one-vault case.
4. The Flutter entry detail already receives attachment metadata, and the
   desktop bridge already implements `EntryRepository.saveAttachmentTo` through
   Rust's atomic attachment export. `_AttachmentSection` exposes only attach and
   detach actions and never calls that method. No dependency-free desktop save
   destination adapter currently exists, so production UI cannot safely ask the
   user where plaintext attachment output should be written.

### Additive fix strategy

First replace the CLI banner with an unambiguous HIDLINS wordmark. Strengthen
the tests to pin the complete banner and its placement in top-level help so a
different product name cannot pass on a generic glyph fragment.

Then redesign the TUI startup surface around recognizable controls. In normal
layouts, render the active vault-path and master-password fields inside their
own titled borders with active-focus styling and a visible terminal cursor. In
compact/accessibility layouts, retain complete text semantics with an explicit
active-input marker when a bordered control cannot fit. Remove `Focused field`
entirely. Render the selected vault row with both a textual marker and theme
highlight so selection remains understandable without color. Replace
`HIDLINS_STARTUP_ART` byte-for-byte with the requested 12-row, 23-cell-wide
Unicode logo and validate terminal-cell width rather than UTF-8 byte length.
The existing direct `unicode-width` dependency is sufficient.

Make the TUI chooser a stable navigation destination for every non-empty
registry. Cancelling a direct password prompt returns to the chooser instead of
quitting, and leaving the lock screen returns to the chooser with the last
vault highlighted. Add a centrally registered, discoverable `Add existing
vault` command on the chooser (default `a`) that reuses the existing
authenticate-before-register onboarding flow. Track whether onboarding began
as first-run or from the chooser so Escape returns to the correct screen.
`Ctrl+Q` remains the explicit exit everywhere. Do not add an unlocked hot-swap
that could bypass the existing sync-on-lock path; switching an unlocked session
continues to require locking first.

Finally add a desktop-only `Save as…` action beside every attachment. A narrow
`AttachmentExportCapability` asks the native desktop shell for a destination
path through one reviewed method channel. macOS uses `NSSavePanel`; Linux uses
the GTK 3 file chooser already linked by the runner. The channel carries only a
sanitized suggested filename and the chosen destination path. Attachment bytes
never enter Dart or the native channel: after selection the widget invokes the
existing repository method, and Rust remains responsible for reading the KDBX
attachment and performing the atomic write. Cancellation is silent; typed
failures are localized and secret-free; success is announced accessibly.

Direct `Open/View` is intentionally not used for this correction. Launching an
external viewer would require creating and governing a plaintext temporary
copy, deciding its retention/deletion lifecycle, and trusting another process.
An explicit user-selected export satisfies attachment access while making the
plaintext destination and persistence decision visible to the user.

No new external dependency is required.

### Architecture and security decisions

- The CLI fix changes presentation only; clap remains the command-surface
  owner and the banner has no product-state responsibility.
- TUI rendering remains in `screens/startup_modal.rs`, while phase provenance,
  registration, selection, and unlock transitions remain in `App`. The command
  registry remains the single source for discoverable keys and palette/help
  projections.
- The chooser is the canonical locked navigation hub whenever at least one
  vault exists. First-run onboarding remains the only zero-vault start state.
- Existing-vault registration still authenticates before the atomic registry
  transaction. Cancelling or failing the add flow cannot register a vault,
  alter an existing KDBX, or lose the prior chooser selection.
- Password input remains masked and zeroize-on-drop. A visual caret may expose
  only position/length, never password characters or content.
- The Flutter widget coordinates presentation only. Native code owns the OS
  save-panel mechanism, while the Rust API remains the only owner of attachment
  bytes and filesystem writes.
- The new channel is added to `channels.json` and the fail-closed platform
  boundary checker. It is desktop-only in the product UI; mobile attachment
  behavior remains unchanged.
- Suggested attachment names are untrusted metadata. Strip path separators and
  control characters and use a safe fallback before presenting a native default
  filename. The native selection is only a suggestion and must not create a
  file before Rust performs the export.

### Automated test strategy

Each code task begins with the smallest regression that fails against the
current implementation and records fail-before/pass-after evidence.

For the CLI, unit and process-level help tests will compare the intended full
banner, require HIDLINS identity, and reject the known Falach fragments.

For the TUI, state-machine tests will cover direct-prompt cancellation into the
chooser, lock-to-chooser behavior, preservation of the selected row, chooser
entry into add-existing onboarding, Escape provenance, authentication before
registration, successful addition/unlock, and failure/cancellation without
registry mutation. Ratatui `TestBackend` journeys, semantic contracts, and
reviewed snapshots will pin bordered/marked fields, textual selection, compact
fallbacks, password concealment, control-character filtering, and the exact new
logo at supported normal sizes. Geometry tests will use display-cell width.

For desktop export, Dart adapter tests will exhaust success, cancellation,
unsupported, malformed, and safe failure envelopes; widget tests will cover
desktop visibility, accessible labels, sanitized suggestions, selected-path
forwarding to `saveAttachmentTo`, cancellation, repository failure, and success
feedback. The platform-boundary checker and its planted negative controls will
pin the new channel's exact Dart/native ownership and method surface. The
existing macOS and Linux app build targets compile the native `NSSavePanel` and
GTK implementations on their respective CI hosts; the round's available-host
final gate compiles macOS. No manual validation gate is added.

### Task sequence

16. `tasks/016-correct-cli-hidlins-banner.md` — replace and strongly pin the
    incorrect CLI wordmark.
17. `tasks/017-redesign-tui-startup-and-vault-selection.md` — make input focus
    obvious, install the requested startup art, and provide a durable chooser
    and add-existing-vault path.
18. `tasks/018-desktop-attachment-export.md` — add dependency-free native
    desktop Save As selection and wire it to Rust's existing atomic export.

Tasks remain sequential under the workflow even though their code surfaces are
largely independent. The standard and final gates remain unchanged: `make
check` includes the CLI/TUI Rust suites, `make app-check` includes Flutter unit
and widget coverage plus boundary checks, `make verify` runs the integrated
package gate, and `make app-build-macos` compiles the available desktop shell.
The existing CI matrix additionally runs `make app-build-linux` on Linux.

### Risks and out of scope

- Unicode block and box-drawing characters occupy terminal cells differently
  from UTF-8 bytes. Layout and tests must use display width, retain a compact
  no-art fallback, and avoid panics at the supported 40×12 floor.
- Adding chooser provenance affects authentication-error recovery as well as
  Escape. Every recovery branch must return to a usable locked state without
  retaining a password buffer.
- Native save panels are interactive OS components and are not reliably driven
  by headless widget tests. Their request/result contracts, registration,
  sanitization, and call sequencing are automated; each runner is compiled on
  its supported CI host. Manual dialog testing remains skipped by user decision.
- CLI subcommand behavior, TUI entry/workspace UX, registry deregistration or
  rename, direct switching while unlocked, mobile attachment export, attachment
  preview, external-viewer launching, and plaintext temporary-file management
  are outside this round.
- The existing attachment Add button is not expanded by this correction. This
  round addresses access to attachments already present in a desktop vault.

### Round 3 acceptance criteria

- `hidlins -h` and `hidlins --help` render a tested HIDLINS banner and contain
  no Falach wordmark fragments.
- Normal TUI startup frames visually separate and mark the active input;
  compact/accessibility frames retain an explicit active-input marker; the
  phrase `Focused field` is absent.
- The TUI renders the user's exact 12-row startup logo without clipping at
  supported normal terminal sizes and safely falls back without art at compact
  sizes.
- From any configured-vault password prompt, the user can reach a chooser; the
  chooser can select a registered vault or start authenticated registration of
  another existing KDBX; `Ctrl+Q` remains the explicit exit.
- After manual or automatic locking, the next key reaches the chooser with the
  prior vault highlighted, allowing selection of another vault before unlock.
- Failed or cancelled TUI addition never mutates the registry or KDBX, and no
  startup frame exposes password or entry-secret contents.
- On macOS and Linux desktop builds, each attachment has an accessible `Save
  as…` action that asks for a destination and invokes Rust's existing atomic
  export. Cancel is non-destructive; success and failure receive clear feedback.
- Attachment bytes never cross the Dart/native method channel, no temporary
  plaintext file is created for viewing, and mobile behavior is unchanged.
- All round-specific, standard, and final automated validation passes with no
  new external dependency and no new manual acceptance gate.

# Flutter alpha release and acceptance

The Flutter alpha baseline is Flutter 3.47.2 / Dart 3.13.2 with Rust 1.95.0.
`make` is the only product build and verification interface. Artifacts are
unsigned development/alpha outputs: this work does not sign, notarize, publish,
or prepare store submissions.

For startup commands, a disposable kitchen-sink vault, manual smoke journeys,
and the corresponding automated gates, see
[`running-and-testing.md`](running-and-testing.md).

## Build matrix

| Output | Host | Command | Location |
| --- | --- | --- | --- |
| CLI, TUI, agent | Linux or macOS | `make release` | `target/release/{hidlins,hidlins-tui,hidlins-agent}` |
| Linux desktop | Linux | `make app-build-linux` | `app/build/linux/<arch>/release/bundle/` |
| macOS desktop | macOS | `make app-build-macos` | `app/build/macos/Build/Products/Release/Hidlins.app` |
| iOS simulator + device/no-sign | macOS | `make app-build-ios` | `app/build/ios/{iphonesimulator,iphoneos}/Runner.app` |
| Android debug + unsigned release | Linux or macOS | `make app-build-android` | `app/build/app/outputs/apk/{debug,release}/` |

All targets run fatal first-party analysis/compilation. Rust inherits
`-D warnings`; Dart uses fatal warnings and infos; Linux Runner C++ uses
`-Werror`; Runner-owned Swift/Objective-C warnings are errors; and Hidlins
Kotlin/Java compilation is warning-fatal. Vendored/generated dependencies are
deliberately outside that first-party policy.

Flutter 3.47.2's Android configuration is intentionally precise: built-in
Kotlin is enabled, `android.newDsl=false` remains required by Flutter's current
Gradle integration, Kotlin 2.4.0 is declared at settings level with
`apply false`, no Hidlins module applies the Kotlin Android plugin, and
dependency validation is not bypassed. `make app-android-config-check` rejects
drift in any of those independent controls.

The Apple Rust bridge remains on the approved CocoaPods/Cargokit path and the
project pins `enable-swift-package-manager: false`. Flutter 3.47.2 still prints
an unconditional future-migration advisory for a local CocoaPods-only plugin;
that message is not a compiler diagnostic and is not represented as warning-free
compilation. Adding a non-functional `Package.swift` merely to suppress it would
misrepresent bridge support. The first-party Xcode compiler policy remains
warning-fatal, and a future SwiftPM migration must preserve the Rust build and
link contract before this exception can be removed.

## Automated acceptance

`make acceptance-evidence-check` validates the machine-readable companion
matrix. It rejects an automatable requirement without a real Make target and
evidence path, a manual status, a skip without a passing automated precursor,
an incomplete artifact/warning inventory, CI/Make drift, Android build-model
drift, or any unclassified one of the 45 July tasks.

Platform-specific suites are documented in
`app/ios/VERIFICATION.md` and `app/android/VERIFICATION.md`. CI runs desktop
builds on their native hosts, iOS on macOS, and the Android API 29/current
phone/tablet matrix on host-native ARM64 and x86_64 runners. Managed MinIO is the
gating network service; live credentialed S3 remains optional and non-gating.

## Explicitly skipped residuals

Manual desktop screen-reader observation, VoiceOver, TalkBack, launcher and
app-switcher OS-shell observation, and physical-device execution are
`SKIPPED — user decision` (physical hardware is also unavailable). Live S3 is
`SKIPPED — user decision / credentials not supplied`. These are not passes.
Their deterministic semantics, focus, scaling, touch-target, concealment,
resource, lifecycle-cover, simulator/emulator, artifact, and managed-MinIO
precursors remain mandatory and are named in the companion JSON.

## Brand provenance

`app/assets/branding/PROVENANCE.md` records the user-supplied archive hash,
canonical SVG and raster masters, separate logo-license posture, every platform
mapping, and the dependency-free update command. Run `make app-brand-generate`
on macOS to regenerate derived committed rasters and `make app-brand-check` on
either host to validate hashes, dimensions, opacity, safe zones, catalogs, and
runtime use.

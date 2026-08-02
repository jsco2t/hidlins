# Android TLS verifier spike harness (T6.2)

**Spike-only** (branch `spike/android-verifier`) — never merged, never
shipped. Proves the two Android TLS claims for the go/no-go memo
(`notebook/.../features/flutter-app/research/android-verifier-spike.md`):

1. `rustls-platform-verifier` initialized through the JNI-exported
   `HidlinsNative.initVerifier` completes a real TLS handshake +
   certificate verification via `hidlins-sync`'s production HTTP stack.
2. A missing init fails immediately with a clear error (no hang), and an
   R8-minified release build keeps working with the documented ProGuard
   keep rules.

## Prerequisites

- `make build-android` run at the worktree root (`ANDROID_NDK_HOME` set) —
  the Gradle build copies `target/aarch64-linux-android/debug/libhidlins_api.so`
  into `jniLibs` at build time (the 110 MB debug artifact is not committed).
- Android SDK with an **arm64-v8a** emulator image — a hard requirement,
  not a suggestion: only the arm64 `.so` is staged into the APK (spike
  used `system-images;android-33;default;arm64-v8a`).
- JDK 17+ (Android Studio's JBR works) and Gradle 9.x (spike validated
  with Gradle **9.4.1**; note `app/android` pins **9.1.0** in its wrapper
  — T8.1 must revalidate under the wrapper's Gradle). Network is required
  for the Gradle plugin portal / Maven pulls (mobile-ecosystem posture
  exception, CONTRIBUTING.md); the verifier AAR itself resolves ONLY from
  the **vendored in-tree** repo at
  `vendor/rustls-platform-verifier-android/maven/` (enforced by an
  `exclusiveContent` filter on the `rustls` group).
- Probe only credential-free URLs. Transport errors are stringified into
  logcat, and `hidlins-sync` error text can embed the URL — a presigned
  S3 URL would write its `X-Amz-Signature` into the log, violating the
  no-secrets-in-logs rule.

## Run

```sh
export ANDROID_HOME=~/Library/Android/sdk
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
gradle assembleDebug assembleRelease

# Boot an emulator, then:
adb install -r app/build/outputs/apk/debug/app-debug.apk

# Positive leg (init → probe):
adb logcat -c   # always clear first — stale SPIKE_RESULT lines from an
                # earlier leg are indistinguishable from this run's
adb shell am start -n app.hidlins.spike/.MainActivity
sleep 15 && adb logcat -d -s HidlinsSpike
# PASS iff: "initVerifier: OK" and "SPIKE_RESULT: OK: HTTP <status> ..."

# Negative control (fresh process, NO init):
adb shell am force-stop app.hidlins.spike
adb logcat -c
adb shell am start -n app.hidlins.spike/.MainActivity --ez skip_init true
sleep 15 && adb logcat -d -s HidlinsSpike
# PASS iff: "initVerifier: SKIPPED" and a prompt "SPIKE_RESULT: PANIC:"
# (or "ERR:") line — an "OK" here means the process was NOT fresh
# (force-stop failed) and the control proved nothing.

# Release leg (R8-minified; must reproduce the positive result):
adb shell am force-stop app.hidlins.spike
adb uninstall app.hidlins.spike
adb install app/build/outputs/apk/release/app-release.apk
adb logcat -c
adb shell am start -n app.hidlins.spike/.MainActivity
sleep 15 && adb logcat -d -s HidlinsSpike
# PASS iff: same "SPIKE_RESULT: OK" line as the debug positive leg.
# (Release installs work unsigned-config-free here only because this
# harness signs release with the debug keystore — a deliberate
# spike-only divergence from app/android, which leaves it unset.)
```

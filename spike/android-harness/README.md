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
- Android SDK with an arm64 emulator image (spike used
  `system-images;android-33;default;arm64-v8a`).
- JDK 17+ (Android Studio's JBR works) and Gradle 9.x on PATH or invoked
  by absolute path. Network is required for the Gradle plugin portal /
  Maven pulls (mobile-ecosystem posture exception, CONTRIBUTING.md); the
  verifier AAR itself resolves from the **vendored in-tree** repo at
  `vendor/rustls-platform-verifier-android/maven/`.

## Run

```sh
export ANDROID_HOME=~/Library/Android/sdk
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
gradle assembleDebug assembleRelease

# Boot an emulator, then:
adb install -r app/build/outputs/apk/debug/app-debug.apk

# Positive leg (init → probe):
adb shell am start -n app.hidlins.spike/.MainActivity
# Negative control (fresh process, NO init):
adb shell am force-stop app.hidlins.spike
adb shell am start -n app.hidlins.spike/.MainActivity --ez skip_init true

# Evidence:
adb logcat -d -s HidlinsSpike
```

Release leg: install `app-release.apk` and repeat the positive run — the
R8-minified build must produce the same `SPIKE_RESULT: OK` line.

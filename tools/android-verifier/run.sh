#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-${HOME:?}/Library/Android/sdk}}"
readonly ADB="$SDK_ROOT/platform-tools/adb"
readonly EMULATOR="$SDK_ROOT/emulator/emulator"
readonly APK="$ROOT/tools/android-verifier/app/build/outputs/apk/release/app-release.apk"
readonly PACKAGE="app.hidlins.verifier"
readonly ACTIVITY="$PACKAGE/.VerifierActivity"
readonly AVD="${HIDLINS_ANDROID_AVD:-hidlins-spike}"
readonly EMULATOR_LOG="$(mktemp)"
started=""

cleanup() {
  "$ADB" uninstall "$PACKAGE" >/dev/null 2>&1 || true
  if [[ -n "$started" ]]; then "$ADB" emu kill >/dev/null 2>&1 || true; fi
  rm -f "$EMULATOR_LOG"
}
trap cleanup EXIT

if ! "$ADB" get-state >/dev/null 2>&1; then
  "$EMULATOR" -avd "$AVD" -no-window -no-audio -no-snapshot-save -wipe-data >"$EMULATOR_LOG" 2>&1 &
  started=1
fi
"$ADB" wait-for-device
for _ in $(seq 1 120); do
  [[ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]] && break
  sleep 1
done
[[ "$("$ADB" shell getprop sys.boot_completed | tr -d '\r')" == 1 ]] || {
  echo "error: Android emulator did not boot" >&2; exit 1;
}

"$ADB" logcat -c
"$ADB" install -r "$APK" >/dev/null
"$ADB" shell am start -W -n "$ACTIVITY" >/dev/null
for _ in $(seq 1 30); do
  if "$ADB" logcat -d -s HidlinsVerifier:I '*:S' | grep -q HIDLINS_VERIFIER_INIT_OK; then
    printf '  OK: release/R8 JNI verifier initialized on emulator %s\n' "$AVD"
    exit 0
  fi
  sleep 1
done
echo "error: verifier success marker missing; sanitized process state follows" >&2
"$ADB" shell dumpsys activity processes | grep -F "$PACKAGE" >&2 || true
exit 1

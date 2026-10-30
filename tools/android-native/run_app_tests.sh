#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "$ROOT/tools/android-native/instrumentation-result.sh"
export ANDROID_AVD_HOME="${ANDROID_AVD_HOME:-${ANDROID_EMULATOR_HOME:-${ANDROID_USER_HOME:-${HOME:?}/.android}}/avd}"
readonly SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-${HOME:?}/Library/Android/sdk}}"
readonly ADB="$SDK_ROOT/platform-tools/adb"
readonly EMULATOR="$SDK_ROOT/emulator/emulator"
readonly APP_APK="$ROOT/app/build/app/outputs/apk/debug/app-debug.apk"
readonly TEST_APK="$ROOT/app/build/app/outputs/apk/androidTest/debug/app-debug-androidTest.apk"
readonly AVD="${HIDLINS_ANDROID_AVD:-hidlins-spike}"
readonly EXPECTED_API="${HIDLINS_ANDROID_EXPECTED_API:-}"
readonly EXPECTED_ABI="${HIDLINS_ANDROID_EXPECTED_ABI:-}"
readonly TEST_COMPONENT="app.hidlins.test/app.hidlins.HidlinsAndroidTest"
readonly EMULATOR_LOG="$(mktemp)"
started=""

cleanup() {
  "$ADB" uninstall app.hidlins.test >/dev/null 2>&1 || true
  "$ADB" uninstall app.hidlins >/dev/null 2>&1 || true
  if [[ -n "$started" ]]; then "$ADB" emu kill >/dev/null 2>&1 || true; fi
  rm -f "$EMULATOR_LOG"
}
trap cleanup EXIT

tools/android-native/gradle.sh -Ptarget-platform=android-arm64,android-x64 \
  :app:assembleDebug :app:assembleDebugAndroidTest
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
  echo "error: Android emulator did not boot" >&2
  exit 1
}
actual_api="$("$ADB" shell getprop ro.build.version.sdk | tr -d '\r')"
actual_abi="$("$ADB" shell getprop ro.product.cpu.abi | tr -d '\r')"
if [[ -n "$EXPECTED_API" && "$actual_api" != "$EXPECTED_API" ]]; then
  echo "error: expected Android API $EXPECTED_API, connected device is API $actual_api" >&2
  exit 1
fi
if [[ -n "$EXPECTED_ABI" && "$actual_abi" != "$EXPECTED_ABI" ]]; then
  echo "error: expected Android ABI $EXPECTED_ABI, connected device is $actual_abi" >&2
  exit 1
fi

"$ADB" install -r "$APP_APK" >/dev/null
"$ADB" install -r "$TEST_APK" >/dev/null
# Headless guests may still be asleep or behind the default swipe keyguard.
"$ADB" shell input keyevent KEYCODE_WAKEUP
"$ADB" shell wm dismiss-keyguard
if result="$(python3 "$ROOT/tools/android-native/run_with_timeout.py" \
    "${HIDLINS_ANDROID_INSTRUMENTATION_TIMEOUT_SECONDS:-300}" \
    "$ADB" shell am instrument -w -r "$TEST_COMPONENT" | tr -d '\r')"; then
  instrumentation_status=0
else
  instrumentation_status=$?
fi
printf '%s\n' "$result"
if [[ "$instrumentation_status" != 0 ]]; then
  hidlins_device_ui_diagnostics "$ADB" >&2
  echo "error: Hidlins Android instrumentation command failed" >&2
  exit "$instrumentation_status"
fi
if ! hidlins_device_tests_passed "$result"; then
  hidlins_device_ui_diagnostics "$ADB" >&2
  echo "error: Hidlins Android instrumentation did not report success" >&2
  exit 1
fi
printf '  OK: Android lifecycle, clipboard, storage, SAF, backup, and sync-permission assertions passed on %s (API %s, ABI %s)\n' "$AVD" "$actual_api" "$actual_abi"

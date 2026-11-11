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
readonly BOOT_TIMEOUT_SECONDS="${HIDLINS_ANDROID_BOOT_TIMEOUT_SECONDS:-120}"
readonly ADB_TIMEOUT_SECONDS="${HIDLINS_ANDROID_ADB_TIMEOUT_SECONDS:-15}"
readonly EMULATOR_PORT="${HIDLINS_ANDROID_EMULATOR_PORT:-5554}"
started_pid=""
installed_app=0
installed_tests=0

adb_bounded() {
  python3 "$ROOT/tools/android-native/run_with_timeout.py" \
    "$ADB_TIMEOUT_SECONDS" "$ADB" "$@"
}

cleanup() {
  if [[ "$installed_tests" == 1 ]]; then
    adb_bounded uninstall app.hidlins.test >/dev/null 2>&1 || true
  fi
  if [[ "$installed_app" == 1 ]]; then
    adb_bounded uninstall app.hidlins >/dev/null 2>&1 || true
  fi
  if [[ -n "$started_pid" ]]; then
    if kill -0 "$started_pid" 2>/dev/null; then
      kill -TERM "$started_pid" 2>/dev/null || true
      for _ in $(seq 1 20); do
        if ! kill -0 "$started_pid" 2>/dev/null; then break; fi
        sleep 0.1
      done
      if kill -0 "$started_pid" 2>/dev/null; then
        kill -KILL "$started_pid" 2>/dev/null || true
      fi
    fi
    wait "$started_pid" >/dev/null 2>&1 || true
  fi
  rm -f "$EMULATOR_LOG"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

[[ "$BOOT_TIMEOUT_SECONDS" =~ ^[1-9][0-9]{0,4}$ ]] || {
  echo "error: Android boot timeout must be a positive integer below 100000 seconds" >&2
  exit 1
}
[[ "$ADB_TIMEOUT_SECONDS" =~ ^([0-9]+([.][0-9]+)?|[.][0-9]+)$ &&
   "$ADB_TIMEOUT_SECONDS" =~ [1-9] ]] || {
  echo "error: Android adb timeout must be a positive number of seconds" >&2
  exit 1
}
[[ "$EMULATOR_PORT" =~ ^[1-9][0-9]{3,4}$ ]] &&
  (( EMULATOR_PORT >= 1024 && EMULATOR_PORT <= 65534 && EMULATOR_PORT % 2 == 0 )) || {
  echo "error: Android emulator port must be an even port from 1024 through 65534" >&2
  exit 1
}

require_running_emulator() {
  if [[ -n "$started_pid" ]] && ! kill -0 "$started_pid" 2>/dev/null; then
    echo "error: Android emulator exited during startup: $AVD" >&2
    # Fresh boot output precedes loading any Hidlins app or vault.
    cat "$EMULATOR_LOG" >&2
    exit 1
  fi
}

tools/android-native/gradle.sh -Ptarget-platform=android-arm64,android-x64 \
  :app:assembleDebug :app:assembleDebugAndroidTest
# Resolve an unambiguous existing device before considering an owned emulator.
if [[ -z "${ANDROID_SERIAL:-}" ]]; then
  devices="$(adb_bounded devices 2>/dev/null || true)"
  connected="$(awk 'NR > 1 && $2 == "device" {print $1}' <<<"$devices")"
  count="$(awk 'NF {n += 1} END {print n + 0}' <<<"$connected")"
  if (( count > 1 )); then
    echo "error: multiple Android devices are connected; set ANDROID_SERIAL" >&2
    exit 1
  fi
  if [[ -n "$connected" ]]; then export ANDROID_SERIAL="$connected"; fi
fi
if ! adb_bounded get-state >/dev/null 2>&1; then
  if [[ -n "${ANDROID_SERIAL:-}" && "$ANDROID_SERIAL" != "emulator-$EMULATOR_PORT" ]]; then
    echo "error: selected Android device is unavailable: $ANDROID_SERIAL" >&2
    exit 1
  fi
  export ANDROID_SERIAL="emulator-$EMULATOR_PORT"
  "$EMULATOR" -avd "$AVD" -port "$EMULATOR_PORT" -no-window -no-audio \
    -gpu swiftshader -no-snapshot-save -wipe-data >"$EMULATOR_LOG" 2>&1 &
  started_pid=$!
fi
booted=0
startup_deadline=$((SECONDS + BOOT_TIMEOUT_SECONDS))
while (( SECONDS < startup_deadline )); do
  require_running_emulator
  if [[ "$(adb_bounded shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]]; then
    booted=1
    break
  fi
  sleep 0.2
done
require_running_emulator
if [[ "$booted" != 1 ]]; then
  echo "error: Android emulator did not boot within $BOOT_TIMEOUT_SECONDS seconds" >&2
  if [[ -n "$started_pid" ]]; then cat "$EMULATOR_LOG" >&2; fi
  exit 1
fi
actual_api="$(adb_bounded shell getprop ro.build.version.sdk | tr -d '\r')"
actual_abi="$(adb_bounded shell getprop ro.product.cpu.abi | tr -d '\r')"
if [[ -n "$EXPECTED_API" && "$actual_api" != "$EXPECTED_API" ]]; then
  echo "error: expected Android API $EXPECTED_API, connected device is API $actual_api" >&2
  exit 1
fi
if [[ -n "$EXPECTED_ABI" && "$actual_abi" != "$EXPECTED_ABI" ]]; then
  echo "error: expected Android ABI $EXPECTED_ABI, connected device is $actual_abi" >&2
  exit 1
fi

adb_bounded install -r "$APP_APK" >/dev/null
installed_app=1
adb_bounded install -r "$TEST_APK" >/dev/null
installed_tests=1
# Headless guests may still be asleep or behind the default swipe keyguard.
adb_bounded shell input keyevent KEYCODE_WAKEUP
adb_bounded shell wm dismiss-keyguard
if result="$(python3 "$ROOT/tools/android-native/run_with_timeout.py" \
    "${HIDLINS_ANDROID_INSTRUMENTATION_TIMEOUT_SECONDS:-300}" \
    "$ADB" shell am instrument -w -r "$TEST_COMPONENT" | tr -d '\r')"; then
  instrumentation_status=0
else
  instrumentation_status=$?
fi
printf '%s\n' "$result"
if [[ "$instrumentation_status" != 0 ]]; then
  hidlins_device_ui_diagnostics adb_bounded >&2
  echo "error: Hidlins Android instrumentation command failed" >&2
  exit "$instrumentation_status"
fi
if ! hidlins_device_tests_passed "$result"; then
  hidlins_device_ui_diagnostics adb_bounded >&2
  echo "error: Hidlins Android instrumentation did not report success" >&2
  exit 1
fi
printf '  OK: Android lifecycle, clipboard, storage, SAF, backup, and sync-permission assertions passed on %s (API %s, ABI %s)\n' "$AVD" "$actual_api" "$actual_abi"

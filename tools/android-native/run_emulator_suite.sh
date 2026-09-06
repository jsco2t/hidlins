#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly APP_DIR="$ROOT/app"
readonly SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-${HOME:?}/Library/Android/sdk}}"
readonly ADB="$SDK_ROOT/platform-tools/adb"
readonly EMULATOR="$SDK_ROOT/emulator/emulator"
readonly MODE="${1:-core}"
readonly RESULTS_DIR="${HIDLINS_ANDROID_RESULTS_DIR:-$ROOT/build/verification/android}"
readonly SHIPPING_DEBUG_APK="$RESULTS_DIR/app-debug.apk"
readonly MATRIX="$(mktemp "${TMPDIR:-/tmp}/hidlins-android-matrix.XXXXXX")"
readonly SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/hidlins-android-suite.XXXXXX")"
CURRENT_SERIAL=""
CURRENT_PID=""
CURRENT_DEFINES=""

cleanup_emulator() {
  if [[ -n "$CURRENT_SERIAL" ]]; then
    "$ADB" -s "$CURRENT_SERIAL" uninstall app.hidlins.test >/dev/null 2>&1 || true
    "$ADB" -s "$CURRENT_SERIAL" uninstall app.hidlins >/dev/null 2>&1 || true
    "$ADB" -s "$CURRENT_SERIAL" emu kill >/dev/null 2>&1 || true
  fi
  if [[ -n "$CURRENT_PID" ]]; then wait "$CURRENT_PID" >/dev/null 2>&1 || true; fi
  CURRENT_SERIAL=""
  CURRENT_PID=""
  CURRENT_DEFINES=""
}

cleanup() {
  cleanup_emulator
  rm -f "$MATRIX"
  case "$SCRATCH" in
    "${TMPDIR:-/tmp}"/hidlins-android-suite.*) rm -rf "$SCRATCH" ;;
    *) echo "error: refusing to remove unexpected Android scratch directory" >&2 ;;
  esac
}
trap cleanup EXIT

case "$MODE" in
  core | minio | real) ;;
  *) echo "usage: $0 {core|minio|real}" >&2; exit 2 ;;
esac

if [[ "$MODE" == "real" && -z "${HIDLINS_ANDROID_S3_CONFIG:-}" ]]; then
  echo "SKIPPED — user decision / credentials not supplied: Android live S3 emulator run"
  exit 0
fi

mkdir -p "$RESULTS_DIR"
chmod 700 "$RESULTS_DIR"
cp "$APP_DIR/build/app/outputs/apk/debug/app-debug.apk" "$SHIPPING_DEBUG_APK"
python3 "$ROOT/tools/android-native/emulator_matrix.py" select >"$MATRIX"

connected_count="$($ADB devices | awk 'NR > 1 && $2 == "device" {count += 1} END {print count + 0}')"
if [[ "$connected_count" != 0 ]]; then
  echo "error: Android matrix requires no pre-connected device so runtime identity is unambiguous" >&2
  exit 1
fi

run_logged() {
  local name="$1"
  shift
  local raw="$SCRATCH/$name.raw.log"
  local saved="$RESULTS_DIR/$name.log"
  chmod 700 "$SCRATCH"
  set +e
  "$@" >"$raw" 2>&1
  local status=$?
  set -e
  local log_status=0
  if [[ -n "$CURRENT_DEFINES" ]]; then
    python3 "$ROOT/tools/android-native/redact_log.py" "$raw" "$saved" \
      --defines "$CURRENT_DEFINES" --require-passed || log_status=$?
  else
    python3 "$ROOT/tools/android-native/redact_log.py" "$raw" "$saved" \
      --require-passed || log_status=$?
  fi
  cat "$saved"
  if [[ $status -ne 0 ]]; then
    echo "error: Android $name failed; redacted log: $saved" >&2
    return "$status"
  fi
  if [[ $log_status -ne 0 ]]; then
    echo "error: Android $name reported a device-test failure; redacted log: $saved" >&2
    return "$log_status"
  fi
}

boot_emulator() {
  local avd="$1"
  local expected_api="$2"
  local expected_abi="$3"
  if ! "$EMULATOR" -list-avds | grep -Fxq "$avd"; then
    echo "error: required AVD is not installed: $avd" >&2
    exit 1
  fi
  local emulator_log="$RESULTS_DIR/$MODE-$avd-emulator.log"
  "$EMULATOR" -avd "$avd" -no-window -no-audio -no-snapshot -wipe-data \
    -no-boot-anim >"$emulator_log" 2>&1 &
  CURRENT_PID=$!
  for _ in $(seq 1 180); do
    CURRENT_SERIAL="$($ADB devices | awk 'NR > 1 && $1 ~ /^emulator-/ && $2 == "device" {print $1; exit}')"
    if [[ -n "$CURRENT_SERIAL" ]] && \
       [[ "$($ADB -s "$CURRENT_SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]]; then
      break
    fi
    sleep 1
  done
  if [[ -z "$CURRENT_SERIAL" ]]; then
    echo "error: Android emulator did not become available: $avd" >&2
    exit 1
  fi
  local actual_api actual_abi actual_avd
  actual_api="$($ADB -s "$CURRENT_SERIAL" shell getprop ro.build.version.sdk | tr -d '\r')"
  actual_abi="$($ADB -s "$CURRENT_SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
  actual_avd="$($ADB -s "$CURRENT_SERIAL" emu avd name | sed -n '1p' | tr -d '\r')"
  if [[ "$actual_api" != "$expected_api" || "$actual_abi" != "$expected_abi" || "$actual_avd" != "$avd" ]]; then
    echo "error: Android runtime identity mismatch: expected $avd/API $expected_api/$expected_abi, got $actual_avd/API $actual_api/$actual_abi" >&2
    exit 1
  fi
  echo "Android emulator: avd=$actual_avd api=$actual_api abi=$actual_abi serial=$CURRENT_SERIAL"
}

exercise_os_lifecycle() {
  "$ADB" -s "$CURRENT_SERIAL" install -r \
    "$SHIPPING_DEBUG_APK" || return 1
  "$ADB" -s "$CURRENT_SERIAL" shell am start -W -n app.hidlins/.MainActivity || return 1
  local before
  before="$($ADB -s "$CURRENT_SERIAL" shell pidof app.hidlins | tr -d '\r')"
  [[ -n "$before" ]] || return 1
  "$ADB" -s "$CURRENT_SERIAL" shell input keyevent KEYCODE_HOME || return 1
  "$ADB" -s "$CURRENT_SERIAL" shell am start -W -n app.hidlins/.MainActivity || return 1
  [[ -n "$($ADB -s "$CURRENT_SERIAL" shell pidof app.hidlins | tr -d '\r')" ]] || return 1
  "$ADB" -s "$CURRENT_SERIAL" shell am force-stop app.hidlins || return 1
  for _ in $(seq 1 50); do
    if [[ -z "$($ADB -s "$CURRENT_SERIAL" shell pidof app.hidlins | tr -d '\r')" ]]; then
      break
    fi
    sleep 0.1
  done
  [[ -z "$($ADB -s "$CURRENT_SERIAL" shell pidof app.hidlins | tr -d '\r')" ]] || return 1
  "$ADB" -s "$CURRENT_SERIAL" shell am start -W -n app.hidlins/.MainActivity || return 1
  local after
  after="$($ADB -s "$CURRENT_SERIAL" shell pidof app.hidlins | tr -d '\r')"
  [[ -n "$after" && "$after" != "$before" ]] || return 1
  echo "  OK: Android background/resume and process recreation relaunched Hidlins"
}

exercise_airplane_recovery() {
  "$ADB" -s "$CURRENT_SERIAL" shell cmd connectivity airplane-mode enable || return 1
  [[ "$($ADB -s "$CURRENT_SERIAL" shell settings get global airplane_mode_on | tr -d '\r')" == 1 ]] || return 1
  "$ADB" -s "$CURRENT_SERIAL" shell cmd connectivity airplane-mode disable || return 1
  [[ "$($ADB -s "$CURRENT_SERIAL" shell settings get global airplane_mode_on | tr -d '\r')" == 0 ]] || return 1
  "$ADB" -s "$CURRENT_SERIAL" shell cmd connectivity airplane-mode >/dev/null || return 1
  echo "  OK: Android airplane mode transitioned offline and recovered"
}

setup_defines() {
  local avd="$1"
  CURRENT_DEFINES="$SCRATCH/$MODE-$avd-defines.json"
  if [[ "$MODE" == "minio" ]]; then
    local bucket="hidlins-android-$PPID-$RANDOM"
    "$ROOT/tools/sync-tests/fixtures/make_bucket.sh" "$bucket"
    python3 "$ROOT/tools/ios-native/secure_s3_config.py" minio \
      "$ROOT/tools/sync-tests/fixtures/.minio-env" "$CURRENT_DEFINES" \
      --bucket "$bucket" --key-prefix hidlins-android-alpha --endpoint-host 10.0.2.2
  else
    python3 "$ROOT/tools/ios-native/secure_s3_config.py" real \
      "$HIDLINS_ANDROID_S3_CONFIG" "$CURRENT_DEFINES" \
      --key-prefix hidlins-android-alpha
  fi
}

while IFS=$'\t' read -r avd api abi form_factor system_image <&3; do
  echo "Android matrix entry: avd=$avd api=$api abi=$abi form_factor=$form_factor image=$system_image"
  boot_emulator "$avd" "$api" "$abi"
  logs=()
  if [[ "$MODE" == "core" ]]; then
    native_name="core-$api-$abi-native"
    run_logged "$native_name" env \
      HIDLINS_ANDROID_AVD="$avd" HIDLINS_ANDROID_EXPECTED_API="$api" \
      HIDLINS_ANDROID_EXPECTED_ABI="$abi" "$ROOT/tools/android-native/run_app_tests.sh"
    logs+=("native=$RESULTS_DIR/$native_name.log")
    ui_name="core-$api-$abi-ui"
    (cd "$APP_DIR" && run_logged "$ui_name" python3 \
      "$ROOT/tools/android-native/run_with_timeout.py" 240 \
      flutter drive --no-pub -d "$CURRENT_SERIAL" \
      --timeout=180 \
      --driver=test_driver/integration_test.dart \
      --target=integration_test/android_emulator_ui_test.dart \
      --android-project-arg=target-platform=android-arm64,android-x64)
    logs+=("ui=$RESULTS_DIR/$ui_name.log")
    lifecycle_name="core-$api-$abi-os-lifecycle"
    run_logged "$lifecycle_name" exercise_os_lifecycle
    logs+=("os-lifecycle=$RESULTS_DIR/$lifecycle_name.log")
    bridge_name="core-$api-$abi-bridge"
    (cd "$APP_DIR" && run_logged "$bridge_name" python3 \
      "$ROOT/tools/android-native/run_with_timeout.py" 240 \
      flutter drive --no-pub -d "$CURRENT_SERIAL" \
      --timeout=180 \
      --driver=test_driver/integration_test.dart \
      --target=integration_test/android_real_bridge_test.dart \
      --android-project-arg=target-platform=android-arm64,android-x64)
    logs+=("bridge=$RESULTS_DIR/$bridge_name.log")
  else
    setup_defines "$avd"
    airplane_name="$MODE-$api-$abi-airplane"
    run_logged "$airplane_name" exercise_airplane_recovery
    logs+=("airplane=$RESULTS_DIR/$airplane_name.log")
    s3_name="$MODE-$api-$abi-sync"
    (cd "$APP_DIR" && run_logged "$s3_name" python3 \
      "$ROOT/tools/android-native/run_with_timeout.py" 480 \
      flutter drive --no-pub -d "$CURRENT_SERIAL" \
      --timeout=420 \
      --driver=test_driver/integration_test.dart \
      --target=integration_test/android_real_bridge_s3_test.dart \
      --android-project-arg=target-platform=android-arm64,android-x64 \
      --dart-define-from-file="$CURRENT_DEFINES" \
    )
    logs+=("sync=$RESULTS_DIR/$s3_name.log")
  fi

  model="$($ADB -s "$CURRENT_SERIAL" shell getprop ro.product.model | tr -d '\r')"
  record_args=(
    --output "$RESULTS_DIR/$MODE-api$api-$abi.json"
    --suite "$MODE" --avd "$avd" --serial "$CURRENT_SERIAL"
    --api "$api" --abi "$abi" --form-factor "$form_factor" --device-model "$model"
    --artifact "debug=$SHIPPING_DEBUG_APK"
    --artifact "release=$ROOT/app/build/app/outputs/apk/release/app-release-unsigned.apk"
  )
  for log in "${logs[@]}"; do record_args+=(--log "$log"); done
  python3 "$ROOT/tools/android-native/record_result.py" "${record_args[@]}"
  cleanup_emulator
done 3<"$MATRIX"

echo "  OK: Android $MODE matrix passed; manifests: $RESULTS_DIR/$MODE-api*.json"

#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
app_dir="$repo_root/app"
device_json="$(mktemp "${TMPDIR:-/tmp}/hidlins-ios-devices.XXXXXX")"
matrix="$(mktemp "${TMPDIR:-/tmp}/hidlins-ios-matrix.XXXXXX")"
trap 'rm -f "$device_json" "$matrix"' EXIT

xcrun simctl list --json devices available >"$device_json"
python3 "$repo_root/tools/ios-native/select_simulators.py" "$device_json" >"$matrix"

while IFS=$'\t' read -r udid name runtime; do
  (
  started=0
  if ! xcrun simctl spawn "$udid" /usr/bin/true >/dev/null 2>&1; then
    started=1
  fi
  cleanup_device() {
    if [[ "${HIDLINS_MOBILE_SYNC_SCENARIOS:-0}" == 1 ]]; then
      xcrun simctl uninstall "$udid" app.hidlins.ios >/dev/null 2>&1 || true
    fi
    if [[ "$started" == 1 ]]; then
      xcrun simctl shutdown "$udid" >/dev/null 2>&1 || true
    fi
  }
  trap cleanup_device EXIT INT TERM
  echo "iOS integration simulator: name=$name runtime=iOS $runtime id=$udid"
  xcrun simctl boot "$udid" >/dev/null 2>&1 || true
  xcrun simctl bootstatus "$udid" -b
  (
    cd "$app_dir"
    flutter test --no-pub -d "$udid" integration_test/ios_simulator_ui_test.dart
  )
  (
    cd "$app_dir"
    flutter test --no-pub -d "$udid" integration_test/ios_real_bridge_test.dart
  )
  if [[ "${HIDLINS_MOBILE_SYNC_SCENARIOS:-0}" == 1 ]]; then
    python3 "$repo_root/tools/local-sync-tests/mobile_scenario.py" ios \
      --device "$udid" --device-name "$name" --runtime "iOS $runtime" \
      --artifact "$app_dir/build/ios/iphonesimulator/Runner.app/Runner"
  fi
  )
done <"$matrix"

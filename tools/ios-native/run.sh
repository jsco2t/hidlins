#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
app_dir="$repo_root/app"
ios_dir="$app_dir/ios"
simulator_destination="${IOS_SIMULATOR_DESTINATION:-platform=iOS Simulator,name=iPhone 17 Pro,OS=26.5}"

prepare_simulator_config() {
  # `flutter test -d <simulator>` rewrites Generated.xcconfig to point at a
  # temporary listener.dart and deletes that file when it exits. Native XCTest
  # must therefore restore a stable application target before invoking Xcode,
  # regardless of which repository target ran immediately beforehand.
  (cd "$app_dir" && flutter build ios --config-only --simulator --debug --no-pub)
}

run_simulator_tests() {
  local derived_data_dir
  derived_data_dir="$(mktemp -d "${TMPDIR:-/tmp}/hidlins-ios-xctest-derived.XXXXXX")"
  trap 'rm -rf "$derived_data_dir"' RETURN
  echo "iOS simulator: $simulator_destination"
  prepare_simulator_config
  xcodebuild test -quiet \
    -workspace "$ios_dir/Runner.xcworkspace" \
    -scheme Runner \
    -destination "$simulator_destination" \
    -derivedDataPath "$derived_data_dir" \
    CODE_SIGNING_ALLOWED=NO
}

run_device_tests() {
  local device_json device_line device_id device_name device_os
  device_json="$(mktemp "${TMPDIR:-/tmp}/hidlins-xcdevice.XXXXXX")"
  trap 'rm -f "$device_json"' RETURN
  xcrun xcdevice list --timeout 5 >"$device_json"
  device_line="$(python3 "$repo_root/tools/ios-native/select_device.py" "$device_json")"
  IFS=$'\t' read -r device_id device_name device_os <<<"$device_line"
  : "${IOS_DEVELOPMENT_TEAM:?set IOS_DEVELOPMENT_TEAM to the Apple development team used for the connected device}"
  echo "iOS physical device: $device_name, iOS $device_os, id $device_id"
  xcodebuild test -quiet \
    -workspace "$ios_dir/Runner.xcworkspace" \
    -scheme Runner \
    -destination "id=$device_id" \
    DEVELOPMENT_TEAM="$IOS_DEVELOPMENT_TEAM" \
    CODE_SIGN_STYLE=Automatic \
    -only-testing:RunnerTests
}

case "${1:-}" in
  check)
    cargo check --offline --locked -p hidlins-api --no-default-features --target aarch64-apple-ios
    cargo check --offline --locked -p hidlins-api --no-default-features --target aarch64-apple-ios-sim
    run_simulator_tests
    ;;
  simulator)
    run_simulator_tests
    ;;
  device)
    run_device_tests
    ;;
  build)
    echo "iOS simulator artifact: Flutter debug, no signing"
    (cd "$app_dir" && flutter build ios --simulator --debug --no-pub)
    python3 "$repo_root/tools/ios-native/verify_artifact.py" \
      "$app_dir/build/ios/iphonesimulator/Runner.app"
    echo "iOS device artifact: Flutter release, no signing"
    (cd "$app_dir" && flutter build ios --release --no-pub --no-codesign)
    python3 "$repo_root/tools/ios-native/verify_artifact.py" \
      "$app_dir/build/ios/iphoneos/Runner.app"
    ;;
  *)
    echo "usage: $0 {check|simulator|device|build}" >&2
    exit 2
    ;;
esac

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

run_simulator_tests() (
  local derived_data_dir
  derived_data_dir="$(mktemp -d "${TMPDIR:-/tmp}/hidlins-ios-xctest-derived.XXXXXX")"
  trap 'rm -rf "$derived_data_dir"' EXIT
  echo "iOS simulator: $simulator_destination"
  prepare_simulator_config
  xcodebuild test \
    -workspace "$ios_dir/Runner.xcworkspace" \
    -scheme Runner \
    -destination "$simulator_destination" \
    -derivedDataPath "$derived_data_dir" \
    CODE_SIGNING_ALLOWED=NO
)

run_publication_tests() (
  local derived_data_dir
  # Keep the dedicated test-only artifact separate from app/build packaging.
  # This also retains diagnostic results and avoids cold Rust rebuilds.
  derived_data_dir="$repo_root/.codex-tmp/ios-publication"
  mkdir -p "$derived_data_dir"
  prepare_simulator_config
  # Preserve configured compiler flags, including Cargo's encoded precedence.
  # This environment is confined to the dedicated debug-simulator artifact.
  CARGO_ENCODED_RUSTFLAGS="$(python3 - <<'PYFLAGS'
import os, shlex
encoded = os.environ.get('CARGO_ENCODED_RUSTFLAGS')
flags = encoded.split('\x1f') if encoded is not None else shlex.split(os.environ.get('RUSTFLAGS', ''))
flags.extend(['-D', 'warnings', '--cfg', 'hidlins_ios_publication_fixture'])
print('\x1f'.join(flags), end='')
PYFLAGS
)"
  export CARGO_ENCODED_RUSTFLAGS
  xcodebuild test \
    -workspace "$ios_dir/Runner.xcworkspace" \
    -scheme Runner -configuration Debug \
    -destination "$simulator_destination" \
    -derivedDataPath "$derived_data_dir" \
    -parallel-testing-enabled NO \
    -only-testing:RunnerTests/NativePublicationTests \
    'SWIFT_ACTIVE_COMPILATION_CONDITIONS=$(inherited) HIDLINS_IOS_PUBLICATION_FIXTURE' \
    'GCC_PREPROCESSOR_DEFINITIONS=$(inherited) HIDLINS_IOS_PUBLICATION_FIXTURE=1' \
    DEAD_CODE_STRIPPING=NO CODE_SIGNING_ALLOWED=NO
  python3 "$repo_root/tools/ios-native/publication_boundary.py" present \
    "$derived_data_dir/Build/Products/Debug-iphonesimulator/Runner.app/Runner"
)

run_device_tests() {
  local device_json device_line device_id device_name device_os
  device_json="$(mktemp "${TMPDIR:-/tmp}/hidlins-xcdevice.XXXXXX")"
  trap 'rm -f "$device_json"' RETURN
  xcrun xcdevice list --timeout 5 >"$device_json"
  device_line="$(python3 "$repo_root/tools/ios-native/select_device.py" "$device_json")"
  IFS=$'\t' read -r device_id device_name device_os <<<"$device_line"
  : "${IOS_DEVELOPMENT_TEAM:?set IOS_DEVELOPMENT_TEAM to the Apple development team used for the connected device}"
  echo "iOS physical device: $device_name, iOS $device_os, id $device_id"
  xcodebuild test \
    -workspace "$ios_dir/Runner.xcworkspace" \
    -scheme Runner \
    -destination "id=$device_id" \
    DEVELOPMENT_TEAM="$IOS_DEVELOPMENT_TEAM" \
    CODE_SIGN_STYLE=Automatic \
    -only-testing:RunnerTests
}

case "${1:-}" in
  check)
    python3 "$repo_root/tools/dev/cargo_artifacts.py" run cargo -- check --offline --locked -p hidlins-api --no-default-features --target aarch64-apple-ios
    python3 "$repo_root/tools/dev/cargo_artifacts.py" run cargo -- check --offline --locked -p hidlins-api --no-default-features --target aarch64-apple-ios-sim
    run_simulator_tests
    ;;
  simulator)
    run_simulator_tests
    ;;
  publication)
    run_publication_tests
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
    echo "usage: $0 {check|simulator|publication|device|build}" >&2
    exit 2
    ;;
esac

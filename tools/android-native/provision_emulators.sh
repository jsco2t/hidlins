#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-${HOME:?}/Library/Android/sdk}}"
readonly SDKMANAGER="${SDKMANAGER:-$SDK_ROOT/cmdline-tools/latest/bin/sdkmanager}"
readonly AVDMANAGER="${AVDMANAGER:-$SDK_ROOT/cmdline-tools/latest/bin/avdmanager}"
readonly EMULATOR="$SDK_ROOT/emulator/emulator"
readonly MATRIX="$(mktemp "${TMPDIR:-/tmp}/hidlins-android-provision.XXXXXX")"
trap 'rm -f "$MATRIX"' EXIT

for tool in "$SDKMANAGER" "$AVDMANAGER"; do
  if [[ ! -x "$tool" ]]; then
    echo "error: Android command-line tool is unavailable: $tool" >&2
    exit 1
  fi
done

python3 "$ROOT/tools/android-native/emulator_matrix.py" select >"$MATRIX"

packages=("platform-tools" "emulator" "ndk;28.2.13676358")
while IFS=$'\t' read -r _ _ _ _ system_image; do
  packages+=("$system_image")
done <"$MATRIX"
"$SDKMANAGER" "${packages[@]}"

while IFS=$'\t' read -r avd _ _ form_factor system_image; do
  if [[ -x "$EMULATOR" ]] && "$EMULATOR" -list-avds | grep -Fxq "$avd"; then
    echo "Android AVD already installed: $avd"
    continue
  fi
  device="pixel_2"
  if [[ "$form_factor" == "tablet" ]]; then device="pixel_c"; fi
  printf 'no\n' | "$AVDMANAGER" create avd --force --name "$avd" \
    --package "$system_image" --device "$device"
  echo "Android AVD installed: $avd ($system_image, $device)"
done <"$MATRIX"

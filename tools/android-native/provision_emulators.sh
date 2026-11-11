#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly SDK_ROOT="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-${HOME:?}/Library/Android/sdk}}"
readonly SDKMANAGER="${SDKMANAGER:-$SDK_ROOT/cmdline-tools/latest/bin/sdkmanager}"
readonly AVDMANAGER="${AVDMANAGER:-$SDK_ROOT/cmdline-tools/latest/bin/avdmanager}"
readonly EMULATOR="$SDK_ROOT/emulator/emulator"
export ANDROID_AVD_HOME="${ANDROID_AVD_HOME:-${ANDROID_EMULATOR_HOME:-${ANDROID_USER_HOME:-${HOME:?}/.android}}/avd}"
mkdir -p "$ANDROID_AVD_HOME"
readonly MATRIX="$(mktemp "${TMPDIR:-/tmp}/hidlins-android-provision.XXXXXX")"
readonly AUTHORITY="$(mktemp "${TMPDIR:-/tmp}/hidlins-android-authority.XXXXXX")"
readonly REPLACEMENT="$(mktemp "${TMPDIR:-/tmp}/hidlins-android-replacement.XXXXXX")"
trap 'rm -f "$MATRIX" "$AUTHORITY" "$REPLACEMENT"' EXIT

# macOS exposes /usr/bin/java even when no system JRE is installed. Prefer the
# JDK bundled with Android Studio, matching the JDK Flutter itself reports and
# avoiding a separate workstation dependency. Linux CI supplies JAVA_HOME.
if [[ -z "${JAVA_HOME:-}" && -x "/Applications/Android Studio.app/Contents/jbr/Contents/Home/bin/java" ]]; then
  export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
  export PATH="$JAVA_HOME/bin:$PATH"
fi
if ! java -version >/dev/null 2>&1; then
  echo "error: Java is required for Android SDK/AVD provisioning" >&2
  echo "       install Android Studio or set JAVA_HOME to a working JDK" >&2
  exit 1
fi

for tool in "$SDKMANAGER" "$AVDMANAGER"; do
  if [[ ! -x "$tool" ]]; then
    echo "error: Android command-line tool is unavailable: $tool" >&2
    exit 1
  fi
done

python3 "$ROOT/tools/android-native/emulator_matrix.py" clients >"$MATRIX"
python3 "$ROOT/tools/android-native/emulator_matrix.py" authority >"$AUTHORITY"
python3 "$ROOT/tools/android-native/emulator_matrix.py" replacement >"$REPLACEMENT"

# Provisioning is an ensure operation, not an update operation. Avoid contacting
# Google's repository when every pinned package and AVD is already present;
# this keeps repeat scenario runs deterministic and usable offline.
# Assignment values do not split; avoid Bash 3.2 retaining the escaped slash
# inside a double-quoted pattern replacement.
packages_installed=1
avds_installed=1
[[ -x "$SDK_ROOT/platform-tools/adb" && -x "$EMULATOR" \
   && -d "$SDK_ROOT/ndk/28.2.13676358" ]] || packages_installed=0
while IFS=$'\t' read -r avd _ _ _ system_image; do
  image_path=${system_image//;/\/}
  [[ -d "$SDK_ROOT/$image_path" ]] || packages_installed=0
  "$EMULATOR" -list-avds 2>/dev/null | grep -Fx "$avd" >/dev/null || avds_installed=0
done <"$MATRIX"
while IFS=$'\t' read -r avd _ _ _ system_image; do
  image_path=${system_image//;/\/}
  [[ -d "$SDK_ROOT/$image_path" ]] || packages_installed=0
  "$EMULATOR" -list-avds 2>/dev/null | grep -Fx "$avd" >/dev/null || avds_installed=0
done <"$AUTHORITY"
while IFS=$'\t' read -r avd _ _ _ system_image; do
  image_path=${system_image//;/\/}
  [[ -d "$SDK_ROOT/$image_path" ]] || packages_installed=0
  "$EMULATOR" -list-avds 2>/dev/null | grep -Fx "$avd" >/dev/null || avds_installed=0
done <"$REPLACEMENT"
if [[ "$packages_installed" == 1 && "$avds_installed" == 1 ]]; then
  echo "Android SDK packages and required AVDs are already installed"
  exit 0
fi

packages=("platform-tools" "emulator" "ndk;28.2.13676358")
while IFS=$'\t' read -r _ _ _ _ system_image; do
  packages+=("$system_image")
done <"$MATRIX"
while IFS=$'\t' read -r _ _ _ _ system_image; do
  packages+=("$system_image")
done <"$AUTHORITY"
while IFS=$'\t' read -r _ _ _ _ system_image; do
  packages+=("$system_image")
done <"$REPLACEMENT"
if [[ "$packages_installed" != 1 ]]; then
  "$SDKMANAGER" "${packages[@]}"
fi

require_visible_avd() {
  if ! "$EMULATOR" -list-avds | grep -Fx "$1" >/dev/null; then
    echo "error: avdmanager succeeded but emulator cannot find AVD: $1 (ANDROID_AVD_HOME=$ANDROID_AVD_HOME)" >&2
    exit 1
  fi
}

while IFS=$'\t' read -r avd _ _ form_factor system_image; do
  if [[ -x "$EMULATOR" ]] && "$EMULATOR" -list-avds | grep -Fx "$avd" >/dev/null; then
    echo "Android AVD already installed: $avd"
    continue
  fi
  device="pixel_2"
  if [[ "$form_factor" == "tablet" ]]; then device="pixel_c"; fi
  "$AVDMANAGER" create avd --force --name "$avd" \
    --package "$system_image" --device "$device" <<< "no"
  require_visible_avd "$avd"
  echo "Android AVD installed: $avd ($system_image, $device)"
done <"$MATRIX"

while IFS=$'\t' read -r avd _ _ form_factor system_image; do
  if [[ -x "$EMULATOR" ]] && "$EMULATOR" -list-avds | grep -Fx "$avd" >/dev/null; then
    echo "Android authority AVD already installed: $avd"
    continue
  fi
  device="pixel_2"
  if [[ "$form_factor" == "tablet" ]]; then device="pixel_c"; fi
  "$AVDMANAGER" create avd --force --name "$avd" \
    --package "$system_image" --device "$device" <<< "no"
  require_visible_avd "$avd"
  echo "Android authority AVD installed: $avd ($system_image, $device)"
done <"$AUTHORITY"

while IFS=$'\t' read -r avd _ _ form_factor system_image; do
  if [[ -x "$EMULATOR" ]] && "$EMULATOR" -list-avds | grep -Fx "$avd" >/dev/null; then
    echo "Android replacement authority AVD already installed: $avd"
    continue
  fi
  device="pixel_2"
  if [[ "$form_factor" == "tablet" ]]; then device="pixel_c"; fi
  "$AVDMANAGER" create avd --force --name "$avd" \
    --package "$system_image" --device "$device" <<< "no"
  require_visible_avd "$avd"
  echo "Android replacement authority AVD installed: $avd ($system_image, $device)"
done <"$REPLACEMENT"

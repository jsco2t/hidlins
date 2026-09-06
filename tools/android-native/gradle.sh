#!/usr/bin/env bash
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" ]]; then
  case "$(uname -s)" in
    Darwin) export ANDROID_HOME="${HOME:?}/Library/Android/sdk" ;;
    Linux) export ANDROID_HOME="${HOME:?}/Android/Sdk" ;;
    *) echo "error: unsupported Android build host" >&2; exit 1 ;;
  esac
fi
export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$ANDROID_HOME}"
if [[ -z "${JAVA_HOME:-}" && -d "/Applications/Android Studio.app/Contents/jbr/Contents/Home" ]]; then
  export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
fi
[[ -n "${JAVA_HOME:-}" && -x "$JAVA_HOME/bin/java" ]] || {
  echo "error: JAVA_HOME must point at a Java 17+ runtime" >&2; exit 1;
}

readonly FLUTTER_ROOT="$(cd "$(dirname "$(command -v flutter)")/.." && pwd)"
readonly WRAPPER_SOURCE="$FLUTTER_ROOT/bin/cache/artifacts/gradle_wrapper/gradle/wrapper/gradle-wrapper.jar"
readonly WRAPPER_TEMP="$(mktemp -d)"
trap 'rm -rf "$WRAPPER_TEMP"' EXIT
mkdir -p "$WRAPPER_TEMP/gradle/wrapper"
cp "$WRAPPER_SOURCE" "$WRAPPER_TEMP/gradle/wrapper/gradle-wrapper.jar"
cp "$ROOT/app/android/gradle/wrapper/gradle-wrapper.properties" \
  "$WRAPPER_TEMP/gradle/wrapper/gradle-wrapper.properties"
cd "$WRAPPER_TEMP"
exec "$JAVA_HOME/bin/java" -classpath gradle/wrapper/gradle-wrapper.jar \
  org.gradle.wrapper.GradleWrapperMain -p "$ROOT/app/android" \
  --no-daemon --stacktrace "$@"

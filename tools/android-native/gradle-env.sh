#!/usr/bin/env bash
# Source this before Flutter or Gradle Android commands. Keep the initialization
# policy active for every included/plugin build, before settings are evaluated.
export GRADLE_USER_HOME="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/.gradle/android"
mkdir -p "$GRADLE_USER_HOME/init.d"
cp "$(dirname "${BASH_SOURCE[0]}")/../../app/android/gradle/no-junit.init.gradle" \
  "$GRADLE_USER_HOME/init.d/hidlins-policy.gradle"

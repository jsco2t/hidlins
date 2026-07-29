#!/usr/bin/env bash
# telemetry-check.sh — enforce NFR-013 (no telemetry) for the Dart/Flutter SDKs.
#
# ONE implementation, used by both `make toolchain` and CI, so the developer
# machine and the build agent can never disagree about what "analytics are off"
# means.
#
# Why this asserts the config FILE rather than parsing `flutter config` output:
#
#   Flutter treats CI as a bot (`bot_detector.dart` matches GITHUB_ACTIONS, CI,
#   ...) and substitutes `NoOpAnalytics`, whose `telemetryEnabled` is hardcoded
#   `false` and whose `setTelemetry` is an empty method. So on CI
#   `flutter config` ALWAYS prints "Analytics reporting is currently disabled."
#   no matter the real state — a grep for that string is a tautology in exactly
#   the environment where the gate runs, and would keep passing if the flag
#   were removed upstream.
#
#   `~/.dart-tool/dart-flutter-telemetry.config` is the actual persisted state,
#   shared by the Dart CLI, the Flutter CLI, and DevTools. Its format is
#   documented in-file: `reporting=0` disables, `reporting=1` enables. Asserting
#   it is environment-independent and fails closed.
#
# Usage: telemetry-check.sh [--write]
#   --write  create/repair the config so reporting=0 (used by `make toolchain`)
#            Without it the script is read-only and purely a gate.

set -euo pipefail

CONFIG="${HOME}/.dart-tool/dart-flutter-telemetry.config"
WRITE=0
[ "${1:-}" = "--write" ] && WRITE=1

if [ "$WRITE" -eq 1 ]; then
  # Best-effort via the official CLIs first. These are no-ops on a bot, which
  # is why the assertion below does not depend on them succeeding.
  command -v flutter >/dev/null 2>&1 && flutter config --no-analytics >/dev/null 2>&1 || true
  command -v dart >/dev/null 2>&1 && dart --disable-analytics >/dev/null 2>&1 || true

  mkdir -p "$(dirname "$CONFIG")"
  if [ ! -f "$CONFIG" ]; then
    printf '# Managed by tools/dev/telemetry-check.sh (Hidlins NFR-013).\nreporting=0\n' > "$CONFIG"
  elif grep -Eq '^reporting=1[[:space:]]*$' "$CONFIG"; then
    # The parser defaults to the conservative value when a name repeats, but
    # rewrite rather than append so the file states one thing.
    sed -i.bak 's/^reporting=1[[:space:]]*$/reporting=0/' "$CONFIG" && rm -f "${CONFIG}.bak"
  elif ! grep -Eq '^reporting=0[[:space:]]*$' "$CONFIG"; then
    printf 'reporting=0\n' >> "$CONFIG"
  fi
fi

if [ ! -f "$CONFIG" ]; then
  printf 'FAIL: NFR-013 — telemetry config absent (%s).\n' "$CONFIG" >&2
  printf '      Run `make toolchain` (or this script with --write) to create it.\n' >&2
  exit 1
fi

if grep -Eq '^reporting=1[[:space:]]*$' "$CONFIG"; then
  printf 'FAIL: NFR-013 — telemetry reporting is ENABLED in %s\n' "$CONFIG" >&2
  exit 1
fi

if ! grep -Eq '^reporting=0[[:space:]]*$' "$CONFIG"; then
  printf 'FAIL: NFR-013 — no `reporting=0` line in %s\n' "$CONFIG" >&2
  printf '      Refusing to assume telemetry is off without an explicit setting.\n' >&2
  exit 1
fi

printf '  OK: Dart/Flutter telemetry disabled (reporting=0 in %s)\n' "$CONFIG"

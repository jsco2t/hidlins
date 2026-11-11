#!/usr/bin/env bash
# Prove that device-test-only Dart packages do not enter the shipping graph.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
production_graph="$(cd "$repo_root/app" && dart pub deps --no-dev --style=list)"

for package in \
  integration_test \
  flutter_driver \
  fuchsia_remote_debug_protocol \
  platform \
  process \
  sync_http \
  webdriver
do
  if printf '%s\n' "$production_graph" | grep -Eq "^[[:space:]]*- ${package}([[:space:]]|$)"; then
    echo "error: device-test-only package entered production graph: $package" >&2
    exit 1
  fi
done

echo "  OK: device-test runner and its network-capable transitives are dev-only"

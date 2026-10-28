#!/usr/bin/env bash
# Explicit native desktop setup; build/check targets never install packages.
set -euo pipefail

check_prerequisites() {
  local missing=0 tool
  for tool in cc c++ clang cmake ninja pkg-config keepassxc-cli; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      printf 'Missing Linux build prerequisite: %s\n' "$tool" >&2
      missing=1
    fi
  done
  if ! pkg-config --exists gtk+-3.0 2>/dev/null; then
    printf 'Missing GTK3 development headers (gtk+-3.0.pc).\n' >&2
    missing=1
  fi
  return "$missing"
}

if [ "$(uname -s)" != Linux ]; then
  echo 'error: this target requires a Linux host.' >&2
  exit 1
fi

case "${1:-}" in
  --install)
    if ! check_prerequisites; then
      command -v apt-get >/dev/null 2>&1 || {
        echo 'error: automatic setup supports Debian/Ubuntu apt hosts; install the reported prerequisites using your distribution.' >&2
        exit 1
      }
      privilege=()
      if [ "$(id -u)" -ne 0 ]; then privilege=(sudo); fi
      "${privilege[@]}" apt-get update
      "${privilege[@]}" apt-get install -y --no-install-recommends \
        build-essential clang cmake ninja-build pkg-config libgtk-3-dev keepassxc
    fi
    ;;
  --check) ;;
  *) echo 'usage: install-linux-app-toolchain.sh --install|--check' >&2; exit 2 ;;
esac

check_prerequisites
printf 'Linux desktop build prerequisites available:\n'
clang --version
cmake --version
ninja --version
pkg-config --modversion gtk+-3.0
keepassxc-cli --version

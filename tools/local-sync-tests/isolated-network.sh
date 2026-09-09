#!/usr/bin/env bash
# Replay the real CLI/server process journey on kernel-isolated private links.

set -euo pipefail

if [ "$(uname -s)" != "Linux" ]; then
  echo "SKIP: Linux network namespaces are unavailable on $(uname -s); deterministic policy tests remain active."
  exit 0
fi
if ! command -v ip >/dev/null 2>&1; then
  echo "SKIP: iproute2 is unavailable; install it to run private-network namespace coverage."
  exit 0
fi
if ! sudo -n true >/dev/null 2>&1; then
  echo "SKIP: passwordless namespace capability is unavailable; loopback process coverage remains active."
  exit 0
fi

test_binary=""
for candidate in target/debug/deps/cli_local_sync_process-*; do
  case "$candidate" in
    *.d) continue ;;
  esac
  if [ -x "$candidate" ] && { [ -z "$test_binary" ] || [ "$candidate" -nt "$test_binary" ]; }; then
    test_binary="$candidate"
  fi
done
if [ -z "$test_binary" ]; then
  echo "error: CLI process test binary missing; run make test-local-sync-integration first" >&2
  exit 1
fi

namespace="hidlins-lns-$$"
cleanup() {
  sudo ip netns delete "$namespace" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

sudo ip netns add "$namespace"
sudo ip -n "$namespace" link set lo up
sudo ip -n "$namespace" link add hidlins0 type dummy
sudo ip -n "$namespace" address add 10.77.0.1/24 dev hidlins0
sudo ip -n "$namespace" -6 address add fd42:4849:444c::1/64 dev hidlins0
sudo ip -n "$namespace" link set hidlins0 up

repo="$(pwd)"
sudo ip netns exec "$namespace" env \
  HIDLINS_TEST_SYNC_ADDRESS=10.77.0.1 \
  "$repo/$test_binary" --test-threads=1
sudo ip netns exec "$namespace" env \
  HIDLINS_TEST_SYNC_ADDRESS=fd42:4849:444c::1 \
  "$repo/$test_binary" --test-threads=1

echo "PASS: real CLI/client/server journey completed on isolated private IPv4 and IPv6 ULA links."

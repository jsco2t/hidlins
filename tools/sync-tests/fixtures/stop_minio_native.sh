#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="${SCRIPT_DIR}/.minio-env"
STATE_FILE="${SCRIPT_DIR}/.minio-native-state"

if [ ! -f "$STATE_FILE" ]; then
	echo "stop_minio_native.sh: native fixture is not running."
	exit 0
fi

# shellcheck disable=SC1090
. "$STATE_FILE"
kill "$HIDLINS_MINIO_NATIVE_PID" >/dev/null 2>&1 || true
wait "$HIDLINS_MINIO_NATIVE_PID" >/dev/null 2>&1 || true
case "$HIDLINS_MINIO_NATIVE_DATA" in
	*/hidlins-minio-native.*) rm -rf -- "$HIDLINS_MINIO_NATIVE_DATA" ;;
	*) echo "stop_minio_native.sh: refusing unexpected data path" >&2; exit 1 ;;
esac
rm -f -- "$STATE_FILE" "$ENV_FILE"
echo "stop_minio_native.sh: stopped and removed native fixture data."

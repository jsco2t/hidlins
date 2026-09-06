#!/usr/bin/env python3
"""Create a mode-0600 Dart-define file without printing S3 credentials."""

from __future__ import annotations

import argparse
import json
import os
import re
import stat
import time
import uuid
from pathlib import Path
from urllib.parse import urlparse, urlunparse

DEFINE_KEYS = {
    "bucket": "HIDLINS_TEST_S3_BUCKET",
    "region": "HIDLINS_TEST_S3_REGION",
    "endpoint": "HIDLINS_TEST_S3_ENDPOINT",
    "path_style": "HIDLINS_TEST_S3_PATH_STYLE",
    "access_key_id": "HIDLINS_TEST_S3_ACCESS_KEY",
    "secret_access_key": "HIDLINS_TEST_S3_SECRET_KEY",
}


def require_private(path: Path) -> None:
    mode = stat.S_IMODE(path.stat().st_mode)
    if mode & 0o077:
        raise SystemExit(f"S3 config must not be group/world accessible: {path}")


def read_real(path: Path) -> dict[str, object]:
    require_private(path)
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise SystemExit("S3 config must be a JSON object")
    missing = sorted(set(DEFINE_KEYS) - set(data))
    if missing:
        raise SystemExit(f"S3 config is missing keys: {', '.join(missing)}")
    unexpected = sorted(set(data) - set(DEFINE_KEYS))
    if unexpected:
        raise SystemExit(f"S3 config has unexpected keys: {', '.join(unexpected)}")
    text_fields = set(DEFINE_KEYS) - {"path_style"}
    empty = sorted(
        key
        for key in text_fields
        if not isinstance(data[key], str) or not data[key].strip()
    )
    if empty:
        raise SystemExit(f"S3 config has empty or non-string fields: {', '.join(empty)}")
    endpoint = urlparse(str(data["endpoint"]))
    if endpoint.scheme != "https" or not endpoint.netloc:
        raise SystemExit("credentialed S3 endpoint must be an absolute HTTPS URL")
    if not isinstance(data["path_style"], bool):
        raise SystemExit("path_style must be a JSON boolean")
    return data


def read_minio(path: Path, bucket: str) -> dict[str, object]:
    values: dict[str, str] = {}
    pattern = re.compile(r'^export ([A-Z0-9_]+)="(.*)"$')
    for line in path.read_text(encoding="utf-8").splitlines():
        match = pattern.match(line)
        if match:
            values[match.group(1)] = match.group(2)
    required = {
        "HIDLINS_MINIO_ENDPOINT",
        "HIDLINS_MINIO_REGION",
        "HIDLINS_MINIO_ACCESS_KEY",
        "HIDLINS_MINIO_SECRET_KEY",
    }
    missing = sorted(required - set(values))
    if missing:
        raise SystemExit(f"managed MinIO config is missing: {', '.join(missing)}")
    return {
        "bucket": bucket,
        "region": values["HIDLINS_MINIO_REGION"],
        "endpoint": values["HIDLINS_MINIO_ENDPOINT"],
        "path_style": True,
        "access_key_id": values["HIDLINS_MINIO_ACCESS_KEY"],
        "secret_access_key": values["HIDLINS_MINIO_SECRET_KEY"],
    }


def write_defines(
    output: Path,
    config: dict[str, object],
    *,
    key_prefix: str = "hidlins-ios-alpha",
    endpoint_host: str | None = None,
) -> None:
    key = f"{key_prefix}/{int(time.time())}-{uuid.uuid4().hex}.kdbx"
    defines = {target: config[source] for source, target in DEFINE_KEYS.items()}
    if endpoint_host is not None:
        parsed = urlparse(str(defines["HIDLINS_TEST_S3_ENDPOINT"]))
        port = f":{parsed.port}" if parsed.port is not None else ""
        defines["HIDLINS_TEST_S3_ENDPOINT"] = urlunparse(
            parsed._replace(netloc=f"{endpoint_host}{port}")
        )
    defines["HIDLINS_TEST_S3_KEY"] = key
    descriptor = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        json.dump(defines, stream, separators=(",", ":"))
        stream.write("\n")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("minio", "real"))
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--bucket")
    parser.add_argument("--key-prefix", default="hidlins-ios-alpha")
    parser.add_argument("--endpoint-host")
    args = parser.parse_args()
    if args.mode == "minio":
        if not args.bucket:
            parser.error("--bucket is required for minio mode")
        config = read_minio(args.source, args.bucket)
    else:
        config = read_real(args.source)
    write_defines(
        args.output,
        config,
        key_prefix=args.key_prefix,
        endpoint_host=args.endpoint_host,
    )


if __name__ == "__main__":
    main()

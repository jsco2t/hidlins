#!/usr/bin/env python3
"""Persist Android verification logs after removing test secrets."""

from __future__ import annotations

import argparse
import os
import re
from pathlib import Path

MARKERS = (
    "integration-master-marker",
    "integration-rotated-marker",
    "integration-entry-secret-marker",
)

FLUTTER_DEVICE_FAILURE_MARKERS = (
    "Some tests failed.",
    "Failure Details:",
)


def sensitive_values() -> list[str]:
    return list(MARKERS)


def redact(text: str, values: list[str]) -> str:
    for value in values:
        text = text.replace(value, "<redacted>")
    return re.sub(
        r"(?i)(master[_ -]?password)"
        r"([=:]\s*)\S+",
        r"\1\2<redacted>",
        text,
    )


def has_flutter_device_failure(text: str) -> bool:
    """Detect flutter drive device failures even when its host exits zero."""
    return any(marker in text for marker in FLUTTER_DEVICE_FAILURE_MARKERS)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--require-passed", action="store_true")
    args = parser.parse_args()
    values = sensitive_values()
    sanitized = redact(args.source.read_text(encoding="utf-8", errors="replace"), values)
    if any(value in sanitized for value in values):
        raise SystemExit("Android verification log redaction failed closed")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    descriptor = os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        stream.write(sanitized)
    if args.require_passed and has_flutter_device_failure(sanitized):
        raise SystemExit("Flutter device test reported a failure in its output")


if __name__ == "__main__":
    main()

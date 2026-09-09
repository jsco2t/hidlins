#!/usr/bin/env python3
"""Reject reintroduction of the retired cloud-object sync implementation."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]

# These are the only reviewed places where the retired transport may be named.
# Workflow documents are frozen historical records; the inventory is approved
# provenance; and this checker must name the concepts it rejects.
ALLOWED_PREFIXES = (
    ".ai/workflow/",
    ".ai/workflow-archive/",
    ".agents/",
    "vendor/",
    "app/vendor-pub/",
    "app/rust_builder/cargokit/build_tool/vendor-pub/",
)
ALLOWED_FILES = {
    "crates/hidlins-sync/docs/s3-removal-inventory.md",
    "tools/dev/s3-removal-check.py",
}

# Deliberately precise product terms. Generic words such as "key", "region",
# and "bucket" have unrelated first-party uses and are not absence proofs by
# themselves. Their retired configuration forms are caught by the named
# transport/provider terms and credential identifiers below.
FORBIDDEN = re.compile(
    r"s3|"
    r"(?<![A-Za-z0-9])aws(?![A-Za-z0-9])|"
    r"sigv4|(?<![A-Za-z0-9])minio(?![A-Za-z0-9])|"
    r"(?<![A-Za-z0-9])etag(?![A-Za-z0-9])|"
    r"access(?:[_ -]?key(?:[_ -]?id)?|Key(?:Id)?)|"
    r"secret(?:[_ -]?access[_ -]?key|AccessKey)",
    re.IGNORECASE,
)

UNRELATED_TOKENS = (
    "aws-lc-rs",
    "aws-lc-sys",
    "webgl_compressed_texture_s3tc",
    "webgl_compressed_texture_s3tc_srgb",
    "pause_on_unhandled_async_exceptions3_test",
    # Established fake-secret canary in TUI tests, unrelated to transport.
    "s3cr3t",
)


def sanitize_unrelated(text: str) -> str:
    """Remove reviewed exact terms that are unrelated to sync transport."""
    for token in UNRELATED_TOKENS:
        text = re.sub(re.escape(token), "unrelated-token", text, flags=re.IGNORECASE)
    return text


def tracked_files() -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )
    return [item.decode() for item in result.stdout.split(b"\0") if item]


def is_allowed(path: str) -> bool:
    return path in ALLOWED_FILES or path.startswith(ALLOWED_PREFIXES)


def findings(paths: list[str]) -> list[str]:
    found: list[str] = []
    for relative in paths:
        if is_allowed(relative):
            continue
        path = ROOT / relative
        # `git ls-files --cached` includes staged-for-deletion paths. Their
        # absence is precisely the desired state, so inspect only files that
        # remain in the working tree.
        if not path.is_file():
            continue
        clean_path = sanitize_unrelated(
            relative.replace("s3-removal-check", "removal-check")
        )
        if match := FORBIDDEN.search(clean_path):
            found.append(f"{relative}: path contains {match.group(0)!r}")
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for line_number, line in enumerate(text.splitlines(), 1):
            # Permit only the Make target's exact self-reference. Any retired
            # term elsewhere on the same line is still rejected.
            candidate = sanitize_unrelated(
                line.replace("s3-removal-check", "removal-check")
            )
            if match := FORBIDDEN.search(candidate):
                found.append(
                    f"{relative}:{line_number}: contains {match.group(0)!r}: "
                    f"{line.strip()}"
                )
    return found


def self_test() -> None:
    rejected = {
        "code.rs": "pub struct S3Config;",
        "registry.toml": '[sync.s3]\nbucket = "vaults"',
        "cli.txt": "--aws-profile personal",
        "Makefile.fixture": "minio-up:",
        "escaped-uppercase-provider.txt": "MINIO",
        "mixed-case-provider.txt": "mInIo",
        "generated.dart": "final String accessKeyId;",
        "ci.yml.fixture": "integration-s3:",
        "protocol.md": "The response carries an ETag.",
    }
    for name, content in rejected.items():
        if not (FORBIDDEN.search(name) or FORBIDDEN.search(content)):
            raise AssertionError(f"negative control was accepted: {name}")
    accepted = (
        "LocalSyncConfig uses a pinned server identity and RemoteVersion. "
        "The encrypted vault remains authoritative."
    )
    if FORBIDDEN.search(accepted):
        raise AssertionError("local-sync control was rejected")


def main() -> int:
    self_test()
    found = findings(tracked_files())
    if found:
        print("error: retired cloud-object sync surface remains:", file=sys.stderr)
        for item in found:
            print(f"  {item}", file=sys.stderr)
        return 1
    print("  OK: retired cloud-object sync surface is absent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

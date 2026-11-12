#!/usr/bin/env python3
"""Check the exact approved EFF asset exception without broadening code licenses."""

import argparse
import hashlib
import json
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[2]
WORDLIST = "crates/hidlins-genpw/src/assets/eff_large_wordlist.txt"
DIGEST = "addd35536511597a02fa0a9ff1e5284677b8883b83e986e43f15a3db996b903e"
NOTICE = "crates/hidlins-genpw/LICENSES/EFF-wordlist-NOTICE.txt"
NOTICE_SOURCE = "crates/hidlins-cli/src/cli.rs"
SPEC_FILES = (
    "kb/supply-chain-policy.md",
    "features/password-generation/plans/implementation-plan.md",
    "features/password-generation/plans/design.md",
)
ATTRIBUTION = (
    "EFF Large Wordlist", "Electronic Frontier Foundation", "Joseph Bonneau",
    "CC-BY-4.0", "https://creativecommons.org/licenses/by/4.0/",
    "https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt", "unmodified",
)


def check(root: Path, specs: Path | None = None) -> list[str]:
    errors = []

    def read(relative: str) -> str:
        path = root / relative
        if not path.is_file():
            errors.append(f"missing {relative}")
            return ""
        return path.read_text(encoding="utf-8")

    wordlist = root / WORDLIST
    if not wordlist.is_file() or hashlib.sha256(wordlist.read_bytes()).hexdigest() != DIGEST:
        errors.append("wordlist digest is outside the exact approved exception")
    policy = read("CLAUDE.md")
    for required in (WORDLIST, DIGEST, "CC-BY-4.0", "non-code asset"):
        if required not in policy:
            errors.append(f"policy lacks exact asset exception field: {required}")
    provenance = read("crates/hidlins-genpw/LICENSES/EFF-wordlist.md")
    if "CC-BY-4.0" not in provenance or DIGEST not in provenance:
        errors.append("provenance lacks the actual license or exact digest")
    if any(claim in provenance for claim in ("CC0-1.0 equivalent", "CC0-1.0-equivalent", "not copyrightable")):
        errors.append("provenance substitutes an unsupported license/copyright assertion")
    notice = read(NOTICE)
    for required in ATTRIBUTION:
        if required not in notice:
            errors.append(f"notice lacks attribution field: {required}")
    source = read(NOTICE_SOURCE)
    if 'include_str!("../../hidlins-genpw/LICENSES/EFF-wordlist-NOTICE.txt")' not in source:
        errors.append("console does not compile the authoritative asset notice")
    if "after_help = AFTER_HELP" not in source:
        errors.append("console does not register the compiled asset notice in help")
    for relative in ("deny.toml", "fuzz/deny.toml"):
        try:
            licenses = tomllib.loads(read(relative)).get("licenses", {})
            if "CC-BY-4.0" in json.dumps(licenses):
                errors.append(f"asset exception broadened code licenses in {relative}")
        except tomllib.TOMLDecodeError as error:
            errors.append(f"invalid {relative}: {error}")
    if specs is not None:
        for relative in SPEC_FILES:
            path = specs / relative
            text = path.read_text(encoding="utf-8") if path.is_file() else ""
            if "CC-BY-4.0" not in text or DIGEST not in text:
                errors.append(f"spec policy lacks exact approved asset terms: {relative}")
            if any(claim in text for claim in ("CC0-1.0-equivalent", "CC0-equivalent", "public-domain status")):
                errors.append(f"spec retains unsupported asset license claims: {relative}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec-root", default="")
    args = parser.parse_args()
    errors = check(ROOT, Path(args.spec_root) if args.spec_root else None)
    for error in errors:
        print(f"ERROR: {error}")
    if errors:
        return 1
    print("OK: exact EFF asset bytes, exception and notices; CC-BY-4.0 excluded from code licenses")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

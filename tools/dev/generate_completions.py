#!/usr/bin/env python3
"""Generate completions with an explicitly supplied, already-built CLI."""
from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[2]
SHELLS = (("bash", "hidlins.bash"), ("zsh", "_hidlins"), ("fish", "hidlins.fish"))


def main() -> int:
    try:
        supplied = os.environ.get("HIDLINS_COMPLETIONS_BIN")
        if not supplied:
            raise ValueError("HIDLINS_COMPLETIONS_BIN must name the already-built CLI")
        binary = Path(supplied).resolve(strict=True)
        scripts = []
        for shell, filename in SHELLS:
            result = subprocess.run([str(binary), "completions", shell],
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
            if not result.stdout:
                raise ValueError(f"CLI generated empty {shell} completions")
            scripts.append((filename, result.stdout))
        # Complete all CLI calls before touching existing outputs. An invalid
        # explicit binary or late generation error must never fall back to
        # Cargo (or leave old nonempty files looking like a successful run).
        output = ROOT / "shell-completions"
        output.mkdir(parents=True, exist_ok=True)
        for filename, contents in scripts:
            path = output / filename
            path.write_bytes(contents)
            print(f"wrote {path}")
        return 0
    except subprocess.CalledProcessError as error:
        sys.stderr.buffer.write(error.stderr or b"")
        print(f"error: explicit CLI completion generation failed ({error.returncode})", file=sys.stderr)
    except (OSError, ValueError) as error:
        print(f"error: completion generation failed: {error}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())

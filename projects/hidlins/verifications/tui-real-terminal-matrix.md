# TUI Real-Terminal Confidence Matrix

**Last Updated:** 2026-08-31
**Status:** Optional, non-gating real-environment acceptance

The repository's in-process Ratatui journeys own deterministic behavior. This
matrix is intentionally limited to behavior only a real terminal emulator can
show. A run records observations; an unexecuted row is not a failure and must
not be reported as a pass.

## Portable setup

Resolve the repository before isolating `HOME`:

```bash
REPO_ROOT=$(git rev-parse --show-toplevel)
TEST_HOME=$(mktemp -d "${TMPDIR:-/tmp}/hidlins-terminal.XXXXXX")
make -C "$REPO_ROOT" build
mkdir -p "$TEST_HOME/vaults"
HOME="$TEST_HOME" "$REPO_ROOT/target/debug/hidlins" vault create \
  --id acceptance \
  --path "$TEST_HOME/vaults/acceptance.kdbx" \
  --no-recovery-warning
```

Enter a disposable master password only at the secure prompts. Do not reuse a
real vault or production secret. Launch from the terminal under test:

```bash
HOME="$TEST_HOME" "$REPO_ROOT/target/debug/hidlins-tui" --vault acceptance
```

Record the OS version, terminal application name/version, session protocol,
commit, date, and result. Remove `"$TEST_HOME"` after the run.

## Matrix

| Environment | Terminal under test | Irreducible observations |
| --- | --- | --- |
| KDE Linux | Konsole; record its installed version | TUI starts in the alternate screen; 60×16 remains legible; color and accessible themes remain readable; holding Shift permits native selection while mouse capture is enabled; `--no-mouse` permits native selection directly; normal quit restores cursor, echo, mouse, and the prior screen |
| Pop!_OS | The terminal actually in use; record its name and version | Same startup/restoration, 60×16 legibility, native-selection/mouse-capture, and color/accessibility smoke observations |
| macOS | The terminal actually in use; record its name and version | Same startup/restoration, 60×16 legibility, native-selection/mouse-capture, and color/accessibility smoke observations |

Do not repeat unlock, navigation, search, editing, configuration, or
secret-redaction scenarios here; those are automated by
`make test-tui-contracts` and `make check`.

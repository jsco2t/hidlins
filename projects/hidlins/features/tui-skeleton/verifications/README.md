# TUI Skeleton Verification

**Feature:** TUI Skeleton (`hidlins-tui`)
**Last Updated:** 2026-08-31

Deterministic TUI behavior is verified automatically in the Hidlins repository.
The former real-display and screen-reader procedure trees were removed because
they duplicated those checks and were not portable.

## Automated evidence

- `make test-tui-contracts` runs scripted Ratatui journeys and accessibility
  semantic contracts over actual rendered buffers.
- `make snapshots-check` verifies the stable screen goldens.
- `make check` runs the complete workspace test and quality gate.
- The work-package coverage ledger maps every removed manual test ID to its
  automated replacement or a narrowly irreducible external check.

The surviving `01-local-headless/` documents are historical command/runbook
references. The repository Make targets above are the current source of truth.

## Real-environment confidence

Only terminal-emulator and platform accessibility integration remain external:

- [TUI real-terminal matrix](../../../verifications/tui-real-terminal-matrix.md)
- [TUI screen readers](../../../verifications/tui-screen-readers.md)

These compact cases are non-gating and are not represented as executed. T7.4 is
complete based on automated deterministic evidence; the external cases provide
additional platform confidence without reopening feature completion.

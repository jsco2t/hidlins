# Task 006: TUI Experience Repairs

Delegation: main-only

## Goal

Repair and test all Task-001 findings in the TUI skeleton and enhancements, including state, persistence, commands, accessibility affordances, search, editing, sync, and error handling.

## Context

The TUI is the reference UX and has a large passing suite, but completion claims still lack required manual verification. The current `lru` advisory arrives through ratatui and requires an explicit compatible disposition.

## Scope

### In scope

- Terminal lifecycle, unlock/lock, tabs, tree/detail, overlays, config, themes, keymaps, palette/hints, search, navigation, mouse, bulk actions, read-only sessions, sync runtime, clipboard integration, snapshots, and performance.
- TUI dependency/advisory repair where compatible.
- Regression tests and snapshot updates for actual intended changes.

### Out of scope

- Flutter UI.
- Human audio judgment itself, which occurs in Task 008.

## Implementation requirements

- Fix every ledger finding assigned to Task 006.
- Add the failing state/widget/snapshot regression first and record the failure before each behavior fix; characterize behavior before splitting responsibilities or state ownership; record the pass afterward.
- Fix every ≥80-confidence architecture finding assigned to Task 006, including concrete god-object/state-ownership/coupling costs if the review validates them, without replacing the TUI with a framework-within-a-framework.
- Preserve full keyboard reachability, secret-free previews/history, lock-time zeroization, read-only enforcement, and terminal restoration.
- Upgrade/remove the affected `lru` path when compatible; otherwise document exact reachability and upstream constraints without inventing a green result.
- Follow `.ai/workflow/dependency-review.md`; verify licenses before use and consider safe removal or narrower features before adding or broadening dependencies.
- Snapshot updates must be reviewed as behavior changes, not blindly accepted.

## Acceptance criteria

- [ ] Every Task-006 ledger finding is fixed or has an approved evidence-backed non-reachability disposition.
- [ ] Every behavior repair has fail-before/pass-after evidence and every architecture refactor has characterization/invariant coverage.
- [ ] Command registry, keybindings, state transitions, persistence, sync, secret handling, and accessibility text retain regression coverage.
- [ ] Snapshot and search-performance gates pass.
- [ ] TUI remains usable without mouse or color-only information.

## Validation

- `cargo test -p hidlins-tui --offline --locked`
- `make snapshots-check`
- `make bench-search-gate`

## Dependencies

- Task 005

## Expected areas of change

- `crates/hidlins-tui/`
- `crates/hidlins-core/` where shared search contracts require repair
- `Cargo.lock`
- `vendor/`
- `README.md`

## Risks / notes

Ratatui dependency movement may be broader than the advisory itself. Prefer the smallest compatible update and review terminal behavior and snapshots carefully.

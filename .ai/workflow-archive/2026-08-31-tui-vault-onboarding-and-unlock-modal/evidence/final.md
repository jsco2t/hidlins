# Final Evidence: TUI Vault Onboarding and Unlock Modal

Completed: `2026-09-01T14:40:57Z`

## Objective

Make TUI startup useful for every registry cardinality: onboard an existing
vault when none is configured, prompt directly when exactly one is configured,
and select then unlock when multiple vaults are configured. Keep the shared
modal keyboard-complete, secret-safe, accessible through textual semantics,
and covered by deterministic in-process automation without adding a dependency.

## Completed tasks

- Task 001 — transactional vault registration: `evidence/001.md`
- Task 002 — startup onboarding state machine: `evidence/002.md`
- Task 003 — branded accessible startup modal: `evidence/003.md`

## Whole-package review

The main thread reviewed the cumulative implementation after all task gates.
The core owns locked, atomic registry persistence; `App` owns startup state and
authentication orchestration; and the shared startup renderer owns presentation.
The requested architecture review found no Critical or Important finding at
confidence 80/100 or higher.

Two high-confidence review findings were repaired before final acceptance:

- Parallel prompt fields were replaced by a typed `UnlockOrigin`, preventing
  impossible startup state and preserving the selected list row by identity.
- The onboarding action was aligned exactly with the frozen package criterion:
  `Enter: Open vault`; its journey, accessibility contract, and golden were
  updated before rerunning the final gate.

No manifest, `Cargo.lock`, or `vendor/` path changed. The approved dependency
fingerprint remains
`e8305fdc948cc1f58b189a88566c1e5f237c2e3827c4672853ea0f3e1b4e2804`.

## Final acceptance coverage

| Acceptance area | Evidence |
| --- | --- |
| Empty registry enters onboarding; portable path resolution and recoverable invalid paths | Task 002 app tests and first-run TestBackend journey |
| Authenticate before registration; atomic full-fidelity save; failure recovery | Task 001 registry tests plus Task 002 onboarding and persistence-failure tests |
| Wrong-password bounds, secret clearing, and non-auth error distinction | Task 002 app tests and first-run journey |
| One-vault direct prompt; explicit `--vault` behavior | Task 002 startup routing tests and configured journey |
| Multiple-vault picker, scrolling, navigation, Continue, Back, same-row restoration, alternate choice | Task 002 state tests plus Task 003 journey and long-list contract |
| Exact requested art at normal sizes; compact/accessibility semantics | Task 003 exact-row contracts and three startup goldens |
| Textual labels, focus/selection/status/actions, color independence, reading order | Task 003 accessibility-contract suite |
| Password masking/canary absence and control-sequence exclusion | Task 003 transition-frame and reviewable-buffer contracts; existing zeroize tests |
| Auto-lock/re-entry, help, exit, and existing workspace behavior | Full workspace test suite and existing journeys |
| No dependency or license-policy expansion | Unchanged dependency inputs, `make deny`, and `make audit` |

Real terminal-emulator and AT-SPI/Orca/VoiceOver acceptance remains explicitly
outside this package's automated claim; it is not represented as completed by
the in-process Ratatui backend.

## Final quality gate

The frozen final commands ran in the declared order:

- `make check` — PASS
- `make test-ignored` — PASS
- `make test-tui-contracts` — PASS (6 journeys, 9 accessibility contracts)
- `make snapshots-check` — PASS (16 goldens current)
- `make doc` — PASS with rustdoc warnings denied
- `make deny` — PASS (existing warnings only; advisories, bans, licenses, and sources OK)
- `make audit` — PASS (467 dependencies scanned; no vulnerability reported)

`git diff --check` also passed. The approved plan hash was revalidated as
`62d34d462ba7bc325a4fd4a36ed2abb1ea2f387ab0cae41fc885fdfe42dd25ed`.

## Revision provenance

The workflow began from recorded baseline
`883535f205d0937d2b90a8bf13051882e1a2c2c8`. During planning, the pre-existing
dirty worktree was committed without content changes, advancing `HEAD` to
`d33113b22721b304f7775bd20aa4b582f572ff02`. This package remains represented
by the reviewed working-tree changes; no package commit was requested. The
recorded final Git revision is therefore that current `HEAD`.

## Remaining findings

None within approved scope.

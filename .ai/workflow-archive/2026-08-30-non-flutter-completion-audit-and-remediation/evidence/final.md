# Final Evidence — Non-Flutter Completion Audit and Remediation

Work ID: `2026-08-30-non-flutter-completion-audit-and-remediation`

Completed: 2026-09-01T02:23:26Z

Baseline and final Git revision: `883535f205d0937d2b90a8bf13051882e1a2c2c8`

Approved plan SHA-256: `cd2254e216b33bd66e52d6c19140f16b18e8bf860a344ca46158bc70227b3286`

## Objective and completed tasks

This package independently audited every claimed-complete non-Flutter feature,
recorded discrepancies before repair, remediated high-confidence correctness,
architecture, testing, documentation, and supply-chain findings, and replaced
deterministic TUI manual checks with in-process automation. All nine approved
tasks are complete:

1. `tasks/001-completion-audit-and-fix-ledger.md` — guardrails,
   supply-chain remediation, and the completion/fix ledger (`evidence/001.md`).
2. `tasks/002-vault-entry-and-generator-repairs.md` — secret-bearing dead API
   removal with unlock characterization (`evidence/002.md`).
3. `tasks/003-security-behavior-repairs.md` — warning-denying feature-gate
   repair without weakening security behavior (`evidence/003.md`).
4. `tasks/004-sync-and-merge-data-integrity-repairs.md` — attachment-preserving
   merge history and KDBX/KeePassXC regressions (`evidence/004.md`).
5. `tasks/005-cli-surface-repairs.md` — secret-free core query records and CLI
   boundary cleanup (`evidence/005.md`).
6. `tasks/006-tui-experience-repairs.md` — valid-state session ownership,
   coordinator extraction, and raw-KDBX boundary removal (`evidence/006.md`).
7. `tasks/007-build-supply-chain-and-doc-repairs.md` — final graph, CI/Make,
   license-policy, and shipped-state reconciliation (`evidence/007.md`).
8. `tasks/008-live-and-accessibility-verification.md` — scripted Ratatui
   journeys and accessibility contracts (`evidence/008.md`).
9. `tasks/009-consolidate-tui-real-environment-verification.md` — removal of
   obsolete manual suites and consolidation of portable real-environment
   documentation (`evidence/009.md`).

## Whole-package review

The primary thread reviewed the cumulative implementation from the baseline,
including relevant untracked first-party sources, tests, workflow documents,
and the intentionally large vendored upgrade/repatch set. Generated outputs,
fixtures, and golden contents were not treated as architecture surfaces.

The required final `$arch-reviewer` pass used diff mode. It reported one
Important finding above the 80/100 threshold and no Critical findings:

- **Cross-entry quadratic attachment ambiguity scan** — Important, confidence
  90/100; simplicity, durability, and performance. In
  `crates/hidlins-sync/src/merge/mod.rs`,
  `ensure_unambiguous_attachment_versions` compared every local version with
  every remote version even though only equal entry UUIDs can conflict. At the
  supported 5,000-entry scale that admitted roughly 25 million irrelevant
  comparisons. The implementation now indexes remote snapshots by UUID with a
  `BTreeMap` and compares only versions of the same logical entry. Focused merge
  tests and the complete final gate pass after the repair.

The same integrated review corrected residual stale shipped-state claims in
the sync and CLI module documentation, current Keepass/Ratatui version prose,
the active findings ledger, and notebook-correction tracking. Final targeted
searches found no remaining matching stale claim. There are no unresolved
in-scope architecture findings at confidence 80/100 or higher.

## Acceptance-criteria evidence

| Approved criterion | Concrete evidence |
| --- | --- |
| Inventory claims before repair | `.ai/workflow/findings.md` records feature/bug verdicts, evidence, severity, confidence, disposition, and task ownership; Task 001 evidence records the pre-repair audit. |
| Explicit test-forward, dependency-minimization, and license rules | Root `AGENTS.md` requires red/green defect work, characterization before structural change, dependency purpose/license/replaceability review, and refusal of forbidden or unverified licenses. |
| Complete direct-dependency review | `.ai/workflow/dependency-review.md` records purpose, verified license, footprint, maintenance/adoption signal, alternatives, and keep/remove/replace rationale for every included direct edge and all changed dependencies. |
| Baseline advisory/yank remediation before Task 002 | Task 001 removed `event-listener 5.4.1`, `lru 0.16.4`, and yanked `chacha20 0.10.0`; `make deny` and `make audit` passed before Task 002 and at final gate without concealment exceptions. |
| Remove avoidable `anyhow` and `hex` | The unused direct TUI `anyhow` edge is gone. `hex` was replaced by narrow first-party lowercase encoders, behavior-pinned, removed from the resolved graph and vendor tree, and reproducibly patched out of four narrow downstream uses. |
| Verify absent manifest license metadata | `.ai/workflow/dependency-review.md` and `evidence/001.md` verify `vendor/allo-isolate/LICENSE` as Apache-2.0; no invented SPDX exception or policy weakening was added. |
| Architecture findings owned and resolved | The initial whole-code architecture findings are task-owned in `.ai/workflow/findings.md`; Tasks 005–006 close the presentation/core and session-ownership findings. The final diff review finding is documented and fixed above, with no unresolved >=80 finding. |
| Fix every high-confidence in-scope ledger item | Evidence 001–009 records each disposition and validation. `.ai/workflow/findings.md` is reconciled to the completed implementation and revised verification model. |
| Test-forward repair evidence | Each defect evidence file records fail-before/pass-after results; structural refactors record pre-change characterization or invariant coverage. Task 001 includes the exposed KDBX write-back red/green regression, Task 003 the warning-denying red/green build, and Task 004 the failing then passing encrypted attachment-history regression. |
| Permitted licenses and minimal dependency surface | `make deny` and `make audit` pass. Dependency removals and upgrades are documented; no license allowance was weakened. Task 008's task-scoped aggregate fingerprint proves its automation changed no manifest, lock, or vendor file. |
| Attachment bytes survive merge history | Task 004 covers zeroizing attachment snapshots, semantic deduplication, ambiguity rejection, save/reopen, and enhanced `make interop-sync`; KeePassXC observes the winner attachment as current and loser attachment in history. |
| No secret leakage | Core query records exclude secret fields; accessibility tests reject master/entry canaries; command-line/environment password prohibitions remain intact; review found no new secret-bearing logs, errors, snapshots, debug output, or plaintext disk state. |
| All ordered quality gates pass | Every command in `gate.json` passed in its declared order; results appear below. |
| Honest live and automated TUI evidence | `evidence/008.md` records MinIO, clipboard, and KeePassXC passes and separately records the Linux logind host-environment halt. Three journeys and four buffer-level contracts run through `make test-tui-contracts` using existing Ratatui/crossterm seams with no new dependency. |
| TUI journey coverage | `crates/hidlins-tui/src/journey_tests.rs` covers unlock/browse/lock, discoverability, palette/search, constrained geometry, keyboard navigation, read-only/bulk state, configuration/theme, and injected/disabled mouse paths. `.ai/workflow/tui-verification-coverage.md` maps every former manual ID. |
| Accessibility contracts | `crates/hidlins-tui/src/accessibility_contract_tests.rs` flattens actual buffers in reading order and asserts important labels/states, secret-canary absence and password masking, theme-equivalent semantics rather than color-only meaning, and absence of raw escape/C0/C1 controls. |
| Remove obsolete manual suites and retain a portable matrix | Task 009 deleted both features' old `02-local-display`, `03-screen-reader`, and manual-only fixture/scratch trees. The central notebook now contains exactly three terminal rows—Konsole, the actual Pop!_OS terminal, and macOS—and exactly two screen-reader cases—Linux Orca/AT-SPI and macOS VoiceOver. Setup derives the repository root before an isolated `HOME`; it contains no Jason-specific absolute path or GNOME Terminal requirement. |
| Honest platform/scope separation | The retained real-terminal/screen-reader cases are optional, non-gating, and explicitly unexecuted. Native macOS/M1, Flutter, abandoned/archive, and unplanned roadmap work are not described as verified by this package. |
| Final mapping exists | This document maps every package and Revision 4/5 criterion to code, tests, command results, evidence, or an explicit external-environment limitation. |

## Final quality gate

The final commands ran in the exact `gate.json` order after whole-package review
and repair.

| Command | Result |
| --- | --- |
| `make check` | PASS — formatting, Clippy with warnings denied, offline locked build/tests/doctests, feature gates, and supported macOS cross-target checks |
| `make test-ignored` | PASS — serialized environment-mutating and interop/security cases |
| `make test-merge-properties` | PASS — 7 tests and 4,096 generated merge cases |
| `make completions-check` | PASS |
| `make snapshots-check` | PASS — all 13 goldens current |
| `make doc` | PASS — rustdoc warnings denied |
| `make deny` | PASS — advisories, bans, licenses, and sources accepted |
| `make audit` | PASS — 467 locked dependencies checked against 1,233 RustSec advisories; no vulnerability finding |
| `make interop` | PASS — US-090, US-091, and US-092 KeePassXC flows |
| `make interop-entry` | PASS — US-010/012/013/014/016/018; optional `oathtool` cross-check unavailable, KeePassXC checks passed |
| `make interop-sync` | PASS — KeePassXC winner-current and loser-history attachment checks |
| `make bench-search-gate` | PASS — substring p99 3.12 ms, wildcard 4.52 ms, fuzzy 14.77 ms against a 50 ms budget |
| `make minio-up` | PASS — pinned MinIO became healthy on isolated localhost storage |
| `make test-s3-integration` | PASS — 11 live sync cases and 1 CLI live-sync case |
| `make minio-down` | PASS — test container removed |

## Cumulative diff

From baseline `883535f205d0937d2b90a8bf13051882e1a2c2c8` to the final
working tree, 339 tracked files changed with 12,359 insertions and 10,074
deletions, plus reviewed first-party source/test/workflow additions. Much of
the tracked volume is the approved, audited, and reproducibly patched vendored
dependency upgrade. Existing user-owned dirty state at the baseline was
preserved rather than reset or overwritten. The repository HEAD did not move,
so the final Git revision equals the baseline; the completed implementation is
the reviewed working-tree state.

## Remaining non-blocking concerns

- Native macOS/M1 execution remains external and unclaimed.
- The optional Konsole, actual Pop!_OS terminal, macOS terminal, Linux
  Orca/AT-SPI, and macOS VoiceOver confidence cases were not executed; their
  documents explicitly make no pass claim.
- The real Linux logind test reached a host-environment halt because this
  process had neither a usable system bus nor an active logind session. It is
  not labeled a product pass or failure.
- `cargo deny` retains documented informational upstream duplicate-version,
  unmatched allowed-license, and `allo-isolate` manifest-metadata warnings.
  The accepted target graph passes license, advisory, ban, and source policy.

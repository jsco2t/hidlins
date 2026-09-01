# Non-Flutter Completion Audit and Remediation

## Objective

Establish an evidence-backed answer to every non-Flutter completion claim in the Hidlins notebook, perform deep correctness and architecture reviews of the shipped Rust application, record a concrete fix ledger before changing product code, repair every high-confidence in-scope defect using test-forward implementation, minimize the external dependency surface without ever violating license policy, and rerun the repository's automated, interop, live-service, and performance verification surfaces. Deterministic TUI behavior is automated; only a compact, portable real-terminal and screen-reader acceptance matrix remains for external environment confidence and does not block this work package.

The result must distinguish implemented behavior from stale tracking, intentionally abandoned work, unplanned roadmap work, and environment-only verification gaps. A passing aggregate command is evidence, but is never treated as a substitute for source review or requirement traceability.

## Current behavior

The notebook and repository disagree in several places:

- The notebook claims `vault-core`, `entry-management`, `password-generation`, `security-behaviors`, `cli-skeleton`, `s3-sync`, `cli-sync-wiring`, `pre-mvp-fixes`, and the code portions of `tui-skeleton` and `tui-enhancements` are complete.
- `tui-skeleton` T7.4 and `tui-enhancements` T5.3 were blocked on duplicated manual suites whose hard-coded paths, terminal-brand assumptions, and human-audio requirement made them non-portable. Revision 4 supersedes that completion model with automated deterministic contracts plus a small non-gating real-environment acceptance matrix.
- Vault-core still records external macOS/CI/M1 verification gaps. This Linux package can inspect the CI configuration and run Linux equivalents, but cannot manufacture macOS/M1 evidence.
- The notebook lists attachment propagation, history-attachment loss, and RST-CRED-1 password-change handling as open/planned bugs. Current source and tests show attachment propagation and credential re-encryption are implemented; history versions still lose attachment bytes.
- `make check` passes at the baseline, including 402 TUI tests, but emits a dead-code warning in the no-default-features security check.
- `make deny` currently fails because production `keepass 0.12.9` resolves the yanked `chacha20 0.10.0`. `make audit` also reports current unsoundness advisories for supported-path `event-listener 5.4.1` and production-path `lru 0.16.4`. These are baseline gate failures to remediate before subsystem repairs, not items to defer until the end of the package.
- The direct dependency audit found that `hidlins-tui` declares `anyhow` without a production call site and that `hidlins-sync` uses `hex` only for lowercase byte encoding. Those are concrete removal/reimplementation candidates. By contrast, KDBX, cryptography, TLS/HTTP, entropy, terminal protocols, Unicode tables, DBus, and platform FFI are not safe "simple dependency" candidates.
- The working tree already contains user-owned changes to `.gitignore`, `CLAUDE.md`, `.agents/`, and `.codex/`. They must be preserved.

## Proposed implementation

1. Before any product repair, strengthen `AGENTS.md` with explicit operational rules for test-forward work, dependency-minimization decisions, dependency-removal review, and absolute license compliance.
2. Complete the audit ledger before source repair. Walk every completed non-Flutter feature/task claim, its authoritative plan/design where applicable, the associated source and tests, and all current repository gates. Use `$arch-reviewer` in whole-code mode, sequentially by crate/boundary, to evaluate architecture as implemented. Write `.ai/workflow/findings.md` as the ordered correctness and architecture fix ledger and `.ai/workflow/dependency-review.md` as the direct-dependency purpose/license/replaceability inventory.
3. Still within Task 001, restore the supply-chain gate before any subsystem repair: remove the unused `anyhow` edge; replace the leaf `hex` dependency with a small tested lowercase encoder if the behavior-pin tests confirm equivalence and the crate leaves the resolved graph; move `event-listener` and `lru` onto patched compatible resolutions; and eliminate the yanked `chacha20` path through the narrowest supported `keepass`/transitive upgrade or a narrowly audited downstream dependency patch. Never reimplement the cipher, KDBX, TLS, HTTP, entropy, terminal protocols, Unicode data, DBus, or platform FFI. Re-vendor exact locked sources and run KDBX, merge, SigV4, TUI snapshot, license, and advisory validation before Task 001 can pass.
4. Repair the remaining ledger items by ownership boundary: core/data, security, sync/data-integrity, CLI, TUI, then final cross-cutting build/documentation reconciliation. Architecture findings are repaired in the owning subsystem rather than papered over with comments.
5. Implement every behavior fix test-forward: first add or strengthen the smallest regression test that fails for the defect, capture the failure, implement the minimum repair, capture the pass, then refactor while green. Architecture-only refactors begin with characterization/invariant tests. Evidence records the red/green sequence. Non-code-only changes must state why a failing test is inapplicable and identify the validation used instead.
6. Before adding or upgrading any dependency, document its purpose, exact license, transitive cost, maintenance signal, narrower alternatives, and whether a small in-repository implementation can safely replace it. Before removing/reimplementing a dependency, pin behavior with tests. Cryptography, TLS, KDBX parsing/writing, and other security primitives remain excluded from hand-rolling.
7. Preserve the completed MinIO, clipboard, KeePassXC, and host-diagnosed Linux OS-lock evidence from the interrupted Task 008. Replace the duplicated display suite with in-process Ratatui journeys and accessibility-contract tests. Remove the old TUI manual-test documents and retain only a compact portable terminal matrix plus one Linux/Orca and one macOS/VoiceOver real-environment acceptance case; their execution is not a completion gate for this package.
8. Re-run `$arch-reviewer` in diff mode during the final whole-package review, fix every in-scope finding at or above its 80% confidence threshold, and reconcile the final claim inventory against actual evidence. Stale external notebook statements will be reported precisely; modifying the notebook is not authorized by this repository-scoped work package.

## Architectural decisions

- The Rust core remains the business-logic and cryptographic authority. CLI and TUI fixes stay thin; shared behavior is repaired below the UI boundary.
- Test-forward means test-before-fix, not merely “tests exist at the end.” A task cannot be accepted without red/green evidence for each behavior defect or an explicit non-code/structural justification paired with characterization or invariant validation.
- Architecture quality is part of completion. `$arch-reviewer` reviews separation of concerns, testability, dependency direction, abstraction fit, hidden control flow, API shape, state ownership, simplicity, and extension points. Only findings with confidence at least 80/100 and a concrete maintenance/testability/coupling cost enter the required fix ledger. Large modules or unusual patterns are not defects by size or taste alone.
- Architecture remediation within the existing non-Flutter product behavior is approved scope, including moving responsibilities, narrowing visibility, correcting dependency direction, and splitting state ownership. Preserve KDBX/on-disk formats, CLI compatibility, and user-observable behavior unless an existing defect requires a correction. Do not introduce speculative frameworks or abstraction layers.
- KDBX round-trip fidelity, atomic writes, advisory locking, protected-field fidelity, zeroization, and loser-as-history preservation remain non-negotiable.
- The known history-attachment defect is not closed by documentation or rolling backups alone. The repair must preserve the losing history version's attachment bytes in the merged KDBX. Extra encrypted backup generations may be added only as defense in depth. If a compatible released `keepass-rs` cannot support the repair, a narrowly scoped, audited downstream API patch is permitted, with exact pinning, vendoring/checksum integrity, license/advisory review, and full interop revalidation.
- Dependency changes are permitted only to close an actual finding. First prefer deleting an unnecessary dependency or implementing a small, well-specified non-cryptographic capability in-repository when that produces less code and risk than the external dependency plus its transitives. Otherwise prefer the narrowest compatible crate or transitive/direct upgrade. Do not add feature-rich crates as workarounds. Every dependency decision is recorded in `.ai/workflow/dependency-review.md` and must keep offline vendoring reproducible.
- License compliance is absolute. No dependency, copied implementation, vendored source, font/data asset, or build tool introduced by this package may use a license outside the repository allow-list. Forbidden, ambiguous, missing/unverified, or linking-exception licenses block the change; audit configuration must never be weakened to waive them. License verification happens before dependency source is executed or product code is written against it, and `make deny` confirms the final graph.
- Current RustSec warnings and the yank are Task-001 remediation requirements. `event-listener` and `lru` must leave the affected versions through compatible patched resolutions; the yanked `chacha20` path must be removed without reimplementing cryptography. Existing `quick-xml` advisory exceptions must be re-evaluated and narrowed or removed wherever an accepted parent upgrade makes that possible. No new blanket ignore or policy weakening is permitted.
- Fixes discovered by the audit may be implemented when they fit an existing subsystem and architecture. A finding that requires a new product feature, a materially different architecture, or expansion into Flutter/SSH/agent scope triggers `PLAN_CHANGE_REQUIRED`.
- User-owned baseline changes are preserved. Reviews and diffs must distinguish this package's changes from the dirty baseline.

## Work included

- Completion-claim and task-acceptance audit for:
  - vault-core;
  - entry-management;
  - password-generation;
  - security-behaviors;
  - cli-skeleton;
  - s3-sync;
  - tui-skeleton code deliverables;
  - pre-mvp-fixes;
  - tui-enhancements code deliverables;
  - cli-sync-wiring.
- Status audit of the three recorded non-Flutter bugs: merge attachment propagation, merge-history attachment loss, and RST-CRED-1 password-change re-encryption.
- Deep review and repair of `hidlins-core`, `hidlins-genpw`, `hidlins-security`, `hidlins-sync`, `hidlins-cli`, and `hidlins-tui`, including their tests and shared integration boundaries.
- Whole-code architecture review of each included crate and the cross-crate dependency graph using `$arch-reviewer`; generated files, vendored code, fixtures, and goldens are excluded from architectural judgment.
- An explicit inventory of each included direct dependency: purpose, license, transitive footprint, necessity, and feasibility of safe in-repository replacement or removal.
- Immediate Task-001 remediation of current yanked/advisory paths, verification of vendored missing-license metadata such as `allo-isolate` against its actual license file, and test-backed removal of unnecessary or safely replaceable simple dependencies.
- A repository-guidance update in `AGENTS.md` that makes test-forward implementation, dependency-minimization review, and absolute license compliance operational and auditable.
- Review and repair of non-Flutter Makefile targets, CI coverage, interop/live-service harnesses, security gates, dependency policy, and in-repository user/developer documentation.
- Automation-first replacement of the TUI display and accessibility verification, plus portable non-gating real-environment acceptance instructions for Konsole, the installed Pop!_OS terminal, macOS terminals, Linux/Orca, and macOS/VoiceOver.
- Regression tests for every code defect fixed.

## Task sequence

1. [`tasks/001-completion-audit-and-fix-ledger.md`](tasks/001-completion-audit-and-fix-ledger.md)
2. [`tasks/002-vault-entry-and-generator-repairs.md`](tasks/002-vault-entry-and-generator-repairs.md)
3. [`tasks/003-security-behavior-repairs.md`](tasks/003-security-behavior-repairs.md)
4. [`tasks/004-sync-and-merge-data-integrity-repairs.md`](tasks/004-sync-and-merge-data-integrity-repairs.md)
5. [`tasks/005-cli-surface-repairs.md`](tasks/005-cli-surface-repairs.md)
6. [`tasks/006-tui-experience-repairs.md`](tasks/006-tui-experience-repairs.md)
7. [`tasks/007-build-supply-chain-and-doc-repairs.md`](tasks/007-build-supply-chain-and-doc-repairs.md)
8. [`tasks/008-live-and-accessibility-verification.md`](tasks/008-live-and-accessibility-verification.md)
9. [`tasks/009-consolidate-tui-real-environment-verification.md`](tasks/009-consolidate-tui-real-environment-verification.md)

## Quality gate

Every task runs:

- `make check` — formatting, clippy with denied warnings, full default Rust tests, workspace build, macOS security cross-checks, and feature-gated compile checks;
- `make deny` — licenses, advisories, bans, and sources;
- `make audit` — independent RustSec scan;
- `make doc` — documentation with warnings denied.

Task 001 is explicitly responsible for making this standard gate green before later repairs begin. The gate is not weakened to accommodate the baseline `make deny` failure.

Task evidence additionally records, for every behavior repair, the exact regression test's failing result before the implementation change and passing result afterward. Aggregate green gates do not replace this red/green evidence. Architecture refactors record the characterization/invariant tests that protected behavior before movement.

Final validation adds ignored tests, the heavier merge property sweep, generated completion and snapshot drift checks, all KeePassXC interop harnesses, the search performance gate, and live MinIO integration. The new TUI journey and accessibility-contract tests run under `make check`; their focused Make target is also task validation. Clipboard and Linux OS-event results already captured by the interrupted Task 008 remain environment evidence. The retained terminal and screen-reader matrix is documentation for real-machine acceptance, not a command in this package gate.

Flutter commands (`make app-check`, bridge codegen, Dart analysis/tests, Flutter builds) are intentionally omitted. The Rust workspace still compiles `hidlins-api` under `make check`; that is regression containment, not a review of the excluded Flutter feature.

## Risks

- The repository is unusually broad and has only one large implementation commit, so completion cannot be inferred from commit granularity. The ledger must cite source/tests for every claim.
- The architecture review covers hundreds of relevant files. Because this broad scope is explicit, it proceeds as sequential crate-level whole-code reviews rather than being narrowed; findings are consolidated and deduplicated before repairs.
- Dependency removal can trade supply-chain surface for bespoke maintenance burden. Replacement is chosen only when the capability is small, stable, well specified, non-cryptographic, and cheaper to test and maintain than the dependency graph it removes.
- The `keepass` and `ratatui` parent upgrades needed to clear transitive supply issues can affect KDBX fidelity and TUI rendering respectively. Task 001 therefore treats the smallest compatible released upgrade as preferred, reviews the full lock/vendor diff, and runs focused interop/snapshot suites in addition to the standard gate.
- Repairing history attachments may require a `keepass-rs` upgrade or narrow downstream patch. That touches the highest-risk KDBX boundary and therefore requires property, fault, and KeePassXC interop coverage.
- Dependency remediation may expand the vendored tree or require network access. Exact pins, diff review, license checks, and user authorization for networked vendoring are mandatory.
- Live MinIO, clipboard, and logind verification can fail because of host configuration. Environment failures are recorded separately from product failures; product failures are fixed. Ratatui `TestBackend` cannot prove terminal-emulator AT-SPI integration or subjective speech quality, so those irreducible checks remain explicitly documented without being fabricated or used to block deterministic completion.
- The external notebook contains stale/conflicted prose but is outside the writable repository. The final ledger will identify exact corrections without silently editing external records.

## Out of scope

- `app/`, Flutter UI behavior, Flutter/Dart tests, mobile/desktop packaging, generated Flutter bridge code, and substantive `hidlins-api` review.
- The abandoned `clock-anomaly-warnings` branch and archived `sync-git` implementation.
- Building the unplanned SSH-key and optional agent roadmap features; their explicit CLI/placeholder slots are reviewed only for honest behavior and documentation.
- Executing the retained Konsole, Pop!_OS-terminal, macOS-terminal, Orca, VoiceOver, or native macOS/M1 acceptance cases from this non-interactive workflow process. The portable cases are retained for appropriate real machines, but their execution is external evidence.
- Editing the external notebook tree. Corrections are enumerated for the human unless separate filesystem authority is granted later.

## Final acceptance criteria

- `.ai/workflow/findings.md` inventories every included completion claim and records evidence, discrepancies, severity/confidence, and disposition before product repairs begin.
- `AGENTS.md` explicitly requires test-before-fix red/green evidence, characterization tests before architecture refactors, dependency purpose/license/replaceability review before manifest changes, and an unconditional refusal of disallowed or unverified licenses.
- `.ai/workflow/dependency-review.md` inventories every included direct dependency and records purpose, verified license, transitive cost, keep/remove/replace decision, and rationale; every dependency changed by this package has a complete pre-change decision record.
- Before Task 002 begins, `make deny` and `make audit` pass without the baseline `event-listener 5.4.1`, `lru 0.16.4`, or yanked `chacha20 0.10.0` findings; no new advisory or license exception was added to hide them.
- The unused `anyhow` direct edge is removed. The `hex` edge is replaced with a small in-repository lowercase encoder and removed from the resolved graph if behavior-pin tests confirm identical output; otherwise Task 001 must stop for a plan change rather than silently defer the decision.
- Every vendored package with absent manifest license metadata has its actual license file verified against the allow-list and recorded, and existing advisory exceptions are reduced wherever the dependency remediation removes their reachability.
- `$arch-reviewer` whole-code findings at confidence 80/100 or higher are incorporated into the fix ledger with concrete costs and owning repair tasks; a final diff-mode architecture review has no unresolved in-scope findings at that threshold.
- Every high-confidence in-scope ledger finding is fixed and has regression coverage, or the workflow stops in a permitted terminal state rather than silently deferring it.
- Every behavior defect repair has recorded fail-before/pass-after test evidence. Every architecture refactor has pre-refactor characterization or invariant coverage and remains green afterward.
- No dependency or asset violates the permitted-license list, no license exception weakens policy, and avoidable dependencies identified by the approved review are removed only with behavior-preserving test coverage.
- Merge history versions retain their attachment bytes through save/reopen and KeePassXC-compatible KDBX serialization; a backup-only workaround is not counted as closure.
- No secret is introduced into logs, errors, command lines, environment-based password input, plaintext disk state, snapshots, or debug output.
- All standard and final commands in `gate.json` pass in order.
- Prior Task 008 evidence honestly records real MinIO, clipboard, and KeePassXC passes plus the Linux logind environment limitation. Every deterministic behavior formerly assigned to the TUI manual suites is mapped to existing or new automation, and the new TUI journey/accessibility-contract target passes without adding a dependency.
- The obsolete TUI manual-test documents and their manual-only fixtures are removed. A compact portable matrix remains for Konsole, the installed Pop!_OS terminal, macOS terminals, Linux Orca/AT-SPI, and macOS VoiceOver; it contains no machine-specific absolute paths or GNOME Terminal requirement and is not executed as a package gate.
- Remaining macOS/M1, Flutter, abandoned, archived, and unplanned-roadmap items are explicitly separated from completed non-Flutter work; no final report describes them as verified.
- Final evidence maps every package criterion to concrete files, tests, command results, or an honestly stated external-environment limitation.

## Approved-plan revision — Revision 4

### Observed acceptance failure

The original Task 008 could not enter its first display test because the
manual documents embedded `/Users/jason/...` paths, assumed GNOME Terminal,
required a real TTY, and coupled completion to human Orca audio judgment. The
five TUI-enhancements display documents also duplicate behavior that is already
largely covered by `App` state tests, deterministic Ratatui rendering, theme
invariants, and snapshots. The older tui-skeleton display and screen-reader
suites create a second overlapping source of manual truth.

### Additive fix strategy

1. Add an in-crate, test-only journey harness around the existing `App`,
   `Terminal<TestBackend>`, fake-clock/test fixtures, and crossterm key/mouse
   events. Journeys drive real application transitions and render checkpoints;
   they do not introduce a PTY, subprocess driver, snapshot library, or new
   dependency.
2. Add accessibility-contract helpers that flatten the Ratatui buffer in
   row-major reading order. Contracts cover unlock, workspace, detail,
   Settings, locked state, hint bar, which-key, palette, search, read-only and
   status affordances. They reject secret canaries and raw control/escape
   sequences. Rendering the same state under color and accessible/monochrome
   themes must preserve the semantic transcript, proving required information
   is not carried by color alone.
3. Map every removed manual test ID to an existing automated test, a new
   journey/contract test, or one of the narrowly irreducible real-environment
   cases. Add a focused Make target for the new category while keeping it under
   the ordinary `make check` suite.
4. Delete the prior manual display and screen-reader test documents from both
   `tui-skeleton/verifications/` and `tui-enhancements/verifications/`, along
   with support fixtures used only by those documents. Replace them with one
   central compact real-terminal matrix and one central screen-reader
   acceptance document. The terminal matrix names Konsole, the terminal
   actually available on Pop!_OS, and a recorded macOS terminal without
   requiring any brand. The screen-reader document contains exactly one Linux
   Orca/AT-SPI speech case and one macOS VoiceOver case.
5. Update authoritative verification indexes, coverage mappings, and live
   feature status so T7.4/T5.3 close on automated deterministic evidence. The
   retained real-machine cases are release/platform confidence checks and do
   not block this package.

### Architecture and test decisions

- Test helpers remain `#[cfg(test)]` inside `hidlins-tui`, where they can reach
  crate-private `App::render` and event seams without widening production APIs.
- The existing Ratatui `TestBackend`, crossterm event types, deterministic
  fixtures, and hand-rolled snapshot infrastructure are sufficient; Cargo
  manifests, `Cargo.lock`, and `vendor/` must not change for this work.
- Journey assertions combine state and rendered semantics. Goldens remain
  useful for stable layouts, but accessibility contracts use focused semantic
  assertions so harmless spacing changes do not erase coverage.
- Automation does not claim to test AT-SPI, speech-dispatcher, Braille hardware,
  terminal-native selection, or subjective speech quality. Those boundaries
  are stated in the retained compact matrix.

### Revised task sequence

8. Task 008 adds and validates the in-process TUI journeys, accessibility
   contracts, coverage mapping, and focused Make target.
9. Task 009 removes the obsolete manual suites and consolidates the remaining
   portable real-terminal and screen-reader acceptance documentation and
   tracking.

### Revision-specific risks

- Transcript flattening can become a weak snapshot clone. Tests therefore
  assert semantic labels, state carriers, secret absence, theme equivalence,
  and control-character exclusion rather than pinning every space.
- Tests can accidentally use a benign password whose characters occur in UI
  prose. A distinctive secret canary is used and asserted absent as a complete
  value; masked output is asserted present where password input is expected.
- Removing historical manual suites can leave dangling links or stale “sole
  open gate” claims. Task 009 performs repository/notebook link and status
  searches after deletion.

### Revision-specific out of scope

- Adding AT-SPI, Orca, speech-dispatcher, PTY, GUI-session, or emulator
  dependencies to automated CI.
- Claiming that a TestBackend transcript proves a particular terminal emulator
  exposes the same text to assistive technology.
- Executing the retained real-machine matrix as part of this work package.

### Revision-specific acceptance criteria

- Scripted journeys exercise the deterministic unlock/browse/lock,
  discoverability, search, navigation, configuration/theme, read-only, bulk,
  and injected mouse paths formerly duplicated in manual documents.
- Accessibility contracts flatten actual rendered buffers and prove required
  labels/states, secret-canary absence, color-independent semantics, and absence
  of raw escape/control sequences across representative states and minimum
  supported geometry.
- The new focused Make target and the unchanged package quality gates pass, and
  no dependency or vendored-source change is introduced.
- All prior TUI manual display/screen-reader test files and manual-only support
  fixtures are deleted; every prior test ID has an automated or retained
  real-environment disposition.
- The remaining central real-environment documentation is portable, concise,
  and limited to the requested terminal matrix, one Linux Orca case, and one
  macOS VoiceOver case.

## Approved-plan revision — Revision 5

### Validation correction

Task 008's previous `git diff --exit-code -- Cargo.lock vendor/` validation was
not task-scoped. Because Tasks 001–007 and Task 008 share one cumulative,
uncommitted worktree, it detected Task 001's completed and approved dependency
remediation and could never pass for Task 008.

Revision 5 replaces that check with a deterministic aggregate SHA-256
fingerprint captured before Task 008 resumes. The fingerprint covers every
non-vendored workspace `Cargo.toml`, `Cargo.lock`, and every regular file under
`vendor/`, including file paths and contents. Its frozen expected value is
stored directly in Task 008's validation command. The check therefore detects
added, removed, renamed, or modified dependency files after this revision while
accepting the dependency state already established by completed Task 001.

No product, dependency, gate, task sequence, or acceptance scope changes in
this revision. Tasks 001–007 and their evidence remain immutable; Tasks 008
and 009 receive fresh retry budgets upon re-approval.

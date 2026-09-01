# Task 008: Automated TUI Journeys and Accessibility Contracts

Delegation: main-only

## Goal

Replace the deterministic portion of the obsolete TUI manual suites with
scripted in-process Ratatui journeys and semantic accessibility-contract tests,
without adding a dependency.

## Context

The interrupted Task 008 already recorded passing MinIO, real-clipboard, and
KeePassXC interop evidence plus a precise Linux logind environment limitation.
Its manual TUI phase could not start because the documents were non-portable and
duplicated deterministic behavior. Revision 4 changes completion evidence: the
application's deterministic states and rendered semantics must be automated;
only emulator/AT-SPI/speech behavior remains in a compact external matrix owned
by Task 009. Revision 5 replaces the invalid cumulative-worktree dependency
check with a frozen fingerprint of the dependency state captured before this
task resumes.

## Scope

### In scope

- A test-only TUI journey harness using the existing `App`, Ratatui
  `TestBackend`, deterministic fixtures/fake time, and crossterm key/mouse
  events.
- Scripted rendered journeys covering unlock/browse/lock, discoverability
  chrome, fuzzy search, navigation, configuration/theme behavior, read-only and
  bulk-operation affordances, and injected mouse behavior/keyboard parity.
- A semantic buffer flattener that returns row-major reviewable text from an
  actual Ratatui render.
- Accessibility contracts for important labels and states, secret-canary
  exclusion, masked password output, color-independent semantics, absence of
  raw escape/control sequences, and minimum supported geometry.
- A coverage map disposing every old tui-skeleton and tui-enhancements manual
  test ID to existing automation, new automation, or the narrowly irreducible
  Task 009 real-environment case.
- A discoverable Make target for the new focused test category.
- Preservation in final Task 008 evidence of the already-recorded live-service
  results and host limitation from the interrupted attempt.

### Out of scope

- PTY, subprocess terminal, AT-SPI, Orca, speech-dispatcher, GUI-session, or
  terminal-emulator automation.
- Adding or upgrading dependencies.
- Executing human audio, Braille, native selection, or subjective legibility
  checks.
- Re-running already-passing MinIO, clipboard, or KeePassXC checks solely to
  replace the preserved interrupted evidence; package final gates still run as
  approved.

## Implementation requirements

- Work test-forward. Add the smallest failing journey/contract assertion for
  each uncovered behavior, capture the failure, then add only the harness or
  production repair required to pass it.
- Keep all new harness code under `#[cfg(test)]` inside `hidlins-tui`; do not
  widen production visibility for tests.
- Drive the real application event and render seams rather than testing a
  parallel model of the UI.
- Reuse `ratatui::backend::TestBackend`, current fixture builders, fake clock,
  crossterm event types, and the existing hand-rolled golden infrastructure.
- The semantic transcript helper must preserve row order, normalize only
  irrelevant trailing whitespace/blank rows, and never strip control
  characters before the control-character assertion examines them.
- Use a distinctive secret canary. Assert the complete canary is absent from
  every reviewable transcript and masked output is present where applicable.
- Prove color independence by comparing semantic transcripts of the same state
  under a color theme and accessible/monochrome theme, and by asserting textual
  carriers for important status such as expired, disabled, scope, selection or
  read-only state where applicable.
- Assert no ESC byte and no disallowed C0/C1 control character appears in the
  flattened transcript.
- Journey assertions must combine application-state outcomes with rendered
  semantics; snapshots alone do not satisfy this task.
- Add `make test-tui-contracts` (or an equivalently clear name) and keep the new
  tests included in ordinary `make test` / `make check`.
- `Cargo.toml`, `Cargo.lock`, and `vendor/` must remain unchanged by this task.
- If automation reveals a product defect, preserve fail-before evidence, repair
  it within the approved architecture, and rerun the focused and standard
  gates.

## Acceptance criteria

- [ ] Scripted in-process journeys render and assert the representative flows
      formerly duplicated by the old display suites.
- [ ] Accessibility contracts use actual Ratatui buffers and assert important
      labels/states across unlock, workspace/detail, Settings, locked,
      discoverability, search, read-only, and constrained-size surfaces.
- [ ] A secret canary never appears in the semantic transcripts, while password
      fields expose only an intentional masked carrier.
- [ ] Equivalent color and accessible/monochrome renders produce the same
      required semantic transcript and important states have textual carriers.
- [ ] Flattened reviewable output contains no raw escape or disallowed control
      sequences.
- [ ] Every removed manual test ID has a documented automated or retained
      real-environment disposition.
- [ ] The focused Make target is documented by `make help`, passes, and remains
      covered by the ordinary Rust test gate.
- [ ] No dependency, lockfile, or vendored-source change is introduced.
- [ ] Task evidence carries forward the interrupted live-service results without
      relabeling the Linux logind environment halt as a pass.

## Validation

- `make test-tui-contracts`
- `make snapshots-check`
- `make check`
- `python3 -c 'from hashlib import sha256; from pathlib import Path; root=Path("."); manifests=sorted(p for p in root.rglob("Cargo.toml") if not ({".git","target","vendor"} & set(p.parts))); files=[Path("Cargo.lock"),*manifests,*sorted(p for p in Path("vendor").rglob("*") if p.is_file())]; h=sha256(); [(h.update(p.as_posix().encode()+b"\0"),h.update(p.read_bytes()),h.update(b"\0")) for p in files]; actual=h.hexdigest(); expected="e8305fdc948cc1f58b189a88566c1e5f237c2e3827c4672853ea0f3e1b4e2804"; assert actual == expected, f"dependency surface changed after Task 008 baseline: expected {expected}, got {actual}"; print(actual)'`

## Dependencies

- Task 007

## Expected areas of change

- `crates/hidlins-tui/src/lib.rs`
- `crates/hidlins-tui/src/*journey*tests.rs`
- `crates/hidlins-tui/src/*accessibility*tests.rs`
- `crates/hidlins-tui/tests/snapshots/` only when a new stable checkpoint
  materially benefits from a golden
- `Makefile`
- `.ai/workflow/tui-verification-coverage.md`
- `.ai/workflow/evidence/008.md`

## Risks / notes

The test backend validates Hidlins' terminal-cell semantics, not what a
particular emulator exports over AT-SPI. Keep that boundary explicit. Avoid
turning semantic contracts into brittle full-screen snapshots: pin meaningful
text and state, not every space. The dependency fingerprint is intentionally
task-scoped: it accepts completed Task 001 changes but fails on any later path
or content change across workspace manifests, the lockfile, or vendored files.

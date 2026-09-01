# Task 009: Consolidate TUI Real-Environment Verification

Delegation: main-only

## Goal

Completely remove the previous duplicated TUI manual-test suites and replace
them with a small, portable, non-gating real-terminal and screen-reader
acceptance matrix backed by the Task 008 automation coverage map.

## Context

The tui-skeleton and tui-enhancements notebook trees each contain large manual
display and screen-reader suites. They duplicate deterministic behavior, embed
machine-specific paths, assume GNOME Terminal in Linux instructions, and leave
feature completion coupled to an unavailable human-audio session. Task 008
automates deterministic coverage. This task retains only the irreducible real
environment checks requested by the human.

## Scope

### In scope

- Delete every prior manual test document under both features'
  `verifications/02-local-display/` and `verifications/03-screen-reader/`
  directories, including superseded/scaffold files.
- Delete verification fixtures and scratch material used only by those manual
  documents.
- Create one central compact real-terminal matrix covering:
  - Konsole on KDE;
  - the terminal actually installed on Pop!_OS, recorded by name/version;
  - a macOS terminal, recorded by name/version.
- Limit the terminal matrix to irreducible emulator behavior: terminal startup
  and restoration, native selection versus mouse capture, minimum-size
  legibility, and platform/color rendering smoke checks.
- Create one central compact screen-reader acceptance document containing:
  - exactly one Linux Orca/AT-SPI speech-integration case;
  - exactly one macOS VoiceOver case;
  - password non-echo and escape-sequence-noise observations.
- Make paths portable: derive the repository root with Git before assigning an
  isolated test HOME; use `mktemp`/platform temporary directories; never embed
  `/Users/jason`, `/home/jason`, or a required notebook checkout location.
- Describe capabilities and prerequisites rather than requiring GNOME Terminal
  or a GNOME desktop.
- Update the central verification index/coverage matrix and both feature
  README/index/status/task-tracking surfaces to point to automated evidence and
  the compact central matrix.
- Close stale “sole open manual gate” claims for tui-skeleton T7.4 and
  tui-enhancements T5.3 based on Task 008 deterministic automation, while
  honestly labeling real-emulator/screen-reader execution as external,
  non-gating acceptance evidence.
- Remove all dangling links and active tracking references to deleted manual
  test files and IDs.

### Out of scope

- Executing the retained terminal, Orca, or VoiceOver cases during this package.
- Claiming Task 008 automation validates AT-SPI, speech output, Braille, or
  subjective usability.
- Rewriting historical narrative solely to erase the fact that earlier plans
  chose manual verification; add concise supersession notes where history must
  remain understandable.
- Changing unrelated feature verification suites.

## Implementation requirements

- Treat the external notebook as authoritative documentation in scope for this
  explicit revision; request filesystem authorization when writing it.
- The old manual test documents must be deleted, not retained with deprecation
  banners.
- The compact matrix must not reproduce the old step-by-step deterministic
  suites. Link each deterministic concern to the Task 008 automated coverage
  map instead.
- Do not name a terminal brand as an accessibility prerequisite. Record the
  terminal under test and require only that it exposes reviewable terminal text
  through the platform accessibility stack.
- Linux screen-reader acceptance uses Orca and AT-SPI. macOS screen-reader
  acceptance uses VoiceOver; do not instruct macOS users to install or run
  Orca.
- Define the retained cases as real-environment confidence checks, not this
  work package's automated quality gate.
- Use `$HOME` only for locating a default checkout before `HOME` is isolated;
  prefer `git rev-parse --show-toplevel` for the repository itself.
- Update all live indexes and coverage tables atomically with deletion so there
  are no dangling links.

## Acceptance criteria

- [ ] Both features' previous `02-local-display/` and `03-screen-reader/`
      manual-test trees are absent.
- [ ] Manual-only fixtures/scratch artifacts are absent or have an explicit
      surviving automated consumer.
- [ ] The central terminal matrix contains only the requested Konsole, actual
      Pop!_OS-terminal, and macOS rows and only irreducible emulator checks.
- [ ] The central screen-reader document contains one Linux Orca/AT-SPI case
      and one macOS VoiceOver case, with password non-echo and control-sequence
      noise criteria.
- [ ] No retained instruction contains a Jason-specific absolute path, assumes
      GNOME Terminal, or resolves the repository through the isolated test HOME.
- [ ] Current indexes, coverage mappings, and feature status contain no dangling
      references or stale mandatory-manual-gate claims.
- [ ] T7.4 and T5.3 are reconciled to Task 008 automated evidence without
      claiming the external real-machine cases were executed.
- [ ] Repository standard quality gates remain green after the documentation
      consolidation.

## Validation

- `test ! -d "$HOME/Developer/sources/personal/notebook/projects/hidlins/features/tui-skeleton/verifications/02-local-display"`
- `test ! -d "$HOME/Developer/sources/personal/notebook/projects/hidlins/features/tui-skeleton/verifications/03-screen-reader"`
- `test ! -d "$HOME/Developer/sources/personal/notebook/projects/hidlins/features/tui-enhancements/verifications/02-local-display"`
- `test ! -d "$HOME/Developer/sources/personal/notebook/projects/hidlins/features/tui-enhancements/verifications/03-screen-reader"`
- `rg -n 'Konsole|Pop!_OS|macOS|Orca|AT-SPI|VoiceOver' "$HOME/Developer/sources/personal/notebook/projects/hidlins/verifications/tui-real-terminal-matrix.md" "$HOME/Developer/sources/personal/notebook/projects/hidlins/verifications/tui-screen-readers.md"`
- `make test-tui-contracts`
- `make check`

## Dependencies

- Task 008

## Expected areas of change

- `$HOME/Developer/sources/personal/notebook/projects/hidlins/features/tui-skeleton/verifications/`
- `$HOME/Developer/sources/personal/notebook/projects/hidlins/features/tui-enhancements/verifications/`
- `$HOME/Developer/sources/personal/notebook/projects/hidlins/verifications/`
- Current tui-skeleton/tui-enhancements feature indexes and task/status tracking
- `.ai/workflow/tui-verification-coverage.md`
- `.ai/workflow/evidence/009.md`

## Risks / notes

The notebook is outside the repository writable root, so implementation will
cross a normal filesystem authorization boundary. Deletion scope must be
resolved exactly before removal. Historical plan prose may retain dated context,
but no current index may continue to present the deleted suites as executable or
mandatory.

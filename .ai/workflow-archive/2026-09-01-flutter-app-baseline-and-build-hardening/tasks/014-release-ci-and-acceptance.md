# Task 014: Finish release, CI, and cross-platform acceptance

Delegation: main-only

## Goal

Close the work package with one documented Makefile-driven artifact/verification
matrix, complete warning enforcement, and auditable cross-platform evidence.

## Context

Platform tasks add their own builds and checks. A final integration pass must
prove they compose, CI matches developer commands, requirements are traceable,
and the reviewed main baseline has not regressed.

## Scope

### In scope

- Audit all Makefile/CI build paths for warning-as-error inheritance and ensure
  every CI command has a discoverable Makefile target.
- Integrate Linux, macOS, iOS, and Android artifact builds plus Rust `release`
  into the appropriate CI matrix without pretending one host builds all targets.
- Complete contributor/release documentation, artifact locations, toolchain pins,
  platform limits, no-signing policy, automated verification commands, and the
  explicit user-skipped validation register.
- Document the brand asset provenance/license posture, canonical masters, platform
  mapping, and how to update icons without adding a generator dependency.
- Map the delivered behavior to relevant PRD requirements and original alpha goals,
  then run the final whole-package review and validation.

### Out of scope

- Signing/notarization, stores, publishing, Windows/web, biometrics, background
  sync, SSH-agent work, or speculative cleanup.
- Archiving the workflow; that requires a later explicit human command.

## Implementation requirements

- CI invokes `make` targets only for repository workflows.
- The final matrix records which Make target ran every platform build and
  automatable acceptance case, with host/simulator-or-emulator/artifact identity and
  secret-bearing evidence redacted.
- Add a deterministic acceptance-evidence audit that fails when an automatable
  criterion lacks results, when an undocumented manual case appears, when a
  skipped residual lacks its required automated precursor, or when skipped work
  is represented as passing.
- Audit warnings in Rust, Dart, Linux, Apple, and Android first-party compilation;
  do not weaken gates to accommodate repository-owned warnings.
- Audit and document the Flutter 3.47.2 Android build-model exception accurately:
  built-in Kotlin enabled, `android.newDsl=false` retained, Kotlin 2.4.0 declared
  with `apply false`, no Hidlins module applying KGP, and no dependency-validation
  bypass. Do not report the required old-DSL flag as unfinished Kotlin migration.
- Whole-package review repairs all high-confidence findings in approved scope
  within the workflow retry limit.
- Record the final Git revision only after all commands and acceptance cases pass.

## Acceptance criteria

- [ ] `make release` produces optimized CLI, TUI, and agent binaries with fatal
  first-party Rust warnings and documented artifact locations.
- [ ] Linux, macOS, iOS, and Android alpha artifacts build through documented
  Makefile targets on their supported hosts; CI calls the same targets.
- [ ] The warning contract and every first-party native/Dart/Rust compiler policy
  are active in normal build targets and the final matrix is warning-free.
- [ ] The final matrix records the checked Flutter 3.47.2 Android configuration
  and distinguishes its required old-DSL compatibility flag and settings-only
  compiler pin from legacy KGP application.
- [ ] Every automatable PRD/alpha requirement has machine-produced evidence,
  including simulator/emulator and managed-MinIO cases; manual acceptance is
  skipped by user decision, optional live credentialed-S3 runs are non-gating,
  and every residual is recorded honestly after its automated precursors.
- [ ] The July task audit records every historical task as already-covered,
  revalidated, superseded with rationale, or represented by completed work in this
  package; no unchecked high-value invariant is lost.
- [ ] `make verify` and the final commands in `gate.json` pass, the whole-package
  review is clean, and `evidence/final.md` records the final revision.

## Validation

- `make release`
- `make acceptance-evidence-check`
- `make verify`
- `make app-build-macos`

## Dependencies

- Task 013

## Expected areas of change

- `Makefile` and CI workflows
- Contributor, build, release, platform-limit, automated verification, and
  user-skipped validation documentation
- `.ai/workflow/evidence/` during approved execution

## Risks / notes

Final evidence spans multiple hosts and simulator/emulator matrices. Missing
required automated results must set the workflow to `BLOCKED`; broad CI green
status does not substitute for the owning test target. Physical-device, manual,
and live credentialed-service evidence are not required under Revision 5, but
their skipped status and residual uncertainty must be explicit.

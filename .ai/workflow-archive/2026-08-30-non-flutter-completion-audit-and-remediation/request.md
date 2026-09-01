# Non-Flutter Completion Audit and Remediation

The Hidlins feature plan was partially implemented. Its authoritative entry point is:

`/home/jason/Developer/sources/personal/notebook/projects/hidlins/index.md`

The expected state is that everything is complete except the Flutter features.

This work package has two goals:

1. Review everything previously communicated as complete and confirm that it is actually complete, because prior implementation reports may have overstated completion or left work partial.
2. Perform an in-depth, part-by-part review of the application.

Create a concrete fix list from the review findings, then implement and validate all approved in-scope fixes.

Flutter features are excluded from this work package.

## Pre-approval clarification — Revision 2

The work package must also satisfy these requirements:

1. All implementation is test-forward, and the repository's `AGENTS.md` must state the operational test-forward rule explicitly.
2. External dependencies must be kept to an absolute minimum. The review must actively consider replacing a simple dependency with a small in-repository implementation when doing so safely reduces the dependency and transitive supply-chain surface.
3. Dependency-license requirements are absolute and may never be violated or bypassed.
4. A completion claim is not sufficient evidence of architectural quality. Completed code must be reviewed for maintainability, testability, separation of concerns, dependency direction, state ownership, abstraction fit, simplicity, and long-term durability.
5. Use the `$arch-reviewer` skill as part of the review and remediation workflow so architecture defects with concrete costs are identified and fixed, not merely documented.

## Plan-change revision — Revision 3

Resolve the supply-chain issues now rather than deferring them to the later cross-cutting repair task.

As part of that remediation, actively remove dependencies taken for simple functionality that can be implemented directly in the repository with less total risk and maintenance burden. Preserve the existing prohibitions on hand-rolled cryptography and other security-critical protocols or formats.

## Approved-plan revision — Revision 4

Replace the obsolete TUI manual-verification approach with automation-first
coverage:

- Move deterministic behavior into automated tests.
- Add scripted TUI journeys using the existing in-process Ratatui test backend,
  without adding a dependency.
- Add accessibility-contract tests that flatten the rendered terminal buffer
  into reading order and assert:
  - important labels and states are present;
  - password characters are absent;
  - text does not depend solely on color;
  - no raw escape sequences leak into reviewable output.
- Retain a small real-terminal matrix for Konsole, the available Pop!_OS
  terminal, and macOS.
- Retain one Linux Orca acceptance test for actual AT-SPI/speech integration.
- Use VoiceOver, not Orca, for macOS screen-reader acceptance.

Completely remove the previous manual verification tests. Cover as much of
their deterministic behavior as possible through the new automation, and do
not keep the work package blocked on executing the retained real-environment
acceptance matrix.

## Approved-plan revision — Revision 5

Fix Task 008's frozen dependency validation. The repository-wide command
`git diff --exit-code -- Cargo.lock vendor/` incorrectly examines the entire
cumulative worktree and therefore fails on Task 001's approved `Cargo.lock`
and `vendor/` changes even though Task 008 introduced no dependency changes.

Replace that command with a task-scoped comparison against the dependency
state at the start of the resumed Task 008, or another task-scoped baseline,
so the validation detects dependency changes owned by Task 008 without
rejecting earlier completed work.

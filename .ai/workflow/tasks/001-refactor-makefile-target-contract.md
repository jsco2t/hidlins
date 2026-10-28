# Task 001: Refactor the Makefile target contract

Delegation: main-only

## Goal

Create and enforce the activity-first Makefile target hierarchy, reduce the public surface, preserve recipe behavior, and document the contract in root `AGENTS.md`.

## Context

The current Makefile exposes 99 public targets with inconsistent activity ordering and several implementation-only helpers. This task establishes the approved target taxonomy before repository callers are migrated. It owns the test-forward fail-before/pass-after evidence for the Makefile itself.

## Scope

### In scope

- Inventory and classify every current target as aggregate, focused public leaf, private helper, or retired/replaced name.
- Add `tools/dev/test_makefile_targets.py` using only the Python standard library.
- Run the new test against the current Makefile and record the expected red result before changing target definitions.
- Refactor target names, dependencies, `.PHONY` declarations, recursive Make calls, comments, error hints, and help rendering inside the Makefile.
- Add truthful aggregate roots where the activity has a coherent complete operation.
- Hide implementation-only helpers from the public help surface.
- Add the public `check-makefile` gate.
- Create root `AGENTS.md` with the approved naming and aggregation rules.
- Record the green contract-test result after implementation.

### Out of scope

- Migrating target references outside `Makefile`, `AGENTS.md`, and the new contract test.
- Changing underlying build/test/tool recipes or platform artifact behavior.
- Editing CI, product docs, source comments, vendored content, or workflow archives.

## Implementation requirements

- Preserve recipe bodies and their security/offline/platform behavior while moving them to activity-first names.
- Rename legacy partial roots to focused leaves before using their unqualified names as aggregates.
- Keep `run-app` and `run-tui` as isolated leaves and do not define `run`.
- Do not create compatibility aliases for any retired target.
- Private helpers must begin with `_`, must not carry public help descriptions, and must not be used by external tracked callers.
- `make help` must be grouped, deterministic, complete, and capable of recognizing digits in valid future target names.
- The contract test must assert at least: naming grammar, activity-first replacement families, aggregate membership, isolated-family exclusions, public count reduction, help completeness/uniqueness, private-helper hiding, required `run-*` behavior, and absence of retired aliases from the Makefile target surface.
- The test must keep the retired mapping declarative so Task 002 can extend it to repository-wide reference scanning.
- `AGENTS.md` must include the literal conceptual rule that `make something` runs all of `something`, `make something-part` runs only that portion, and names follow `[major-activity]-[portion]-[subportion-if-needed]`.

## Acceptance criteria

- [ ] A fail-before result from the new contract test is recorded before the Makefile refactor.
- [ ] The documented public target count is lower than 99 and implementation helpers are no longer public.
- [ ] Public target names, roots, aggregates, and isolated families satisfy the approved architecture.
- [ ] `run-app` and `run-tui` are public; `run` and `app-run` are absent.
- [ ] No retired target remains as a compatibility alias.
- [ ] `make help` is grouped and exactly matches the public target contract.
- [ ] Root `AGENTS.md` documents the requested standard and exceptions.
- [ ] The contract test passes after the refactor without external dependencies.
- [ ] Existing recipes retain their commands, flags, guards, and artifacts under their replacement targets.

## Validation

- `python3 -B tools/dev/test_makefile_targets.py`
- `make help`
- `make -n check-rust`
- `git diff --check`

## Dependencies

None

## Expected areas of change

- `Makefile`
- `AGENTS.md`
- `tools/dev/test_makefile_targets.py`

## Risks / notes

This task makes the architectural naming and aggregation decisions and is therefore main-only. External references are expected to remain stale until Task 002, so the first version of the contract test must validate the Makefile surface without prematurely failing on repository callsites.

# Task 007: Build, Supply-Chain Reconciliation, and Documentation Repairs

Delegation: main-only

## Goal

Repair all remaining Task-001 findings in build/CI workflows, harness integrity, and in-repository documentation; verify that later subsystem work did not regress the supply-chain posture restored in Task 001; then reconcile the fix ledger.

## Context

Task 001 owns the current advisory/yank remediation and simple-dependency removals because no later task can pass the standard gate until those are fixed. This task is the final cross-cutting reconciliation point: passing commands must remain warning-clean, recurring workflows must be exposed through `make`, and repository documentation must describe actual behavior.

## Scope

### In scope

- Makefile, CI, deny/audit configuration, offline vendoring, scripts, committed generated artifacts, README/CONTRIBUTING, and crate documentation after the Task-001 supply baseline is green.
- Reconcile every fix-ledger entry to fixed, verified, optional, environment-only, or plan-change-required.
- Record exact external notebook corrections without editing the notebook.

### Out of scope

- Flutter-only build/test commands except preventing regressions in shared workspace commands.
- External notebook writes.
- Deferring or first addressing `event-listener 5.4.1`, `lru 0.16.4`, yanked `chacha20 0.10.0`, unused `anyhow`, local replacement of `hex`, or missing-license verification; Task 001 must complete those items.

## Implementation requirements

- Fix every ledger finding assigned to Task 007.
- Add a failing harness/policy regression before behavior-changing build or CI fixes when feasible; for policy/docs-only changes, record the exact static validation that substitutes for red/green evidence.
- Any new recurring command or harness must have a Makefile target and CI/dev parity.
- Supply-chain warnings must remain surfaced honestly. Re-run and reconcile the Task-001 graph decisions after all subsystem changes; no later task may reintroduce a removed advisory/yank path or unexplained exception.
- Reconcile `.ai/workflow/dependency-review.md` against the final manifests/lock/vendor tree. Every kept or changed dependency must have a verified allowed license; no forbidden, ambiguous, missing/unverified, or linking-exception license may be waived. Do not weaken `deny.toml` or `.cargo/audit.toml` to make an unacceptable dependency pass.
- For every dependency changed after Task 001, record the before/after direct and transitive footprint plus why an in-repository implementation was selected or rejected. Behavior-pin tests must precede removals/reimplementations. This is reconciliation, not authorization to postpone known Task-001 changes.
- Remove conflict markers and stale claims from in-repository docs.
- Update `.ai/workflow/findings.md` dispositions without deleting original observations.

## Acceptance criteria

- [ ] Every actionable ledger item has a final disposition and evidence.
- [ ] Test-forward evidence is complete for all behavior repairs and structural refactors; non-code exceptions identify their substitute validation.
- [ ] Makefile and CI commands agree for non-Flutter gates.
- [ ] Offline vendor, license, ban, source, and advisory policies pass without unexplained output.
- [ ] The Task-001 removals/upgrades remain present and no later subsystem change reintroduces `anyhow`, external `hex`, or the remediated advisory/yank versions.
- [ ] `.ai/workflow/dependency-review.md` matches the final dependency graph and contains no unverified or disallowed license.
- [ ] In-repository documentation describes actual behavior and limitations.
- [ ] Exact notebook drift/corrections are listed for the human.

## Validation

- `git diff --check`
- `make completions-check`
- `make snapshots-check`
- `make doc`

## Dependencies

- Task 006

## Expected areas of change

- `Makefile`
- `.github/workflows/ci.yml`
- `deny.toml`
- `.cargo/audit.toml`
- `.cargo/config.toml`
- `tools/`
- `README.md`
- `CONTRIBUTING.md`
- crate documentation
- `.ai/workflow/findings.md`
- `.ai/workflow/dependency-review.md`

## Risks / notes

Do not modify user-owned baseline files unless a ledger finding specifically requires an overlapping edit; inspect and preserve their existing changes first.

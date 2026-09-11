# Task 006: Coverage Contract Audit

Delegation: main-only

## Goal

Reconcile the five repaired state machines with their tests and active coverage claims, closing any remaining adjacent high-value gaps without adding redundant tests.

## Context

The escaped defects passed the aggregate suite because several tests bypassed production orchestration or asserted only a narrower property than their labels and matrices implied. Each repair task audits its immediate transitions; this final task verifies the combined coverage contract and removes remaining overstatement.

## Scope

### In scope

- Inventory of tests, stable identifiers, names, comments, and assertions covering the five findings.
- Active `protocol-v1`, threat-model, security-coverage, test-matrix, and release-review claims.
- Cross-surface verification that tests enter production seams instead of manually arranging the postcondition being claimed.
- Additional tests for remaining distinct high-value security, recovery, durability, concurrency, and interface boundaries adjacent to the repairs.
- Superseding evidence for inaccurate historical claims without modifying archived workflow files.

### Out of scope

- Broad repository line-coverage goals.
- Tests that duplicate an already exercised production transition without a distinct assertion or failure mode.
- Unrelated feature bugs or speculative future-platform coverage.
- Editing `.ai/workflow-archive/`.

## Implementation requirements

- Build a finding-to-test map that names the production entry point, injected failure/order, observable assertion, and owning test for each repaired issue.
- Compare test names/comments and active documentation to the executable path. Narrow or rename overstatements; never retain a claim based only on a helper called directly when production orchestration is the intended guarantee.
- Review the adjacent-transition decisions recorded in Tasks 001–005. Add any remaining test only when absence could plausibly allow a security, data-loss, stuck-worker, stale-callback, or cross-interface regression to escape.
- If this audit discovers a new behavior defect within the same five state machines, add a failing regression first, record red/green evidence in `evidence/006.md`, and repair it within this task. If it would materially expand scope or redesign the approved architecture, enter `PLAN_CHANGE_REQUIRED`.
- Preserve stable test identifiers where they remain accurate; update matrices and identifiers together when semantics change.
- Explicitly distinguish automated assertions from manual observations and unexecuted optional scenarios.

## Acceptance criteria

- [ ] Every finding maps to at least one test that executes the relevant production seam and asserts the escaped failure boundary.
- [ ] No active test name, comment, coverage matrix, protocol note, threat-model control, or release-review statement overstates the path or boundary actually executed.
- [ ] Each adjacent candidate gap has a recorded value judgment: covered, newly covered with rationale, or omitted as duplicative/low-value with evidence.
- [ ] Any newly confirmed in-scope defect has its own fail-before/pass-after record.
- [ ] Archived workflow files are unchanged and current evidence clearly supersedes inaccurate historical claims.
- [ ] Focused local-sync security, discovery, integration, and acceptance-evidence checks pass together.

## Validation

- `make test-local-sync-security`
- `make test-local-sync-discovery`
- `make test-local-sync-integration`
- `make acceptance-evidence-check`

## Dependencies

- Task 005

## Expected areas of change

- Tests under `crates/hidlins-sync/tests/`, `crates/hidlins-api/tests/`, `crates/hidlins-cli/tests/`, and `crates/hidlins-tui/tests/`
- Native tests under `app/ios/RunnerTests/` and analogous platforms only where justified
- `crates/hidlins-sync/docs/protocol-v1.md`
- `crates/hidlins-sync/docs/threat-model.md`
- `crates/hidlins-sync/docs/security-coverage.md`
- `crates/hidlins-sync/docs/test-matrix.md`
- `docs/local-network-sync-release-review.md`
- `.ai/workflow/evidence/`

## Risks / notes

Historical evidence is provenance and remains immutable even when its claim was too broad. The correction mechanism is accurate current tests/docs plus explicit superseding evidence in this work package, not rewriting history.


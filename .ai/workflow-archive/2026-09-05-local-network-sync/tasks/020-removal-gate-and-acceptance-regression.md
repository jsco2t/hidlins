# Task 020: Close the Removal-Gate Gap and Re-run Integrated Acceptance

Delegation: main-only

## Goal

Make retired cloud-object transport enforcement case-complete, remove the remaining active stale reference, and prove all Round 2 repairs together across security, application, native, and simulator gates.

## Context

The S3-removal checker is case-sensitive for the MinIO name and therefore accepts uppercase `MINIO`; a stale first-party test-harness comment demonstrates the false negative. Tasks 016–019 also require a final integrated regression pass so injected test seams cannot mask live behavior again.

## Scope

### In scope

- Case-insensitive retired-transport matching and negative controls.
- Removal of remaining active stale terminology.
- Coverage/evidence documentation updates for the eleven repaired defects.
- Focused integrated security and cross-surface review.
- Complete unchanged standard and final workflow gates, including mobile scenarios.

### Out of scope

- Historical frozen workflow material and the approved removal inventory.
- New feature work beyond the eleven accepted findings.
- Manual accessibility or physical-router execution.

## Implementation requirements

- The checker must reject upper-, lower-, and mixed-case spellings without weakening reviewed unrelated-token handling.
- A checked-in negative control must reproduce the prior uppercase bypass.
- Active documentation/test matrices must name the new runtime-level coverage and must not claim injected routing proves native discovery/startup behavior.
- The final review must explicitly recheck revocation, pairing, deadlines, streaming, candidate fallback, interface refresh, startup orchestration, TUI binding, local-only enforcement, NCSA isolation, and total retired-transport removal.

## Acceptance criteria

- [ ] `MINIO`, `MinIO`, `minio`, and mixed-case retired transport/provider names are rejected outside the reviewed historical allowlist.
- [ ] The stale active reference is removed and the removal gate's self-test includes the exact escaped case.
- [ ] Test/security matrices accurately map all eleven findings to automated runtime or surface tests.
- [ ] Focused review finds no unresolved high-confidence critical/important defect in the repaired paths.
- [ ] Every standard and final command in unchanged `gate.json` passes in order.
- [ ] Round 2 evidence records fail-before/pass-after results and distinguishes simulator routing limits from real native discovery coverage.

## Validation

- `make s3-removal-check`
- `make test-local-sync-security`
- `make test-local-sync-discovery`
- `make test-local-sync-integration`
- `make acceptance-evidence-check`
- `make ncsa-boundary-check`

## Dependencies

Task 019

## Expected areas of change

- `tools/dev/s3-removal-check.py`
- `tools/interop-tests/sync_us-044.sh`
- `crates/hidlins-sync/docs/security-coverage.md`
- `crates/hidlins-sync/docs/test-matrix.md`
- `docs/local-network-sync-release-review.md`
- Tests and documentation directly affected by Tasks 016–019

## Risks / notes

Case-insensitive matching can create noise in vendored or unrelated identifiers; preserve the narrow path/token exclusions and prove both rejection and accepted controls. This task does not replace the workflow's final whole-package review or final gate.

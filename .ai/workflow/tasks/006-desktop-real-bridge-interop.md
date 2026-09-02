# Task 006: Verify the real desktop bridge and interoperability

Delegation: main-only

## Goal

Prove the desktop alpha against the real Rust bridge, KeePassXC-compatible KDBX
files, and a two-session S3-compatible sync environment.

## Context

Widget/unit tests cannot prove native loading, secure lifecycle behavior, KDBX
round-tripping, or real merge/conflict semantics. The expected integration and
app interoperability harnesses are not present in the current branch.

## Scope

### In scope

- Add discoverable Makefile targets and deterministic harnesses for real-bridge
  desktop integration and application-level interoperability.
- Exercise vault create/import/unlock/lock/auto-lock and the representative entry
  types through compiled native code.
- Prove KeePassXC round-trip and two independent app sessions syncing through
  managed MinIO, including non-conflicting merge and conflict history retention.
- Prove sync-first bootstrap happy/rollback cases, password-change RST-CRED-1
  re-encryption/remediation, and a genuinely unresolvable fixture rather than a
  normal merge mislabeled as a conflict.
- Exercise advisory-lock coexistence with CLI/TUI and verify externally written
  changes become visible after the supported refresh/sync path.
- Cover network loss/retry/error presentation without logging credentials or
  secret values.
- Repeat the 5,000-entry search/list sanity through the compiled bridge using a
  bounded automated harness and machine-readable timing evidence.

### Out of scope

- Physical iOS/Android testing.
- Real public S3 credentials in CI.
- Changes to core KDBX or merge semantics absent a failing regression.

## Implementation requirements

- Integration tests must load the generated bridge/native library rather than a
  fake repository.
- Fixtures and credentials must be ephemeral, scoped, and scrubbed from output.
- Record red/green evidence for every newly discovered bridge or interop defect.
- Keep all harness commands behind Makefile targets and add them to CI at the
  cheapest appropriate tier.
- No interop criterion may rely on a human-driven app walkthrough; KeePassXC,
  MinIO, process coexistence, network interruption, and performance assertions
  must be scripted, bounded, self-cleaning, and failure-reporting.
- Extend/reuse the managed MinIO fixture with a configurable LAN bind and bucket
  bootstrap so later physical-device tests use the same controlled environment.
- A conflict test must inspect that the losing value is preserved in KDBX history
  under the same UUID.

## Acceptance criteria

- [ ] Real-bridge tests prove vault lifecycle, auto-lock, and representative CRUD
  without secret leakage.
- [ ] KeePassXC opens/saves a Hidlins vault and Hidlins observes no data loss on
  re-open.
- [ ] Two sessions prove non-conflicting merge and same-field conflict history
  through managed MinIO.
- [ ] Bootstrap validates before atomic install and cleans every failure artifact;
  password change preserves or explicitly remediates sealed sync credentials.
- [ ] App/CLI/TUI advisory locking and external-edit refresh behavior are proven on
  both desktop hosts.
- [ ] A real unresolvable fixture reaches the dialog with its backup path and both
  pre-conflict sides remain recoverable.
- [ ] Network interruption produces a recoverable, honest UI state and leaves the
  local vault intact.
- [ ] The automated real-bridge 5,000-entry sanity meets the PRD budget in the
  controlled measurement environment and records enough host/build identity to
  interpret the result.
- [ ] All new commands are documented Makefile targets used by CI.

## Validation

- `make app-test-integration`
- `make app-test-integration-minio`
- `make app-test-integration-performance`
- `make interop-app`
- `make app-build-macos`

## Dependencies

- Task 005

## Expected areas of change

- `app/integration_test/`
- `tools/interop-tests/`
- `Makefile` and CI workflows
- Focused bridge/repository repairs if tests expose defects

## Risks / notes

Integration output is a potential disclosure channel. Sanitize subprocess and
test failure output and never embed actual master passwords or S3 secrets in logs.

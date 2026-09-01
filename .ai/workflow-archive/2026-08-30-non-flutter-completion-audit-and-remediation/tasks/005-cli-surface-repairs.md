# Task 005: CLI Surface Repairs

Delegation: main-only

## Goal

Repair and test all Task-001 findings in the one-shot CLI, including vault/entry/generation/sync integration, secure prompts, output schemas, exit codes, and completions.

## Context

The CLI is the scriptable presentation layer. It must remain thin over repaired shared crates and preserve stable machine contracts.

## Scope

### In scope

- Parsing/dispatch, vault and entry commands, generation, sync wiring, prompt handling, JSON/human views, errors/exit codes, and completions.
- Honest behavior of unimplemented SSH/agent slots without implementing those features.
- CLI regression and command-boundary tests.

### Out of scope

- New SSH or agent functionality.
- Business logic that belongs in shared crates.

## Implementation requirements

- Fix every ledger finding assigned to Task 005.
- Write the command-boundary regression test first and record its failure before each behavior fix; add characterization tests before structural refactors; record the green result afterward.
- Fix every ≥80-confidence architecture finding assigned to Task 005, preserving a thin presentation boundary and removing duplicated business logic where found.
- Never accept a master password by command line or environment.
- Keep secrets redacted unless an explicit show/copy action authorizes egress.
- Preserve JSON schemas and documented exit codes unless the audit proves the current contract incorrect; compatibility changes require `PLAN_CHANGE_REQUIRED`.

## Acceptance criteria

- [ ] Every Task-005 ledger finding is fixed with command-level regression coverage.
- [ ] Every behavior repair has fail-before/pass-after evidence and every architecture refactor has characterization/invariant coverage.
- [ ] Secure prompt, redaction, JSON, exit-code, sync-conflict, and completion contracts pass.
- [ ] No core/sync business logic is duplicated into the CLI.

## Validation

- `cargo test -p hidlins-cli --offline --locked`
- `make completions-check`

## Dependencies

- Task 004

## Expected areas of change

- `crates/hidlins-cli/`
- `shell-completions/`
- `README.md`
- `CONTRIBUTING.md`

## Risks / notes

Generated completions are a committed compatibility surface and must be regenerated only through the Makefile target.

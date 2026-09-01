# Task 003: Security Behavior Repairs

Delegation: main-only

## Goal

Repair and test all Task-001 findings in runtime hardening, idle/OS lock behavior, clipboard handling, and secret-buffer hygiene.

## Context

The security crate crosses process, signal, DBus, clipboard, thread, and platform boundaries. The baseline feature-gated build warning and current `event-listener` advisory path require explicit disposition.

## Scope

### In scope

- Auto-lock configuration/controller and clock behavior.
- Clipboard copy/compare/auto-clear and secret ownership.
- process hardening/core-dump suppression.
- SIGTSTP/logind/IOKit sources and shutdown behavior.
- Feature-gate correctness and transitive advisory remediation for this boundary.

### Out of scope

- TUI presentation of these capabilities.
- macOS runtime execution on the Linux host.

## Implementation requirements

- Fix every ledger finding assigned to Task 003.
- Add the failing regression test before each behavior fix; add characterization/invariant coverage before any responsibility or boundary refactor; record red/green results in evidence.
- Fix every ≥80-confidence architecture finding assigned to Task 003 without weakening fail-locked behavior or introducing general-purpose frameworks.
- Make feature-gated configurations warning-clean; do not hide warnings crate-wide.
- Preserve fail-locked behavior, prompt secrecy, zeroization, and bounded background-thread shutdown.
- Upgrade/remove the affected `event-listener` path when compatible; otherwise document a code-grounded reachability decision without weakening global policy.
- Follow `.ai/workflow/dependency-review.md`; verify every changed dependency's license before use and prefer a smaller feature set or safe in-repository replacement only when it does not recreate OS, concurrency, or security primitives poorly.

## Acceptance criteria

- [ ] Every Task-003 ledger finding is fixed or has an approved evidence-backed non-reachability disposition.
- [ ] Every behavior repair has fail-before/pass-after evidence and every architecture refactor has characterization/invariant coverage.
- [ ] Default, no-default, clipboard, logind, and IOKit compile surfaces remain valid.
- [ ] All lock triggers and clipboard clear paths retain regression coverage.
- [ ] Feature-gated checks are warning-clean.

## Validation

- `cargo test -p hidlins-security --offline --locked`
- `RUSTFLAGS="-D warnings" cargo check -p hidlins-security --offline --locked --no-default-features`
- `make check-feature-gates`

## Dependencies

- Task 002

## Expected areas of change

- `crates/hidlins-security/`
- `Cargo.lock`
- `vendor/`
- `deny.toml`
- `.cargo/audit.toml`

## Risks / notes

OS-event and clipboard code has environment-dependent runtime coverage; Task 008 owns the real-host runs after source repair.

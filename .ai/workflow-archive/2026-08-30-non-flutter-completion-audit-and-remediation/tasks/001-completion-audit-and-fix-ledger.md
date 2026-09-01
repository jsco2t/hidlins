# Task 001: Guardrails, Supply-Chain Remediation, and Completion Audit

Delegation: main-only

## Goal

Codify the clarified engineering guardrails, complete the evidence-backed non-Flutter correctness/architecture/dependency audit, and restore a clean supply-chain gate before any subsystem repair begins.

## Context

The notebook contains stale, conflicting, and partially reconciled status claims. Aggregate tests currently pass while known data-integrity and verification gaps remain. The first attempted Task-001 gate also established that `make deny` is not green: `keepass 0.12.9` resolves yanked `chacha20 0.10.0`, while `make audit` reports supported-path `event-listener 5.4.1` and production-path `lru 0.16.4` advisories. Deferring those issues until Task 007 makes the standard per-task gate impossible to satisfy and conflicts with the user's requirement to resolve supply-chain issues now.

This task therefore establishes the audit ledgers first, then performs only the cross-cutting dependency remediation needed to restore the gate. Broader product repairs remain in their owning later tasks.

## Scope

### In scope

- Read the included features' implementation plans, designs, task indexes, verification matrices, source, tests, and current docs.
- Update root `AGENTS.md` before product repair so it explicitly requires test-before-fix red/green evidence, characterization/invariant tests before structural refactors, dependency necessity/removal analysis before manifest changes, pre-use license verification, and unconditional rejection of forbidden or unverified licenses.
- Trace every claimed-complete task or requirement to concrete implementation and validation evidence.
- Review each included crate part by part for correctness, security, concurrency, data integrity, compatibility, observability, test gaps, and unnecessary complexity.
- Apply `$arch-reviewer` in whole-code mode sequentially to `hidlins-core`, `hidlins-genpw`, `hidlins-security`, `hidlins-sync`, `hidlins-cli`, `hidlins-tui`, and their cross-crate dependency boundaries. Include only findings at confidence >=80/100 with a concrete cost.
- Inventory every direct dependency used by the included crates, verify its license against the allow-list, identify its purpose and meaningful transitive footprint, and decide whether to keep, feature-prune, remove, replace locally, or upgrade it.
- Create and finalize `.ai/workflow/findings.md` and `.ai/workflow/dependency-review.md` before dependency/product source remediation begins.
- Remove the unused direct `anyhow` dependency from `hidlins-tui` and correct any stale documentation that claims it is used.
- Pin the existing lowercase hexadecimal behavior with focused tests, implement the small non-cryptographic encoder in repository code, migrate all production and test call sites, and remove `hex` from the resolved graph. Do not generalize the helper beyond actual Hidlins needs.
- Upgrade the `event-listener` resolution off 5.4.1 using the narrowest compatible parent/transitive change and validate the supported `logind` feature path.
- Upgrade the `lru` resolution off 0.16.4 using the narrowest compatible `ratatui`/`ratatui-core` change and validate TUI behavior and snapshots.
- Remove the yanked `chacha20 0.10.0` path through the narrowest supported `keepass`/transitive upgrade or a narrowly audited downstream dependency patch. Never implement or modify the cipher algorithm. Revalidate KDBX round trips, merge semantics, and KeePassXC interoperability.
- Reassess existing `quick-xml` advisory exceptions after parent upgrades and remove or narrow every exception whose affected path is no longer present. Retain an exception only when exact supported-target reachability and the lack of a compatible fixed release are documented; never add an exception for the three baseline supply findings above.
- Verify the actual license file for every resolved/vendored package whose manifest has absent or ambiguous license metadata, including `allo-isolate`, and record the evidence against the repository allow-list.
- Regenerate the exact lockfile and complete vendored source tree, review the full manifest/lock/vendor diff, and preserve offline locked builds.

### Out of scope

- Broader correctness or architecture repairs assigned to Tasks 002-006.
- Hand-rolling or locally replacing cryptography, KDBX parsing/writing, TLS/HTTP, OS entropy, terminal protocols, Unicode tables, DBus, or platform FFI.
- Adding a new general-purpose dependency to work around a parent upgrade.
- Weakening `deny.toml`, `.cargo/audit.toml`, the permitted-license list, or the standard gate.
- Flutter/API implementation review, abandoned/archived work, new SSH/agent feature delivery, and external notebook edits.

## Implementation requirements

- Audit all completion claims listed in `plan.md -> Work included` and finalize the ledgers before changing manifests or product source.
- Make `AGENTS.md` rules operational: require evidence fields and ordering, not aspirational wording alone. Preserve the non-negotiable prohibition on hand-rolled cryptography/security primitives.
- Include known discrepancies: history attachments, security no-default warning, advisory/yank paths, stale bug status, pending Orca/manual checks, and external macOS/M1 gaps.
- Search for stubs, panics, TODO/FIXME markers, feature-gated uncompiled paths, ignored tests, false-green gates, missing command producers, secret-bearing debug/error paths, and docs that overstate behavior.
- Do not treat test presence as proof without checking what it asserts, and do not classify module size or style preference as an architecture defect without a concrete cost.
- For each dependency change, record purpose, exact source/license, maintenance signal, compatible alternatives, before/after direct and transitive footprint, vendored-tree impact, and the explicit local-implementation decision before changing the manifest.
- Test forward: add behavior-pin/characterization coverage and capture it passing against the old dependency before removal or upgrade; where a known defect can be safely reproduced, capture fail-before/pass-after evidence. KDBX and TUI parent upgrades additionally require existing interop/snapshot suites to remain green.
- `anyhow` must leave the direct manifest. `hex` must leave both the direct manifest and resolved graph after all call sites use a narrowly scoped local helper.
- `event-listener 5.4.1`, `lru 0.16.4`, and yanked `chacha20 0.10.0` must not appear in the final Task-001 resolved graph. Do not suppress their reports.
- Prefer released, compatible upstream versions. A downstream dependency patch is permitted only when a released parent cannot clear the yanked crypto path, and must be narrowly scoped, source-pinned/checksummed, license-compatible, locally audited, fully vendored, and covered by the same KDBX/interop gates.
- Any dependency-source download or vendoring that needs network access must use the normal Codex authorization boundary; lack of pre-existing authorization is not permission to defer the approved work.
- Do not begin Task-002 repairs until every Task-001 validation and standard command passes.

## Acceptance criteria

- [ ] `AGENTS.md` contains the clarified test-forward, dependency-minimization, dependency-removal, and absolute-license rules before any product repair task begins.
- [ ] Every included feature/bucket and recorded bug has a completion verdict with cited code/test evidence.
- [ ] Every included crate and cross-cutting harness has a documented review result.
- [ ] Every included crate and cross-crate boundary has an `$arch-reviewer` whole-code result; every >=80-confidence finding records its architecture dimension, evidence, concrete cost, and repair direction.
- [ ] Every included direct dependency has a verified-license and keep/remove/replace decision in `.ai/workflow/dependency-review.md`.
- [ ] Every actionable high-confidence defect has a unique finding ID and an owning repair task; environment-only and out-of-scope gaps are separate.
- [ ] `.ai/workflow/findings.md` contains the ordered fix list before manifest or product-source remediation begins.
- [ ] The unused `anyhow` direct dependency is removed and no stale code comment describes it as part of the TUI error path.
- [ ] A tested in-repository lowercase hexadecimal encoder covers all Hidlins formatting needs, all call sites use it, and `hex` is absent from the resolved graph.
- [ ] `event-listener 5.4.1`, `lru 0.16.4`, and yanked `chacha20 0.10.0` are absent from the resolved graph without a new ignore or weakened policy.
- [ ] Existing advisory exceptions are reconciled against the new graph and every remaining exception has exact path/non-reachability and upstream-fix evidence.
- [ ] Every missing/ambiguous manifest license is resolved from authoritative package files and is allowed; no license exception is added.
- [ ] `Cargo.lock` and `vendor/` exactly match the accepted dependency graph, and offline locked checks plus focused SigV4, merge, TUI snapshot, and KeePassXC interop validation pass.
- [ ] The full standard gate passes before Task 002 begins.

## Validation

- `test -s .ai/workflow/findings.md`
- `test -s .ai/workflow/dependency-review.md`
- `rg -n "test-forward|red.*green|dependency|license" AGENTS.md`
- `git diff --check`
- `cargo tree --workspace --offline --locked`
- `make test-sigv4`
- `make test-merge-properties`
- `make snapshots-check`
- `make interop`
- `make interop-entry`
- `make interop-sync`
- `make deny`
- `make audit`

## Dependencies

None

## Expected areas of change

- `AGENTS.md`
- `.ai/workflow/findings.md`
- `.ai/workflow/dependency-review.md`
- workspace and crate `Cargo.toml` files
- `Cargo.lock`
- `vendor/`
- `.cargo/audit.toml`
- sync encoding helper/call sites and focused tests
- TUI bootstrap documentation and dependency metadata
- KDBX/TUI compatibility adaptations required by narrow parent upgrades

## Risks / notes

This is the broadest and most security-sensitive task and remains main-only. A KDBX parent upgrade or downstream crypto-dependency patch has high blast radius even when the source change is narrow; exact interop and no-data-loss gates are mandatory. The architecture scope exceeds the review skill's usual narrowing threshold because the human explicitly requested a whole-application review, so it is performed sequentially by crate and consolidated.

If no compatible released or narrowly patchable graph can remove one of the three required baseline supply findings without violating license, cryptographic, or interoperability constraints, stop in `PLAN_CHANGE_REQUIRED`; do not retain the finding and call the task complete.

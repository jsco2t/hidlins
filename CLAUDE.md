# Hidlins

Hidlins (Scots: "in secret, in hiding", pronounced HID-linz) is an offline-first secrets manager built on a Rust core with thin cross-platform UIs. Vaults are stored in the KDBX (KeePass) format via the `keepass-rs` crate, so every Hidlins vault is directly interoperable with KeePassXC, KeeWeb, KeePass2, and mobile KDBX clients. Default sync transport is S3-compatible object storage (PRD §6.5 v1.1), with in-app three-way merge at the entry level.

**License:** MIT. **Status:** Phase-0 MVP code-complete; manual accessibility verification remains.

## Planning docs (authoritative)

Engineering plans live outside this repo in the project notebook. Read these when scoping work — do not duplicate them here.

- **PRD:** `$HOME/Developer/sources/personal/notebook/projects/hidlins/prd.md` — the product spec; section IDs (FR-xxx, NFR-xxx) are referenced throughout the plans.
- **Project index:** `$HOME/Developer/sources/personal/notebook/projects/hidlins/index.md`
- **Verifications (user scenarios):** `…/hidlins/verifications/` — every Must-Have FR maps to a US-xxx scenario.
- **Knowledge base:** `…/hidlins/kb/` — KDBX format, three-way merge algorithm, memory hygiene, supply-chain policy, library notes.
- **Features:** `…/hidlins/features/<slug>/` — per-feature implementation plan, design, task plan, follow-ups. First feature: `features/vault-core/`.

When implementing a feature, the corresponding `plans/implementation-plan.md` and `plans/design.md` are the source of truth for that feature's scope and architecture.

## Architecture posture

- **Rust core, thin UIs.** All business logic and crypto live in the Rust core library. CLI and TUI are presentation layers over the same core. Future Flutter UIs (Phase 1+) bind to the same core via FFI. No UI-layer logic leaks into the core.
- **TUI is the reference UX.** Every feature lands in the TUI with full keyboard parity before any GUI work begins. CLI provides one-shot scriptable access to every core operation.
- **Offline-first.** Every feature works without network. Sync is opt-in and operates on top of the offline core — never a precondition.
- **Sync trait abstraction.** Phase 0 sync is **S3-compatible object storage** per PRD §6.5 v1.1 (2026-05-27 — replaces the originally-planned git transport, which was abandoned mid-implementation; see notebook `features/archive/sync-git/` and PRD §C decision #17). Later transports (NFS, Samba, WebDAV, possibly a reconsidered git transport) implement the same `SyncTransport` trait. CRUD and merge logic stay transport-agnostic. The gix-based scaffolding and its vendored deps were removed in `features/s3-sync/` T1; the S3 transport (hand-rolled SigV4 + `ureq`, design.md ADR-1) landed across `features/s3-sync/` Phases 2–6.

## Engineering principles

1. **KDBX interop is a product promise, not an implementation detail.** A Hidlins vault must round-trip through KeePassXC (≥2.7) with zero observable data loss. Any custom data written into a vault must be readable by a standards-compliant KDBX client. Round-trip CI tests gate every change to the vault layer.
2. **Test-forward means test-before-fix.** For every behavior defect, first add or strengthen the smallest regression test that demonstrates the failure, record the red result, implement the minimum repair, and record the green result before refactoring. Architecture-only refactors require characterization or invariant tests before responsibilities move. A code task is not complete merely because tests exist or the aggregate suite passes; its evidence must identify the fail-before/pass-after sequence. If a change is genuinely non-code or cannot meaningfully produce a failing test, document why and name the static or manual validation used instead. Every Must-Have FR also has at least one automated test AND one user-scenario verification in `notebook/.../verifications/`. Property-based / fuzz tests for the merge engine and fault-injection tests for atomic writes are first-class deliverables, not nice-to-haves.
3. **Simplicity over cleverness.** Prefer obvious code, narrow abstractions, and well-trodden patterns. Don't introduce generality for hypothetical future requirements. Three similar lines beats a premature abstraction.
4. **No data loss, ever.** During sync conflicts, the loser is preserved as a KDBX history entry under the same UUID. Atomic writes (write-then-rename) ensure a crash never leaves a corrupted vault. Pre-merge state is kept as `.kdbx.bak`.
5. **Honest about limits.** Document platform limitations (encrypted swap, `mlock` quotas, OS lock-event reliability) rather than papering over them. Cosmic-ray paranoia (Rowhammer, cold-boot DMA) is explicitly out of scope.
6. **Limited External Dependencies** Only add external dependencies when it is very clear they solve a major gap in functionality. For minor features it's worth evaluating writing the code directly in the repo (vs adding N more dependencies).

## Security rules (non-negotiable)

- **Zeroize on drop** for every type holding sensitive bytes (master keys, derived keys, entry passwords, master-password input buffers). Use `zeroize::ZeroizeOnDrop`.
- **`mlock` the master key** where the platform supports it (best-effort; quotas are small, so don't lock everything).
- **No plaintext to disk, ever.** The unlocked KDBX is never serialized in plaintext. Never log entry contents. Never include secret material in error messages.
- **Master password collection: secure stdin prompt only.** Never accept on the command line, never accept as an env var (leaks via shell history / `ps`).
- **No master-password recovery.** Warn at vault creation; there is no escrow, backdoor, or recovery service.
- **Atomic writes.** Always write to a sibling temp file and `rename(2)`. Never truncate the live vault.
- **Advisory file locking** coordinates concurrent CLI/TUI/agent processes. Concurrent reads OK; writes serialized.
- **Disable core dumps** for the agent and TUI (`setrlimit(RLIMIT_CORE, 0)`).
- **No telemetry, no analytics, no crash reporting, no update checks.** The only network calls are user-configured sync.
- **No hand-rolled crypto.** Cryptography comes from `keepass-rs` and the RustCrypto family (Argon2, AES, HMAC, SHA). KDBX4 KDF is Argon2id, tuned to ~1s on target hardware.
- **CSPRNG only.** Password generation uses `OsRng` from `rand`.

## Supply chain rules (non-negotiable)

- **Minimize external dependencies — actively and audibly.** Before adding, upgrading, or retaining a direct dependency for new work, record its purpose, exact license, meaningful transitive cost, maintenance signal, narrower alternatives, and whether a small in-repository implementation can safely replace it. Before removing or reimplementing a dependency, pin its required behavior with tests. Prefer deletion, feature reduction, or a small well-specified in-repository implementation when that is simpler to test and maintain than the dependency plus its transitives (the SigV4 signer in `features/s3-sync/` is the worked example). Do not hand-roll cryptography, TLS, KDBX parsing/writing, authentication primitives, or other security-sensitive machinery; use the narrowest established crate that closes the actual gap.
- **Permissive licenses only — no exceptions:** MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unlicense, CC0-1.0, Unicode-3.0, Unicode-DFS-2016. Verify the exact license before executing newly obtained dependency code or writing product code against it. A forbidden, ambiguous, missing, unverified, or linking-exception license blocks the change. This applies equally to crates, copied implementations, vendored source, fonts/data assets, and build tools. Never weaken `deny.toml`, audit configuration, or a gate to rationalize an otherwise disallowed dependency.
- **Forbidden:** GPL-2.0, GPL-3.0, LGPL-2.1/3.0, AGPL-3.0, SSPL, Commons Clause, anything copyleft. **GPL-with-linking-exception is also forbidden** (e.g., libgit2 — the ambiguity isn't worth it; `git2` / `libgit2-sys` are explicitly banned in `deny.toml`).
- **Pinned exact versions.** `Cargo.lock` is the source of truth and is committed.
- **Vendored dependency tree.** All crates vendored at `vendor/` via `cargo vendor`. Builds run `--offline` / `CARGO_NET_OFFLINE=true`.
- **Off-target vendored sources are unavoidable; budget for them.** `Cargo.lock` is target-agnostic by design — the resolver records every platform-conditional dep across every triple a transitive could ever reach (Windows, Android, wasm32, etc.) so the lockfile is reproducible across hosts. Cargo's source-replacement (`replace-with = "vendored-sources"`) then validates the *entire* lockfile against `vendor/` at resolution time, so every locked package must exist on disk even if rustc will never compile it for our Phase-0 triples (macOS aarch64/x86_64, Linux x86_64/aarch64). The cost is disk space — `vendor/` currently carries ~90 directories of Windows / Android / wasm sources that rustc skips via `#[cfg(target_os = ...)]`. The supply-chain audit (`cargo deny`) sees through this via `deny.toml`'s `targets = [...]` constraint, so licenses + advisories only check the four target triples we actually ship. **This makes "minimize external dependencies" non-negotiable** — every new direct dep can drag Windows / Android / wasm transitives into `vendor/` that bloat the tree without ever being shipped. Recent example: adding `ureq 3.3.0` for the S3 transport pulled ~51 new vendored crates, of which roughly 20 are off-target placeholders (mostly Android `jni`-family + Windows `schannel` + wasm `webpki-root-certs`).
- **No build-script networking.** Dependencies' `build.rs` must not fetch anything or shell out to undocumented executables.
- **Dependency-add/upgrade/removal checklist** (documented in task evidence or the PR description): purpose, exact license, upstream maintenance signal, popularity baseline, transitive footprint, feature reduction, diff review of vendored sources, behavior-preserving tests for removal, and an honest assessment of whether a small non-security capability should live in-repository instead.
- **Enforcement:** `cargo deny` for license + advisory + banned crates; `cargo audit` for RustSec advisories. Both are CI gates.

## Technical constraints

- **Language:** Rust. No `unsafe` outside well-justified, locally-audited blocks. No FFI in the core (FFI lives at UI boundaries only).
- **Target platforms (Phase 0):** macOS (aarch64 + x86_64) and Linux (x86_64 + aarch64). Windows is not a Phase 0 target.
- **Storage format:** KDBX4 (write); KDBX3 (read, for migration). No proprietary format.
- **State / vault default directory:** `$HOME/.local/state/hidlins/` on both macOS and Linux (deliberately consistent — not following macOS `Application Support/`). Per-vault override allowed.
- **Vault registration:** TOML at `$HOME/.local/state/hidlins/vaults.toml`.
- **Key libraries (locked in):**
  - `keepass-rs` (MIT) — KDBX read/write.
  - `zeroize` (MIT/Apache-2.0) — memory hygiene.
  - `rand` / `OsRng` (MIT/Apache-2.0) — CSPRNG.
  - `clap` (MIT/Apache-2.0) — CLI parsing.
  - `ratatui` (MIT) — TUI framework.
  - RustCrypto family — crypto primitives.
- **Phase-0 sync transport (lands in `features/s3-sync/` Phases 2–6):** S3-compatible object storage via a hand-rolled SigV4 signer + `ureq 3.3` (MIT OR Apache-2.0) over `rustls` with the `ring` provider, per design.md ADR-1. The earlier `gitoxide` / `gix-*` stack was removed in `features/s3-sync/` T1.

## Build system

**`make` is the build system of record.** Every developer-facing workflow — build, test, lint, format, vendor, supply-chain audit, docs — has a target in the top-level [`Makefile`](Makefile). Developer workstations and CI run identical commands by going through `make`; raw `cargo` invocations are reserved for ad-hoc exploration. Run `make` (or `make help`) to list every target.

The canonical targets:

| Target              | What it does                                                                                            |
| ------------------- | ------------------------------------------------------------------------------------------------------- |
| `make toolchain`    | One-time dev bootstrap: Rust (via `rustup`), `keepassxc-cli`, `cargo-deny`/`audit` (x-plat, idempotent) |
| `make build`        | `cargo build --workspace --offline --locked`                                                            |
| `make test`         | Default-parallel tests (`--offline --locked`)                                                           |
| `make test-ignored` | `#[ignore]`d tests, serial (`--test-threads=1`) — env-mutating tests, etc.                              |
| `make test-all`     | Both of the above                                                                                       |
| `make fmt`          | Auto-format the workspace                                                                               |
| `make fmt-check`    | Format check (CI gate)                                                                                  |
| `make lint`         | `cargo clippy ... -- -D warnings` (CI gate)                                                             |
| `make lint-fix`     | Apply safe clippy suggestions                                                                           |
| `make check`        | `fmt-check` + `lint` + `build` + `test` (the full local CI gate)                                        |
| `make deny`         | `cargo deny check` (license + advisory + bans)                                                          |
| `make audit`        | `cargo audit` (RustSec advisories)                                                                      |
| `make doc`          | Generate API docs (`cargo doc --no-deps --offline`)                                                     |
| `make vendor`       | Re-vendor deps (the only target that needs network)                                                     |
| `make clean`        | Remove build artifacts                                                                                  |

### Keeping the Makefile up to date — a project rule

**When new functionality introduces a new developer or CI command, add a corresponding `make` target in the same change.** This includes:

- A new lint, formatter, or static-analysis pass.
- A new test runner or test category (benchmarks, fuzzers, property-based, integration harnesses).
- A new code-generation step.
- A new shell-script harness (e.g., Phase-6 `tools/interop-tests/`).
- A new release / packaging / signing step.

Two reasons this rule is non-negotiable:

1. **Discoverability.** Every workflow lives in `make help`; no command exists only inside someone's shell history, a CI YAML, or a commit message.
2. **CI / dev parity.** CI invokes `make`, so anything CI does is reproducible locally with the same target name. The day a CI step diverges from a `make` target is the day "works on my machine" becomes possible.

Concretely: if a PR adds a `run: cargo ...` (or any shell command) to a CI workflow, the equivalent `make` target must land in the same PR. The Phase-6 tasks T6.2 (interop-tests harness), T6.3 (CI matrix completion), and T6.4 (KDF benchmark) will each add Makefile targets.

If a target's command becomes long or grows multiple cases, prefer adding flag variables (`CLIPPY_FLAGS`, etc.) over inlining; keeps the recipe readable and the variation surface visible.

## CLI conventions

- One-shot subcommands for every core operation.
- `--format json` for machine-readable output; secrets emitted only with explicit flags (e.g., `entry get --show-password`).
- Stable, documented exit codes: `0` success, `1` user error, `2` vault locked / auth failure, `3` sync conflict requiring user, `10+` internal errors.
- Shell completions for bash, zsh, fish produced by the build.

## Performance targets

- Vault open (Argon2id decrypt + parse, ≤500 entries) <500ms on M1 Mac / mid-tier x86_64 Linux.
- Search across an unlocked vault <50ms for ≤5,000 entries.

## Repo state

**Phase 0 progress (as of 2026-08-31): MVP complete.** All six MVP-critical
features and their deterministic verification are implemented. Optional
real-terminal and platform screen-reader acceptance cases remain documented as
non-gating confidence checks and have not been executed.

- **`vault-core`: complete.** KDBX read/write wrapper, vault registration, master-password unlock, atomic writes, advisory locking (20/20 tasks).
- **`entry-management`: complete.** CRUD across credential / secure-note / TOTP / attachment types; groups; tags; search; history (16/16 tasks).
- **`password-generation`: complete.** Random + EFF diceware via `getrandom` (11/11 tasks).
- **`security-behaviors`: complete.** Idle auto-lock, OS lock events (`LogindSource`/`IoKitSource`), clipboard auto-clear (12/12 tasks, all 5 phases).
- **`cli-skeleton`: complete.** `clap`-based one-shot subcommands, secure-stdin prompt, JSON output, stable exit codes, shell completions (all 4 phases).
- **`s3-sync`: complete.** Sync trait + S3-compatible transport (hand-rolled SigV4 + `ureq`) + in-app three-way merge with loser-as-history preservation (25/25 tasks; merged to `main` 2026-05-31).
- **`tui-skeleton`: complete.** `ratatui` tabbed-workspace shell — unlock,
  navigable tree, scrollable multi-type detail, action overlays
  (search/edit/generate/history), pinned tabs, MRU recents, Settings + live
  `hidlins-sync` integration with secure S3-credential entry (32/32 tasks; all
  7 phases). Deterministic journeys and accessibility semantics run in-process
  through Ratatui's test backend; optional Linux Orca/AT-SPI and macOS VoiceOver
  cases remain external, non-gating, and unexecuted.

**Open by design (not gating MVP code-completion):** the `pre-mvp-fixes` bucket (5/5 — all fixed as of 2026-06-11; see `features/index.md`) and the post-MVP `ssh-keys` + `agent` features (not yet planned).

See `…/features/index.md` for the per-feature rollup and each feature's `tasks/index.md` for live task-tracking tables.

## No deferred work

**Never defer, skip, or stub out work without explicitly asking the user first.** If a task's acceptance criteria cannot be fully met — because a dependency is missing, an API doesn't exist yet, a crate needs adding — stop and ask. Do not silently mark items as "wiring-pending", "deferred to follow-up", "lands later", or "simplified version". If implementing the full requirement requires adding a dependency, writing more code, or solving a harder problem, do that work rather than shipping a stub. If it is genuinely impossible, say so up front as the first thing communicated, not buried in a report footnote.

## Git commit messages

Never include `Claude-Session:` lines, session URLs, or any Claude/AI attribution metadata in commit messages. Commit messages describe the change — they are not a place for tool provenance.

## Shell command style

Prefer running commands as seperate Bash tool calls rather than chaining them with `&&`, `||`, `;`,
or pipes. Each command should be its own invocation so the permission matcher can authorize them
individual.

Exceptions where chaining is fine:

- Pipes that are part of a single logical operation (`grep ... | wc -l`, `cat foo | jq .bar`) - these
  only make sense as one command.

- `cd <dir> && <cmd>` when the directory change must scope to that one command and not persist.

When in doubt, run them separately.

## Codex feature workflow

This repository uses the `$feature-workflow` Skill for planned feature,
bug, and maintenance implementation.

A workflow represents one human-approved work package. A work package may
contain one or multiple features, bugs, or related implementation items.

### Active workflow authority

`.ai/workflow/state.json` is the authoritative runtime state of the
active work package.

`.ai/workflow/status.md` is a human-readable projection only.

If they disagree, trust `state.json`.

Exactly one active work package is permitted.

If `.ai/workflow/state.json` does not exist, no workflow is active.

### Human approval boundary

Before a work package is approved:

- repository investigation is allowed;
- planning files under `.ai/workflow/` may be created or revised;
- product/source implementation must not begin.

Only an explicit:

`$feature-workflow approve`

instruction from the human authorizes implementation.

Do not infer approval from phrases such as:

- "looks good";
- "that seems fine";
- discussion of the plan;
- edits to planning files;
- ordinary conversation.

### Frozen approved plan

After approval, these files are immutable:

- `.ai/workflow/request.md`
- `.ai/workflow/plan.md`
- `.ai/workflow/gate.json`
- `.ai/workflow/tasks/*.md`

If implementation requires a material change to any of them:

1. set the workflow phase to `PLAN_CHANGE_REQUIRED`;
2. record the reason;
3. update status;
4. stop.

Never silently alter the approved plan and continue.

### Execution ownership

The primary Codex thread owns:

- orchestration;
- task ordering;
- architecture;
- difficult implementation work;
- delegation decisions;
- review;
- quality gates;
- repairs;
- final whole-work-package review;
- final acceptance.

Tasks are executed sequentially in numeric order.

Never start task N+1 until task N:

1. satisfies its acceptance criteria;
2. has been reviewed by the main thread;
3. passes its task validation;
4. passes the standard quality gate;
5. has an evidence file;
6. is marked COMPLETE.

### Subagent policy

Default to implementing tasks in the primary thread.

At most one subagent may be active.

A subagent may be used only for implementation of the current task and only
when the task document says:

`Delegation: worker-eligible`

Use the project `worker` agent.

Do not delegate:

- planning;
- architectural decisions;
- task sequencing;
- task acceptance;
- per-task review;
- quality-gate decisions;
- final whole-package review;
- archival.

A worker must never spawn another agent.

After a worker returns, the primary thread must inspect the actual
implementation and owns all repair and acceptance decisions.

### Autonomy after approval

After approval, continue through every approved task without asking the
human whether to proceed.

Do not stop merely to provide progress.

Do not ask:

- "Should I continue?"
- "Would you like me to start the next task?"
- "Task 2 is complete. Proceed with Task 3?"

Continue automatically.

Stop before DONE only for:

- `BLOCKED`
- `PLAN_CHANGE_REQUIRED`
- `RETRY_BUDGET_EXCEEDED`
- an operation requiring human authorization under the active Codex
  security/permission policy.

### Retry limits

Each implementation task receives at most three implementation/repair
attempts.

The final whole-package review and gate receives at most two repair
attempts.

When a retry budget is exhausted:

set:

`phase = RETRY_BUDGET_EXCEEDED`

record the reason and stop.

### Work-package completion

A task is COMPLETE only after:

1. implementation satisfies its task acceptance criteria;
2. the primary agent reviews the implementation;
3. task-specific validation passes;
4. every command in
   `.ai/workflow/gate.json -> standard_commands` passes;
5. its evidence file exists.

The overall work package is DONE only after:

1. every task is COMPLETE;
2. the primary thread performs a whole-package review;
3. all high-confidence findings within approved scope are fixed;
4. all final validation commands pass;
5. `.ai/workflow/evidence/final.md` exists;
6. the final Git revision has been recorded.

DONE does not mean archived.

### Work-package archives

`.ai/workflow/` contains the only active or complete-but-unarchived work
package.

`.ai/workflow-archive/` contains historical completed work packages.

Archived work packages are immutable.

During normal planning and execution:

- do not modify archived files;
- do not treat archived state as current state;
- do not automatically read old plans;
- do not automatically read old task documents;
- do not inherit requirements from previous work packages.

Only inspect an archived work package when:

- the current request explicitly refers to previous work;
- repository history is necessary to understand existing implementation;
- the human explicitly requests comparison with earlier work.

The current repository implementation is the primary representation of
previous completed work.

Archives provide provenance, not default prompt context.

### Archival boundary

Archival requires an explicit human command:

`$feature-workflow archive`

Never archive automatically.

Archival is permitted only when:

`phase == DONE`

A completed work package must be archived before another work package may
begin.

Never overwrite an archive.

Never silently delete a completed workflow.

After successful archival:

`.ai/workflow/`

must not exist.

The absence of `.ai/workflow/state.json` indicates that the repository is
ready for a new work package.

### Follow-up work

Do not automatically promote:

- review observations;
- optional improvements;
- technical-debt ideas;
- future cleanup;
- low-confidence findings

into subsequent work packages.

Record them as historical observations only.

The human decides which work enters a future work package.

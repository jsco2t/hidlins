# Local Sync Review Repairs

## Objective

Repair the five verified local-network-sync defects with production-path, fail-before/pass-after regression evidence; make pairing, cancellation, discovery, credential rotation, and iOS discovery restart behavior durable and deterministic; and align active coverage claims with what the tests actually exercise. Audit adjacent failure boundaries in these same state machines and add only tests that protect meaningful security, durability, concurrency, or recovery behavior.

## Current behavior

- Pairing uses a prepare/activate exchange, but the server can activate a client before the client durably records recoverable provisional state. Production callers persist only after confirmation, while the existing state-model test manually persists provisional state and calls recovery helpers outside the production protocol path.
- The TUI spawns pairing work independently of its lock/cancel lifecycle. Locking clears visible state without guaranteeing that the worker has stopped or joined, so late completion can still mutate persistent state and the worker can retain password material.
- CLI, API, and TUI discovery consumers treat the first non-empty poll as the complete candidate set. Existing discovery coverage presents records together or passes multiple routes directly to transport code, so it does not test a wrong early route followed by a valid later discovery response.
- Password rotation atomically replaces the KDBX and atomically rewrites registry data, but those two commits are not one recoverable transaction. Existing fault injection terminates before the KDBX rename and does not exercise the cross-file commit boundary.
- iOS schedules unscoped delayed discovery completion. Stopping and immediately restarting can allow the first attempt's delayed callback to stop the second. Existing native tests cover payload/address behavior, not overlapping generations.
- Several test names and active coverage documents imply stronger production-path or temporal coverage than the assertions provide.

## Proposed implementation

Work proceeds defect by defect. Each task begins by adding or strengthening a targeted regression against the real production seam, running it against the current implementation, and recording the expected failure in task evidence. The task then repairs the implementation and records the same test passing before any cleanup. Each task also inspects neighboring transitions in its state machine and records whether additional coverage is valuable.

Pairing will durably persist a sealed, non-usable provisional transaction before server authorization and will expose an authenticated, idempotent production recovery path after interrupted acknowledgement or local finalization. Existing-vault and import pairing must share the same safety property without registering or exposing an imported vault before bilateral completion.

TUI pairing will become an owned cancellable operation. Lock, explicit cancel, quit, and replacement must signal cancellation and wait for the worker to finish before locked-state cleanup returns; cancellation must prevent late persistence/import and release captured secrets.

Discovery polling will use one shared bounded candidate-collection policy across Rust application surfaces. It will retain deduplicated admissible candidates across temporal batches within the configured budget and let authenticated transport try them in deterministic priority order. A first unauthenticated or stale route must not suppress a later pinned authority.

Password and sync-identity rotation will use a recoverable multi-file transaction with sibling staged files and a non-secret durable marker. Recovery will deterministically finish or roll back an interrupted commit, remain idempotent, preserve unrelated registry records under locking, and never serialize plaintext.

iOS discovery will associate delegates, timers, and completion with a monotonically increasing attempt generation (or equivalent cancellation identity). Stale callbacks will become no-ops and completion will remain exactly once for the active generation.

Finally, active protocol/security/test-matrix/release documents and test labels will be reconciled with executable behavior. The immutable archived workflow remains untouched; this work package supplies superseding evidence.

## Architectural decisions

- Pairing authorization is not considered recoverable unless the client has durably stored the sealed identity, pinned server key, transaction identifier, expiry, and other minimum retry state before requesting activation. Provisional state cannot authorize normal vault access.
- Pairing recovery must traverse the production authenticated protocol, be bound to the same client/server identities and transaction, reject mismatched, expired, or revoked state, and be safe to retry after lost responses.
- Pending import state is stored as a sealed transaction artifact rather than a usable vault registration. Normal registration/import becomes visible only after bilateral confirmation.
- Cancellation is cooperative through the networking/pairing stack and ownership is explicit. UI lock and shutdown cannot merely discard a receiver while a secret-bearing worker continues.
- Discovery's total time, candidate cap, address admission, identity pinning, and ordering remain bounded and deterministic. Candidate accumulation changes temporal completeness, not trust policy.
- Password rotation uses fixed sibling artifact names or validated internally generated names. Recovery never follows arbitrary paths from untrusted marker content. File and directory durability follows the repository's existing atomic-write rules.
- The transaction marker contains no master password, derived key, plaintext vault data, or unsealed identity material. KDBX and registry staged content retain their existing encryption/integrity protections.
- iOS callbacks validate attempt identity at the point of mutation, not only when the timer is created. Stop is idempotent and invalidates every callback from the prior attempt.
- No dependency is expected. If implementation reveals that a new dependency or a material protocol redesign is necessary, the workflow enters `PLAN_CHANGE_REQUIRED` before product code proceeds.
- Archived planning/evidence files are immutable. Corrections apply to source tests, active maintained documentation, and this workflow's evidence.

## Work included

1. Durable pairing provisional state and production recovery for existing-vault and import flows, including acknowledgement loss and local finalization failure.
2. TUI pairing cancellation/ownership on lock, cancel, quit, and replacement.
3. Temporal discovery candidate accumulation and fallback across CLI, API, TUI, and Flutter-facing Rust paths.
4. Crash-recoverable KDBX password plus sync-identity rewrap transaction.
5. Generation-safe iOS discovery timeout/delegate handling.
6. A value-based audit of directly adjacent state transitions and correction of overstated executable coverage claims.

Every behavior repair has an explicit red test recorded before implementation and the identical green test recorded afterward. Adjacent tests are added when they protect a distinct transition or recovery invariant, not when an existing test already exercises the same production path and assertion.

## Task sequence

1. [Task 001: Pairing Durability and Recovery](tasks/001-pairing-durability-and-recovery.md)
2. [Task 002: TUI Pairing Cancellation](tasks/002-tui-pairing-cancellation.md)
3. [Task 003: Temporal Discovery Fallback](tasks/003-temporal-discovery-fallback.md)
4. [Task 004: Atomic Password and Identity Rotation](tasks/004-atomic-password-and-identity-rotation.md)
5. [Task 005: iOS Discovery Generation Safety](tasks/005-ios-discovery-generation-safety.md)
6. [Task 006: Coverage Contract Audit](tasks/006-coverage-contract-audit.md)

## Quality gate

After every task:

- `make check` verifies Rust formatting, lint, build, and the default Rust tests.
- `make app-check` verifies Flutter formatting, analysis, and tests.
- `make ncsa-boundary-check` preserves the repository's production/development license boundary.

For final acceptance:

- `make verify` runs the repository's complete verification aggregate.
- `make ncsa-boundary-check` explicitly records the supply-chain boundary result.
- `make check-ios`, `make app-test-ios-simulator`, and `make app-build-ios` compile and execute the native/iOS paths affected by generation-safe discovery.
- `make android-emulator-provision` and `make test-local-sync-mobile-scenarios HIDLINS_ANDROID_STRICT=1` retain cross-mobile local-sync confidence and ensure shared Flutter/Rust discovery changes did not regress Android.

No new developer workflow is introduced, so no new Makefile target is required.

## Risks

- Pairing repair changes a security-sensitive distributed state machine. Incorrect ordering could create premature authorization, unusable trust, replay, or an import visible before confirmation.
- Cancellation crosses UI threads and blocking network I/O. A repair must avoid both detached work and an indefinitely blocked UI shutdown.
- Waiting for temporal discovery candidates can add latency. Collection must preserve the existing total budget and stop at a bounded cap.
- Multi-file rotation cannot be made crash-safe by rename ordering alone. Recovery must cover every durable boundary and concurrent registry writers without weakening atomic KDBX replacement.
- iOS timer/delegate tests can become timing-sensitive if they use wall-clock sleeps. Tests should use an injectable scheduler or an equivalent deterministic production seam.
- Renaming tests or tightening coverage documents can expose additional gaps. Only distinct, high-value gaps within the five affected state machines are in scope; unrelated feature expansion is not.

## Out of scope

- New sync features, public-network support, protocol suite negotiation, or changes to the merge algorithm.
- Broad UI redesign beyond ownership and feedback required for safe pairing cancellation.
- Rewriting the vault registry or KDBX layer outside what the recoverable rotation transaction requires.
- General test-count or line-coverage targets.
- Editing any file under `.ai/workflow-archive/`.
- Adding dependencies without a separately approved plan revision and the repository supply-chain review.

## Final acceptance criteria

- A production-path test proves that interruption after server activation cannot strand an unrecoverable client: durable provisional state survives restart and authenticated recovery completes or safely rejects by identity/transaction/expiry/revocation policy.
- Existing-vault and import pairing expose no usable authorization or registration before bilateral confirmation, and retry/lost-ack paths are idempotent.
- Deterministic TUI tests prove lock, cancel, quit, and operation replacement stop and join pairing work, prevent late persistent mutation/import, and release captured secret-bearing state.
- A deterministic temporal discovery test proves that a stale or wrong-key first batch does not prevent a later valid pinned authority from succeeding within the same bounded attempt on every Rust application surface.
- Process-level fault injection proves password rotation recovers to a consistent KDBX-password/registry-identity pair at every cross-file commit boundary, including repeat recovery and the supported key-file path.
- Deterministic iOS native tests prove callbacks from an earlier stopped attempt cannot stop, complete, or mutate a restarted attempt, and active completion occurs exactly once.
- Test names and assertions accurately describe production behavior, and active protocol, security coverage, test matrix, and release-review documents do not claim unexecuted recovery, temporal, cancellation, or crash-boundary coverage.
- Each task's evidence records the exact regression command failing before its fix and passing afterward, plus the adjacent coverage decisions made.
- All task-specific, standard, and final gate commands pass from the final worktree.


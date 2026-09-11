# Task 001: Pairing Durability and Recovery

Delegation: main-only

## Goal

Make the pairing transaction durable and recoverable before server authorization, and prove the real production protocol safely completes or rejects interrupted existing-vault and import pairings.

## Context

The current server activates after `PairActivate`, but production clients persist trust only after `PairActivated`. The existing recovery-oriented test manually saves provisional state and invokes a helper directly, so it does not cover the production ordering that can strand a client after acknowledgement loss or persistence failure.

## Scope

### In scope

- Existing-vault and import pairing transaction ordering.
- Durable sealed client provisional state before activation.
- Authenticated, identity-bound, idempotent production recovery after restart.
- Lost activation acknowledgement, client persistence/import failure, expiry, mismatch, and revocation boundaries.
- Pairing protocol, threat-model, test-matrix, and coverage wording directly affected by the repair.

### Out of scope

- TUI worker ownership and lock cancellation, owned by Task 002.
- General discovery candidate policy, owned by Task 003.
- New cryptographic primitives or protocol suite negotiation.

## Implementation requirements

- Before product changes, add a regression that executes the production pairing protocol, interrupts after server activation and before client finalization, reloads client state, and fails because current production code cannot recover.
- Record the exact red command and failure in `evidence/001.md`; after repair, record the identical green command.
- Persist the minimum sealed provisional transaction before asking the server to activate. The artifact must be atomically written, contain no plaintext secret material, and not grant normal vault access.
- Pending import state must not create a usable vault registration before bilateral confirmation.
- Recovery must authenticate the original static identities, bind the transaction identifier and vault, enforce expiry/revocation, and tolerate repeated requests or lost responses.
- Server and client cleanup must not delete the only recovery state before both sides can converge.
- Inspect adjacent transitions: failure before prepare, after prepare, after client durable prepare, after server activation, after acknowledgement, and during local import/registry finalization. Add tests for distinct high-value transitions and record why duplicative cases were omitted.
- Correct existing test names/assertions and active documentation if they currently imply production recovery that they do not exercise.

## Acceptance criteria

- [ ] A production-path test fails on the baseline at the server-active/client-unfinalized boundary and passes after the repair.
- [ ] A restarted client loads durable provisional state and completes recovery through the authenticated network protocol without manual test-only persistence or direct helper invocation.
- [ ] Replayed recovery is idempotent; mismatched identity/transaction/vault, expired state, and revoked clients fail closed.
- [ ] Existing-vault and import paths cannot expose a usable relationship or registration before bilateral confirmation.
- [ ] Every durable provisional/final record is sealed where required and atomically written without secret material in logs or errors.
- [ ] Adjacent pairing boundary coverage and corrected claims are documented in task evidence.

## Validation

- `cargo test -p hidlins-sync --offline --locked --test identity_pairing_trust --test server_integration -- --test-threads=1`
- `cargo test -p hidlins-api --offline --locked --test us_094_sync_api -- --test-threads=1`
- `cargo test -p hidlins-cli --offline --locked --test cli_local_sync_process -- --test-threads=1`

## Dependencies

None

## Expected areas of change

- `crates/hidlins-sync/src/pairing.rs`
- `crates/hidlins-sync/src/client/`
- `crates/hidlins-sync/src/server/`
- `crates/hidlins-sync/src/config/`
- `crates/hidlins-sync/src/protocol.rs`
- `crates/hidlins-sync/tests/identity_pairing_trust.rs`
- `crates/hidlins-sync/tests/server_integration.rs`
- `crates/hidlins-api/src/api/sync.rs`
- `crates/hidlins-api/tests/us_094_sync_api.rs`
- `crates/hidlins-cli/tests/cli_local_sync_process.rs`
- `crates/hidlins-sync/docs/`

## Risks / notes

This is a security- and data-integrity-sensitive distributed transaction. The repair must preserve the distinction between provisional state and usable authorization and must not turn recovery into an authentication bypass. Any material wire-format/state-machine departure from the approved plan requires `PLAN_CHANGE_REQUIRED`.


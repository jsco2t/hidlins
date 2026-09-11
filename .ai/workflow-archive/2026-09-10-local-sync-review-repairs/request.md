# Local Sync Review Repairs

## Human request

> Fix the five review findings using test-before-fix, correct overstated tests and coverage claims, and evaluate adjacent missing high-value coverage.

## Review findings in scope

1. Pairing can authorize a client on the server before the client has durably stored enough provisional state to recover, leaving an authorized-but-unusable relationship after acknowledgement loss or a local persistence failure.
2. TUI lock and cancellation paths do not stop or join detached pairing work, so a pairing operation may complete, persist/import state, or retain the master password after the UI has locked.
3. Discovery consumers stop after the first non-empty discovery result, so an early stale or wrong-key candidate can prevent a later valid pinned authority from being tried within the discovery budget.
4. Master-password rotation commits the KDBX file and sync-identity rewrap in separate transactions, so a crash between them can leave the vault password and registry-wrapped identity inconsistent.
5. The iOS discovery timeout is not scoped to a discovery generation, so a delayed timeout from a stopped attempt can terminate a newer attempt.

## Required implementation posture

- For every defect, first add or strengthen the smallest production-path regression test that demonstrates the failure and record its failing result.
- Implement the minimum durable repair, rerun the same test, and record its passing result before refactoring.
- Correct test names, assertions, comments, matrices, and active documentation that claim coverage beyond what is actually executed.
- Evaluate adjacent failure boundaries in the same state machines. Add tests only where they close a meaningful regression, security, durability, or concurrency gap; do not add duplicative tests for test count alone.
- Preserve the immutable completed workflow under `.ai/workflow-archive/`; corrections and superseding evidence belong to this work package and active documentation.
- Run the repository-standard gates after every task and the complete local-sync/mobile gate before declaring the work package done.


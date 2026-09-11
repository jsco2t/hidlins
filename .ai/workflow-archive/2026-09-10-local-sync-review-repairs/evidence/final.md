# Final Evidence — Local Sync Review Repairs

Work ID: `2026-09-10-local-sync-review-repairs`

Baseline and final Git revision: `0725603cc9b746f3632a0e02f8f81a55930721b0`

The final revision is the checked-out revision; the approved implementation remains in the working tree for human review and commit.

## Completed tasks

1. [Task 001 — Pairing Durability and Recovery](001.md): durable provisional state, authenticated restart recovery, idempotent activation, and fail-closed transaction/identity/expiry/revocation checks.
2. [Task 002 — TUI Pairing Cancellation](002.md): owned cancellation and bounded joining for lock, cancel, quit, replacement, and drop, with late persistence prevented.
3. [Task 003 — Temporal Discovery Fallback](003.md): bounded candidate accumulation and deterministic fallback shared by CLI, API, TUI, and Flutter-facing Rust paths.
4. [Task 004 — Atomic Password and Identity Rotation](004.md): crash-recoverable KDBX-password and sealed registry transaction with process-level fault injection at every durable boundary.
5. [Task 005 — iOS Discovery Generation Safety](005.md): generation-scoped iOS completion and the confirmed analogous Android stale-callback repair.
6. [Task 006 — Coverage Contract Audit](006.md): corrected test names, stable IDs, matrices, security claims, and release-review language; adjacent state transitions were evaluated by value.

## Test-before-fix evidence

Each approved behavioral finding has its exact failing-before and passing-after command recorded in its task evidence. The final whole-package review also found and repaired three directly relevant escaped failures:

- A sole discovered authority was not retried after a transient handshake failure. The new production-path regression returned `Err(Unreachable)` before the repair and passed after the client divided the existing total attempt budget across two tries. Multi-route behavior and the total deadline remain unchanged.
- The strict Android API 29 scenario accepted matching subnet metadata even when the emulator peer route was not live. The strengthened harness assertion first failed on client-before-authority boot ordering, then failed until peer reachability became an explicit prerequisite. The focused API 29/API 36 scenario passed twice from independent output directories after the repair.
- The brand raster golden could deadlock while decoding an asset in fake async. Removing the pre-cache exposed a deterministic 96.63% golden mismatch; running the pre-cache through `tester.runAsync` made the exact navy regression and two complete-file repetitions pass.

No speculative discovery-timeout change remains in the final diff.

## Final acceptance review

- Pairing interruption after server activation is recoverable from durable client state and rejects mismatched, expired, or revoked recovery attempts.
- Existing-vault and import pairing do not expose active authorization or registration before bilateral confirmation; lost acknowledgements and replay are idempotent.
- TUI lock, cancel, quit, and replacement synchronously cancel and join pairing work, prevent late mutation, and release worker-owned secret-bearing state.
- Temporal discovery preserves the total bound while allowing a later pinned authority to succeed across every Rust application surface.
- Password rotation fault injection covers each cross-file durable boundary, repeat recovery, unrelated registry changes, and key-file use while preserving KDBX interoperability and the sync public key.
- iOS attempt generations make stale callbacks inert and completion exactly once; Android's analogous token lifecycle is covered and exercised in the native matrix.
- Active test names and documentation now match the executable recovery, cancellation, discovery, durability, and native lifecycle guarantees.
- Archived workflow material is unchanged, no dependency was added, and `git diff --check` passes.

The final integrated diff review found no remaining high-confidence issue within the approved scope.

## Final quality gate

All commands were run against the final worktree and passed:

| Command | Result |
| --- | --- |
| `make verify` | PASS |
| `make ncsa-boundary-check` | PASS |
| `make check-ios` | PASS — 14 native tests |
| `make app-test-ios-simulator` | PASS |
| `make app-build-ios` | PASS — simulator and device artifacts |
| `make android-emulator-provision` | PASS |
| `make test-local-sync-mobile-scenarios HIDLINS_ANDROID_STRICT=1` | PASS — iPhone, iPad, Android API 29, and Android API 36 scenarios |

The previously unstable Android scenario was run through the shortened feedback loop before the complete catalog: harness regression suite, one focused strict run, a second independent focused strict run, and finally the complete strict mobile scenario gate.

## Remaining concerns

None within the approved work package. Physical-router multicast and platform assistive-technology observations remain accurately classified as optional external confidence checks, not automated guarantees.

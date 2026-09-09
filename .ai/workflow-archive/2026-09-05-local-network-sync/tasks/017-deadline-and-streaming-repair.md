# Task 017: Enforce Absolute Deadlines and Stream Fetch Responses

Delegation: main-only

## Goal

Make the declared handshake, connection, idle, host-operation, and total-session limits govern blocking I/O while eliminating vault-sized fetch-message duplication.

## Context

Round 1 constants and in-memory handshake checks do not interrupt blocking socket calls. Pairing can block for the entire window, candidate handshakes can exceed the total budget, response writes can outlive the session deadline, and fetch constructs copied chunk messages for the full encrypted vault.

## Scope

### In scope

- Absolute monotonic deadline propagation through framing, handshake, client candidate selection, host waits, and server response writes.
- Secret-free and pre-authentication-indistinct timeout mapping.
- File-backed coherent fetch snapshots and one-frame-at-a-time response streaming.
- Bounded cancellation and resource release for slow readers/writers and disconnected clients.
- Deterministic slow-peer and maximum-size ownership/resource regression tests.

### Out of scope

- Async runtimes or new dependencies.
- Protocol wire-format changes unless a strictly internal streaming representation requires no peer-visible change.
- Merge/KDBX behavior changes.

## Implementation requirements

- Each socket read/write timeout is the minimum positive remainder of all applicable deadlines.
- The ten-second client connection budget covers TCP connect plus Noise authentication across all candidates.
- The five-second Noise deadline covers preface/handshake blocking I/O on both sides.
- Pairing confirmation uses only the remaining pairing-window time.
- The 300-second session deadline and cancellation state are checked during multi-frame responses, not only before reading a request.
- Fetch coherence survives atomic replacement of the live vault while network transmission proceeds without holding the unlocked-vault event-loop owner.
- No response path may allocate a second collection proportional to the complete encrypted vault.

## Acceptance criteria

- [ ] Slow preface/Noise peers are disconnected within the handshake deadline on client and server.
- [ ] Candidate selection never exceeds the total connection budget and can continue within the remaining time.
- [ ] Pairing, host waits, and slow response writes stop at their applicable absolute deadline.
- [ ] Maximum-size fetches stream one bounded frame at a time from a coherent snapshot without a vault-sized `Vec<Message>` or equivalent duplication.
- [ ] Timeout/disconnect paths release sockets, staging/snapshot resources, and connection capacity.
- [ ] Regression tests record fail-before/pass-after results without timing-flaky unbounded sleeps.

## Validation

- `cargo test -p hidlins-sync --offline --locked --test server_integration -- --test-threads=1`
- `cargo test -p hidlins-sync --offline --locked --test client_policy -- --test-threads=1`
- `cargo test -p hidlins-sync --offline --locked --test noise_security`
- `make test-local-sync-security`
- `make test-local-sync-integration`

## Dependencies

Task 016

## Expected areas of change

- `crates/hidlins-sync/src/framing.rs`
- `crates/hidlins-sync/src/noise/mod.rs`
- `crates/hidlins-sync/src/client/mod.rs`
- `crates/hidlins-sync/src/server/mod.rs`
- `crates/hidlins-sync/src/server/runtime.rs`
- `crates/hidlins-sync/tests/noise_security.rs`
- `crates/hidlins-sync/tests/client_policy.rs`
- `crates/hidlins-sync/tests/server_integration.rs`

## Risks / notes

Keep remote errors generic so more precise local deadline enforcement does not create a handshake oracle. A file descriptor for an atomically replaced path can preserve the old inode as an immutable encrypted snapshot; tests must prove the selected approach remains coherent on supported platforms.

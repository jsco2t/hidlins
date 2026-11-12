# Local-network sync release review

This review index describes the retained Rust implementation and its regression
coverage. It is not a new execution record; current command results belong in
the change's verification evidence. See the [test matrix](../crates/hidlins-sync/docs/test-matrix.md)
and [verification procedure](local-network-sync-manual-verification.md).

## Focused security review

| Boundary | Review result and evidence |
| --- | --- |
| Protocol and cryptographic integration | Frozen version/preface/state transitions, Noise XX pairing, IK pinned reconnect, downgrade/key-substitution/replay tests, and bounded fuzz targets pass. Hidlins does not negotiate algorithms or implement primitives. See `protocol-v1.md`, `threat-model.md`, and `security-coverage.md`. |
| Zeroization | Hidlins secret owners use zeroizing wrappers/drop behavior; exact Snow secret owners and consumed transitions are covered by the maintained digest-guarded patch. Vendor-patch negative controls fail closed. See `snow-dependency.md`. |
| Discovery and address boundary | Discovery crosses only address/port/scope candidate data and grants no authority. `LocalEndpoint` is required before connect, after `peer_addr`, before bind, and after accept. IPv4 `10/8`, `172.16/12`, `192.168/16`, `169.254/16`, `127/8` and IPv6 `fc00::/7`, scoped `fe80::/10`, `::1` are the entire allowlist; mapped, public, special, DNS, and missing/extra-scope forms fail without override. |
| Pre-auth privacy and authorization | Normal advertisements contain only routing data; bounded pairing advertisements are explicit and temporary. Unknown/revoked keys receive no vault metadata or differentiated authorization oracle. Live trust replacement closes removed peers, and revocable host permits prevent already-dequeued commits from crossing the revocation boundary. SAS confirmation is bilateral and transaction-bound before activation; the live listener enforces one candidate, three failures, expiry, and explicit close. |
| Resource limits and availability | Frame, message, chunk, vault, candidate, connection, queue, retry, and absolute handshake/session deadlines are enforced before unbounded work. A sole discovered authority gets one bounded retry after a transient pre-authentication drop, split within the existing handshake allowance; multiple-route priority and deadlines are unchanged. File-backed fetch snapshots remain coherent across atomic replacement and emit one bounded frame at a time. Guard-owned connection slots recover after rejected/disconnected peers, and completed connection workers are reaped during service. |
| Concurrency, ownership, and CAS | One owner moves an unlocked vault into a host operation; UI/API lock and shutdown cancel or discard returned state safely. Server commits use expected versions; simultaneous-client tests prove one winner and a valid recoverable loser/retry path. Trust preparation/activation/revocation writes are transactional. |
| Atomic storage and no data loss | Uploads stage to sibling files, validate length/digest/order, then commit through locked atomic persistence. Pre-merge `.kdbx.bak`, loser-as-history, fault injection, properties, and KeePassXC round trips pass. Interrupted pairing leaves sealed non-authorizing recovery state; provisional or invalid imports create neither a KDBX nor a registration. Master-password rotation stages the encrypted KDBX and sealed registry output behind a non-secret durable marker; startup recovery is idempotent across every staged/rename/cleanup boundary and merges unrelated registry edits. |
| Logs, errors, and process output | Errors, status DTOs, CLI JSON, Rust API events, discovery payloads, and process tests exclude keys, transcripts, SAS internals, vault contents, and master material. Telemetry/crash/update endpoints remain absent. |
| API and UI policy | Rust owns protocol, trust, address admission and storage. CLI/TUI provide explicit serving; native client-only APIs reject server start before bind. The TUI uses a bind-only ephemeral endpoint and publishes the assigned nonzero route. |
| Discovery separation | The desktop Rust adapter publishes and browses routes only; authentication remains pinned Noise. Platform-neutral candidate and permission ports remain available to API consumers. |

## Dependency and licensing disposition

The final counts, provenance, exact licenses, features, maintenance signals,
alternatives, transitives, vendor delta, and patch obligations are in
[`../crates/hidlins-sync/docs/dependency-review.md`](../crates/hidlins-sync/docs/dependency-review.md).
`make deny`, `make audit`, and `make vendor-patches` are clean in the automated
precursor record.

NCSA is permitted only for exact `libfuzzer-sys 0.4.13` in the isolated
development/test fuzz workspace. It compiles bundled LLVM libFuzzer into fuzz
executables only; Hidlins has no direct dependency on a system LLVM library.
Those fuzz executables are non-distributable. Root `deny.toml` rejects NCSA,
the production lock and graph exclude the crate, and `fuzz/deny.toml` cannot
authorize any other package/version.

The NCSA checker validates the Rust release target's pre-build and post-build
hooks, and scans the produced console/agent artifacts. Negative controls cover
policy expansion, production manifest/lock injection, missing hooks, copied
fuzz executables, raw binaries and recursive archives. `make license-check`
adds repository inventory, vendored integrity and exact asset-notice checks.

## Composite-review repair register

| Finding | Fixed boundary | Regression evidence |
| ---: | --- | --- |
| 1 | Live and queued authorization is revocable | `LNS-REVIEW-001` |
| 4 | TUI uses a checked ephemeral bind and publishes a nonzero endpoint | `LNS-REVIEW-004` |
| 5 | Live pairing enforces candidate, failure, window, and cancellation limits | `LNS-REVIEW-005` |
| 6 | Absolute deadlines bound every blocking handshake/session boundary | `LNS-REVIEW-006` |
| 7 | Connection capacity is guard-owned and cannot underflow | `LNS-REVIEW-007` |
| 8 | Fetch is coherent, file-backed, and one-frame-at-a-time | `LNS-REVIEW-008` |
| 9 | Untrusted wrong-key/stale routes cannot block later pinned routes | `LNS-REVIEW-009` |
| 10 | Authorities safely rebind and republish allowed interface changes | `LNS-REVIEW-010` |
| 11 | Retired-transport matching rejects upper/lower/mixed case | `LNS-REVIEW-011` |

## Five-finding repair register (2026-09-10)

| Finding | Repaired boundary | Production-path regression evidence |
| ---: | --- | --- |
| 1 | Client provisional trust is durable before activation; registered and pending-import commands recover the same authenticated transaction after a lost acknowledgement | `LNS-PROCESS-002/003`; lower-level `LNS-PAIR-005/006` are supporting protocol tests, not orchestration claims |
| 2 | TUI cancellation owns and joins pairing work on cancel, replacement, lock, and quit; live socket cancellation interrupts established I/O | `LNS-TUI-012..015`, `LNS-PAIR-007` |
| 3 | Rust application discovery accumulates temporal batches and transport can reach a later pinned authority after a wrong first route | `LNS-DISCOVERY-006`, `LNS-SERVER-006` |
| 4 | Password and sync-identity rotation recovers one consistent credential pair across every durable boundary | `LNS-IDENTITY-004..011` |

Adjacent authority-restart coverage now verifies that active server-side trust
is loaded from the sealed registry by a fresh `hidlins sync serve` process
before the client's first authenticated IK sync (`LNS-PROCESS-004`).

The TUI tests inject an owned barrier worker to verify lifecycle ownership,
while `LNS-PAIR-007` separately proves cancellation of an established production
socket. Physical router multicast, human SAS comparison and terminal
screen-reader speech remain optional observations.

# Local Network Sync Threat Model

Status: implementation baseline for protocol V1. This review must be updated
when a trust boundary, wire message, dependency, platform adapter, or persistent
state changes.

## Security objectives

1. Only a holder of an explicitly paired private identity can learn or modify
   an authoritative vault's encrypted bytes.
2. Discovery, routing, and local-network position confer no trust.
3. Hidlins never initiates or accepts sync traffic through a public or otherwise
   non-allowlisted IP address.
4. A malformed, interrupted, concurrent, or hostile operation cannot corrupt
   or replace the last valid KDBX vault.
5. Private identities, Noise session secrets, master passwords, decrypted entry
   data, SAS inputs, and protocol payloads do not persist in plaintext or reach
   logs, errors, events, UI diagnostics, or bridge diagnostics.
6. Resource use is bounded so an unauthenticated or authenticated LAN peer
   cannot grow memory, disk staging, threads, queues, or connection time without
   a fixed limit.
7. Mobile builds remain foreground clients and cannot expose a sync listener or
   persistent/background service.

## Assets

- KDBX encrypted bytes and their availability/integrity.
- Decrypted vault database and entry history in process memory.
- Master password, keyfile material, KDBX derived keys, and sync identity
  sealing keys.
- Per-vault/per-installation X25519 private identity.
- Trusted client public keys and the client's pinned authoritative-server key.
- Pairing transaction state, handshake hash, and displayed SAS.
- Divergence pointers, canonical remote version, peer names, and routing hints.
- Process availability and bounded host resources.

Vault names, peer display names, public keys, versions, sizes, and whether a
vault exists are metadata assets even though they are not plaintext secrets.

## Trust boundaries

| Boundary | Untrusted side | Trusted side | Required control |
| --- | --- | --- | --- |
| DNS-SD/mDNS | multicast packets and service records | candidate endpoint list | metadata minimization, parsing bounds, address policy, deduplication, expiry |
| TCP | arbitrary LAN socket peer | Noise handshake | actual-peer address recheck, timeouts, connection cap, silent pre-auth failure |
| Noise | unauthenticated transcript | authenticated secure transport | fixed suite/prologue, XX+SAS or pinned mutual IK, zeroization patch |
| Application codec | authenticated hostile bytes | typed protocol state | exact lengths/types/order, allocation bounds, replay/duplicate rejection |
| Network worker | authenticated request | host vault owner | bounded queue, typed authorization, no direct mutable vault access |
| Host vault owner | staged encrypted bytes | canonical live vault | expected-version CAS, KDBX identity/KDF validation, backup, atomic save |
| Registry/disk | local files and crash boundaries | usable identity/trust | mode 0600, authenticated sealing, atomic writes, provisional records non-authorizing, recoverable password/identity rewrap transactions |
| Rust API/FFI | UI/native DTO and event values | core operations | typed narrow DTOs, secret-free errors, target compile-time server denial |
| Native discovery | OS Bonjour/NSD callbacks | Rust candidate policy | bounded endpoint-only adapter, Rust revalidation, permission-state enum |
| UI/CLI | human input/output | security action | secure password prompt, explicit SAS confirmation, no key/payload output |

## Attacker capabilities

The design assumes an attacker may:

- join or control the local network, observe multicast, spoof DNS-SD records,
  race DHCP changes, redirect traffic, accept TCP connections, and perform MITM;
- send arbitrary, fragmented, reordered, duplicated, truncated, or oversized
  bytes at every connection stage;
- open many connections, operate slowly, fill queues, repeat failed pairing,
  disconnect at every persistent-state or commit boundary, and race a legitimate
  client or local edit;
- possess an old revoked public/private client identity or a copied stale
  registry file;
- read Hidlins logs, CLI output, UI error text, and bridge/native diagnostics;
- crash or terminate either process and cause ordinary filesystem I/O errors;
- publish public, special-use, mapped, link-local-without-scope, or stale
  endpoints through discovery and manual input.

The attacker does not possess the current master password, the authoritative
server's private identity, or an active non-revoked client's private identity;
breaking X25519, ChaCha20-Poly1305, SHA-256, Argon2id, or KDBX cryptography is
out of scope.

## Threats and required controls

### Discovery spoofing and metadata disclosure

Threat: advertisements redirect clients, fingerprint vaults, or exhaust
candidate storage.

Controls: random per-start service labels; only `v=1` TXT; no stable/vault/key
metadata; 32-entry normalized cache; removal/expiry; eight attempts and a
ten-second connect budget; strict address checks; pinned Noise identity.
Discovery success never changes trust state.

### Public-network escape

Threat: a hostname, mapped address, interface race, NAT special range, or forged
advertisement causes Internet traffic.

Controls: IP literals only; exhaustive allowlist; IPv4-mapped normalization;
mandatory IPv6 link-local scope; validation at advertisement, candidate,
pre-connect/bind, and actual socket peer; no override or environment flag.
Property tests enumerate allowed ranges and adjacent/special ranges.

Residual risk: an allowlisted address can be routed through an unusual private
network or VPN by the operating system. The product promises address-range
containment, not physical-LAN proof. VPN exceptions are not recognized and
cannot relax the policy.

### MITM, impersonation, downgrade, and replay

Threat: a LAN attacker substitutes keys during pairing, impersonates the
server later, selects a weaker suite/version, or replays a prior transcript.

Controls: fixed preface bound as Noise prologue; fixed XX and IK suites; no
negotiation; human comparison of a locally derived 30-bit SAS; static keys
bound into pairing commit; pinned mutual keys for IK; fresh Noise ephemeral
keys; unique transaction/request IDs; strict state machine; fresh IK required
after pairing. Key mismatch never triggers automatic repair or re-pairing.

Residual risk: 30-bit SAS is suitable only with explicit, rate-limited human
comparison during a short local pairing window. A user who approves mismatched
codes defeats MITM protection. The UI must make rejection as prominent as
acceptance and never auto-confirm.

### Partial pairing and trust corruption

Threat: a crash between peer writes leaves a half-trusted relationship.

Controls: atomically written provisional records; provisional server records
are excluded from IK authorization; transcript/key/role/transaction binding;
expiry; idempotent retry of the same transaction; explicit revocation and fresh
pairing for replacement. Vault operations require a separate IK connection.

Residual risk: distributed persistence cannot be literally atomic. The final
activation response can be lost after the server activates the exact client
key. The client remains provisional and refuses vault operations; no third
party gains the private key. Recovery is retry during the pairing window or
explicit revoke/re-pair.

### Secret lifetime and local storage

Threat: identity/session secrets remain in memory, are swapped/core-dumped, or
are stored plaintext in `vaults.toml`.

Controls: identity private key sealed with the existing Argon2id plus
ChaCha20-Poly1305 technique and domain-separated associated data; mode-0600
atomic registry writes; `ZeroizeOnDrop` wrappers; best-effort `mlock` where
applicable; core dumps disabled on long-running surfaces; maintained exact Snow
patch zeroizing handshake, cipher, chaining-key, hash, ephemeral, static, and
transport state; rewrap preserves identity while clearing plaintext buffers.
Password changes stage both encrypted KDBX and sealed registry outputs, commit
under vault-then-registry advisory locks, and recover from a durable non-secret
phase marker without persisting either password or an unsealed identity.

Residual risk: Rust moves, allocator copies, OS swap, suspend, crash snapshots,
and platform debugging can retain bytes beyond language-level drop. The project
documents platform memory-hygiene limits and does not claim perfect erasure.

### Unauthorized disclosure before authentication

Threat: probing reveals vault existence, version, size, KDF, clients, or bytes.

Controls: preface/address/Noise/authorization complete before application
responses; invalid pre-auth input closes silently; generic post-auth error
codes contain no strings; discovery has no vault metadata; server operation
queue receives an already-authorized peer identity.

### Malformed framing and resource exhaustion

Threat: attacker-controlled lengths allocate unbounded memory/disk, slow-read
threads forever, or fill work queues.

Controls: validate two-byte frame prefix before allocation; exact application
payload lengths; 60 KiB chunks; 256 MiB transfer; one stream/connection; eight
connections; 16 host operations; five-second handshake, 15-second idle, and
300-second session deadlines; bounded staging; checked integer arithmetic;
discard on error. Fuzzing and boundary tests cover below/equal/above values.

### Concurrent writes, stale state, and data loss

Threat: a remote commit races a local edit or another client; retry merges
against stale data; a crash truncates the live vault.

Controls: one host-owned vault operation queue; network workers never mutate a
`Vault`; expected SHA-256 version checked at commit; one re-fetch/re-merge retry
after the initial CAS failure; coherent read of canonical encrypted bytes;
KDBX identity/password/keyfile/KDF validation; loser-as-history; `.kdbx.bak`;
sibling temp plus rename; advisory locking; fault injection at every boundary.

### Revocation and stale clients

Threat: a revoked client continues using an existing or new session.

Controls: authorization checked at handshake and before queueing every
operation; revocation removes active key atomically and closes matching live
sessions; stale/provisional keys fail generically; no key-based discovery data.

Residual risk: a client that fetched encrypted KDBX bytes before revocation
retains those ciphertext bytes. Revocation cannot erase prior copies and does
not rotate the KDBX master password automatically.

### Logs, errors, UI, and bridge leakage

Threat: debugging surfaces expose keys, SAS inputs, vault contents, protocol
payloads, or credentials.

Controls: structured enums with generic codes; peer display names are local
configuration only; no byte dumps; explicit redaction tests over Rust, CLI JSON,
TUI/Flutter semantics, native logs, and FFI errors; no telemetry, crash
reporting, or remote diagnostics.

### Mobile lifecycle and permissions

Threat: a mobile listener/background worker violates product policy or a denied
permission causes unsafe fallback.

Controls: no server DTO/action in mobile UI; Rust server start rejects mobile
targets before binding; native adapters expose only bounded endpoints and
permission state; denied/restricted/not-found remain distinct; no hostname or
public-address fallback; sync runs only while the app is foregrounded.

## Excluded threats

- Compromise of the host OS, root account, process debugger, or unlocked UI.
- Physical memory extraction, Rowhammer, cold-boot/DMA, malicious firmware, or
  hardware keylogging.
- Cryptanalytic breaks in approved primitives or flaws inside KDBX/keepass-rs
  outside the integration behavior Hidlins tests.
- Denial of the entire LAN, multicast, device, or filesystem by an OS/network
  administrator.
- Recovery or remote deletion of encrypted bytes legitimately copied before
  revocation.
- Public-Internet relays, cloud accounts, peer mesh, and server election.

## Verification obligations

The security suite must assign stable IDs to and automate address boundaries,
mapped/scope cases, discovery spoof/flood/staleness, suite/prologue conformance,
SAS mismatch, MITM substitution, IK key mismatch, revocation, pairing crash
transitions, frame/message mutation, replay/order/duplicate/truncation, resource
limits, CAS races, commit crashes, zeroization guards, secret-free diagnostics,
mobile server absence, and permission adapters. Stateful codecs and message
ordering are fuzzed; committed minimized corpora replay under stable tests.

Optional human observation remains only for SAS perception, exact physical
iOS/Android permission presentation, representative-router multicast behavior,
and assistive-technology speech. These do not weaken or block the automated
security model: real simulator/emulator application scenarios, process tests,
namespace tests, and semantics contracts are the acceptance authority.

# Hidlins Local Sync Protocol V1

Status: frozen implementation contract for the local-network-sync work package.

This document fixes every wire constant, state transition, and resource limit
needed to implement protocol major version 1. It is normative. Implementations
must fail closed when input does not match this document; there is no version,
cipher-suite, address-policy, or Internet-connectivity fallback.

## Security boundary

The protocol transports encrypted KDBX bytes only. Noise authenticates the
connection and protects it in transit, but it does not replace KDBX encryption.
Discovery supplies untrusted socket candidates. A socket is usable only after:

1. its advertised address passes the local-address policy;
2. the address used for `connect` or returned by `accept` passes the policy;
3. the actual connected peer address passes the policy again; and
4. Noise XX pairing plus human SAS confirmation, or Noise IK with pinned static
   keys, succeeds.

Before all four checks succeed, the server writes no application response and
discloses no vault existence, name, version, length, KDF data, peer list, or
encrypted bytes. Handshake failure is reported locally as a generic peer error.

## Transport preface and Noise suites

Every TCP connection starts with exactly twelve bytes:

| Offset | Size | Value |
| --- | ---: | --- |
| 0 | 8 | ASCII `HIDLINS` followed by `0x00` |
| 8 | 1 | protocol major `0x01` |
| 9 | 1 | mode: pairing `0x01`, trusted `0x02` |
| 10 | 2 | reserved, both bytes `0x00` |

The complete twelve bytes are the Noise prologue. Any missing, extra, altered,
unknown-mode, unknown-major, or nonzero-reserved byte terminates the connection
without an application response.

- Pairing uses exactly `Noise_XX_25519_ChaChaPoly_SHA256`.
- Trusted reconnect uses exactly `Noise_IK_25519_ChaChaPoly_SHA256`.
- There is no suite negotiation, fallback, resumption, or zero-RTT data.
- IK initiators pin the authoritative server static public key. The responder
  accepts only an active client static public key for this vault.
- Network and application bytes are accepted only after Noise reaches transport
  mode.

Pairing SAS is the first 30 bits of the final XX handshake hash, interpreted in
network bit order and encoded as six uppercase Crockford Base32 characters
using `0123456789ABCDEFGHJKMNPQRSTVWXYZ`. It is displayed as `XXX-XXX`. SAS
characters are derived independently on both endpoints and are never read from
peer application data.

## Encrypted record framing

Noise transport messages are carried as:

```text
u16 ciphertext_length | ciphertext[ciphertext_length]
```

The length is unsigned big-endian and must be in `1..=65535`. EOF within either
the prefix or ciphertext is truncation. A zero length, oversized logical
record, timeout, Noise authentication failure, or trailing partial record
terminates the session. Buffers are allocated only after the prefix is checked.

The decrypted application record is:

```text
u8 message_type | u32 request_id | u32 payload_length | payload[payload_length]
```

Integers are unsigned big-endian. `payload_length` must exactly equal the
remaining decrypted bytes. Request ID zero is reserved for connection-scoped
messages; request IDs `1..=0xffffffff` are chosen by the requester, must
increase monotonically, and may not be reused within a session. Monotonic IDs
make exact replay rejection constant-space instead of retaining an unbounded
set for the session. Unknown types, duplicate/non-increasing IDs, and extra
bytes are protocol errors. Strings are UTF-8 encoded as
`u16 length | bytes`, limited to 255 bytes even though the prefix admits more.
Booleans are exactly `0x00` or `0x01`. Versions and digests are exactly 32 raw
SHA-256 bytes.

## Application messages

| Type | Name | Payload |
| ---: | --- | --- |
| `0x01` | `HEAD_REQUEST` | empty |
| `0x02` | `HEAD_RESPONSE` | `u8 present`; if present, `version[32]` |
| `0x03` | `FETCH_REQUEST` | `u8 known_present`; if present, `known_version[32]` |
| `0x04` | `FETCH_UNCHANGED` | empty |
| `0x05` | `FETCH_BEGIN` | `version[32] | u64 total_length | digest[32]` |
| `0x06` | `FETCH_CHUNK` | `u32 sequence | u16 chunk_length | chunk` |
| `0x07` | `FETCH_COMMIT` | `u32 chunk_count` |
| `0x08` | `UPLOAD_BEGIN` | `u8 expected_present`; optional `expected_version[32]`; then `u64 total_length | digest[32]` |
| `0x09` | `UPLOAD_CHUNK` | `u32 sequence | u16 chunk_length | chunk` |
| `0x0a` | `UPLOAD_COMMIT` | `u32 chunk_count` |
| `0x0b` | `UPLOAD_ACCEPTED` | `version[32]` |
| `0x0c` | `CANCEL` | empty |
| `0x0d` | `ERROR` | `u16 error_code` |
| `0x20` | `PAIR_COMMIT` | `transaction_id[16] | transcript_digest[32] | u8 requested_role` |
| `0x21` | `PAIR_PREPARED` | `transaction_id[16] | transcript_digest[32]` |
| `0x22` | `PAIR_ACTIVATE` | `transaction_id[16] | transcript_digest[32]` |
| `0x23` | `PAIR_ACTIVATED` | `transaction_id[16] | transcript_digest[32]` |

`requested_role` is client `0x01`; all other values are invalid. Pair messages
are valid only in pairing mode. Vault messages are valid only in trusted mode.
A request ID identifies one operation; every response and stream record for
that operation repeats it. Only one fetch or upload stream may be active on a
connection.

The fixed generic errors are:

| Code | Meaning |
| ---: | --- |
| `0x0001` | invalid request |
| `0x0002` | not authorized |
| `0x0003` | busy |
| `0x0004` | stale version |
| `0x0005` | too large |
| `0x0006` | timed out |
| `0x0007` | cancelled |
| `0x0008` | internal failure |

Error payloads contain no strings. Authentication and authorization failures
close silently before application mode. Post-authentication internal errors use
only the codes above and are logged locally without secret material.

## Trusted-operation state machines

### Head

`HEAD_REQUEST -> HEAD_RESPONSE | ERROR`. The response describes the canonical
encrypted KDBX file currently owned by the server host. The version is SHA-256
of those exact bytes.

### Fetch

`FETCH_REQUEST -> FETCH_UNCHANGED` when the supplied version equals the current
version. Otherwise:

```text
FETCH_REQUEST -> FETCH_BEGIN -> FETCH_CHUNK* -> FETCH_COMMIT
```

Chunk sequences start at zero and increase by one. `FETCH_COMMIT.chunk_count`
must equal the number received. The receiver verifies total length and SHA-256
before parsing KDBX. Cancellation or failure discards the partial bytes.

### Conditional upload

```text
UPLOAD_BEGIN -> UPLOAD_CHUNK* -> UPLOAD_COMMIT
             -> UPLOAD_ACCEPTED | ERROR
```

An absent expected version means first seed and is valid only when no server
object exists. A present expected version must equal the server version sampled
at commit time. Chunks start at sequence zero, are contiguous, and may not be
duplicated. The server stages bytes in a sibling temporary file, verifies count,
length, SHA-256, KDBX identity, password/keyfile, and KDF compatibility, then
enqueues a conditional host commit. It writes `.kdbx.bak` before replacement
and uses the core atomic-save path. A stale version discards staging and returns
`stale version`; it never changes live or backup state.

`CANCEL` is valid only for the request ID of an in-progress stream. It discards
staging and ends that operation. Records after completion, error, or cancellation
for the same request ID are invalid.

## Pairing transaction

Pairing is available only during an explicit 180-second server window. A window
accepts at most three failed handshakes/confirmations and exposes at most one
active SAS candidate. Opening a new window invalidates all expired provisional
records; it never reactivates them.

After XX completes, both endpoints display the locally derived SAS and wait for
independent local confirmation. A rejection, expiry, disconnect before prepare,
or SAS mismatch counts as failure and creates no record.

After both humans have confirmed:

1. The client generates a CSPRNG 16-byte transaction ID and sends
   `PAIR_COMMIT`, bound to the final handshake hash, protocol major, both static
   public keys, server/client roles, and transaction ID through
   `transcript_digest = SHA-256(canonical_binding)`.
2. The server validates the binding and writes a non-authorizing provisional
   record atomically, then returns `PAIR_PREPARED` with the same identifiers.
3. The client validates it, writes its pinned-server record as provisional, and
   sends `PAIR_ACTIVATE`.
4. The server atomically promotes the matching record to active and returns
   `PAIR_ACTIVATED`. It never consults provisional records during trusted IK
   authorization.
5. The client validates the response and atomically promotes its record. A
   subsequent fresh IK session, not the XX session, is required for all vault
   access.

There is an unavoidable crash asymmetry after step 4: the server may hold an
active key while the client remains provisional if the final response is lost.
That key grants nothing to an attacker without the client's private key, the
client refuses vault operations while provisional, and both sides retain the
same bounded transaction receipt so an IK-authenticated recovery channel may
retry only `PAIR_ACTIVATE`/`PAIR_ACTIVATED` during the still-open window. The
receipt itself never authorizes head, fetch, or upload and expires after the
pairing window. Replays of the exact transaction are idempotent; a different
transaction for an already-active key is rejected. Pairing never silently
changes a client's pinned authority.

## Address policy

The exhaustive allowed endpoint ranges are:

- IPv4: `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`,
  `169.254.0.0/16`, `127.0.0.0/8`.
- IPv6: `fc00::/7`, `fe80::/10`, `::1`.

Every other address is forbidden, including unspecified, multicast,
documentation, benchmarking, carrier-grade NAT, and globally routable ranges.
IPv4-mapped IPv6 is normalized to IPv4 before classification. IPv6 link-local
endpoints require a nonzero interface scope ID. Sync never accepts DNS names or
an override. mDNS multicast is used only to discover candidates.

## Discovery contract

- Trusted service: `_hidlins-sync._tcp.local.`
- Pairing-window service: `_hidlins-pair._tcp.local.`
- Instance label: 128 random bits encoded as 26 lowercase Crockford Base32
  characters, freshly generated on each server start.
- TXT: one entry, `v=1`; no other key is valid in V1.

Advertisements contain no vault name, stable identifier, public key, version,
size, role list, capability list, or human device name. Discovery results are
deduplicated after address normalization and limited to 32 current canonical
scoped endpoints. Removal events and expiry evict entries. Candidate identity
is established only by Noise.

## Fixed resource limits

| Resource | Limit |
| --- | ---: |
| encrypted vault | 268,435,456 bytes |
| application chunk | 61,440 bytes |
| Noise ciphertext frame | 65,535 bytes |
| pairing window | 180 seconds |
| failures per pairing window | 3 |
| active SAS candidates | 1 |
| connect timeout per candidate | 2 seconds |
| candidate attempts per operation | 8 |
| total connect budget | 10 seconds |
| Noise handshake | 5 seconds |
| authenticated idle | 15 seconds |
| complete normal sync session | 300 seconds |
| accepted concurrent connections per vault | 8 |
| pending host vault operations | 16 |
| conditional-commit retries after first attempt | 1 |
| cached discovery endpoints | 32 |

Below-limit and exactly-at-limit values are accepted; above-limit values are
rejected before allocation, file mutation, or queue insertion. Timeouts use a
monotonic clock. Cancellation, shutdown, lock, or fatal error closes listeners,
removes advertisements, discards staging, and zeroizes session/identity state.

## Compatibility and evolution

Major version 1 has no extensions. Reserved bytes, unknown messages, unknown
fields, and trailing payload bytes are errors. A future major version requires
a new preface value and explicit product/design approval. V1 contains no
alternate schema, parser, migration, alias, or compatibility behavior.

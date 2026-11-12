# Snow 0.10.0 Dependency and Patch Dossier

Status: accepted for the local-network sync implementation, subject to the
exact-version, vendored-source, zeroization-patch, and audit gates below.

## Purpose and decision

Snow owns the security-sensitive Noise state machine, X25519 Diffie-Hellman
integration, transcript hashing, HKDF, ChaCha20-Poly1305 transport state, and
the XX/IK handshake implementations. Hidlins does not implement or negotiate
these cryptographic mechanisms. All product access is constrained by
`crate::noise` to these two names:

- `Noise_XX_25519_ChaChaPoly_SHA256`
- `Noise_IK_25519_ChaChaPoly_SHA256`

The dependency is exactly `snow = 0.10.0`; default features are disabled.
Only `std`, `use-curve25519`, `use-chacha20poly1305`, `use-sha2`, and
`use-getrandom` are requested. The vendored patch changes Snow's `std` feature
edges to weak optional edges (`dependency?/std`), preventing `std` from
implicitly enabling Ring or Blake2. `cargo tree -p snow -e features` is the
compiled-graph oracle: Snow contains X25519, ChaCha20-Poly1305, SHA-2,
getrandom, subtle, and zeroize, with no Ring, AES-GCM, Blake2, P-256, HFS, raw
split, or alternate resolver.

## License, source, and maintenance

- Package license: `Apache-2.0 OR MIT`, verified in both vendored manifests
  and the included `LICENSE-APACHE` and `LICENSE-MIT` files. Both choices are
  allowed by Hidlins' MIT-compatible policy.
- Source: <https://github.com/mcginty/snow>, registry release `0.10.0`, tag
  commit `4bb43f5`.
- Maintenance signal recorded 2026-09-06: the stable release was published
  2026-07-19; GitHub reported thirteen later commits, active 2026 pull
  requests, about 1.1k stars, and about 145 forks.
- Security lineage: Trail of Bits' public catalog records two four-engineer-
  week Snow reviews in 2024. The public nonce-increment advisory
  `RUSTSEC-2024-0011`/`GHSA-7g9j-g5jg-3vv3` affects versions before 0.9.5;
  0.10.0 includes the repair. `make audit` and `make deny` remain release
  gates. This does not establish that every 0.10.0 line or the Hidlins patch
  received an independent third-party audit.
- Upstream zeroization gap: issue 203 explicitly identifies missing erasure in
  `TransportState`, `HandshakeState`, `CipherState`, and `SymmetricState` for
  0.10.0. Shipping the unpatched crate is forbidden.

## Build and transitive cost

Snow's `build.rs` reads the Rust compiler channel through `rustc_version` and
emits Cargo configuration/warnings only. It performs no network access, file
generation, native compilation, or undocumented command execution.

The lockfile delta is four packages: `snow 0.10.0`,
`curve25519-dalek 4.1.3`, `curve25519-dalek-derive 0.1.1`, and
`fiat-crypto 0.2.9`. All other compiled packages in Snow's selected feature
subtree were already present in the workspace. Cargo's target-independent
vendoring copied those four new package directories. The dependency does not
introduce an async runtime, TLS/PKI stack, discovery framework, or native
library. The maintained vendor patch also removes Snow's weak `ring?/std`
feature edge: although it did not compile Ring, Cargo otherwise retained Ring
and its target-specific transitives in the lockfile and vendor tree. Snow's
optional Ring resolver remains available only in upstream source, not in the
curated Hidlins dependency graph.

## Alternatives considered

- Implementing Noise or its primitives in Hidlins was rejected: it violates
  the no-hand-rolled-crypto rule and would create a much larger audit burden.
- A general peer-to-peer stack such as libp2p was rejected: discovery,
  multiplexing, transports, runtimes, and identity formats exceed the required
  two-device LAN channel and greatly enlarge the dependency/license closure.
- TLS with a private certificate lifecycle was rejected for V1: it replaces a
  fixed Noise handshake with certificate generation, validation, pinning, and
  renewal machinery without improving the local trust ceremony.
- Other Rust Noise implementations did not offer a narrower, established,
  zeroizing XX/IK implementation. Replacing Snow with less mature code would
  not remove the need for source review or memory-hygiene work.

Snow is retained because it closes the major cryptographic gap with the
narrowest established implementation found. The remaining memory-hygiene gap
is explicit and locally reviewable rather than hidden behind a larger stack.

## Zeroization patch inventory

`tools/dev/vendor.py` owns the complete reproducible patch. It refuses any
version other than 0.10.0, applies exact source-context replacements, verifies
whole-file SHA-256 digests for every patched file, and refreshes Cargo's vendor
checksum. `make vendor-patches` proves idempotency and runs negative tests
against a missing patch marker, arbitrary source drift, and an unknown version.

| Secret-bearing state or transition | Patch control |
| --- | --- |
| Generated static keypair | `Keypair: ZeroizeOnDrop`; Hidlins copies the private half into `Zeroizing<[u8; 32]>` |
| Default X25519 static/ephemeral state | `Dh25519: Zeroize + ZeroizeOnDrop` |
| Default ChaChaPoly key | `CipherChaChaPoly: Zeroize + ZeroizeOnDrop` |
| Handshake chaining key/hash and checkpoint | `SymmetricStateData: ZeroizeOnDrop`; checkpoints clone into the same type |
| Handshake cipher key, HKDF, HMAC, rekey, and DH temporaries | `Zeroizing` stack buffers |
| Optional PSK copies | owned `Zeroizing<[u8; 32]>` values |
| SHA-256/SHA-512 working state | `Drop` resets the digest state before deallocation |
| Stateful and stateless transport keys | explicit `Drop` calls through both cipher-state collections; concrete cipher keys also zeroize on drop |
| Consumed handshake-to-transport transition | moved transport ciphers remain protected; all non-moved handshake fields drop through their zeroizing owners |

The patch also covers disabled built-in cipher/DH implementations so enabling
one later cannot silently create an unguarded key-holding type; enabling any
such feature still changes the guarded manifests and therefore requires a new
dependency review.

## Evidence boundary and upgrade procedure

Safe Rust cannot soundly read an object after `drop`, and allocator-after-free
inspection would itself invoke undefined behavior. The automated evidence is
therefore source/ownership based: derive and `Drop` coverage, exact patched
digests, transition inventory, functional XX/IK tests, redacted formatting,
and encrypted transport tests. This proves that every reviewed owner invokes
erasure; it does not promise recovery-resistant RAM under compiler moves,
allocator copies, swap, suspend images, crash snapshots, or a hostile OS.

To upgrade Snow: update the exact version, re-run the full dependency dossier
and advisories, review every secret owner and transition in the new source,
port or remove each patch deliberately, update the whole-file digests, prove
the negative guard tests still fail, and run `make test-local-sync-security`,
`make deny`, and `make audit`. An upstream zeroization claim is not sufficient
without matching this inventory.

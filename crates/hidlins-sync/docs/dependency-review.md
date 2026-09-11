# Local-network sync final dependency and license review

Review date: 2026-09-07. Baseline revision:
`e6df55b84edb53649d4f7f6c36842803c08884d8`. Counts include the named crate
itself and were generated from the checked-in lockfiles and vendored sources.

## Before and after

| Measure | Retired transport baseline | Local-network production | Delta |
| --- | ---: | ---: | ---: |
| `hidlins-sync` unconditional direct normal dependencies | 16 | 13 | -3 |
| Desktop-only optional direct dependencies | 0 | 2 (`mdns-sd`, `if-addrs`) | +2 |
| Normal closure, host graph | 156 | 133 | -23 |
| Normal closure, macOS aarch64 | 156 | 133 | -23 |
| Normal closure, Linux x86_64 | 152 | 131 | -21 |
| Root `Cargo.lock` packages | 467 | 448 | -19 |
| Production vendor directories | 459 | 440 | -19 |

The repository has 442 vendor directories in total: the production 440 plus
isolated fuzz-only `libfuzzer-sys` and `jobserver`. The fuzz lock contains 165
packages because it reuses the production libraries under test; its activated
fuzzing closure is 136 packages. The three committed corpora contain 30 inputs
and approximately 120 KiB. These development assets are not application
artifacts.

The production direct set is `argon2`, `base64`, `chacha20poly1305`, `chrono`,
`getrandom`, `hidlins-core`, `serde`, `sha2`, `snow`, `tempfile`, `thiserror`,
`toml`, and `zeroize`. Desktop discovery additionally enables exact
`mdns-sd 0.20.3` and exact `if-addrs 0.15.0` only on macOS/Linux. iOS and
Android cannot resolve or link those two dependencies.

## New retained dependencies

### Snow 0.10.0

- Purpose: established Noise XX/IK state machines and RustCrypto X25519,
  ChaCha20-Poly1305, SHA-256, and HKDF integration. This closes a major
  security-sensitive gap that must not be implemented in-repository.
- License/provenance: `MIT OR Apache-2.0`, registry release from
  `mcginty/snow`, tag commit `4bb43f5`; both license texts are vendored.
- Maintenance/popularity signal: released 2026-07-19; thirteen later commits,
  active 2026 pull requests, roughly 1.1k stars/145 forks at review time, and
  two public Trail of Bits review engagements in 2024. This is a maintenance
  signal, not a warranty or an audit of the Hidlins patch.
- Features: exact version; defaults off; only `std`, `use-curve25519`,
  `use-chacha20poly1305`, `use-sha2`, and `use-getrandom`. Ring, AES-GCM,
  Blake2, P-256, HFS, and raw split are absent from the resolved graph.
- Transitive/vendor cost: four added packages/directories—Snow,
  `curve25519-dalek`, `curve25519-dalek-derive`, and `fiat-crypto`; other
  selected primitives were already present. No native library or networked
  build step.
- Narrower alternatives: an in-repository implementation violates the
  no-hand-rolled-crypto rule; libp2p is much larger; TLS adds certificate
  lifecycle; less established Noise crates did not close the zeroization and
  maturity gap.
- Patch: the repository maintains an exact-source, exact-digest zeroization and
  feature-closure patch through `tools/dev/vendor.py`. Secret-owner and
  transition coverage, limitations, upstream issue 203, build-script behavior,
  and upgrade procedure are reviewed in
  [`snow-dependency.md`](snow-dependency.md). `make vendor-patches` includes
  source-drift, missing-marker, unknown-version, and idempotency negative tests.

### mdns-sd 0.20.3 and if-addrs 0.15.0

- Purpose: synchronous desktop DNS-SD/mDNS advertising/browsing and safe
  interface enumeration. Discovery is routing only; it never authorizes a
  peer or bypasses `LocalEndpoint`.
- License/provenance: both are `MIT OR Apache-2.0`; `mdns-sd` is the exact
  `keepsimple1/mdns-sd` registry release. License manifests/texts were reviewed
  in the vendor delta.
- Maintenance/popularity signal: `mdns-sd` is an active focused project with
  current registry releases and downstream use; `if-addrs` is a small mature
  platform enumerator. Exact pins prevent unreviewed surface changes.
- Features: `mdns-sd` defaults are disabled, excluding its async, logging, and
  serde surfaces. Hidlins uses the synchronous receiver and bounded internal
  DTOs. Both direct dependencies are target-scoped and optional under
  `desktop-discovery`.
- Transitive/vendor cost: the desktop selection accounts for six directories,
  approximately 1.9 MiB: `mdns-sd`, `if-addrs`, `flume`, `socket-pktinfo`,
  `socket2`, and `spin`. Neither direct dependency has a build script; no
  mobile product graph includes them.
- Narrower alternatives: hand-writing cross-platform multicast/interface FFI
  would add unsafe platform code and testing burden; a full service-discovery or
  peer-to-peer stack is broader. The selected pair is the narrowest reviewed
  portable desktop adapter over the protocol's independent trust boundary.

### libfuzzer-sys 0.4.13 — development/testing only

- Purpose: compiler-instrumented, coverage-guided fuzzing of the address,
  frame/state, and pairing parsers through `cargo-fuzz 0.13.2` and pinned
  `nightly-2026-09-01`.
- License/provenance: `(MIT OR Apache-2.0) AND NCSA`, exact Rust Fuzz registry
  crate. The default `link_libfuzzer` feature compiles its bundled upstream LLVM
  libFuzzer source. Hidlins has no direct dependency on a separately installed
  LLVM library.
- Maintenance/popularity signal: maintained by the Rust Fuzz project; 0.4.13
  was released June 2026 and was the current reviewed version. `cargo-fuzz` is
  the standard Rust coverage-guided fuzz workflow.
- Features/transitives/vendor cost: activated only by the isolated fuzz
  binaries' `fuzzing` feature. It adds approximately 652 KiB plus `jobserver`
  (approximately 156 KiB); stable corpus replay does not link it.
- Narrower alternatives: an in-repository coverage engine cannot replace
  compiler instrumentation/sanitizer integration safely. Other engines add a
  second runtime without narrowing the audit boundary.
- Shipping boundary: fuzz executables are non-distributable. Production
  `deny.toml` rejects NCSA. Only `fuzz/deny.toml` allows exact
  `libfuzzer-sys@0.4.13`. `make ncsa-boundary-check` proves manifest/lock
  isolation, production-closure exclusion, exact exception scope, packaging
  hook coverage, negative contaminated raw/bundle/APK/IPA controls, and scans
  produced CLI/TUI/agent/desktop/iOS/Android artifacts. Full details are in
  [`../../../fuzz/DEPENDENCIES.md`](../../../fuzz/DEPENDENCIES.md).

## Removal and policy disposition

The retired HTTP/cloud-specific direct dependencies and unreachable
transitives were removed only after transport-neutral merge/CAS/atomic-write
characterization and replacement tests existed. The reproducible baseline,
reachability rules, deleted areas, and final footprint remain in the dedicated
removal-inventory provenance document; there is no compatibility or migration
path.

`make deny`, `make audit`, `make vendor-patches`, `make s3-removal-check`, and
`make ncsa-boundary-check` are release gates. Any Snow source/feature drift,
additional NCSA package, production NCSA edge, unscanned packaging path,
forbidden license, advisory, or missing vendored package blocks release.

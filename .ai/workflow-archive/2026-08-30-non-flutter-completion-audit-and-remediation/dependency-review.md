# Direct Dependency Review

Audit date: 2026-08-31

Source of truth: the six included manifests, `Cargo.lock`, offline `cargo metadata`, inverse `cargo tree` checks for advisory/yank paths, and final `make deny`/`make audit` results.

All resolved direct dependencies use an allowed license expression. No manifest dependency with a missing, ambiguous, copyleft-only, Commons-Clause, SSPL, or forbidden linking-exception license was found. `make deny` passes. This document records the human review behind that automated gate rather than replacing it.

Footprint labels are comparative maintenance categories, not precise crate counts:

- leaf: zero or a few small transitives;
- shared: already required broadly in the workspace;
- medium: a focused subsystem graph;
- major: a platform/framework/crypto stack requiring continued audit.

## First-party edges

| Dependency | Consumers | License | Purpose | Footprint | Decision |
| --- | --- | --- | --- | --- | --- |
| `hidlins-core` | sync, CLI, TUI | MIT | Vault, KDBX, entry, registry authority | first-party | Keep; narrow raw model exposure under `HIDLINS-ARCH-002`. |
| `hidlins-genpw` | CLI, TUI | MIT | Password/passphrase generation | first-party | Keep. |
| `hidlins-security` | CLI, TUI | MIT | Clipboard, auto-lock, OS events, hardening | first-party | Keep. |
| `hidlins-sync` | CLI, TUI; self dev edge for test helpers | MIT | S3 transport and merge orchestration | first-party | Keep; self dev edge is the Cargo feature seam for integration tests. |

## Shared foundational dependencies

| Dependency | Resolved version | Consumers | License | Purpose / footprint | Replaceability decision |
| --- | --- | --- | --- | --- | --- |
| `serde` | 1.0.228 | core, security, sync, CLI, TUI | MIT OR Apache-2.0 | Shared serialization derive ecosystem. | Keep; replacing would duplicate format-critical serializers. |
| `serde_json` | 1.0.149 | sync, CLI | MIT OR Apache-2.0 | JSON CLI schemas and IMDS response parsing; shared. | Keep; hand parsing is less safe and not meaningfully smaller. |
| `toml` | 0.8.23 | core, security, sync, CLI, TUI | MIT OR Apache-2.0 | Registry/config persistence; shared. | Keep; format-critical parser. |
| `thiserror` | 2.0.18 | all six | MIT OR Apache-2.0 | Typed error derivation; leaf/shared. | Keep; removal would add repetitive manual impls without reducing the graph materially. |
| `zeroize` | 1.8.2 | core, genpw, sync, CLI, TUI; security `desktop` feature | MIT OR Apache-2.0 | Secret-memory destruction and marker derives; leaf/shared. | Keep; security primitive, do not reimplement. Task 003 gates the security edge with its sole clipboard/secret consumer so no-default builds do not compile an unused dependency. |
| `chrono` | 0.4.44 | core, sync, CLI, TUI | MIT OR Apache-2.0 | KDBX timestamps, sync timing, presentation; shared. | Keep with minimized features. |
| `uuid` | 1.23.1 | core, CLI, TUI | Apache-2.0 OR MIT | Stable entry/group identity; shared. | Keep; KDBX/API identity primitive. |
| `libc` | 0.2.186 | core, security, CLI, TUI on Unix | MIT OR Apache-2.0 | mlock, setrlimit, termios/signal boundaries; shared leaf binding. | Keep; avoids duplicating unsafe platform declarations. |
| `tempfile` | 3.27.0 | core normal; security/TUI dev | MIT OR Apache-2.0 | Atomic sibling temporary files and isolated tests; shared. | Keep in core because atomic replacement is security/data-integrity critical. |

## Vault and entry core

| Dependency | Version | License | Purpose / footprint | Replaceability decision |
| --- | --- | --- | --- | --- |
| `keepass` | 0.13.25 | MIT | KDBX3/4 parsing, encryption, writing, and merge; major crypto/format graph. | Keep; product-defining format/security primitive. Upgraded in Task 001 to remove the yanked cipher release and lift runtime XML parsing to fixed `quick-xml 0.41.0`; never reimplement KDBX or crypto. Its two simple hexadecimal decoding calls use the audited vendored patch described below. Task 004 adds a version-pinned API repair for pool-safe historical attachment mutation: mutable history access, fail-closed attachment resolution, attachment-name enumeration, parent-id inspection, history-index-aware reference removal, and deterministic rebuilding of current/history binary back-references after parse, merge, and tracked edits. This is narrower than forking KDBX serialization or using unsafe layout access, and is covered by encrypted save/reopen plus KeePassXC interop tests. |
| `rust-argon2` | 3.0.0 | MIT/Apache-2.0 | KDF config type compatibility for new KDBX databases; crypto graph. | Keep while required by `keepass` config API; do not reimplement. |
| `base32` | 0.5.1 | MIT OR Apache-2.0 | TOTP secret decoding; leaf. | Keep for now; a small decoder is possible but security-sensitive parsing and minimal footprint make replacement higher risk than value. |
| `hmac` | 0.12.1 | MIT OR Apache-2.0 | TOTP HMAC; crypto leaf/shared. | Keep; cryptographic primitive. |
| `sha1` | 0.10.6 | MIT OR Apache-2.0 | Standards-required TOTP SHA-1 option. | Keep; cryptographic primitive, not a password-hash choice. |
| `sha2` | 0.11.0 | MIT OR Apache-2.0 | TOTP SHA-256/512 and hashes; shared crypto. | Keep; cryptographic primitive. |

## Password generation

| Dependency | Version | License | Purpose / footprint | Replaceability decision |
| --- | --- | --- | --- | --- |
| `getrandom` | 0.4.2 | MIT OR Apache-2.0 | Direct OS CSPRNG; leaf/platform adapters. | Keep; the in-repository rejection sampler already avoids the larger `rand` graph. Never reimplement OS entropy access. |

## Security behaviors and platform features

| Dependency | Version | Target/feature | License | Purpose / footprint | Replaceability decision |
| --- | --- | --- | --- | --- | --- |
| `arboard` | 3.6.1 | `desktop` | MIT OR Apache-2.0 | Cross-platform clipboard; medium platform graph with defaults trimmed. | Keep; clipboard protocols are platform-specific and security-sensitive. Continue without image features. |
| `signal-hook` | 0.3.18 | `desktop`, Unix use | Apache-2.0/MIT | Async-signal-safe handling without local unsafe; leaf/medium. | Keep; safer and narrower than hand-written signal globals. |
| `zbus` | 5.15.0 | Linux `logind` | MIT OR Apache-2.0 | DBus/logind subscription; major optional graph. | Keep; DBus is a complex wire/protocol stack, and its supported graph now resolves patched `event-listener 5.4.2` without an advisory exception. |
| `async-channel` | 2.5.0 | Linux `logind` | MIT OR Apache-2.0 | Shutdown/signal race, already in zbus graph; leaf incremental cost. | Keep while current blocking adapter requires it. |
| `futures-lite` | 2.6.1 | Linux `logind` | MIT OR Apache-2.0 | Minimal future racing for zbus streams; shared in optional graph. | Keep; replacing an executor/stream combinator locally is not justified. |
| `objc2` | 0.6.4 | macOS `iokit` | MIT OR Apache-2.0 | Audited Objective-C runtime binding; medium optional graph. | Keep; unsafe FFI binding primitive. |
| `objc2-foundation` | 0.3.2 | macOS `iokit` | MIT OR Apache-2.0 | Foundation notifications/types with curated features. | Keep, feature-minimized. |
| `objc2-app-kit` | 0.3.2 | macOS `iokit` | MIT OR Apache-2.0 | NSWorkspace sleep notification. | Keep, feature-minimized. |
| `objc2-core-foundation` | 0.3.2 | macOS `iokit` | MIT OR Apache-2.0 | CFRunLoop/CF types. | Keep, feature-minimized. |

## Sync, authentication, and S3

| Dependency | Version | License | Purpose / footprint | Replaceability decision |
| --- | --- | --- | --- | --- |
| `argon2` | 0.5.3 | MIT OR Apache-2.0 | RST-CRED-1 key derivation; crypto medium. | Keep; cryptographic primitive. The second Argon2 crate exists because `keepass` exposes a different implementation/version family. |
| `chacha20poly1305` | 0.10.1 | Apache-2.0 OR MIT | RST-CRED-1 authenticated encryption; crypto medium. | Keep; cryptographic primitive. Its direct graph is distinct from the yanked `chacha20` under `keepass`. |
| `base64` | 0.22.1 | MIT OR Apache-2.0 | RST-CRED-1 text container; leaf. | Keep; tiny, ubiquitous, and safer than duplicating edge-case parsing. |
| `gethostname` | 1.1.0 | Apache-2.0 | Non-secret sync-log host label; leaf with platform bindings. | Keep. The standard library has no safe portable hostname API; a local replacement would add unsafe platform declarations and divergent macOS/Linux behavior for negligible graph reduction. |
| `getrandom` | 0.3.4 | MIT OR Apache-2.0 | RST-CRED-1 salt/nonce entropy; shared crypto/platform. | Keep; OS entropy primitive. Version duplication is upstream-compatibility driven and should collapse only through safe upgrades. |
| `secrecy` | 0.10.3 | Apache-2.0 OR MIT | Secret access/session token exposure discipline; leaf/shared zeroize graph. | Keep; explicit exposure control is security value beyond a trivial wrapper. |
| `hmac` | 0.13.0 | MIT OR Apache-2.0 | SigV4 signing; shared with keepass generation. | Keep; cryptographic primitive. |
| `sha2` | 0.11.0 | MIT OR Apache-2.0 | SigV4 and vault hashes; shared crypto. | Keep. |
| `hex` | removed | n/a | Formerly provided only lowercase encoding in sync plus simple downstream encode/decode operations. | Removed in Task 001 after behavior-pin tests. Hidlins owns a 16-digit lowercase encoder; narrow audited vendored patches cover `keepass` key material, Flutter bridge metadata, zbus SASL text, and termwiz capability-name decoding. The package is absent from manifests, `Cargo.lock`, `vendor/`, and the all-features/all-targets graph. No hash, signing, cipher, KDBX, DBus, or terminal protocol primitive was reimplemented. |
| `ureq` | 3.3.0 | MIT OR Apache-2.0 | Blocking HTTP over rustls/platform verifier; major TLS/platform graph (~51 vendored additions documented at introduction). | Keep; HTTP/TLS must not be hand-rolled. Current features intentionally exclude forbidden MPL webpki roots and compression. |

## CLI

| Dependency | Version | License | Purpose / footprint | Replaceability decision |
| --- | --- | --- | --- | --- |
| `clap` | 4.6.1 | MIT OR Apache-2.0 | Stable subcommand/argument parser; medium derive graph with features trimmed. | Keep; project-standard parser and compatibility surface. |
| `clap_complete` | 4.6.5 | MIT OR Apache-2.0 | Bash/zsh/fish generation; leaf over clap. | Keep; shell grammar generation is not a worthwhile local reimplementation. |
| `nix` | 0.30.1 | MIT | Safe Unix termios prompt handling with only `term`; focused platform binding. | Keep; avoids local unsafe and the much larger `rpassword` off-target graph. |

## TUI

| Dependency | Version | License | Purpose / footprint | Replaceability decision |
| --- | --- | --- | --- | --- |
| `ratatui` | 0.30.2 | MIT | Rendering/layout/widgets; major TUI framework graph. | Keep as the foundational UI framework. Task 001 selected the narrow compatible patch release that resolves `ratatui-core 0.1.2` and fixed `lru 0.18.3`; the complete TUI test and snapshot surfaces pass. |
| `crossterm` | 0.29.0 | MIT | Terminal events/raw mode/backend; medium platform graph. | Keep; terminal protocol handling is not a safe small reimplementation. |
| `tui-input` | 0.15.3 | MIT | Single-line editing state and crossterm event translation across search, palette, bulk, edit, and sync-config overlays. Its dependency graph is the already-required ratatui/crossterm/Unicode stack. | Keep. Task 006 confirmed it serves five production overlay families, while the local text area deliberately solves multiline notes. Reimplementing cursor/edit semantics would add application code without removing a resolved package family or improving the security boundary. |
| `unicode-width` | 0.2.2 | MIT OR Apache-2.0 | Correct terminal cell width; leaf with Unicode tables. | Keep; Unicode width tables should not be copied/hand-maintained. |
| `anyhow` | removed direct edge | MIT OR Apache-2.0 | Previously declared but unused by the TUI. | Removed in Task 001; the typed exit path already covers startup/runtime failures. The package remains transitively required by unrelated framework crates, so this removes review responsibility for a needless first-party edge without claiming a graph-wide package removal. |

## Test-only direct dependencies

| Dependency | Consumers | License | Purpose / decision |
| --- | --- | --- | --- |
| `nix` | security tests | MIT | Keep for safe signal delivery in integration helpers. |
| `fastrand` | core randomized round-trip test | MIT OR Apache-2.0 | Keep; already present through `tempfile`, so the direct dev edge adds no resolved package and gives deterministic lightweight test data. |
| `secrecy` | CLI sync-credential test | Apache-2.0 OR MIT | Keep; already a production sync dependency and required to inspect the protected value in the boundary assertion. |
| `proptest` | sync tests | MIT OR Apache-2.0 | Keep; high-value no-data-loss/idempotence property coverage. |
| `proptest-derive` | sync tests | MIT OR Apache-2.0 | Keep while used by scenario generation. |
| `arbitrary` | sync tests | MIT OR Apache-2.0 | Keep; fuzz-seed compatibility and structured scenario generation. |
| `sha2`, `tempfile` | sync/security/TUI tests | allowed as above | Reuse production/shared primitives; no incremental policy concern. Existing test-only `hex` calls migrate to the Task-001 local encoder and the crate leaves the graph. |

## Immediate advisory, yank, and metadata decisions — Task 001

| Item | Reachability | Current decision |
| --- | --- | --- |
| `event-listener 5.4.2` | Optional but supported Linux logind graph through zbus/async crates | Resolved: upgraded from 5.4.1 to the upstream patched release; the logind feature compiles under `make check`. Apache-2.0 OR MIT. No ignore added. |
| `lru 0.18.3` | Production TUI graph through `ratatui-core 0.1.2` | Resolved: upgraded through `ratatui 0.30.2`; full TUI tests and snapshots pass. MIT. No ignore added. |
| `chacha20 0.10.2` | Production KDBX graph through `keepass 0.13.25` | Resolved: upgraded only through released upstream crypto/KDBX packages. The yanked 0.10.0 is absent; no cipher algorithm was modified locally. MIT OR Apache-2.0. |
| Existing `quick-xml` advisory exceptions | `quick-xml 0.41.0` in KDBX runtime; `0.39.4` only in Wayland code generation | Reconciled: the untrusted KDBX runtime path is fixed. The two existing RustSec ignores now describe only `wayland-scanner` parsing trusted crate-shipped protocol XML at build time; no fixed parent release exists and no ignore was added or broadened. |
| `allo-isolate 0.1.27` missing manifest license field | Resolved Flutter/API package is compiled by shared workspace containment gates | Verified: `vendor/allo-isolate/LICENSE` is the complete Apache License 2.0 and the published manifest points to it with `license-file = "LICENSE"`. `cargo-deny` recognizes the file and passes licenses while warning that upstream omitted an SPDX `license` field; this metadata warning is explained here rather than suppressed or assumed away. |

## Task-001 resolved-graph footprint

The remediation removes `hex` entirely and removes one unused direct `anyhow` edge. `event-listener` also drops its `concurrent-queue` edge. The released Keepass upgrade adds `indexmap` usage and a second `base64` line (`0.23.1`) while removing the vulnerable runtime XML version. The narrow Ratatui patch upgrade necessarily adds its current color/backend support packages: `approx`, `by_address`, `critical-section`, `libm`, `palette`, `palette_derive`, `palette_math`, `ratatui-termina`, and `termina`; it also updates the already-present Ratatui component, `bitflags`, `hashbrown`, and `strum` lines. This is a real net footprint increase accepted to remove reachable unsound `lru 0.16.4` while staying on the existing foundational TUI framework; replacing terminal rendering locally would be much larger and less safe.

The four downstream `hex` removals and the Task-004 Keepass historical-attachment API repair are reproducible: `tools/dev/vendor.py` verifies the exact audited upstream versions, refuses unknown patch contexts, refreshes Cargo checksums, and is wired into `make vendor` plus the offline `make vendor-patches` check. The zbus and termwiz codecs additionally have standalone unit coverage for all nibbles, mixed case, odd length, and invalid input. Any upstream version change therefore fails closed until the patch is re-reviewed. `.gitattributes` narrowly exempts upstream `quick-xml` CRLF bytes and one Ratatui Markdown hard break from Git's trailing-whitespace diagnostic; the vendored bytes and checksums remain identical to their published crates so supply diffs stay reviewable instead of being obscured by repository-wide line-ending rewrites.

## Task-001 dependency change record

| Change | Authoritative source and license | Maintenance / adoption signal | Alternatives and local-implementation decision | Validation and footprint result |
| --- | --- | --- | --- | --- |
| `event-listener 5.4.1 -> 5.4.2` | crates.io release from `smol-rs/event-listener`; Apache-2.0 OR MIT | Upstream published the precise RustSec repair in the active 5.4 line; it remains a shared async-ecosystem primitive. | Direct transitive pin was the narrowest compatible change. Reimplementing listener synchronization would be unsafe and would not remove zbus's async stack. | Drops `concurrent-queue` from this edge; logind compile and full gate pass. |
| `ratatui 0.30.0 -> 0.30.2`, `lru 0.16.4 -> 0.18.3` | crates.io releases from `ratatui/ratatui` and `jeromefroe/lru-rs`; MIT | Ratatui is the existing reference TUI framework and its maintained patch line adopted the fixed LRU series. | A transitive LRU source patch or local terminal renderer would create more long-term ownership than the compatible Ratatui patch. No new direct crate was introduced. | Full 405-test TUI surface, four serialized accessibility-mode tests, and all 13 checked-in snapshots pass. Net additions are listed in the footprint section above. |
| `keepass 0.12.9 -> 0.13.25`, `chacha20 0.10.0 -> 0.10.2` | crates.io releases from `snapview/keepass-rs` (MIT) and RustCrypto `stream-ciphers` (MIT OR Apache-2.0) | Both are active upstream release lines; 0.13.25 is the released Keepass parent that selects the non-yanked cipher and fixed XML parser. | Keepass/KDBX and ChaCha20 remain external security primitives. No cryptographic source was changed. Only Keepass's two non-cryptographic hex decode calls are locally patched. | Upgrade exposed KeePassXC's valid KDBX 4.0 write-back: red interop plus two red unit tests led to core normalization of KDBX3/4.0 to current KDBX4 while preserving cipher/KDF settings. All vault, merge, and three KeePassXC interop targets pass. |
| Remove `hex 0.4.3` | Former package was crates.io `KokaKiwi/rust-hex`, MIT OR Apache-2.0 | Stable but unnecessary for the small fixed behaviors in this graph. | Behavior was pinned first. Small lowercase encode and mixed-case decode loops are locally auditable; cryptographic hashes/signatures and DBus/terminal state machines remain upstream. Version-pinned patches cover keepass 0.13.25, Flutter bridge macros 2.12.0, zbus 5.15.0, and termwiz 0.23.3. | Removes the package from manifests, lockfile, vendor tree, and all-feature/all-target resolution. Focused encoder/keyfile/codec tests plus SigV4, logind, workspace, and interop gates pass. |
| Remove direct TUI `anyhow` | crates.io `dtolnay/anyhow`; MIT OR Apache-2.0 | Maintained and widely used, but unused by Hidlins TUI. | Deletion is strictly smaller than replacement; the typed TUI exit path already covers all behavior. | Direct manifest edge removed. The package remains transitively required by framework crates, which is recorded rather than hidden. |
| `quick-xml 0.40.1 -> 0.41.0` on KDBX runtime | crates.io `tafia/quick-xml`; MIT | Active upstream release contains the advisory repairs selected by Keepass 0.13.25. | Parent upgrade is preferable to a local XML parser or advisory patch. `wayland-scanner 0.31.10` has no released fixed parent, so its trusted build-time-only 0.39.4 path retains the two pre-existing narrow ignores. | Malicious KDBX input no longer reaches the affected parser; `make deny` and `make audit` pass with the exact remaining path documented. |
| Verify `allo-isolate 0.1.27` | Vendored package `LICENSE`; Apache License 2.0 | Existing Flutter bridge transitive; no new adoption decision in non-Flutter scope. | No replacement is justified in containment-only API compilation. Missing SPDX metadata was not treated as a license grant. | `cargo-deny` recognizes and accepts the file; its upstream `no-license-field` metadata warning is explicitly explained above. |

## Dependency-change rules for the remaining tasks

1. No new dependency is approved merely because it appears convenient; update this file before any manifest edit.
2. Verify exact license and source before executing new dependency code or writing against it.
3. Pin behavior with a failing/characterization test before removal, replacement, or upgrade.
4. Prefer removing an unused crate or narrowing features when that actually removes resolved/vendored packages.
5. Do not locally reimplement cryptography, OS entropy, TLS/HTTP, KDBX, terminal protocols, Unicode tables, DBus, or unsafe platform bindings.
6. Rerun offline locked build/tests, `make deny`, `make audit`, relevant interop, and inspect the vendored/lockfile diff for every accepted graph change.

## Task-007 final reconciliation

- Tasks 002–007 added no direct dependency, enabled no new dependency feature,
  changed no dependency version beyond the Task-001 reviewed graph, and added no
  license/advisory exception. Task 004's vendored Keepass API additions are the
  exact-version, checksum-refreshed downstream patch already recorded above.
- `hex` remains absent from manifests, `Cargo.lock`, `vendor/`, and the
  all-feature/all-target Cargo graph. The similarly named `hex-literal` package
  is a RustCrypto transitive macro and is not the removed general-purpose
  encoder.
- The removed TUI direct `anyhow` edge remains absent. `anyhow 1.0.103` is still
  transitively required only by the Flutter bridge containment graph
  (`flutter_rust_bridge` / `allo-isolate`), which this non-Flutter package did
  not claim to remove.
- The remediated supported paths resolve `event-listener 5.4.2`, `lru 0.18.3`,
  and `chacha20 0.10.2`; the affected 5.4.1, 0.16.4, and yanked 0.10.0 releases
  are absent.
- The repository's authoritative allow-list excludes license exceptions.
  Task 007 removed the stale `Apache-2.0 WITH LLVM-exception` allowance from
  both `deny.toml` and contributor guidance. The final targeted `cargo deny`
  graph requires no such license and passes without an exception.
- Every first-party direct dependency now has a final keep/remove decision in
  this file. All retained third-party dependencies have verified allow-listed
  licenses; there is no forbidden, ambiguous, missing-unverified, copyleft, or
  linking-exception license waiver. The upstream `allo-isolate` manifest
  metadata warning remains backed by its verified vendored Apache-2.0 license
  file rather than a policy exception.

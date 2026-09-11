# S3 Removal and Dependency Baseline

Captured from Git revision `e6df55b84edb53649d4f7f6c36842803c08884d8`
for Task 001. This is a deletion checklist, not a compatibility or migration
plan. Historical immutable workflow archives are excluded.

## Reproduce the baseline

Run from the repository root:

```sh
rg -l -i 's3|sigv4|minio|aws' \
  --glob '!vendor/**' \
  --glob '!app/vendor-pub/**' \
  --glob '!app/rust_builder/cargokit/build_tool/vendor-pub/**' \
  --glob '!.git/**' \
  --glob '!.ai/workflow-archive/**' .

cargo tree --offline --locked -p hidlins-sync -e normal \
  --prefix none --format '{p}' | sort -u | wc -l

cargo tree --offline --locked -p ureq -e normal \
  --prefix none --format '{p}' | sort -u | wc -l

find vendor -mindepth 1 -maxdepth 1 -type d | wc -l
```

Baseline counts:

- `hidlins-sync` has 16 direct normal dependencies, including the first-party
  `hidlins-core` dependency.
- Its normal dependency closure contains 156 unique package/version entries,
  including the workspace crate itself.
- The `ureq 3.3.0` normal closure contains 40 unique package/version entries,
  including `ureq` itself. This is an upper bound on removable HTTP/TLS nodes;
  shared packages must remain when another reachable consumer uses them.
- `vendor/` contains 459 top-level crate directories.

Task 012 must recalculate all four values and use `cargo tree -i <crate>` before
removing any shared dependency. Directory-count deltas are evidence, not proof
of reachability; `Cargo.lock`, offline builds, target checks, and `cargo deny`
are authoritative.

## Rust implementation

Delete or replace every S3-specific item in these areas:

- `crates/hidlins-sync/src/s3/` — endpoint construction, blocking HTTP client,
  SigV4 signer, ETag parsing, S3 error classification, and test hooks.
- `crates/hidlins-sync/src/auth/` — AWS environment/profile/IMDS credentials and
  the S3-specific RST-CRED-1 credential shape. The underlying authenticated
  sealed-secret technique may be generalized only through new tested types.
- `crates/hidlins-sync/src/transport/s3.rs` and S3 exports in
  `src/transport/mod.rs`.
- `crates/hidlins-sync/src/config.rs` — `TransportKind::S3`, `S3Config`, target
  uniqueness, bucket/key/region/endpoint, credential variants, If-Match probe
  state, and ETag-shaped divergence names.
- `crates/hidlins-sync/src/sync.rs` — production S3 construction, AWS
  credentials, HTTP user agent, and ETag names. Preserve the generic transport
  state machine, changing only the intentionally frozen version/retry naming
  and semantics.
- `crates/hidlins-sync/src/error.rs`, `src/sync_log.rs`, `src/lib.rs`, crate
  README/package description, and backend conformance documentation.
- `crates/hidlins-api/src/api/sync.rs`, `api/bootstrap.rs`, DTOs, errors, events,
  sync port, generated bridge, and API tests exposing S3 configuration or
  bootstrap.
- `crates/hidlins-cli/src/cli.rs`, `commands/sync.rs`, `commands/vault.rs`,
  views, prompts, exits, sync/set-sync tests, and generated shell completions.
- `crates/hidlins-tui/src/overlay/sync_config.rs`, sync runtime/config/settings,
  application actions/events/rendering, snapshots, journeys, and accessibility
  contracts.
- `crates/hidlins-agent` remains an unimplemented excluded surface; it must not
  gain an S3 compatibility shim or be mistaken for the approved foreground CLI
  server.

## Rust tests and fixtures

Remove S3-specific tests after replacement coverage exists:

- `crates/hidlins-sync/tests/sigv4_aws_test_vectors.rs`
- `crates/hidlins-sync/tests/data/aws_sigv4_vectors/` and its provenance file
- `crates/hidlins-sync/tests/minio_integration.rs`
- `crates/hidlins-sync/tests/common/minio_env.rs`
- `crates/hidlins-sync/tests/us_040_configure_target.rs`
- S3/ETag-specific assertions in `us_041` through `us_046`, while preserving
  transport-neutral behavior through renamed tests and fixtures
- `crates/hidlins-cli/tests/cli_set_sync.rs`, `cli_sync_minio.rs`, and
  S3-shaped portions of `cli_sync.rs`/exit-code tests
- S3-shaped API/TUI tests and generated-binding fixtures

The merge property tests, merge semantics, fault injection, KDBX round-trip,
and KeePassXC sync interoperability behavior are retained and renamed only when
needed for transport-neutral terminology.

## Flutter and native surfaces

Replace or delete:

- S3 models/repository methods in `app/lib/src/data/models.dart`,
  `repositories.dart`, and `bridge_repositories.dart`.
- `app/lib/src/features/sync/sync_controller.dart` and `sync_page.dart` S3
  forms/status behavior.
- `app/lib/src/features/vaults/connect_sync_dialog.dart` S3 bootstrap.
- S3 strings in `app/l10n/app_en.arb` and generated localization files.
- Generated Dart/Rust bridge S3 DTOs and calls.
- Flutter sync, vault-management, fake-repository, failure, real-bridge MinIO,
  Android S3, and iOS S3 tests/fixtures.
- `tools/ios-native/secure_s3_config.py`, `s3_integration.sh`, and S3-specific
  native harness assertions.
- Android native/emulator S3 configuration and MinIO assertions.

Platform files are then updated for Bonjour/NSD discovery permissions and thin
endpoint/permission adapters; they do not retain cloud credential APIs.

## Build, CI, harness, and generated artifacts

Remove or replace:

- `tools/sync-tests/` MinIO fixtures and documentation.
- `tools/dev/fetch-sigv4-corpus.sh`.
- S3/MinIO Make targets: SigV4, MinIO lifecycle/bucket, S3 integration,
  managed-MinIO, S3 interop, desktop real-bridge MinIO, Android emulator S3 or
  MinIO, and iOS simulator S3 or MinIO variants.
- S3/MinIO CI jobs, services, secrets, artifact names, and commands under
  `.github/workflows/`.
- Generated shell completions containing `set-sync` or old `sync` arguments.
- Vendor checksums and vendored crates only after dependency reachability proves
  they are unused.

New local-sync test commands must be Make targets before CI invokes them.

## Documentation and active requirements

Update active `README.md`, `CONTRIBUTING.md`, `CLAUDE.md`,
`docs/running-and-testing.md`, Flutter/iOS/Android release and verification
documents, PRD requirements/decisions, project index, and user-scenario
verifications. Historical archive/provenance documents are immutable and may
retain S3 mentions.

## Dependency candidates

The following direct dependencies are S3-only or partially S3-motivated and
must be re-evaluated after code deletion:

| Dependency | Current purpose | Removal decision rule |
| --- | --- | --- |
| `ureq 3.3.0` | blocking S3 HTTP over rustls | remove completely |
| `serde_json 1.x` | IMDS credential JSON | remove from sync if no new sync consumer |
| `hmac 0.13` | SigV4 | remove from sync if no new sync consumer |
| `gethostname 1.x` | S3 sync log host field | remove if local logs do not require it |
| `base64 0.22` | credential container/HTTP | retain only if generic identity sealing needs it |
| `argon2 0.5` | credential sealing | retain for identity sealing after type generalization |
| `chacha20poly1305 0.10` | credential sealing | retain for identity sealing |
| `getrandom 0.3` | credential salt/nonce | retain for identity and discovery randomness |
| `secrecy 0.10` | AWS credential strings | remove unless a concrete local-sync secret uses it |
| `sha2 0.11` | SigV4 and sync versions | retain for canonical versions/protocol digests |

Transitive removals are determined from the final workspace graph. Likely
HTTP/TLS-only families include `ureq-proto`, `rustls`,
`rustls-platform-verifier`, `rustls-webpki`, `ring`, `security-framework`,
`web-time`, and their platform-specific support, but none is deleted solely by
this list.

## Static completion rule

Task 012 adds an allowlist-based absence checker. Outside immutable archives,
the final active tree may use `s3` only where it is an unrelated substring in
third-party vendored data or an objectively unrelated domain term. Production
source, tests, UI, DTOs, configuration, Make targets, CI, harnesses, active
documentation, and first-party filenames must contain no AWS, S3, SigV4,
MinIO, bucket/key/region credential, or ETag compatibility surface.

## Final footprint

Recalculated after the removal and reproducible vendor refresh:

- `hidlins-sync` has 13 direct normal dependencies, down from 16. The current
  set is `argon2`, `base64`, `chacha20poly1305`, `chrono`, `getrandom`,
  `hidlins-core`, `serde`, `sha2`, `snow`, `tempfile`, `thiserror`, `toml`, and
  `zeroize`.
- Its target-independent normal closure contains 133 unique package/version
  entries, down from 156 (23 removed).
- The macOS aarch64 compiled closure is 133 entries, down from 156; the Linux
  x86_64 compiled closure is 131, down from 152.
- The workspace lockfile contains 448 packages, down from 467. The vendored
  tree contains 440 registry crate directories, down from 459. The 19-directory
  net deletion includes the retired HTTP/TLS graph despite adding the local
  Noise and discovery crates.
- `ureq`, `rustls`, the Android Rustls platform adapter, `rustls-webpki`,
  `webpki-root-certs`, `security-framework`, `web-time`, Ring, and Ring's
  `untrusted` dependency are absent from both the lockfile and vendored tree.

The remaining similarly motivated packages all have concrete local consumers:
`argon2`, `base64`, and `chacha20poly1305` protect persisted local identity;
`getrandom` supplies identity, pairing, and discovery randomness; `sha2`
provides protocol/version digests; and `snow` owns the authenticated XX/IK
session machinery. `gethostname` and `secrecy` remain only through non-sync
workspace consumers (clipboard/KDBX and test utilities) and are no longer
direct `hidlins-sync` dependencies.

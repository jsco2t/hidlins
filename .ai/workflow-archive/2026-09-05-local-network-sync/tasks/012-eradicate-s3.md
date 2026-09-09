# Task 012: Eradicate S3 and Clean the Dependency Graph

Delegation: main-only

## Goal

Delete every remaining active S3/AWS/SigV4/MinIO implementation and remove its exclusive dependency/vendor footprint, leaving no migration or compatibility code.

## Context

Earlier tasks keep some S3 internals temporarily so each intermediate task passes the full workspace gate while callers move to LAN APIs. Every surface is now migrated, so the old implementation can be removed atomically and proven absent.

## Scope

### In scope

- Delete S3 transport, HTTP client, endpoint/ETag, SigV4, AWS credential sources, IAM/profile/env resolution, S3 configuration, MinIO tests/fixtures/tools, real-S3 protected-config tools, and obsolete docs/help/comments.
- Delete all S3-specific API/CLI/TUI/Flutter/native tests and generated remnants not already removed.
- Remove `ureq`, S3-only rustls/ring/platform-verifier promotion, and every unreachable transitive/target/vendor crate; retain a dependency only with a documented non-S3 consumer.
- Remove/rename S3/MinIO/SigV4 Makefile targets and CI jobs; keep generic merge/KeePassXC interop under local-neutral names.
- Delete old `[sync.s3]`, `TransportKind::S3`, ETag, credential, and legacy parsing code. Do not translate existing files.
- Add a `s3-removal-check` script/Make target with a reviewed allowlist limited to immutable archives, approved historical provenance, and the removal check itself.
- Record before/after direct, transitive, compiled-target, and vendored footprint.

### Out of scope

- Editing immutable `.ai/workflow-archive/` history.
- S3 migration/import/aliases/deprecation warnings or support for old registry data.
- Deleting crypto/storage dependencies still required by local identity or Noise.

## Implementation requirements

- Pin removal behavior with tests before deleting implementation; retain transport-neutral merge/memory tests and KeePassXC sync interop.
- Old S3 `[sync]` data is an unknown/unconfigured block under registry forward-compatibility behavior; no local-sync code recognizes or rewrites it.
- The static check covers production code, build/CI scripts, generated bridge/completions, app/native harnesses, active docs, manifests, and dependency manifests/lockfile, with narrow explicit historical exceptions.
- Refresh Cargo and Dart/native dependency/vendor integrity files using repository workflows; inspect the full vendored diff.
- Update descriptions and module docs so local sync is the only active transport story.
- Capture red-before/green-after removal and dependency graph evidence.

## Acceptance criteria

- [ ] No active S3/AWS/SigV4/MinIO code, configuration, credentials, UI, API, tests, harness, target, CI job, or user documentation remains.
- [ ] No migration, compatibility, legacy parser, alias, or deprecation implementation exists.
- [ ] `ureq` and every S3-only direct/transitive/vendor dependency are absent; all retained related crates have a documented local-sync consumer.
- [ ] The static removal target fails on representative reintroduced S3 code/config/command strings and passes the final tree with only approved historical exceptions.
- [ ] Generic sync state/merge/fault/KDBX/KeePassXC behavior remains green after deletion.
- [ ] Lockfiles, vendored sources, integrity hashes, Make help, CI, generated bindings, and completions are internally consistent.

## Validation

- `make s3-removal-check`
- `make vendor-patches`
- `make completions-check`
- `make interop-sync`
- `make deny`
- `make audit`

## Dependencies

- Task 011

## Expected areas of change

- `crates/hidlins-sync/`
- `crates/hidlins-api/`
- `crates/hidlins-cli/`
- `crates/hidlins-tui/`
- `app/`
- `tools/sync-tests/`
- `tools/ios-native/`
- `tools/android-native/`
- `Cargo.toml`
- `Cargo.lock`
- `vendor/`
- `Makefile`
- `.github/workflows/ci.yml`
- active repository documentation

## Risks / notes

Broad text matching can flag unrelated Dart packages such as `sync_http` or historical provenance. The check must match S3/AWS/MinIO concepts precisely and keep every exception explicit, path-scoped, and reviewed.

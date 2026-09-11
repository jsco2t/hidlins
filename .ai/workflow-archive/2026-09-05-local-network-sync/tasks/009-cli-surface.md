# Task 009: Replace the CLI Sync Surface

Delegation: main-only

## Goal

Deliver scriptable local discovery/pairing/import/sync/status/peer management and safe foreground serving, plus pre-operation startup sync for every CLI command that unlocks a configured vault.

## Context

The CLI currently has one S3 sync command and `vault set-sync`. Its command handlers independently open vaults, so startup behavior must be centralized without leaking master passwords or making sync failure fatal to local work.

## Scope

### In scope

- Replace the command tree with `sync now`, `sync serve`, `sync pair`, `sync import`, `sync status`, and `sync peers list|rename|revoke`.
- Restricted `--address`/`--port` diagnostic fallback and explicit server pairing-window option/action.
- Secure prompts for vault master/keyfile use and bilateral SAS confirmation.
- Foreground server readiness, pairing events, `Ctrl+C`, clean shutdown, exit codes, human output, JSON schemas, and secret redaction.
- A common vault-open helper that performs one configured pre-operation sync for all vault-opening CLI operations, then continues locally with a warning on failure.
- Removal of `vault set-sync`, all S3 flags/credential prompts, and CLI MinIO/S3 tests.
- Regenerated bash/zsh/fish completions and docs/help contracts.

### Out of scope

- TUI/Flutter UI, mobile behavior, daemonizing/service managers, or background CLI serving.
- Final core S3 module/dependency deletion.

## Implementation requirements

- `sync serve` stays in the foreground and owns one unlocked vault until `Ctrl+C`, lock/fatal error, or termination; no fork/daemon flag exists.
- Pair/import never accepts master passwords or private material through arguments or environment variables.
- Startup sync precedes requested reads/mutations, never follows save, and a failure does not change the local command's success semantics. Human and JSON-mode warning behavior must be stable and documented without corrupting JSON stdout.
- Exit 3 remains reserved for a manual sync conflict requiring user action; auth remains 2, user errors 1, internal 10+, with distinct local-network categories mapped consistently.
- Pairing output may display only the SAS and non-secret transaction context, never keys/transcripts.
- Add red-before/green-after spawned-process tests, including signal cleanup and no-S3 argument acceptance.

## Acceptance criteria

- [ ] Every required command succeeds through real local client/server process tests and has stable human/JSON contract coverage.
- [ ] `sync serve` reports readiness, advertises only allowed interfaces, and `Ctrl+C` removes service/listener and zeroizes/cleans in-flight state.
- [ ] Pair/SAS/import and peer revocation flows are usable without command-line/environment secrets.
- [ ] Every CLI operation that unlocks a configured vault attempts pre-operation sync exactly once, never post-save, and continues with a clear warning when offline.
- [ ] S3 flags, `vault set-sync`, credential-source grammar, and MinIO/S3 CLI paths are rejected/absent.
- [ ] Generated completions exactly match the new command tree.

## Validation

- `cargo test -p hidlins-cli --offline --locked`
- `make test-local-sync-integration`
- `make test-local-sync-security`
- `make completions-check`

## Dependencies

- Task 008

## Expected areas of change

- `crates/hidlins-cli/src/cli.rs`
- `crates/hidlins-cli/src/commands/`
- `crates/hidlins-cli/src/views/`
- `crates/hidlins-cli/src/exit.rs`
- `crates/hidlins-cli/tests/`
- `shell-completions/`
- `Makefile`

## Risks / notes

Applying startup sync consistently across independently implemented commands is easy to miss. A table-driven command inventory and a static/behavior test must prove every vault-opening route uses the common helper.

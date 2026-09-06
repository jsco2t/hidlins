# Task 015: Add authenticated existing-vault CLI registration

Delegation: main-only

## Goal

Make an arbitrary existing supported KDBX vault usable by the CLI through an
authenticated, atomic `hidlins vault register` command, and correct the
application-running guide to exercise the kitchen-sink fixture directly.

## Context

The CLI currently creates and registers new vaults, and opens already-registered
vaults by ID, but it has no path that registers an existing KDBX. The TUI already
proves the architecture supports authenticate-before-register, and the core
already owns a concurrency-safe `register_and_save` transaction. The testing
guide exposed this omission by requiring a second disposable CLI vault.

The guide, `make demo-vault` target, and generator documentation were drafted
after acceptance round 1 completed. This task owns reviewing, correcting, and
validating those follow-up changes along with the CLI capability they exposed.

## Scope

### In scope

- A `vault register` CLI verb with required `--id` and `--path` and optional
  `--keyfile`.
- Secure authentication before registration, canonical persisted paths, and an
  atomic concurrency-safe registry update.
- Human and JSON output/help contracts and generated bash/zsh/fish completions.
- End-to-end regression, failure, security, byte-preservation, keyfile, and
  command-contract tests.
- Accurate CLI/subcommand documentation.
- Review and validation of `docs/running-and-testing.md`, `make demo-vault`, and
  the disposable kitchen-sink generator documentation.

### Out of scope

- Passing unregistered paths directly to entry or sync commands.
- Changing the ID-based `vault open` probe or adding agent caching.
- Prohibiting multiple names that refer to the same vault path.
- Adding deregistration, rename, or registry migration features.
- Adding a desktop native existing-vault file picker.
- Any new dependency.

## Implementation requirements

1. Before product implementation, add the smallest end-to-end CLI regression
   that invokes `vault register` against an unregistered fast-KDF KDBX fixture,
   run it against the current command surface, and record the expected red
   result.
2. Add the clap command and arguments without accepting master passwords through
   flags or environment variables.
3. Resolve the target registry and reject an existing ID before prompting.
4. Use the existing secure master-password prompt and `Vault::open` with the
   optional keyfile. Persist nothing until authentication succeeds.
5. After a successful open, canonicalize the vault and optional keyfile paths,
   drop the open vault handle, then use `VaultRegistry::register_and_save` so a
   concurrent duplicate registration cannot be lost or overwrite newer state.
6. Do not write or migrate the existing KDBX. Tests must compare its bytes
   before and after success and representative failure paths.
7. Return established typed exit behavior for duplicate IDs, authentication
   failure, missing/invalid KDBX, invalid keyfile, registry contention, and I/O.
   Do not include a password, keyfile contents, entry contents, or other secret
   material in output or errors.
8. Add a dedicated registration view with pinned human and JSON behavior.
9. Regenerate and commit the bash, zsh, and fish completions through
   `make completions`.
10. Update relevant CLI documentation and replace the guide's separate
    `vault create` workaround with registration and use of the generated
    kitchen-sink vault.
11. Review the new `demo-vault` target for safe absolute-path handling and
    overwrite refusal. Keep generation offline and warning-fatal through
    `make`; do not commit a generated KDBX binary.
12. Record fail-before/pass-after evidence for the CLI regression. For the
    documentation and Makefile portions, record the static and execution
    validation used where a meaningful red product test does not apply.

## Acceptance criteria

- [ ] `hidlins vault register --id NAME --path FILE [--keyfile FILE]` appears in
      help and all committed completion scripts.
- [ ] A correct master password registers an existing KDBX under the requested
      ID, persists canonical paths, leaves KDBX bytes unchanged, and makes
      `vault open` plus an ID-based entry operation succeed.
- [ ] A correct password plus required keyfile registers and reopens a
      keyfile-protected KDBX.
- [ ] Wrong passwords, missing/invalid vaults, bad keyfiles, and duplicate IDs
      do not create or mutate registry state or modify the KDBX.
- [ ] Concurrent registry mutation is protected by the core's transactional
      registration path.
- [ ] Human and JSON registration output are stable and secret-free.
- [ ] `docs/running-and-testing.md` demonstrates the generated kitchen-sink
      vault through the isolated CLI, TUI, desktop, iOS Simulator, and Android
      emulator workflows without claiming unsupported behavior.
- [ ] `make demo-vault` creates a KeePassXC-readable synthetic fixture and
      rejects missing, relative, and existing output paths.
- [ ] No external dependency is added.

## Validation

- `make completions-check`
- `make test`
- `git diff --check`

## Dependencies

Tasks 001–014 (completed acceptance round 1)

## Expected areas of change

- `crates/hidlins-cli/src/cli.rs`
- `crates/hidlins-cli/src/commands/vault.rs`
- `crates/hidlins-cli/src/views/vault.rs`
- `crates/hidlins-cli/tests/cli_vault.rs`
- `crates/hidlins-cli/tests/common/mod.rs`
- `shell-completions/`
- `CONTRIBUTING.md`
- `docs/running-and-testing.md`
- `docs/flutter-alpha-release.md`
- `Makefile`
- `crates/hidlins-core/examples/seed_demo_vault.rs`

## Risks / notes

- Registration is security- and data-integrity-sensitive, so the task remains
  main-only even though the code change is narrow.
- Canonicalization intentionally resolves symlinks and relative paths after a
  successful authentication probe.
- The existing core transaction, rather than a new CLI-owned locking scheme,
  remains authoritative for concurrent registration.

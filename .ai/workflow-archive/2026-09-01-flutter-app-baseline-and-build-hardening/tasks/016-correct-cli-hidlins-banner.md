# Task 016: Correct the CLI Hidlins banner

Delegation: worker-eligible

## Goal

Make top-level CLI help identify the product as Hidlins with an unambiguous,
fully regression-tested ASCII wordmark.

## Context

The clap command is named `hidlins`, but `BANNER` still renders the historical
Falach wordmark. Current tests check only generic underscores and a fragment of
the wrong logo, so they preserve the acceptance bug.

## Scope

### In scope

- Replace the top-level help banner with a HIDLINS ASCII wordmark.
- Strengthen unit and process help tests to pin the intended identity and
  complete banner placement.
- Correct stale comments and test names that describe the Falach `F` glyph.

### Out of scope

- CLI command behavior, descriptions, output schemas, or shell completions.
- TUI startup art or Flutter brand assets.
- Any new dependency.

## Implementation requirements

1. First replace or strengthen the process-level regression so it requires the
   intended HIDLINS banner and rejects the known Falach fragment; run it against
   the current implementation and record the red result.
2. Replace `BANNER` with a readable block-letter HIDLINS wordmark using portable
   ASCII characters only.
3. Pin the complete `BANNER` constant in a focused unit test and prove clap emits
   it before the top-level help body for both `-h` and `--help`.
4. Require the rendered output to identify HIDLINS and explicitly reject the
   old `/_/    \\__,_/` fragment so a generic glyph test cannot regress again.
5. Keep all existing help, version, security-policy, and subcommand assertions.
6. Record fail-before/pass-after evidence. Do not regenerate completions because
   banner text is not part of completion output.

## Acceptance criteria

- [ ] `hidlins -h` and `hidlins --help` show an unambiguous HIDLINS ASCII
      wordmark before the help body.
- [ ] Neither help form contains the pinned Falach fragment.
- [ ] Tests compare the complete intended banner rather than checking generic
      punctuation.
- [ ] Existing CLI help and parsing contracts remain green.
- [ ] No external dependency is added.

## Validation

- `cargo test -p hidlins-cli --offline --locked --test cli_help`
- `cargo test -p hidlins-cli --offline --locked --lib cli::tests`
- `git diff --check`

## Dependencies

Tasks 001–015 (completed acceptance rounds 1–2)

## Expected areas of change

- `crates/hidlins-cli/src/cli.rs`
- `crates/hidlins-cli/tests/cli_help.rs`

## Risks / notes

- The wordmark must be readable as HIDLINS without relying on font-specific
  Unicode rendering; the user-supplied Unicode lock logo belongs to Task 017.

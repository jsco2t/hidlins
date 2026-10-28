# Task 002: Migrate every target reference

Delegation: main-only

## Goal

Move CI and all tracked mutable first-party references to the approved target names, prove retired names are absent, and preserve CI/developer workflow parity.

## Context

The baseline scan found exact current-target references in 70 tracked first-party files. Because Task 001 intentionally deletes old target names rather than retaining aliases, every invocation, instruction, comment, diagnostic, and verification record must use its canonical replacement.

## Scope

### In scope

- Update `.github/workflows/ci.yml` to invoke only approved public targets while preserving each job's existing work and platform guards.
- Update recursive/executable callers and user-facing diagnostics under `tools/`, configuration, scripts, and source/test files.
- Update root, application, crate, platform, benchmark, security, release, and verification documentation.
- Update mutable first-party generated files if they contain an exact retired target reference.
- Extend `tools/dev/test_makefile_targets.py` to scan tracked mutable first-party files for retired target tokens and validate CI target existence/public visibility.
- Record the scan's red result before migration and green result after migration.
- Review all replacements in context rather than performing unbounded substring replacement.

### Out of scope

- Editing third-party vendored Rust, Dart, or Cargokit sources.
- Editing `.agents/`, `.ai/workflow/`, or immutable `.ai/workflow-archive/` content.
- Redesigning GitHub Actions job topology or changing the commands executed by the replacement target recipes.
- Adding aliases for stale names.

## Implementation requirements

- Use the canonical retired-name-to-replacement mapping created in Task 001.
- Match complete target tokens so ordinary English and unrelated identifiers are not modified.
- Cover direct `make`, recursive `$(MAKE)`, shell strings, comments, error messages, Markdown/code blocks, JSON evidence text, and CI `run:` entries.
- Ensure every CI `make` invocation resolves to an existing public target and no CI job calls a private helper.
- Preserve target-specific variables such as `APP_TEST_FLAGS`, `HIDLINS_ANDROID_STRICT`, `DEMO_VAULT`, and `PROPTEST_CASES` at their callsites.
- Keep historical statements accurate by naming the current canonical workflow rather than leaving obsolete commands as purported evidence.
- The stale-reference test must use tracked-file enumeration and narrow documented exclusions so new stale references fail deterministically.

## Acceptance criteria

- [ ] A fail-before stale-reference result is recorded before callsite migration.
- [ ] GitHub Actions invokes only existing approved public targets and preserves all previous CI work.
- [ ] Every exact retired target reference in tracked mutable first-party content is replaced.
- [ ] No external caller invokes an underscore-prefixed private helper.
- [ ] Documentation examples and diagnostics use activity-first names and retain their original variables/options.
- [ ] The contract test's tracked-file scan and CI validation pass.
- [ ] No compatibility alias was added to work around a missed reference.
- [ ] The final diff contains no unintended edits to vendored or archived content.

## Validation

- `python3 -B tools/dev/test_makefile_targets.py`
- `make check-makefile`
- `git diff --check`
- `git status --short`

## Dependencies

- Task 001

## Expected areas of change

- `.github/workflows/ci.yml`
- `CLAUDE.md`
- `CONTRIBUTING.md`
- `README.md`
- `app/**` first-party documentation, configuration, source, and tests
- `crates/**` first-party manifests, documentation, source, and tests
- `docs/**`
- `projects/hidlins/**`
- `tools/**`
- `.cargo/audit.toml`
- `.gitignore`
- `rust-toolchain.toml`
- `tools/dev/test_makefile_targets.py`

## Risks / notes

The migration crosses CI, docs, scripts, and code diagnostics, so it remains main-only despite much of the editing being mechanical. The primary thread must inspect exact matches and distinguish live first-party references from excluded third-party or immutable history.

# Makefile Target Cleanup

## Objective

Replace the current flat, inconsistent 99-target public Makefile surface with a smaller activity-first hierarchy that is predictable to discover and use. Aggregate roots must run the complete applicable activity, focused targets must use `major-activity` → `portion` → optional `subportion` ordering, intentionally isolated workflows such as application launch must expose only leaf targets, and every tracked first-party caller or reference must move to the new name. Record the convention in a new root `AGENTS.md` and enforce it with dependency-free regression tests.

## Current behavior

- The top-level Makefile documents 99 public targets in one undifferentiated help list.
- Equivalent concepts use conflicting word orders: `app-build-*`, `build-android`, `app-run`, `*-check`, `api-gen*`, `pub-vendor*`, and `fuzz-local-sync-*`.
- Several public targets are implementation helpers rather than developer workflows, including harness checks, artifact checks, prechecks, and integrity-test helpers.
- Several unqualified roots are only partial operations: `build`, `test`, `check`, `interop`, `vendor`, and `bench` currently mean a Rust or single-subsystem slice even though neighboring targets expose other portions of the same activity.
- The help parser only recognizes a restricted target spelling and provides no grouping or machine-checked guarantee that every documented public target appears exactly once.
- Exact references to current target names occur in 70 tracked first-party files across CI, contributor and platform documentation, scripts, configuration comments, source comments, error messages, and test diagnostics. Vendored trees and immutable workflow archives are separate, non-editable scopes.
- There is no Makefile-surface regression test. Existing Python test helpers use the standard library `unittest` style and require no additional dependency.

## Proposed implementation

1. Add a dependency-free Makefile contract test under `tools/dev/` before changing target names. Record its failing result against the current target surface, then refactor the Makefile and record the passing result.
2. Reorganize public targets around major activities such as `setup`, `build`, `test`, `check`, `format`, `lint`, `generate`, `interop`, `benchmark`, `audit`, `vendor`, and `verify`. Put the activity first in every focused target.
3. Rename partial legacy roots to honest leaves before introducing an aggregate root. Examples include old `build` → `build-rust`, old default `test` → `test-rust-default`, old `check` → `check-rust`, old `interop` → `interop-vault`, old `vendor` → `vendor-rust`, and old `bench` → a `benchmark-*` leaf.
4. Apply consistent family rewrites: `app-build-*` → `build-app-*`, `app-test-*` → `test-app-*`, `test-local-sync-*` → `test-sync-*`, `app-run` → `run-app`, `api-gen*` → `generate-api` / `check-generated-api`, `app-fmt*` → `format-*`, `app-analyze` → `lint-app`, `fuzz-local-sync-*` → `fuzz-sync-*`, and suffix-style checks → `check-*` unless the suffix describes a subordinate action such as `format-check-*`.
5. Convert recipe-only nodes that are not supported developer or CI entry points into underscore-prefixed private helpers. Private helpers remain available to Makefile dependency graphs but do not appear in help and are not treated as compatibility aliases.
6. Add readable grouping to `make help`, retain concise descriptions, and ensure all supported target characters (including digits in future names) are recognized.
7. Create root `AGENTS.md` with the requested activity/portion/subportion rule, aggregate-root semantics, isolated-family rule, and the prohibition on retaining obsolete aliases.
8. Migrate every exact first-party reference, including recursive `$(MAKE)` calls, GitHub Actions, scripts, docs, configuration comments, source comments, test messages, and platform verification guides. Extend the contract test with the approved retired-name map so stale names cannot return.

## Architectural decisions

- A public target is a supported human or CI workflow. Recipe decomposition alone does not justify public visibility; implementation-only nodes are private and underscore-prefixed.
- Public focused names follow `major-activity-portion-subportion`. The major activity is always first. `run-app` is valid; `app-run` is not.
- When a public root exists, it is an aggregate. It depends on every deterministic portion of that activity that applies to the current host and preserves the existing workflow boundaries. Unsupported-host, physical-device, interactive/manual, explicitly destructive regeneration, and other inherently isolated operations are not silently folded into an aggregate.
- An activity with no coherent or safe aggregate exposes leaves only. `run-*` is the required example and there must be no `run` target. Any additional isolated family must be explicitly documented in `AGENTS.md` and enumerated in the contract test rather than inferred ad hoc.
- Aggregate targets compose prerequisites; recipe bodies remain at leaves so CI and developers execute identical commands without duplication.
- Removed names are deleted, not retained as aliases. A canonical retired-name-to-replacement registry in the contract test supports both migration verification and future regression protection.
- Existing recipes, security boundaries, offline flags, environment variables, platform guards, and produced artifacts remain behaviorally unchanged except where dependency composition must change to create truthful aggregates.
- The scan covers tracked, mutable, first-party content. It excludes `.git/`, third-party vendored Rust/Dart/Cargokit trees, `.agents/` skill implementation, `.ai/workflow/` planning artifacts, and immutable `.ai/workflow-archive/` history.
- No external dependency is needed; the new test uses Python's standard library and invokes read-only Make introspection/help commands.

## Work included

- Public/private target inventory and classification.
- Activity-first renaming and coherent aggregate construction.
- Reduction of the documented public target count below the 99-target baseline.
- Help grouping and complete public-target discoverability.
- Makefile contract tests with recorded fail-before/pass-after evidence.
- Root `AGENTS.md` target naming and aggregation rules.
- Migration of all first-party target invocations and prose/code references.
- CI/local parity preservation through renamed targets.
- Verification that retired names are absent and no compatibility aliases survive.

## Task sequence

1. [`tasks/001-refactor-makefile-target-contract.md`](tasks/001-refactor-makefile-target-contract.md) — add the red/green contract test, refactor the Makefile surface, and document the naming rule in root `AGENTS.md`.
2. [`tasks/002-migrate-target-references.md`](tasks/002-migrate-target-references.md) — migrate CI and every tracked first-party reference, then enforce the retired-name scan.

## Quality gate

Standard task completion runs:

- `make check-makefile` — the new focused structural/usage regression gate for the target hierarchy, help output, aggregates, CI references, and retired names.
- `make check-rust` — the renamed form of the repository's existing Rust-only formatting, lint, build, test, cross-target, and feature-gate check. This preserves the normal per-task engineering gate without requiring unrelated platform hardware.

Final package validation runs:

- `make check-makefile` — repeats the target-surface and repository-reference contract after the integrated review.
- `make verify` — the repository's complete existing Rust, Flutter, interoperability, supply-chain, documentation, and acceptance verification aggregate, updated to the new target graph.

There is no separate type checker outside the existing Rust/Dart checks, and no new generic command is invented. Platform-device and interactive observation targets remain non-gating because the repository already classifies those as isolated or optional rather than deterministic completion gates.

## Risks

- A missed caller can break CI or leave documentation instructing users to invoke a deleted target; the retired-name registry and tracked-file scan mitigate this.
- Over-aggregation can accidentally invoke an unsupported platform or hardware workflow. Aggregate membership will be explicit and contract-tested, with host/manual exceptions documented.
- Renaming partial roots can subtly change expected scope. Existing recipes will move intact to focused leaves, and new roots will contain dependency composition only.
- Recursive Make calls can silently retain old names even after public docs are fixed. They are included in the same mapping and tests.
- Generated or historical content may legitimately contain old text. Exclusions are narrow and limited to third-party vendored content, workflow machinery, and immutable archives; mutable first-party generated files remain in scope when they contain an exact target reference.
- A broad mechanical replacement can alter unrelated prose. Migration will use exact target-token matches and each changed source/error string will be reviewed in context.

## Out of scope

- Changing the underlying Rust, Flutter, mobile, interoperability, fuzzing, benchmark, or supply-chain behavior.
- Adding dependencies or changing dependency policy.
- Rewriting CI job topology beyond the target-name and aggregation adjustments needed for parity.
- Editing vendored dependency trees or archived work packages.
- Removing legitimate isolated workflows merely to reach an arbitrary target-count quota.
- Executing optional physical-device or human-observation procedures.

## Final acceptance criteria

- The documented public target count is strictly lower than the 99-target baseline, and every remaining public target is a supported developer/CI workflow rather than an implementation helper.
- Every focused public target follows `major-activity-portion-subportion`; activity words are never reversed.
- Every public root with portions is a tested aggregate of all applicable deterministic portions, with explicit tested exceptions for isolated/manual/hardware workflows.
- `run-app` and `run-tui` exist, `run` and `app-run` do not exist, and removed targets do not survive as aliases.
- `make help` groups the public surface, lists every public target exactly once, hides private helpers, and supports valid names containing digits.
- Root `AGENTS.md` states the requested naming pattern, aggregate semantics, isolated-family exception, and no-alias migration rule.
- The new standard-library contract test demonstrates a recorded fail-before/pass-after sequence and passes through `make check-makefile`.
- Every exact reference to a retired target in tracked mutable first-party files is replaced with its approved new form, including CI, recursive Make, scripts, documentation, comments, error messages, and test diagnostics.
- GitHub Actions continues to invoke only existing public targets, and CI slices preserve the recipes/gates they ran before the rename.
- `make check-makefile` and `make verify` pass.

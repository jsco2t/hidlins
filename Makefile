# Hidlins — Makefile
#
# This is the canonical build / test / lint interface for the project.
# Every developer-facing workflow has a target here so that local runs
# and CI run identical commands. Raw `cargo` invocations are reserved
# for ad-hoc exploration; anything that's part of the project's normal
# loop lives here.
#
# Run `make help` (or `make` alone) to list every target.
#
# All builds are offline against the vendored dependency tree. The
# project's `.cargo/config.toml` already enforces `[net] offline = true`;
# the explicit `--offline --locked` flags below are belt-and-suspenders
# so the intent is visible at the call site.

# Linux and macOS workflows use Bash.
SHELL          := bash

CARGO          := cargo
# Query Cargo lazily: bootstrap/help targets must work before Cargo is installed.
CARGO_ARTIFACT_DIR = $(or $(shell python3 tools/dev/cargo_artifacts.py target-dir -- $(CARGO)),$(error Cargo artifact directory could not be resolved))
CARGO_COMPILE = python3 tools/dev/cargo_artifacts.py run $(CARGO) --
CARGO_FLAGS    := --workspace --offline --locked
# Synthetic Cargo fixtures can clear this because they have no security crate.
CARGO_TEST_FLAGS ?= --features hidlins-security/test-binaries
CLIPPY_FLAGS   := --workspace --all-targets --offline --locked -- -D warnings
SPEC_POLICY_ROOT ?=

# Rustfmt does not traverse include! modules inside App's private test scope.
TUI_APP_REGRESSION_SOURCES := $(wildcard crates/hidlins-tui/src/app_*_regressions.rs)

# Every Rust compilation launched through make inherits fatal warnings. Use
# `override` so a caller-provided RUSTFLAGS/RUSTDOCFLAGS value is preserved and
# the repository policy is appended instead of being replaced by command-line
# variable precedence.
override RUSTFLAGS    := $(strip $(RUSTFLAGS) -D warnings)
override RUSTDOCFLAGS := $(strip $(RUSTDOCFLAGS) -D warnings)
export RUSTFLAGS RUSTDOCFLAGS

# Host OS — used to pick the platform-appropriate OS-event source test
# (logind on Linux, IOKit on macOS); the wrong-platform feature won't
# even resolve (`logind`→`zbus` is Linux-target-only; `iokit`→`objc2`
# is macOS-target-only), so `test-os-events` must not invoke both.
UNAME_S        := $(shell uname -s)
API_SHARED_LIBRARY := $(if $(filter Darwin,$(UNAME_S)),libhidlins_api.dylib,libhidlins_api.so)

# `make` with no args shows the help screen — friendlier for first-run.
# Devs who know what they want type `make build`, `make test`, `make check`.
.DEFAULT_GOAL := help

.PHONY: help toolchain fuzz-toolchain build release demo-vault \
        build-policy-check _build-policy-check run-tui drift-fixture-host-check drift-hash-check vendor-cache-check \
        vendor-cache-callers-check vendor-cache-precedence-check cargo-artifact-path-check check-macos test-api-lock-features test-api-lock-cache \
        build-api-native-test-harness test-api-lock-cache-native-ticket test-api-worker-panic-ticket test-api-inbound-secret-erasure test-api-server-lifecycle test-api-server-unlock \
        test-reserved-custom-fields test-api-create-registry-commit test-api-server-config-commit test-server-shutdown test-api-peer-revocation test-tui-peer-revocation \
        test-api-incoming-commit test-api-native-publication test-api-copy-origin test-api-copy-origin-native test-api-generated-copy test-api-generated-copy-native \
        test-api-pairing-lifetime test-api-pairing-lifetime-native test-api-discovery-kind test-api-discovery-cancel test-api-discovery-stop-controls test-api-expiration-save \
        check-feature-gates test test-cli-foreground-lock test-cli-fixture-paths test-sync-fixture-paths test-sync-listener-readiness \
        lock-offline test-ignored test-ignored-render-routing test-macos-manual-routing test-headless-stdin test-all \
        test-tui-contracts test-update-snapshots snapshots-check test-clipboard test-sync-preserved-contracts test-noise-erasure \
        test-local-sync-security test-local-sync-discovery test-local-sync-integration fuzz-local-sync-corpus fuzz-local-sync-ci test-merge-properties \
        test-native-helper-features test-logind-fixture test-dbus-maintenance test-os-events test-tui-formatting-host test-tui-formatting \
        test-tui-terminal test-tui-terminal-backend test-tui-os-events test-tui-readonly test-tui-password-memory test-tui-note-memory \
        test-tui-edit-validation test-tui-configuration test-tui-state test-tui-peers test-tui-render-memory test-tui-framework-memory \
        test-ratatui-memory-patches bench-tui-render test-tui-output bench-tui-output test-tui-detail-scroll test-tui-history-scroll \
        test-tui-edit-viewport test-tui-paragraph-position test-ratatui-geometry-patch bench-tui-edit test-tui-reserved-bindings fmt \
        fmt-check lint lint-fix check verify asset-license-check \
        interop interop-sync interop-entry bench bench-search bench-search-gate \
        bench-search-gate-ci deny audit doc completions test-nested-completions \
        completions-check vendor vendor-patches ncsa-boundary-precheck ncsa-boundary-check s3-removal-check \
        clean test-console test-api test-api-native test-console-features test-console-process-interop \
        test-dev-tools ci-toolchain-check boundary-check


help:  ## Show this help.
	@awk 'BEGIN {FS = ":.*##"; print "Usage: make <target>\n\nTargets:"} /^[a-zA-Z_-]+:.*##/ {printf "  %-15s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# ---------------------------------------------------------------------------
# One-time environment setup — run this first on a fresh machine.
# ---------------------------------------------------------------------------

toolchain:  ## Install all dev tooling (Rust toolchain, keepassxc-cli, cargo-deny/audit). Run first on a new machine.
	# Cross-platform bootstrap (Linux + macOS). Idempotent — re-running
	# skips already-installed tools. Tool versions are pinned to match CI
	# (.github/workflows/ci.yml). After this, `make check` works offline.
	# Invoked directly (not via `sh`) so its bash shebang is honoured —
	# the script uses `set -o pipefail`, which POSIX `sh`/dash lacks.
	tools/dev/install-toolchain.sh

fuzz-toolchain:  ## Install exact cargo-fuzz and pinned nightly (development/testing only).
	tools/dev/install-fuzz-toolchain.sh

# ---------------------------------------------------------------------------
# Core developer loop — these match CI exactly.
# ---------------------------------------------------------------------------

build:  ## Build the workspace (offline, vendored).
	$(CARGO_COMPILE) build $(CARGO_FLAGS)

release: ncsa-boundary-precheck license-check  ## Build optimized console and agent artifacts in target/release/ (host platform).
	$(CARGO_COMPILE) build $(CARGO_FLAGS) --release
	python3 tools/dev/ncsa-boundary-check.py artifacts "$(CARGO_ARTIFACT_DIR)/release/hidlins" "$(CARGO_ARTIFACT_DIR)/release/hidlins-agent" "$(CARGO_ARTIFACT_DIR)/release/$(API_SHARED_LIBRARY)" "$(CARGO_ARTIFACT_DIR)/release/libhidlins_api.a"

demo-vault:  ## Create a disposable kitchen-sink KDBX vault at DEMO_VAULT (absolute path; refuses overwrite).
	@output="$(DEMO_VAULT)"; \
	if [ -z "$$output" ]; then \
		echo "error: set DEMO_VAULT=/absolute/path/to/demo.kdbx" >&2; exit 2; \
	fi; \
	case "$$output" in \
		/*) ;; \
		*) echo "error: DEMO_VAULT must be an absolute path" >&2; exit 2 ;; \
	esac; \
	if [ -e "$$output" ]; then \
		echo "error: refusing to overwrite existing path: $$output" >&2; exit 2; \
	fi; \
	mkdir -p "$$(dirname "$$output")"
	$(CARGO_COMPILE) run -p hidlins-core --example seed_demo_vault --offline --locked -- "$(DEMO_VAULT)"
	@echo "Demo vault password: Password123"

build-policy-check:  ## Prove first-party warnings are fatal and platform build policies stay scoped.
	$(MAKE) --no-print-directory _build-policy-check RUSTFLAGS="$(RUSTFLAGS) -C debuginfo=0"

_build-policy-check:
	tools/dev/build-policy-check.sh

run-tui:  ## Launch the interactive terminal UI (hidlins).
	$(CARGO_COMPILE) build -p hidlins-cli --offline --locked
	"$(CARGO_ARTIFACT_DIR)/debug/hidlins"

drift-fixture-host-check:  ## Verify isolated drift fixtures reach real integrity comparisons through native Make.
	$(MAKE) --no-print-directory -f tools/dev/drift-fixture-host-tests.mk test

drift-hash-check: drift-fixture-host-check  ## Verify portable drift hashing rejects changed or unreadable output.
	$(MAKE) -f tools/dev/drift-hash-tests.mk test

vendor-cache-check: vendor-cache-callers-check vendor-cache-precedence-check  ## Verify vendored-byte invalidation, warm retries and failure safety (Rust only).
	python3 -B tools/dev/test_vendor_cache.py \
	  CacheTests.test_make_real_library_macro_and_host_build_dependency \
	  CacheTests.test_raw_checksum_tamper_cannot_hide_behind_warm_artifact \
	  CacheTests.test_vendor_symlink_read_failure_stops_before_compile \
	  CacheTests.test_cleanup_failure_keeps_old_generation_and_retries \
	  CacheTests.test_failed_first_party_compile_preserves_warm_dependency_retry \
	  CacheTests.test_stamp_publication_and_partial_cleanup_fail_closed \
	  CacheTests.test_fuzz_runner_forwards_guarded_explicit_target_to_real_cargo

vendor-cache-callers-check:  ## Verify snapshot/benchmark cache adoption and clean recovery.
	$(MAKE) -f tools/dev/vendor-cache-caller-tests.mk test

vendor-cache-precedence-check:  ## Verify explicit Cargo output flags override inherited cache settings.
	$(MAKE) -f tools/dev/vendor-cache-precedence-tests.mk test

cargo-artifact-path-check:  ## Verify configured Cargo roots reach build artifact consumers.
	$(MAKE) -f tools/dev/artifact-path-tests.mk test

check-macos:  ## Type-check hidlins-security against macOS targets (Level-1 cross-compile gate; no link, no SDK).
	# `cargo check` does macro expansion + type/borrow checking but
	# does NOT invoke the linker, so the absence of Apple frameworks
	# (IOKit, AppKit, Foundation) on Linux does not block this gate.
	# Catches the realistic refactor-breakage class for Phase 5 T5.2's
	# `os_events/macos.rs`: trait-impl drift, signature changes,
	# `objc2` macro errors, missing-arm matches. Does NOT catch
	# wrong-extern-symbol-name or behavioural bugs — for those you
	# need a real macOS host (see CONTRIBUTING.md "macOS verification").
	#
	# `--features iokit` is passed so `os_events/macos.rs` actually
	# compiles under this gate (default features `#[cfg]` it out, which
	# would make the "catches macos.rs drift" claim above hollow). The
	# `iokit` feature only pulls the `objc2` ecosystem, all macOS-target
	# deps already in the vendored tree, so it resolves `--offline`.
	# `--all-features` is intentionally NOT used: it would also flip on
	# `logind`, whose `zbus` dep is Linux-target-only and would fail to
	# resolve for an apple-darwin target.
	$(CARGO_COMPILE) check --target aarch64-apple-darwin --offline --locked -p hidlins-security --features iokit
	$(CARGO_COMPILE) check --target x86_64-apple-darwin  --offline --locked -p hidlins-security --features iokit

test-api-lock-features:  ## Type-check API OS-lock features on Linux and both supported Apple architectures.
	python3 tools/dev/vendor_cache.py run -- python3 tools/dev/test_api_lock_features.py

test-api-lock-cache:  ## Verify API lock, worker ownership and TOTP cache revocation interleavings.
	$(CARGO_COMPILE) test -p hidlins-api --lib --offline --locked lock_cache_regressions

# A self dev-dependency unifies desktop features in `cargo test`, even with
# --no-default-features. Compile the real unit harness without dev-dependency
# activation, then require its native-only case before running it.
API_NATIVE_UNIT_TEST_BIN = $(CARGO_ARTIFACT_DIR)/debug/deps/hidlins_api
build-api-native-test-harness:  ## Build the API unit harness without development-feature unification.
	$(CARGO_COMPILE) rustc -p hidlins-api --lib --no-default-features --features test-fixtures --offline --locked -- --test

test-api-lock-cache-native-ticket: build-api-native-test-harness  ## Run actual native-ticket lock/cache cases without desktop feature unification.
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list lock_cache_regressions | grep -Fq 'pending_idle_lock_revokes_native_clipboard_ticket_before_handback: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" lock_cache_regressions

test-api-worker-panic-ticket: build-api-native-test-harness  ## Verify native clipboard revocation before a worker-panic Locked event.
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list worker_panic_ticket_regressions | grep -Fq 'caught_panic_revokes_ticket_before_locked_event: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" worker_panic_ticket_regressions

test-api-inbound-secret-erasure:  ## Observe API master/keyfile erasure on validation and registration paths.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --test inbound_secret_erasure_regressions --offline --locked

test-api-server-lifecycle:  ## Verify API server authority and controls across host ownership loans.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked server_lifecycle_regressions

test-api-server-unlock:  ## Verify a host ownership loan excludes competing vault unlocks.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked server_unlock_regressions

test-reserved-custom-fields:  ## Verify standard-field protection across core and API custom-field edits.
	$(MAKE) --no-print-directory -k -f tools/dev/reserved-fields-tests.mk test

test-api-create-registry-commit:  ## Verify vault creation preserves registry rename ownership on errors.
	$(MAKE) --no-print-directory -f tools/dev/create-registry-tests.mk test

test-api-server-config-commit:  ## Verify server authority follows registry rename commitment on errors.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked server_config_commit_regressions

test-server-shutdown:  ## Verify queued and active sync operations respect host shutdown.
	$(CARGO_COMPILE) test -p hidlins-sync --lib --offline --locked shutdown_regressions

test-api-peer-revocation:  ## Verify API peer revocation reconciles committed configuration and runtime trust.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked revoke_reconciliation_regressions

test-tui-peer-revocation:  ## Verify TUI peer revocation reconciles committed configuration and runtime trust.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked revoke_reconciliation_regressions

test-api-incoming-commit:  ## Verify incoming commits revoke stale secrets and notify providers at the rename boundary.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked incoming_commit_regressions

test-api-native-publication:  ## Verify native publication fences, owner independence and consumed-buffer erasure.
	$(MAKE) --no-print-directory -f Makefile -f tools/dev/native-publication-tests.mk test-native-publication

test-api-copy-origin:  ## Verify desktop clipboard publication origin and failed-copy erasure.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked copy_origin_regressions

test-api-copy-origin-native: build-api-native-test-harness  ## Verify actual native clipboard origin and single-use tickets.
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list copy_origin_regressions | grep -Fq 'native_current_ticket_is_single_use: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" copy_origin_regressions

test-api-generated-copy:  ## Verify exact generated-result copying, revocation and erasure on desktop.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked generated_copy_regressions

test-api-generated-copy-native: build-api-native-test-harness  ## Verify generated-result native tickets without desktop feature unification.
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list generated_copy_regressions | grep -Fq 'native_generated_ticket_is_single_use: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" generated_copy_regressions

test-api-pairing-lifetime:  ## Verify pairing/import cancellation, origin and commit lifetimes.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked pairing_lifetime_regressions

test-api-pairing-lifetime-native: build-api-native-test-harness  ## Verify pairing/import lifetimes without desktop feature unification.
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list pairing_lifetime_regressions | grep -Fq 'native_locked_pair_and_import_control: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" pairing_lifetime_regressions

test-api-discovery-kind:  ## Verify typed discovery routes, manual priority and legacy Trusted wrappers.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked discovery_kind_regressions

test-api-discovery-cancel:  ## Verify discovery cannot delay session revocation or publish after shutdown.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked discovery_cancel_regressions

test-api-discovery-stop-controls:  ## Verify discovery stop, queued polls, restart and cleanup ownership.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked discovery_stop_controls

test-api-expiration-save:  ## Verify entry fields and expiration share one validated atomic save.
	$(CARGO_COMPILE) test -p hidlins-api --test expiration_save_regressions --offline --locked

check-feature-gates: test-console-features test-api-lock-features test-api-lock-cache-native-ticket test-api-worker-panic-ticket test-api-copy-origin-native test-api-pairing-lifetime-native  ## Type-check gated suites and verify native API lock/cache behavior.
	# Compile-only drift gate. The display-dependent clipboard suites
	# (`clipboard-tests`) and the OS-event helper binaries are excluded
	# from every runtime CI sweep by design — but without this gate they
	# are never even *compiled* in CI, so an API refactor of
	# `hidlins_security::Clipboard` or the CLI's `--copy` path could
	# silently break them. `cargo check --tests` compiles the gated test
	# targets without running them (no display needed).
	$(CARGO_COMPILE) check -p hidlins-security --offline --locked --features clipboard-tests --tests
	$(CARGO_COMPILE) check -p hidlins-cli --offline --locked --features clipboard-tests --tests
	$(CARGO_COMPILE) check -p hidlins-security --offline --locked --no-default-features
	$(CARGO_COMPILE) check -p hidlins-api --offline --locked --features test-fixtures --tests
	# The desktop mDNS adapter must compile on a desktop host, while the
	# target-specific dependency must remain absent from both mobile graphs.
	$(CARGO_COMPILE) check -p hidlins-sync --offline --locked --features desktop-discovery
	@android_mdns="$$( $(CARGO) tree -p hidlins-sync --offline --locked --features desktop-discovery --target aarch64-linux-android -i mdns-sd 2>/dev/null )"; \
		test -z "$$android_mdns" || { echo "error: mdns-sd entered Android graph" >&2; exit 1; }
	@ios_mdns="$$( $(CARGO) tree -p hidlins-sync --offline --locked --features desktop-discovery --target aarch64-apple-ios -i mdns-sd 2>/dev/null )"; \
		test -z "$$ios_mdns" || { echo "error: mdns-sd entered iOS graph" >&2; exit 1; }
ifeq ($(UNAME_S),Darwin)
	# iokit compiles natively here; logind's zbus tree is Linux-only.
	$(CARGO_COMPILE) check -p hidlins-security --offline --locked --features test-binaries,iokit --tests
else
	$(CARGO_COMPILE) check -p hidlins-security --offline --locked --features test-binaries,logind --tests
endif

# Headless tests need a real nonterminal pipe, not host-specific /dev/null
# terminal errors (ENODEV on macOS). Keep prompt error handling fail-closed.
test:  ## Run default-parallel tests with empty pipe stdin (offline, vendored).
	# Acceptance wrappers spawn OS-event helpers even in the non-ignored sweep.
	# Build them here so a clean checkout needs no earlier test-os-events run.
	printf '' | $(CARGO_COMPILE) test $(CARGO_TEST_FLAGS) $(CARGO_FLAGS)

test-cli-foreground-lock:  ## Verify foreground CLI lock/sleep shutdown with owned processes/private D-Bus.
	$(CARGO_COMPILE) test -p hidlins-cli --test cli_foreground_lock_regressions --test cli_foreground_bus_transport_regressions --offline --locked

test-cli-fixture-paths:  ## Verify shared CLI fixture paths are absolute and physical on Mac and Linux.
	$(MAKE) --no-print-directory -f tools/dev/cli-fixture-path-tests.mk test

test-sync-fixture-paths:  ## Verify shared sync fixture paths and cloned ciphertext on Mac and Linux.
	$(MAKE) --no-print-directory -f tools/dev/sync-fixture-path-tests.mk test

test-sync-listener-readiness:  ## Repeat real listener recovery and queued-connection regressions with bounded readiness.
	$(MAKE) --no-print-directory -f tools/dev/listener-readiness-tests.mk test

lock-offline:  ## Resolve reviewed manifest edges into Cargo.lock using only vendored sources.
	$(CARGO) metadata --offline --format-version=1 > /dev/null

# These post-MVP observations require a human to trigger real sleep/lock.
# Keep them in test-os-events, outside the unattended aggregate on macOS.
ifeq ($(UNAME_S),Darwin)
TEST_IGNORED_MANUAL_FLAGS := --skip sleep_transition_locks_helper --skip screen_lock_locks_helper
endif

test-ignored:  ## Run automated #[ignore]d tests serially (env-mutating + signal-handler tests, etc).
	# `--features test-binaries` enables hidlins-security's `sigstop_helper`
	# test binary, needed by `tests/us_052_sigstop_lock.rs`. Other workspace
	# members don't define the feature; cargo silently no-ops on them.
	# Display-dependent clipboard cases remain feature-gated and run only
	# through their dedicated target.
	# Build vault_holder (hidlins-core test binary) — needed by
	# hidlins-api's contended_vault_surfaces_holder_pid test.
	$(CARGO_COMPILE) build -p hidlins-core $(CARGO_FLAGS) --bin vault_holder
	# The render benchmark needs cursor-query replies from its owned terminal.
	# Run that same case through its serial release target after ordinary tests.
	printf '' | $(CARGO_COMPILE) test $(CARGO_FLAGS) --features test-binaries -- --ignored --test-threads=1 --skip framework_memory_regressions::render_workload_performance_baseline $(TEST_IGNORED_MANUAL_FLAGS)
	$(MAKE) bench-tui-render

test-ignored-render-routing:  ## Verify headless ignored tests retain the owned-terminal render benchmark.
	$(MAKE) -f tools/dev/ignored-render-tests.mk test

test-macos-manual-routing:  ## Verify unattended macOS gates retain automated cases and isolate human sleep/lock observations.
	$(MAKE) --no-print-directory -f tools/dev/macos-manual-routing-tests.mk test

test-headless-stdin:  ## Verify headless Rust tests receive an empty pipe and preserve flags, failures and terminal routing.
	$(MAKE) --no-print-directory -f tools/dev/headless-stdin-tests.mk test

test-all: test test-ignored  ## Run both default and #[ignore]d tests.

test-tui-contracts:  ## Run in-process TUI journeys and accessibility semantic contracts.
	$(CARGO_COMPILE) test -p hidlins-tui --offline --locked --lib journey_tests
	$(CARGO_COMPILE) test -p hidlins-tui --offline --locked --lib accessibility_contract_tests

test-update-snapshots:  ## Regenerate the hidlins-tui snapshot golden files (tui-skeleton T3.5).
	# Re-renders the Secrets-tab tree/detail snapshots and rewrites the golden
	# files under crates/hidlins-tui/tests/snapshots/. This is the only target
	# that mutates them — review the diff before committing. Normal `make test`
	# compares against the committed goldens and never rewrites. Delegates to the
	# same script as `snapshots-check` so the regenerate and dry-run paths share
	# one implementation.
	$(CURDIR)/tools/dev/update-snapshots.sh

snapshots-check:  ## Check for missing or stale snapshot golden files (dry-run).
	# Reports which golden files are missing or would change without modifying
	# anything. Exit code 1 means issues found; 0 means all goldens are up to date.
	$(CURDIR)/tools/dev/update-snapshots.sh --check

test-clipboard:  ## Run clipboard tests across hidlins-security + hidlins-cli (requires real display; wrap in xvfb-run on CI).
	# Spans two crates: the security crate's US-053 auto-clear cases and
	# the CLI's `entry get --copy` hand-off. Both files are gated behind
	# each crate's `clipboard-tests` feature so they stay out of the
	# blanket `make test-ignored` sweep; this target is the only one that
	# enables the feature. They drive the real system clipboard, so they
	# need a usable display/pasteboard (xvfb on Linux CI).
	$(CARGO_COMPILE) test -p hidlins-security --offline --locked --features clipboard-tests --test us_053_clipboard_autoclear -- --ignored --test-threads=1
	$(CARGO_COMPILE) test -p hidlins-cli --offline --locked --features clipboard-tests --test cli_clipboard_handoff -- --ignored --test-threads=1

test-sync-preserved-contracts:  ## Pin retained sync CAS, KDF, pointer and registry contracts cited by docs.
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --test preserved_contract_characterization --test us_041_fast_replace_open

test-noise-erasure: vendor-patches  ## Verify actual owned SHA state and scratch erasure used by Noise.
	python3 tools/noise-erasure-tests/run.py

test-local-sync-security: vendor-patches test-noise-erasure  ## Run local-sync protocol, owned-memory and cryptographic security tests.
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --test noise_security
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --lib noise::tests
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --test local_protocol
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --test identity_pairing_trust
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --test fault_injection
	$(CARGO_COMPILE) test -p hidlins-cli --offline --locked --test cli_sync

test-local-sync-discovery: vendor-patches  ## Run deterministic discovery policy/simulation tests plus the desktop adapter gate.
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --features desktop-discovery --test secure_discovery
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --features desktop-discovery --lib discovery

test-local-sync-integration: vendor-patches  ## Run real-loopback authoritative local-sync server integration tests.
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --test server_integration
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --test client_policy
	$(CARGO_COMPILE) test -p hidlins-cli --offline --locked --test cli_local_sync_process -- --test-threads=1
	$(CARGO_COMPILE) test -p hidlins-api --offline --locked --test us_094_sync_api -- --test-threads=1
	tools/local-sync-tests/isolated-network.sh

fuzz-local-sync-corpus: vendor-patches  ## Replay committed local-sync fuzz corpora using stable Rust offline.
	$(CARGO_COMPILE) test --manifest-path fuzz/Cargo.toml --offline --locked --no-default-features

FUZZ_RUNS ?= 512
fuzz-local-sync-ci: vendor-patches  ## Run bounded local-sync libFuzzer campaigns (requires make fuzz-toolchain).
	FUZZ_RUNS=$(FUZZ_RUNS) python3 tools/local-sync-tests/run-fuzz.py

# Heavier merge-engine property sweep. The properties run at 256 cases under
# the blanket `make test`; this target raises the count for a deeper sweep
# (PRD §11 Risk #2 / impl-plan §8). Override the iteration count via
# PROPTEST_CASES (e.g. `make test-merge-properties PROPTEST_CASES=10000`).
PROPTEST_CASES ?= 4096
test-merge-properties:  ## Run the merge-engine property tests with a heavier case count (PROPTEST_CASES, default 4096).
	PROPTEST_CASES=$(PROPTEST_CASES) $(CARGO_COMPILE) test -p hidlins-sync --offline --locked --test merge_property_tests

# ---------------------------------------------------------------------------
test-native-helper-features:  ## Verify native helper feature unification and unsupported-host rejection.
	$(MAKE) --no-print-directory -f tools/dev/native-helper-tests.mk test

test-logind-fixture:  ## Verify prepared and standalone logind regression entry modes on Linux.
	$(MAKE) --no-print-directory -f tools/dev/logind-fixture-tests.mk test

test-dbus-maintenance:  ## Verify D-Bus codec/patch replay, OS-event consumers and atomic filesystem contracts.
	$(MAKE) --no-print-directory -f Makefile -f tools/dev/dbus-maintenance-tests.mk test-dbus-characterization

test-os-events:  ## Run hidlins-security OS-event integration tests (logind on Linux, IOKit on macOS).
	# Picks the host-appropriate source test. Both files are
	# `#[ignore]`d so the default `make test` skips them.
	#
	# Linux: `--features test-binaries,logind` enables the
	# `logind_helper` binary and compiles `os_events/logind.rs` + its
	# `zbus` dep tree. A private dbus-daemon hosts a fixture that owns
	# login1 and emits from its owning connection; no host logind or root.
	#
	# macOS: `--features test-binaries,iokit` enables the `iokit_helper`
	# binary and compiles `os_events/macos.rs` + its `objc2` dep tree.
	# The cases are manual (a human triggers sleep / screen lock); they
	# print `INSTRUCTION:` lines and wait. GitHub-hosted macOS runners
	# have no interactive session, so CI skips them.
	#
	# `--test <name>` selects only the post-MVP file so neither the
	# clipboard nor SIGTSTP integration tests run here.
ifeq ($(UNAME_S),Darwin)
	# First the automated `os_events::macos` lib unit tests (incl. the
	# T5.0 clean-shutdown contract test, which registers real observers
	# and pumps a real CFRunLoop) — they pass without interaction. Then
	# the manual integration cases that need a human to trigger sleep /
	# screen lock.
	$(CARGO_COMPILE) test -p hidlins-security --offline --locked --features iokit --lib os_events::macos -- --ignored --test-threads=1
	$(CARGO_COMPILE) test -p hidlins-security --offline --locked --features test-binaries,iokit --test us_052_post_mvp_iokit -- --ignored --test-threads=1
else
	$(CARGO_COMPILE) test -p hidlins-security --offline --locked --features logind --test logind_startup_shutdown_regressions
	$(CARGO_COMPILE) test -p hidlins-security --offline --locked --features test-binaries,logind --test logind_integration_sender_regressions
	$(CARGO_COMPILE) test -p hidlins-security --offline --locked --features test-binaries,logind --test us_052_post_mvp_logind -- --ignored --test-threads=1
endif

test-tui-formatting-host:  ## Verify native fixture invocation and real rustfmt rejection without Linux-specific paths.
	$(MAKE) --no-print-directory -f tools/dev/tui-format-fixture-host-tests.mk test

test-tui-formatting: test-tui-formatting-host  ## Verify formatting of TUI include! test modules with isolated negative controls.
	python3 tools/test_tui_formatting.py

test-tui-terminal:  ## Verify real TUI signal/quit cleanup on owned controlling terminals.
	$(CARGO_COMPILE) test -p hidlins-cli --test terminal_signal_regressions --offline --locked

test-tui-terminal-backend:  ## Verify actual macOS controlling-TTY and Linux terminal feature boundaries.
	python3 tools/dev/test_tui_terminal_backend.py -- $(CARGO)

test-tui-os-events:  ## Verify TUI native lock/sleep lifetime with owned processes and a private bus.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked native_lock_regressions

test-tui-readonly:  ## Verify read-only TUI shared locking, writer exclusion and query behavior.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked readonly_lock_regressions

test-tui-password-memory:  ## Verify password input erases allocations before growth and release.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked password_memory_regressions

test-tui-note-memory:  ## Verify secure-note editor, view and history allocations erase before release.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked note_memory_regressions

test-tui-edit-validation:  ## Verify Edit rejects field collisions, failed generation and implicit disclosure.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked edit_validation_regressions

test-tui-configuration:  ## Verify effective keyboard dispatch, counted jumps and content-safe warnings.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked configuration_regressions

test-tui-state:  ## Verify UI-state merge-on-write, advisory locking and error preservation.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked ui_state_regressions

test-tui-peers:  ## Verify selected peer visibility and matching persisted revocation.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked peers_visibility_regressions

test-tui-render-memory:  ## Verify first-party secret render allocations erase before release.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked render_memory_regressions

test-tui-framework-memory:  ## Verify Ratatui symbol and screen-buffer erasure across allocation lifetimes.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked framework_memory_regressions -- --test-threads=1

test-ratatui-memory-patches:  ## Verify maintained Ratatui patch guards, checksums and idempotence.
	python3 tools/dev/test_ratatui_memory_patches.py

bench-tui-render:  ## Measure serial release redraw/unchanged-frame work against the existing 20-FPS budget.
	python3 tools/dev/test_ratatui_memory_patches.py --render-benchmark $(CARGO_COMPILE) test -p hidlins-tui --lib --release --offline --locked framework_memory_regressions::render_workload_performance_baseline -- --exact --ignored --nocapture --test-threads=1

test-tui-output:  ## Verify owned output erasure, batching, partial writes, errors and panic cleanup.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked terminal_output -- --test-threads=1

bench-tui-output:  ## Measure real redirected stdout rendering without recording synthetic payloads.
	python3 -c 'import subprocess,sys; subprocess.run(sys.argv[1:], stdout=subprocess.DEVNULL, check=True)' $(CARGO_COMPILE) test -p hidlins-tui --lib --release --offline --locked terminal_output::tests::render_ -- --ignored --nocapture --test-threads=1

test-tui-detail-scroll:  ## Verify wrapped detail reachability and resize bounds on Secrets and pinned tabs.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked detail_scrolling_regressions

test-tui-history-scroll:  ## Verify newest-first history, detail paging, resize and unchanged version navigation.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked history_scrolling_regressions

test-tui-edit-viewport:  ## Verify editor focus, exact wrapped caret, errors, masking and resize.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked edit_viewport_regressions

test-tui-paragraph-position:  ## Compare exact span positions with real Paragraph rendering.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked paragraph_position_regressions

test-ratatui-geometry-patch:  ## Verify exact maintained renderer-query replay and drift rejection.
	python3 tools/dev/test_ratatui_geometry_patch.py

bench-tui-edit:  ## Measure complete editor focus/caret/error rendering against the existing frame budget.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --release --offline --locked paragraph_position_regressions::editor_render_budget -- --exact --ignored --nocapture --test-threads=1

test-tui-reserved-bindings:  ## Verify unreachable key overrides retain working defaults and editor controls.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked reserved_binding_regressions

fmt:  ## Auto-format the workspace.
	$(CARGO) fmt --all
	rustfmt --edition 2021 tools/noise-erasure-tests/probe.rs tools/noise-erasure-tests/observer.rs $(TUI_APP_REGRESSION_SOURCES)

fmt-check:  ## Verify formatting without modifying files (CI gate).
	$(CARGO) fmt --all --check
	rustfmt --edition 2021 --check tools/noise-erasure-tests/probe.rs tools/noise-erasure-tests/observer.rs $(TUI_APP_REGRESSION_SOURCES)

lint:  ## Run clippy with `-D warnings` (CI gate).
	$(CARGO_COMPILE) clippy $(CLIPPY_FLAGS)

lint-fix:  ## Apply clippy auto-fixes where safe.
	$(CARGO_COMPILE) clippy --workspace --all-targets --offline --locked --fix --allow-dirty

# ---------------------------------------------------------------------------
# Full local CI gate — run before pushing.
# ---------------------------------------------------------------------------

check: test-headless-stdin build-policy-check fmt-check test-tui-formatting lint build test check-macos check-feature-gates  ## Warning policy, formatting, lint, build, tests and feature checks.

verify: release test-dev-tools test-tui-terminal-backend test-macos-manual-routing check license-check test-ignored doc deny audit interop interop-entry interop-sync test-local-sync-security test-local-sync-discovery test-local-sync-integration fuzz-local-sync-corpus s3-removal-check boundary-check test-console-process-interop  ## Full Rust, tooling, license, interop and sync verification.

asset-license-check:  ## Check the exact EFF asset exception, notices and unchanged code-license policy.
	python3 -m unittest discover -s tools/dev -p 'test_asset_licenses.py'
	python3 tools/dev/asset_license_check.py --spec-root "$(SPEC_POLICY_ROOT)"

interop:  ## Run vault-core KeePassXC interop shell tests (requires keepassxc-cli).
	$(CARGO_COMPILE) build -p hidlins-core --bin hidlins-test-driver --offline --locked
	HIDLINS_TEST_DRIVER="$(CARGO_ARTIFACT_DIR)/debug/hidlins-test-driver" sh tools/interop-tests/us_090_rust_to_kpxc.sh
	HIDLINS_TEST_DRIVER="$(CARGO_ARTIFACT_DIR)/debug/hidlins-test-driver" sh tools/interop-tests/us_091_kpxc_to_rust.sh
	HIDLINS_TEST_DRIVER="$(CARGO_ARTIFACT_DIR)/debug/hidlins-test-driver" sh tools/interop-tests/us_092_round_trip.sh

interop-sync:  ## Run the transport-neutral KeePassXC merge-interop test (US-044; requires keepassxc-cli).
	# Builds the test-only merge driver (gated behind `test-helpers` via
	# required-features, so `make build` never compiles it), then runs the
	# shell harness that opens the merged vault in keepassxc-cli and asserts
	# the collision loser survives as a history entry (NFR-009 / FR-043).
	$(CARGO_COMPILE) build -p hidlins-sync --bin merge-interop-driver --features test-helpers --offline --locked
	HIDLINS_SYNC_MERGE_DRIVER="$(CARGO_ARTIFACT_DIR)/debug/merge-interop-driver" sh tools/interop-tests/sync_us-044.sh

interop-entry:  ## Run entry-management KeePassXC interop shell tests (requires keepassxc-cli; oathtool optional).
	$(CARGO_COMPILE) build -p hidlins-core --bin hidlins-test-driver --offline --locked
	@for script in tools/interop-tests/entry_us-010.sh \
	               tools/interop-tests/entry_us-012.sh \
	               tools/interop-tests/entry_us-013.sh \
	               tools/interop-tests/entry_us-014.sh \
	               tools/interop-tests/entry_us-016.sh \
	               tools/interop-tests/entry_us-018.sh; do \
		echo "==> $$script"; \
		HIDLINS_TEST_DRIVER="$(CARGO_ARTIFACT_DIR)/debug/hidlins-test-driver" "$$script" || exit $$?; \
	done

bench:  ## Run informational benchmarks.
	$(CARGO_COMPILE) bench -p hidlins-core --bench vault_open --offline --locked

bench-search:  ## Run the informational entry-search benchmark.
	$(CARGO_COMPILE) bench -p hidlins-core --bench bench_search --offline --locked

bench-search-gate:  ## Local NFR-002 gate: p99 must stay within 50ms (BUDGET_MS overridable).
	BENCH_STAT=p99 tools/bench/bench_search_gate.sh

bench-search-gate-ci:  ## Hosted-CI search gate: median within 75ms; report p99 against the 50ms NFR.
	BENCH_STAT=median BUDGET_MS="$${BUDGET_MS:-75}" tools/bench/bench_search_gate.sh

# ---------------------------------------------------------------------------
# Supply-chain and docs.
# ---------------------------------------------------------------------------

deny:  ## Run cargo-deny license + advisory + ban checks (requires cargo-deny installed).
	$(CARGO) deny check

audit:  ## Run cargo-audit against RustSec advisories (requires cargo-audit installed).
	$(CARGO) audit

doc:  ## Generate API docs locally.
	$(CARGO_COMPILE) doc --no-deps --offline

# ---------------------------------------------------------------------------
# Shell completions (FR-064) — re-generated by `make completions`, checked
# into shell-completions/ for packaging + the CI drift gate.
# ---------------------------------------------------------------------------

# Cargo integration tests supply their exact CARGO_BIN_EXE_hidlins. Execute
# that binary without a nested build; normal invocations keep guarded Cargo.
# Export the path as data, without interpolating it into shell source.
export HIDLINS_COMPLETIONS_BIN
completions:  ## Re-generate shell-completions/hidlins.{bash,fish} + _hidlins.
	@if test -n "$$HIDLINS_COMPLETIONS_BIN"; then \
		python3 tools/dev/generate_completions.py; \
	else \
		$(CARGO_COMPILE) run --example gen_completions -p hidlins-cli --offline --locked; \
	fi

test-nested-completions:  ## Verify nested completion generation and guarded vendor-byte invalidation.
	$(MAKE) --no-print-directory -f tools/dev/nested-completions-tests.mk test

completions-check:  ## Re-generate completions and fail the build if shell-completions/ drifted (CI gate).
	@before="$$(python3 tools/dev/content_fingerprint.py tree shell-completions)" || exit 1; \
	$(MAKE) --no-print-directory completions || { \
		echo "error: completion generation failed — the hash comparison below would be vacuous" >&2; exit 1; }; \
	test -s shell-completions/hidlins.bash && \
	test -s shell-completions/_hidlins && \
	test -s shell-completions/hidlins.fish || { \
		echo "error: completion generation did not produce every expected Hidlins completion file" >&2; exit 1; }; \
	after="$$(python3 tools/dev/content_fingerprint.py tree shell-completions)" || exit 1; \
	if [ "$$before" != "$$after" ]; then \
		echo "error: shell-completions/ drifted. Run \`make completions\` and commit the result." >&2; \
		git --no-pager diff -- shell-completions/ >&2; \
		exit 1; \
	fi; \
	echo "  OK: shell completions match the generated Hidlins command surface"

# ---------------------------------------------------------------------------
# Dependency vendoring — the only target that needs network access.
# See CONTRIBUTING.md "Adding a dependency" for the full 5-step workflow.
# ---------------------------------------------------------------------------

vendor:  ## Re-vendor dependencies into vendor/. REQUIRES NETWORK ACCESS.
	python3 tools/dev/vendor.py

vendor-patches:  ## Reapply and verify audited patches to vendored crates (offline).
	python3 tools/dev/vendor.py --patch-only
	python3 tools/dev/test_vendor_patches.py
	python3 tools/dev/test_noise_erasure_patches.py
	python3 tools/dev/test_kdf_admission_patches.py
	python3 tools/dev/test_header_bounds_patches.py

ncsa-boundary-precheck:
	python3 tools/dev/ncsa-boundary-check.py check

ncsa-boundary-check: ncsa-boundary-precheck  ## Prove NCSA/libFuzzer remains development/test-only and outside every application artifact.
	python3 tools/dev/ncsa-boundary-check.py self-test
	# Select the isolated workspace and policy through the working directory;
	# cargo-deny versions differ on where they accept an explicit --config.
	cd fuzz && $(CARGO) deny --all-features --offline --locked check licenses

s3-removal-check:  ## Reject reintroduction of the retired cloud-object sync surface.
	python3 tools/dev/s3-removal-check.py

# ---------------------------------------------------------------------------
# Housekeeping.
# ---------------------------------------------------------------------------

clean:  ## Remove build artifacts (target/).
	$(CARGO_COMPILE) clean

test-console:  ## Verify unified console routing, introspection and real terminal cleanup.
	$(CARGO_COMPILE) test -p hidlins-cli --test console_modes --test console_registry_preferences --test console_keymap_output --test terminal_signal_regressions --test asset_notices --offline --locked

test-api:  ## Verify the reusable Rust application API and DTO contracts.
	$(CARGO_COMPILE) test -p hidlins-api --offline --locked

test-api-native: build-api-native-test-harness  ## Verify API behavior without desktop feature unification.
	"$(API_NATIVE_UNIT_TEST_BIN)" --test-threads=1

test-console-features:  ## Verify the standalone CLI without the terminal UI feature.
	$(CARGO_COMPILE) check -p hidlins-cli --no-default-features --features hidlins-sync/desktop-discovery --offline --locked
	$(CARGO_COMPILE) test -p hidlins-cli --no-default-features --features hidlins-sync/desktop-discovery --test console_cli_only --offline --locked

test-console-process-interop:  ## Verify the TUI and a real CLI process share advisory vault locking.
	$(CARGO_COMPILE) build -p hidlins-cli --offline --locked
	HIDLINS_CLI_BIN="$(CARGO_ARTIFACT_DIR)/debug/hidlins" $(CARGO_COMPILE) test -p hidlins-tui --features process-interop-tests --lib --offline --locked app::tests::tui_session_and_cli_process_share_the_advisory_lock -- --exact

test-dev-tools:  ## Run the retained Python developer-tool regression suites.
	python3 -B -m unittest discover -s tools/dev -p 'test_*.py'

ci-toolchain-check: test-dev-tools  ## Check developer tooling through the same regressions used locally.

boundary-check:  ## Verify Rust API artifact and network dependency boundaries.
	python3 -B tools/dev/api_boundary_check.py

.PHONY: license-check license-policy-check
license-policy-check:  ## Verify strict license policies, source inventory and vendored provenance with negative controls.
	python3 -B -m unittest discover -s tools/dev -p 'test_repository_licenses.py'
	python3 -B tools/dev/repository_licenses.py

license-check: license-policy-check asset-license-check ncsa-boundary-check  ## Audit all retained feature/target licenses, exact exceptions and compiled asset notices.
	$(CARGO) deny --all-features --offline --locked check licenses
	$(CARGO_COMPILE) test -p hidlins-cli --test asset_notices --offline --locked

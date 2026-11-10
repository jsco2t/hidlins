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

# Recipes require bash, not POSIX sh: `app-deps` captures a pipeline status
# via PIPESTATUS. GNU Make defaults SHELL to /bin/sh, which is dash on
# Debian/Ubuntu (where CI runs) and raises "Bad substitution" on bashisms.
# Both target platforms (Linux, macOS) ship bash; a bare name makes make(1)
# resolve it from PATH. (Helper scripts under tools/dev/ carry their own
# bash shebangs and do not depend on this.)
SHELL          := bash

CARGO          := cargo
# Query Cargo lazily: bootstrap/help targets must work before Cargo is installed.
CARGO_ARTIFACT_DIR = $(or $(shell python3 tools/dev/cargo_artifacts.py target-dir -- $(CARGO)),$(error Cargo artifact directory could not be resolved))
CARGO_COMPILE = python3 tools/dev/cargo_artifacts.py run $(CARGO) --
CARGO_FLAGS    := --workspace --offline --locked
CLIPPY_FLAGS   := --workspace --all-targets --offline --locked -- -D warnings
APP_TEST_FLAGS ?=
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

# `make` with no args shows the help screen — friendlier for first-run.
# Devs who know what they want type `make build`, `make test`, `make check`.
.DEFAULT_GOAL := help

.PHONY: help asset-license-check toolchain fuzz-toolchain build release demo-vault build-policy-check _build-policy-check acceptance-evidence-check check-android build-android android-artifacts-check android-harness-check android-emulator-provision android-scenario-boundary-check app-android-config-check app-build-android app-test-android-integration app-test-android-emulator check-ios ios-harness-check app-test-ios-simulator app-test-ios-integration app-test-ios-device app-build-ios app-prepare-ios-observation test test-ignored test-all test-tui-contracts test-update-snapshots test-clipboard test-os-events \
        test-local-sync-security test-local-sync-discovery test-local-sync-integration test-local-sync-mobile-harness test-local-sync-android-lan-discovery test-local-sync-mobile-scenarios-ios test-local-sync-mobile-scenarios-android test-local-sync-mobile-scenarios fuzz-local-sync-corpus fuzz-local-sync-ci ncsa-boundary-precheck ncsa-boundary-check interop-sync s3-removal-check \
        fmt fmt-check lint lint-fix check-feature-gates \
        check verify interop interop-app interop-entry bench bench-search bench-search-gate bench-search-gate-ci \
        vendor vendor-patches deny audit doc clean completions completions-check run-tui \
        snapshots-check \
        api-gen api-gen-check app-build-linux app-build-macos app-brand-generate app-brand-check app-analyze app-fmt app-fmt-check \
        app-test app-test-performance app-test-integration app-test-integration-performance app-goldens-update app-test-bridge app-deps app-run boundary-check pub-vendor pub-vendor-check pub-vendor-hash-test pub-production-check \
        flutter-version-check telemetry-check app-check check-macos test-merge-properties

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

app-linux-toolchain:  ## Install native Flutter build prerequisites on Debian/Ubuntu (explicit setup; needs network).
	bash tools/dev/install-linux-app-toolchain.sh --install

app-linux-prereqs:  ## Check Linux native build prerequisites without installing anything.
	bash tools/dev/install-linux-app-toolchain.sh --check

app-l10n: app-deps  ## Regenerate Dart localizations from the app's ARB source.
	cd app && flutter gen-l10n

.PHONY: app-linux-toolchain app-linux-prereqs app-l10n

.PHONY: ci-flutter-version flutter-sdk-prepare android-ndk-toolchain ci-toolchain-check

ci-flutter-version:  ## Export the exact repository Flutter pin to GitHub step outputs.
	python3 tools/dev/ci-toolchain.py flutter-version

flutter-sdk-prepare:  ## Bootstrap Flutter SDK tools outside the reviewed app cache (requires network).
	python3 tools/dev/ci-toolchain.py flutter-bootstrap

android-ndk-toolchain:  ## Install the exact Android NDK from the artifact contract (requires network).
	python3 tools/dev/ci-toolchain.py android-ndk

ci-toolchain-check: vendor-cache-check android-gradle-sdk-check android-release-clipboard-host-check android-release-build-cleanup-check drift-hash-check cargo-artifact-path-check test-ignored-render-routing  ## Test CI SDK selection and NDK version checks with disposable fixtures.
	python3 -B tools/dev/test_ci_toolchains.py

fuzz-toolchain:  ## Install exact cargo-fuzz and pinned nightly (development/testing only).
	tools/dev/install-fuzz-toolchain.sh

# ---------------------------------------------------------------------------
# Core developer loop — these match CI exactly.
# ---------------------------------------------------------------------------

build:  ## Build the workspace (offline, vendored).
	$(CARGO_COMPILE) build $(CARGO_FLAGS)

release: ncsa-boundary-precheck asset-license-check  ## Build optimized CLI, TUI, and agent artifacts in target/release/ (host platform).
	$(CARGO_COMPILE) build $(CARGO_FLAGS) --release
	python3 tools/dev/ncsa-boundary-check.py artifacts "$(CARGO_ARTIFACT_DIR)/release/hidlins" "$(CARGO_ARTIFACT_DIR)/release/hidlins-tui" "$(CARGO_ARTIFACT_DIR)/release/hidlins-agent"

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

acceptance-evidence-check:  ## Audit release/CI/PRD evidence, skipped residuals, and all 45 July tasks.
	python3 tools/acceptance/check.py self-test
	python3 tools/acceptance/check.py check

build-policy-check:  ## Prove first-party warnings are fatal and platform build policies stay scoped.
	$(MAKE) --no-print-directory _build-policy-check RUSTFLAGS="$(RUSTFLAGS) -C debuginfo=0"

_build-policy-check:
	tools/dev/build-policy-check.sh

run-tui:  ## Launch the interactive terminal UI (hidlins-tui).
	$(CARGO_COMPILE) build -p hidlins-tui --offline --locked
	"$(CARGO_ARTIFACT_DIR)/debug/hidlins-tui"

# ---------------------------------------------------------------------------
# Flutter app (app/) — bridge codegen, builds, analysis, formatting.
# Requires Flutter SDK (see .flutter-version); Rust side builds offline
# against vendor/ via Cargokit.
# ---------------------------------------------------------------------------

# Every app-* / api-gen target routes pub through the vendored cache. Setting
# it here rather than per-recipe means an implicit `pub get` fired by
# `flutter build` / `flutter test` / build_runner is ALSO offline-bound and
# fails loudly instead of silently reaching pub.dev (T2.4's stated guarantee).
export PUB_CACHE := $(CURDIR)/app/vendor-pub
export GRADLE_USER_HOME := $(CURDIR)/.gradle/android
# SDK command-line tools and emulator releases can choose different defaults.
# Share one explicit AVD home across provisioning and every scenario process.
export ANDROID_AVD_HOME ?= $(or $(ANDROID_EMULATOR_HOME),$(ANDROID_USER_HOME),$(HOME)/.android)/avd

FRB_RUST_OUT   := crates/hidlins-api/src/frb_generated.rs
FRB_DART_OUT   := app/lib/src/bridge/

api-gen:  ## Re-generate flutter_rust_bridge bindings (codegen tool required).
	@command -v flutter_rust_bridge_codegen >/dev/null 2>&1 || { \
		echo "error: flutter_rust_bridge_codegen not installed. Run \`make toolchain\`." >&2; exit 1; }
	@command -v cargo-expand >/dev/null 2>&1 || { \
		echo "error: cargo-expand not installed (frb codegen needs it). Run \`make toolchain\`." >&2; exit 1; }
	python3 tools/dev/vendor_cache.py run -- flutter_rust_bridge_codegen generate --config-file flutter_rust_bridge.yaml
	# Canonicalize generated Dart with the same formatter enforced by
	# app-fmt-check. FRB's formatter and the pinned Dart SDK can otherwise
	# disagree on line wrapping, making api-gen-check and app-fmt-check
	# mutually exclusive.
	dart format $(FRB_DART_OUT)
	python3 tools/dev/normalize-generated-dart.py $(FRB_DART_OUT)

.PHONY: drift-hash-check drift-fixture-host-check
drift-fixture-host-check:  ## Verify isolated drift fixtures reach real integrity comparisons through native Make.
	$(MAKE) --no-print-directory -f tools/dev/drift-fixture-host-tests.mk test

drift-hash-check: drift-fixture-host-check  ## Verify portable drift hashing rejects changed or unreadable output.
	$(MAKE) -f tools/dev/drift-hash-tests.mk test

.PHONY: vendor-cache-check
vendor-cache-check: vendor-cache-callers-check vendor-cache-precedence-check  ## Verify vendored-byte invalidation, warm retries and failure safety (Rust only).
	python3 -B tools/dev/test_vendor_cache.py \
	  CacheTests.test_make_real_library_macro_and_host_build_dependency \
	  CacheTests.test_raw_checksum_tamper_cannot_hide_behind_warm_artifact \
	  CacheTests.test_vendor_symlink_read_failure_stops_before_compile \
	  CacheTests.test_cleanup_failure_keeps_old_generation_and_retries \
	  CacheTests.test_failed_first_party_compile_preserves_warm_dependency_retry \
	  CacheTests.test_stamp_publication_and_partial_cleanup_fail_closed \
	  CacheTests.test_fuzz_runner_forwards_guarded_explicit_target_to_real_cargo

.PHONY: vendor-cache-callers-check
vendor-cache-callers-check:  ## Verify snapshot/benchmark/startup cache adoption and clean recovery.
	$(MAKE) -f tools/dev/vendor-cache-caller-tests.mk test

.PHONY: vendor-cache-precedence-check
vendor-cache-precedence-check:  ## Verify explicit Cargo output flags override inherited cache settings.
	$(MAKE) -f tools/dev/vendor-cache-precedence-tests.mk test

.PHONY: cargokit-cache-check
cargokit-cache-check: cargokit-cache-config-check  ## Verify byte-derived Cargokit kernels and shared Cargo cache safety (Dart SDK).
	$(MAKE) -f tools/dev/vendor-cache-tests.mk test

.PHONY: cargokit-cache-config-check
cargokit-cache-config-check:  ## Verify Cargo config and existing home-toolchain lookup in cache metadata.
	$(MAKE) -f tools/dev/cargokit-cache-config-tests.mk test

.PHONY: cargo-artifact-path-check
cargo-artifact-path-check:  ## Verify configured Cargo roots reach build artifact consumers.
	$(MAKE) -f tools/dev/artifact-path-tests.mk test

api-gen-check:  ## Re-generate frb bindings and fail if output drifted (CI gate). NOTE: rewrites the generated files.
	@# Content-hash the generated tree, regenerate, compare. Deliberately NOT
	@# `git status --porcelain`: that conflates "drifted" with "not yet
	@# committed" (untracked files report as `??`), so it fails on a branch
	@# where the bindings are new — and, worse, would pass silently if the
	@# generated paths were ever gitignored. Hashing measures the thing the
	@# gate is actually about: does codegen still produce the committed bytes?
	@before="$$(python3 tools/dev/content_fingerprint.py tree $(FRB_RUST_OUT) $(FRB_DART_OUT))" || exit 1; \
	$(MAKE) --no-print-directory api-gen || { \
		echo "error: api-gen failed — the hash comparison below would be vacuous" >&2; exit 1; }; \
	test -s $(FRB_RUST_OUT) || { echo "error: codegen produced no $(FRB_RUST_OUT)" >&2; exit 1; }; \
	after="$$(python3 tools/dev/content_fingerprint.py tree $(FRB_RUST_OUT) $(FRB_DART_OUT))" || exit 1; \
	if [ "$$before" != "$$after" ]; then \
		echo "error: frb bindings drifted. Run \`make api-gen\` and commit the result." >&2; \
		git --no-pager diff -- $(FRB_RUST_OUT) $(FRB_DART_OUT) >&2 || true; \
		exit 1; \
	fi; \
	echo "  OK: frb bindings match the committed codegen output"

# `--no-pub` on every flutter build/test/run: PUB_CACHE redirection alone
# does NOT disable networking — an implicit `pub get` fired by the flutter
# tool would happily fetch a missing package from pub.dev straight into the
# vendored cache. `--no-pub` suppresses the implicit resolve entirely; the
# only resolve path is `app-deps`, which is `--offline --enforce-lockfile`.
.PHONY: app-linux-lock-feature-check
app-linux-lock-feature-check:  ## Verify Linux OS-lock features and mobile feature boundaries.
	$(MAKE) -f tools/dev/cargokit-feature-tests.mk test

.PHONY: cargokit-policy-check
cargokit-policy-check: app-linux-lock-feature-check cargokit-cache-check  ## Verify exact compiler selection and offline Cargokit prerequisites.
	$(MAKE) -f tools/dev/cargokit-toolchain-tests.mk test

app-build-linux: ncsa-boundary-precheck app-deps app-analyze cargokit-policy-check  ## Build the Flutter desktop app for Linux.
	cd app && flutter build linux --no-pub
	python3 tools/dev/ncsa-boundary-check.py artifacts app/build/linux

app-build-macos: ncsa-boundary-precheck app-deps app-analyze  ## Build the Flutter desktop app for macOS (requires macOS host).
	cd app && flutter build macos --no-pub
	python3 tools/dev/ncsa-boundary-check.py artifacts app/build/macos

.PHONY: app-macos-lock-feature-check
app-macos-lock-feature-check:  ## Verify Cargokit selects iokit for both macOS architectures in every shipping profile.
	$(MAKE) --no-print-directory -f tools/dev/cargokit-apple-feature-tests.mk test

.PHONY: cargokit-host-fixture-check
cargokit-host-fixture-check:  ## Verify real Cargokit cache fixtures select the current Mac or Linux Rust host.
	$(MAKE) --no-print-directory -f tools/dev/cargokit-host-fixture-tests.mk test

check-ios: app-deps app-analyze app-brand-check  ## Cross-check iOS Rust targets and run native simulator security/storage tests.
	tools/ios-native/run.sh check

app-test-ios-simulator: app-deps  ## Run bounded native iOS tests on the configured simulator.
	tools/ios-native/run.sh simulator

.PHONY: app-test-ios-native-publication ios-publication-boundary-check ios-publication-lint
app-test-ios-native-publication: app-deps  ## Prove real iOS C consumption keeps revocation fenced through native pasteboard publication.
	tools/ios-native/run.sh publication

ios-publication-boundary-check:  ## Reject test controls in unsupported builds and verify ordinary iOS artifacts exclude them.
	python3 tools/ios-native/publication_boundary.py check

ios-publication-lint:  ## Lint the isolated debug-simulator publication fixture with fatal warnings.
	python3 tools/ios-native/publication_boundary.py lint

.PHONY: ios-simulator-toolchain-check
ios-simulator-toolchain-check:  ## Verify bootstrap provisions both simulator architectures selected by Cargokit.
	$(MAKE) --no-print-directory -f tools/dev/ios-simulator-targets.mk test

ios-harness-check:  ## Test simulator selection, direct VM attachment, and cleanup.
	python3 tools/ios-native/harness_test.py

.PHONY: ios-semantics-startup-check
ios-semantics-startup-check:  ## Verify delayed iOS semantics initialization precedes integration leak baselines.
	$(MAKE) --no-print-directory -f tools/dev/ios-semantics-tests.mk test

.PHONY: ios-vendor-cache-check
ios-vendor-cache-check:  ## Verify native iOS checks invalidate changed vendor bytes and reuse warm configured caches.
	$(MAKE) --no-print-directory -f tools/dev/ios-vendor-cache-tests.mk test

app-test-ios-integration: app-deps app-analyze ios-harness-check  ## Run UI and real-bridge journeys on an available iPhone and iPad simulator.
	tools/ios-native/integration.sh

app-test-ios-device: app-deps  ## Run native iOS tests on a connected, trusted physical iPhone (requires IOS_DEVELOPMENT_TEAM).
	tools/ios-native/run.sh device

app-build-ios: ncsa-boundary-precheck app-deps app-analyze  ## Build simulator debug and no-codesign device release iOS artifacts.
	tools/ios-native/run.sh build
	python3 tools/dev/ncsa-boundary-check.py artifacts app/build/ios

app-prepare-ios-observation: app-build-ios ios-harness-check  ## Install and launch the inspected build for optional non-gating iOS observations.
	tools/ios-native/prepare_observation.sh

check-android:  ## Cross-check hidlins-api for both supported Android ABIs with the pinned Flutter NDK.
	python3 tools/android-native/artifacts.py self-test
	tools/android-native/build.sh check

build-android: ncsa-boundary-precheck  ## Build/stage release Rust libraries for arm64-v8a + x86_64 and verify the artifact contract.
	python3 tools/android-native/artifacts.py self-test
	tools/android-native/build.sh build
	python3 tools/dev/ncsa-boundary-check.py artifacts app/android/native-artifacts/jniLibs

android-artifacts-check:  ## Reject stale, wrong-profile, wrong-ABI, misaligned, or modified staged Android libraries.
	python3 tools/android-native/artifacts.py check

.PHONY: android-gradle-prepare android-dependency-check

android-gradle-prepare:  ## Install the JUnit-free policy before Android Gradle/plugin resolution.
	bash tools/android-native/gradle-env.sh

.PHONY: android-gradle-sdk-check
android-gradle-sdk-check:  ## Verify Gradle resolves Flutter SDK shims and preserves project and argument forwarding.
	$(MAKE) -f tools/android-native/gradle-sdk-tests.mk test

.PHONY: app-test-android-release-clipboard android-release-clipboard-host-check
app-test-android-release-clipboard:  ## Verify shipping release Rust clipboard handoff and erasure on Android 29/36.
	$(MAKE) -f tools/android-native/release-jni-tests.mk test

android-release-clipboard-host-check:  ## Check JNI inspection and runtime-receipt controls without a device.
	$(MAKE) -f tools/android-native/release-jni-tests.mk self-test

.PHONY: android-release-build-cleanup-check
android-release-build-cleanup-check:  ## Verify timeout and cancellation retire all owned Android build processes.
	$(MAKE) -f tools/android-native/build-ownership-tests.mk test

android-dependency-check: android-gradle-prepare android-storage-directory-check  ## Verify JUnit exclusion with real offline Gradle fixtures and device-test contracts.
	python3 tools/android-native/test_dependency_policy.py

app-android-config-check: app-deps android-dependency-check  ## Verify Flutter 3.47.2 built-in Kotlin/old-DSL pins and reject legacy KGP application.
	python3 tools/android-native/config_check.py self-test
	python3 tools/android-native/config_check.py check
	python3 tools/android-native/plugin_registrant.py self-test
	python3 tools/android-native/plugin_registrant.py check

app-build-android: ncsa-boundary-precheck build-android app-deps app-analyze app-brand-check app-android-config-check cargokit-policy-check  ## Build and inspect Android debug and unsigned release APKs.
	tools/android-native/build_app.sh
	python3 tools/dev/ncsa-boundary-check.py artifacts app/build/app/outputs/flutter-apk

app-test-android-integration: app-build-android  ## Build Android device tests and run them on the selected emulator.
	tools/android-native/run_app_tests.sh

android-harness-check: ios-harness-check  ## Validate Android API/ABI/form-factor selection and secret-safe result tooling.
	python3 tools/android-native/emulator_matrix.py check
	python3 tools/android-native/harness_test.py

android-emulator-provision: android-harness-check  ## Provision API 29/36 native/UI and API 30/36 shared-LAN AVDs.
	tools/android-native/provision_emulators.sh

.PHONY: android-emulator-matrix
android-emulator-matrix:  ## Show native/UI and shared-LAN client/authority selections.
	@echo "Native and UI clients (API 29/36):"
	python3 tools/android-native/emulator_matrix.py select
	@echo "Shared-LAN clients (API 30/36):"
	python3 tools/android-native/emulator_matrix.py lan
	@echo "Shared-LAN authorities:"
	python3 tools/android-native/emulator_matrix.py authority
	@echo "Shared-LAN replacement authorities:"
	python3 tools/android-native/emulator_matrix.py replacement

android-scenario-boundary-check: app-build-android  ## Prove emulator-only authority/NSD registration is confined to test artifacts.
	tools/android-native/gradle.sh -Ptarget-platform=android-arm64,android-x64 :app:assembleDebugAndroidTest
	python3 tools/android-native/scenario_boundary.py

app-test-android-emulator: app-build-android android-harness-check  ## Run native, UI, bridge, lifecycle, and recreation checks on the host Android matrix.
	tools/android-native/run_emulator_suite.sh core

app-brand-generate:  ## Regenerate committed iOS/Android brand rasters (requires macOS CoreGraphics).
	@if [ "$(UNAME_S)" != "Darwin" ]; then \
		echo "error: app-brand-generate requires macOS CoreGraphics" >&2; exit 1; fi
	tools/dev/generate-brand-assets.swift

app-brand-check: app-deps  ## Verify brand hashes, dimensions, platform resources, safe zone, and widget behavior.
	cd app && dart tool/brand_check.dart
	cd app && flutter test --no-pub test/ui/widgets/brand_mark_test.dart

app-analyze:  ## Run Dart static analysis (CI gate).
	cd app && dart analyze --fatal-warnings --fatal-infos

app-fmt:  ## Format Dart code.
	cd app && dart format lib/ test/ test_bridge/ test_driver/ integration_test/ tool/

app-fmt-check:  ## Verify Dart formatting (CI gate).
	cd app && dart format --output=none --set-exit-if-changed lib/ test/ test_bridge/ test_driver/ integration_test/ tool/

app-test: app-deps  ## Run Flutter widget/unit tests.
	@if [ -z "$$(find app/test -name '*_test.dart' 2>/dev/null)" ]; then \
		echo "error: no Flutter tests found (app/test/*_test.dart). Add at least a probe test." >&2; \
		exit 1; \
	fi
	cd app && flutter test --no-pub $(APP_TEST_FLAGS)

app-test-performance: app-deps  ## Run deterministic 5,000-entry Flutter list/search sanity checks.
	cd app && flutter test --no-pub test/performance/desktop_performance_test.dart

app-goldens-update: app-deps  ## Regenerate Flutter golden files (requires HIDLINS_UPDATE_GOLDENS=1).
	@if [ "$(UNAME_S)" != "Linux" ]; then echo "error: regenerate committed goldens on Linux." >&2; exit 1; fi
	@if [ "$${HIDLINS_UPDATE_GOLDENS}" != "1" ]; then \
		echo "error: set HIDLINS_UPDATE_GOLDENS=1 to confirm golden regeneration." >&2; \
		echo "       Review the diff before committing." >&2; exit 1; fi
	cd app && flutter test --update-goldens --no-pub $(APP_TEST_FLAGS)

# The cdylib the smoke test loads. `flutter test` runs on the host VM, so
# this is always the host target dir; only the extension is platform-bound.
ifeq ($(UNAME_S),Darwin)
HIDLINS_API_LIB ?= $(CARGO_ARTIFACT_DIR)/debug/libhidlins_api.dylib
HIDLINS_API_RELEASE_LIB ?= $(CARGO_ARTIFACT_DIR)/release/libhidlins_api.dylib
else
HIDLINS_API_LIB ?= $(CARGO_ARTIFACT_DIR)/debug/libhidlins_api.so
HIDLINS_API_RELEASE_LIB ?= $(CARGO_ARTIFACT_DIR)/release/libhidlins_api.so
endif

# The bridge smoke tests live in app/test_bridge/ (not app/test/) so plain
# `make app-test` stays runnable without a Rust build. Headless: the test
# opens the cdylib directly instead of resolving it from an app bundle, so
# it exercises library loading, SSE codecs, opaque receivers, typed errors,
# and the lock-event stream without a display or device.
app-test-bridge: app-deps  ## Build the hidlins-api cdylib and run the headless Dart↔Rust bridge smoke tests.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/bridge_smoke_test.dart

.PHONY: app-test-lifecycle-async
app-test-lifecycle-async: app-deps  ## Verify lifecycle reconciliation uses the actual asynchronous native bridge.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/lifecycle_async_regression_test.dart

.PHONY: app-test-attachment-refresh
app-test-attachment-refresh: app-deps  ## Verify local attachment mutations refresh cached details through the real bridge.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/attachment_refresh_regression_test.dart

.PHONY: app-test-generated-copy
app-test-generated-copy: app-deps  ## Verify generated-result copy codecs and origin revocation through the actual bridge.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/generated_copy_regression_test.dart

.PHONY: app-test-sync-cancel
app-test-sync-cancel: app-deps  ## Verify foreground discovery cancellation through the actual async bridge repository.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/sync_cancel_regression_test.dart

.PHONY: app-test-import-lifetime
app-test-import-lifetime: app-deps  ## Verify disposed locked import dialogs against the actual async bridge repository.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/connect_import_lifetime_regression_test.dart

.PHONY: app-test-manual-route
app-test-manual-route: app-deps  ## Verify explicit sync routes survive discovery through the actual bridge and Rust admission.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/manual_route_regression_test.dart

.PHONY: app-test-pairing-presentation
app-test-pairing-presentation: app-deps  ## Verify off-page inbound SAS presentation and confirmation through real loopback Rust pairing.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/pairing_presentation_regression_test.dart

.PHONY: app-test-imported-keyfile
app-test-imported-keyfile: app-deps  ## Verify shipping import/unlock UI with a real keyfile-protected KDBX.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/imported_keyfile_regression_test.dart

.PHONY: app-test-touch-actions
app-test-touch-actions: app-deps  ## Verify shipping touch create/delete/manual-lock actions against a real Rust vault.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/touch_actions_regression_test.dart

.PHONY: app-test-vault-password-target
app-test-vault-password-target: app-deps  ## Verify selected-vault password rotation through the real Dart/Rust management UI.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/vault_password_target_regression_test.dart

.PHONY: app-test-vault-management-route
app-test-vault-management-route: app-deps  ## Verify shipping nonempty-registry management preserves both real KDBX vaults.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/vault_management_route_regression_test.dart

.PHONY: app-test-history-access
app-test-history-access: app-deps  ## Verify shipping edit/history reads the real prior KDBX snapshot and clears it on lock.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/history_access_regression_test.dart

.PHONY: app-test-entry-mutation-actions
app-test-entry-mutation-actions: app-deps  ## Verify shipping move and confirmed purge preserve native identity/history and unrelated vault data.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/entry_mutation_actions_regression_test.dart

.PHONY: app-test-expiration-editor
app-test-expiration-editor: app-deps  ## Verify shipping expiration edits use one native save and retain exact untouched timestamps.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/expiration_editor_regression_test.dart

.PHONY: app-test-group-management
app-test-group-management: app-deps  ## Verify shipping group operations retain UUIDs, recycled data and encrypted reopen.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/group_management_regression_test.dart

.PHONY: app-test-attachment-source
app-test-attachment-source: app-deps  ## Verify shipping source selection keeps attachment bytes in Rust and round-trips the encrypted vault.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/attachment_source_regression_test.dart

.PHONY: app-test-linux-attachment-source
app-test-linux-attachment-source:  ## Exercise actual GTK attachment OPEN/SAVE dialogs on an isolated local display (Linux x64).
	$(MAKE) -f tools/linux-native/attachment-source.mk test

.PHONY: app-test-macos-attachment-source
app-test-macos-attachment-source:  ## Exercise native Cocoa attachment OPEN/SAVE handlers and real panel cancellation.
	$(MAKE) --no-print-directory -f tools/macos-native/attachment-source.mk test

.PHONY: app-test-macos-attachment-selection
app-test-macos-attachment-selection:  ## Verify native chooser selected-path/results through deterministic Cocoa panel callbacks.
	$(MAKE) --no-print-directory -f tools/macos-native/selection-tests.mk test

.PHONY: app-test-android-native
app-test-android-native: app-deps android-gradle-prepare  ## Run Android mechanism regressions, including native lifecycle acknowledgement ownership.
	# Android artifact staging uses the repository's canonical target directory.
	CARGO_TARGET_DIR=$(CURDIR)/target $(MAKE) build-android
	tools/android-native/run_app_tests.sh

.PHONY: app-test-android-saf
app-test-android-saf:  ## Verify asynchronous SAF ownership and retained native controls on Android 29/36 emulators.
	$(MAKE) -f tools/android-native/saf-worker-tests.mk test

.PHONY: android-storage-directory-check
android-storage-directory-check:  ## Verify peer directory creation preserves imports and invalid paths still fail.
	$(MAKE) -f tools/android-native/storage-directory-tests.mk test

.PHONY: app-test-startup-discovery
app-test-startup-discovery:  ## Verify Rust startup eligibility and fresh Android 29/36 permission journeys.
	python3 tools/dev/vendor_cache.py run -- $(MAKE) -f tools/android-native/startup-discovery-tests.mk host
	$(MAKE) -f tools/android-native/startup-discovery-tests.mk self-test runtime

app-test-integration: app-deps  ## Run real native-bridge desktop lifecycle and CRUD integration tests.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	$(CARGO_COMPILE) build -p hidlins-cli --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/real_bridge_integration_test.dart
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" HIDLINS_CLI_BIN="$(CARGO_ARTIFACT_DIR)/debug/hidlins" flutter test --no-pub test_bridge/real_bridge_process_coexistence_integration_test.dart
	HIDLINS_CLI_BIN="$(CARGO_ARTIFACT_DIR)/debug/hidlins" $(CARGO_COMPILE) test -p hidlins-tui --features process-interop-tests --lib app::tests::tui_session_and_cli_process_share_the_advisory_lock -- --exact

.PHONY: test-broad-search-performance
test-broad-search-performance:  ## Measure a 5,000-hit search through the optimized native bridge.
	$(MAKE) --no-print-directory -f Makefile -f tools/dev/broad-search-tests.mk test-broad-search

app-test-integration-performance: app-deps ncsa-boundary-precheck  ## Measure 5,000-entry list/search through the optimized native bridge.
	$(CARGO_COMPILE) build -p hidlins-api --bin api-test-driver --features test-fixtures --offline --locked
	$(CARGO_COMPILE) build -p hidlins-api --release --offline --locked
	@set -e; fixture="$$(mktemp -d)"; log="$$(mktemp)"; \
	trap 'rm -rf "$$fixture"; rm -f "$$log"' EXIT; \
	printf '%s\n' 'performance-integration-master-marker' | \
		"$(CARGO_ARTIFACT_DIR)/debug/api-test-driver" create-search-corpus "$$fixture" performance 5000; \
	cd app; \
	HIDLINS_API_LIB="$(HIDLINS_API_RELEASE_LIB)" \
	HIDLINS_PERF_STATE_DIR="$$fixture" \
	HIDLINS_FLUTTER_VERSION="$$(cat ../.flutter-version)" \
	HIDLINS_GIT_REVISION="$$(git -C .. rev-parse HEAD)" \
	flutter test --reporter expanded --no-pub test_bridge/real_bridge_performance_integration_test.dart 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; test "$$status" -eq 0; \
	grep -Eo 'HIDLINS_PERF_JSON:\{.*\}' "$$log"

app-deps: android-gradle-prepare  ## Install pub dependencies (offline from the vendored cache).
	@log="$$(mktemp)"; trap 'rm -f "$$log"' EXIT; \
	cd app && flutter pub get --offline --enforce-lockfile 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	if grep -q "doesn't match contents" "$$log"; then \
		echo "error: vendored pub cache content-hash mismatch — app/vendor-pub/hosted-hashes/ is missing or stale." >&2; \
		echo "       \`flutter pub get\` reports this but exits 0, so it must be gated here." >&2; \
		exit 1; \
	fi; \
	if [ "$$status" -ne 0 ]; then exit "$$status"; fi; \
	cd .. && python3 tools/android-native/plugin_registrant.py install

app-run: app-deps  ## Launch the Flutter app on the default device.
	cd app && flutter run --no-pub

pub-vendor:  ## Re-vendor pub dependencies into app/vendor-pub/. REQUIRES NETWORK.
	cd app && dart pub get
	tools/dev/pub-vendor-hash.sh generate
	@echo "Vendored pub cache updated. Commit app/vendor-pub/ (incl. hosted-hashes/),"
	@echo "app/pubspec.lock, and tools/dev/pub-vendor.SHA256SUMS."

pub-vendor-hash-test:  ## Exercise pub integrity checks with isolated Git fixtures.
	python3 -B tools/dev/test_pub_vendor_hash.py

pub-vendor-check: pub-vendor-hash-test  ## Verify both vendored pub caches are up to date and unmodified (CI gate).
	@# Byte-level integrity first: every file in both committed pub caches
	@# must match the manifest `make pub-vendor` wrote. pub's own
	@# content-hash check only compares the lockfile against the *archive*
	@# hashes in hosted-hashes/ — a modified file inside an extracted
	@# package tree would execute without it noticing. This is the pub
	@# equivalent of the .cargo-checksum.json verification cargo performs
	@# on vendor/ natively.
	tools/dev/pub-vendor-hash.sh verify
	@# Then resolution drift: hash rather than `git status`, so the gate
	@# measures "does an offline re-resolve change anything?" instead of
	@# "are these files committed yet?". The lockfile is what an unfaithful
	@# cache actually perturbs; the package listings catch add/remove drift.
	@# (cache roots mirrored in TREES in tools/dev/pub-vendor-hash.sh)
	@sig() { \
		python3 tools/dev/content_fingerprint.py pub-resolution app/pubspec.lock \
			app/vendor-pub/hosted app/rust_builder/cargokit/build_tool/vendor-pub/hosted; \
	}; \
	before="$$(sig)" || exit 1; \
	$(MAKE) --no-print-directory app-deps >/dev/null || { \
		echo "error: app-deps failed — the drift comparison below would be vacuous" >&2; exit 1; }; \
	after="$$(sig)" || exit 1; \
	if [ "$$before" != "$$after" ]; then \
		echo "error: pub vendor cache drifted. Run \`make pub-vendor\` and commit the result." >&2; \
		git --no-pager diff -- app/pubspec.lock >&2 || true; \
		exit 1; \
	fi; \
	echo "  OK: vendored pub caches match the lockfile"

pub-production-check: app-deps  ## Prove device-test-only Dart packages are absent from the shipping graph (CI gate).
	tools/dev/pub-production-check.sh

flutter-version-check:  ## Assert the installed Flutter matches .flutter-version (CI gate).
	tools/dev/flutter-version-check.sh --strict

telemetry-check:  ## Assert Dart/Flutter telemetry is disabled — NFR-013 (CI gate).
	tools/dev/telemetry-check.sh

boundary-check: test-native-harness-allowlist  ## Verify the API boundary is enforced (CI gate).
	tools/dev/boundary-check.sh

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

.PHONY: test-api-lock-features
test-api-lock-features:  ## Type-check API OS-lock features on Linux and both supported Apple architectures.
	python3 tools/dev/vendor_cache.py run -- python3 tools/dev/test_api_lock_features.py

.PHONY: test-api-lock-cache
test-api-lock-cache:  ## Verify API lock, worker ownership and TOTP cache revocation interleavings.
	$(CARGO_COMPILE) test -p hidlins-api --lib --offline --locked lock_cache_regressions

# A self dev-dependency unifies desktop features in `cargo test`, even with
# --no-default-features. Compile the real unit harness without dev-dependency
# activation, then require its native-only case before running it.
API_NATIVE_UNIT_TEST_BIN = $(CARGO_ARTIFACT_DIR)/debug/deps/hidlins_api
.PHONY: build-api-native-test-harness test-api-lock-cache-native-ticket test-api-worker-panic-ticket
build-api-native-test-harness:  ## Build the API unit harness without development-feature unification.
	$(CARGO_COMPILE) rustc -p hidlins-api --lib --no-default-features --features test-fixtures --offline --locked -- --test

test-api-lock-cache-native-ticket: build-api-native-test-harness  ## Run actual native-ticket lock/cache cases without desktop feature unification.
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list lock_cache_regressions | grep -Fq 'pending_idle_lock_revokes_native_clipboard_ticket_before_handback: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" lock_cache_regressions

test-api-worker-panic-ticket: build-api-native-test-harness  ## Verify native clipboard revocation before a worker-panic Locked event.
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list worker_panic_ticket_regressions | grep -Fq 'caught_panic_revokes_ticket_before_locked_event: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" worker_panic_ticket_regressions

.PHONY: test-api-inbound-secret-erasure
test-api-inbound-secret-erasure:  ## Observe API master/keyfile erasure on validation and registration paths.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --test inbound_secret_erasure_regressions --offline --locked

.PHONY: test-api-bridge-boundary
test-api-bridge-boundary:  ## Generate isolated bindings and reject private cache exports.
	python3 tools/dev/vendor_cache.py run -- python3 tools/dev/test_api_bridge_boundary.py

.PHONY: test-native-harness-allowlist
test-native-harness-allowlist:  ## Verify exact native-test admission and rejection of unreviewed Dart capabilities.
	$(MAKE) --no-print-directory -f tools/dev/native-harness-tests.mk test

.PHONY: test-api-server-lifecycle
test-api-server-lifecycle:  ## Verify API server authority and controls across host ownership loans.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked server_lifecycle_regressions

.PHONY: test-api-server-unlock
test-api-server-unlock:  ## Verify a host ownership loan excludes competing vault unlocks.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked server_unlock_regressions

.PHONY: test-api-server-config-commit
.PHONY: test-reserved-custom-fields
test-reserved-custom-fields:  ## Verify standard-field protection across core and app custom-field edits.
	$(MAKE) --no-print-directory -k -f tools/dev/reserved-fields-tests.mk test

.PHONY: test-api-create-registry-commit
test-api-create-registry-commit:  ## Verify vault creation preserves registry rename ownership on errors.
	$(MAKE) --no-print-directory -f tools/dev/create-registry-tests.mk test

test-api-server-config-commit:  ## Verify server authority follows registry rename commitment on errors.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked server_config_commit_regressions

.PHONY: test-server-shutdown
test-server-shutdown:  ## Verify queued and active sync operations respect host shutdown.
	$(CARGO_COMPILE) test -p hidlins-sync --lib --offline --locked shutdown_regressions

.PHONY: test-api-peer-revocation test-tui-peer-revocation
test-api-peer-revocation:  ## Verify API peer revocation reconciles committed configuration and runtime trust.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked revoke_reconciliation_regressions

test-tui-peer-revocation:  ## Verify TUI peer revocation reconciles committed configuration and runtime trust.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked revoke_reconciliation_regressions

.PHONY: test-api-incoming-commit
test-api-incoming-commit:  ## Verify incoming commits revoke stale secrets and notify providers at the rename boundary.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked incoming_commit_regressions

.PHONY: test-api-native-publication
test-api-native-publication:  ## Verify native publication fences, owner independence and consumed-buffer erasure.
	$(MAKE) --no-print-directory -f Makefile -f tools/dev/native-publication-tests.mk test-native-publication

.PHONY: test-api-copy-origin test-api-copy-origin-native
test-api-copy-origin:  ## Verify desktop clipboard publication origin and failed-copy erasure.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked copy_origin_regressions

test-api-copy-origin-native: build-api-native-test-harness  ## Verify actual native clipboard origin and single-use tickets.
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list copy_origin_regressions | grep -Fq 'native_current_ticket_is_single_use: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" copy_origin_regressions

.PHONY: test-api-generated-copy test-api-generated-copy-native
test-api-generated-copy:  ## Verify exact generated-result copying, revocation and erasure on desktop.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked generated_copy_regressions

test-api-generated-copy-native: build-api-native-test-harness  ## Verify generated-result native tickets without desktop feature unification.
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list generated_copy_regressions | grep -Fq 'native_generated_ticket_is_single_use: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" generated_copy_regressions

.PHONY: test-api-pairing-lifetime test-api-pairing-lifetime-native
test-api-pairing-lifetime:  ## Verify pairing/import cancellation, origin and commit lifetimes.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked pairing_lifetime_regressions

test-api-pairing-lifetime-native: build-api-native-test-harness  ## Verify pairing/import lifetimes without desktop feature unification.
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list pairing_lifetime_regressions | grep -Fq 'native_locked_pair_and_import_control: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" pairing_lifetime_regressions

.PHONY: test-api-discovery-kind app-test-discovery-kind
test-api-discovery-kind:  ## Verify typed discovery routes, manual priority and legacy Trusted wrappers.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked discovery_kind_regressions

app-test-discovery-kind: app-deps  ## Verify typed discovery services through the actual generated bridge.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/discovery_kind_regression_test.dart

.PHONY: test-api-discovery-cancel test-api-discovery-stop-controls app-test-discovery-stop
test-api-discovery-cancel:  ## Verify discovery cannot delay session revocation or publish after shutdown.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked discovery_cancel_regressions

test-api-discovery-stop-controls:  ## Verify discovery stop, queued polls, restart and cleanup ownership.
	$(CARGO_COMPILE) test -p hidlins-api --features test-fixtures --lib --offline --locked discovery_stop_controls

app-test-discovery-stop: app-deps  ## Verify discovery stop through the actual generated bridge.
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/discovery_stop_regression_test.dart

.PHONY: test-api-expiration-save
test-api-expiration-save:  ## Verify entry fields and expiration share one validated atomic save.
	$(CARGO_COMPILE) test -p hidlins-api --test expiration_save_regressions --offline --locked

check-feature-gates: test-api-lock-features test-api-lock-cache-native-ticket test-api-worker-panic-ticket test-api-copy-origin-native test-api-pairing-lifetime-native  ## Type-check gated suites and verify native API lock/cache behavior.
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
	printf '' | $(CARGO_COMPILE) test $(CARGO_FLAGS)

.PHONY: test-cli-foreground-lock
test-cli-foreground-lock:  ## Verify foreground CLI lock/sleep shutdown with owned processes/private D-Bus.
	$(CARGO_COMPILE) test -p hidlins-cli --test cli_foreground_lock_regressions --test cli_foreground_bus_transport_regressions --offline --locked

.PHONY: test-cli-fixture-paths
test-cli-fixture-paths:  ## Verify shared CLI fixture paths are absolute and physical on Mac and Linux.
	$(MAKE) --no-print-directory -f tools/dev/cli-fixture-path-tests.mk test

.PHONY: test-sync-fixture-paths
test-sync-fixture-paths:  ## Verify shared sync fixture paths and cloned ciphertext on Mac and Linux.
	$(MAKE) --no-print-directory -f tools/dev/sync-fixture-path-tests.mk test

.PHONY: test-sync-listener-readiness
test-sync-listener-readiness:  ## Repeat real listener recovery and queued-connection regressions with bounded readiness.
	$(MAKE) --no-print-directory -f tools/dev/listener-readiness-tests.mk test

.PHONY: lock-offline
lock-offline:  ## Resolve reviewed manifest edges into Cargo.lock using only vendored sources.
	$(CARGO) metadata --offline --format-version=1 > /dev/null

.PHONY: test-atomic-recovery-matrix
test-atomic-recovery-matrix:  ## Run atomic recovery regressions on host and real API29/API36 Android (cached SDK/AVDs).
	python3 tools/android-native/run_rust_tests.py

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

.PHONY: test-ignored-render-routing
test-ignored-render-routing:  ## Verify headless ignored tests retain the owned-terminal render benchmark.
	$(MAKE) -f tools/dev/ignored-render-tests.mk test

.PHONY: test-macos-manual-routing
test-macos-manual-routing:  ## Verify unattended macOS gates retain automated cases and isolate human sleep/lock observations.
	$(MAKE) --no-print-directory -f tools/dev/macos-manual-routing-tests.mk test

.PHONY: test-headless-stdin
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

.PHONY: test-sync-preserved-contracts
test-sync-preserved-contracts:  ## Pin retained sync CAS, KDF, pointer and registry contracts cited by docs.
	$(CARGO_COMPILE) test -p hidlins-sync --offline --locked --test preserved_contract_characterization --test us_041_fast_replace_open

.PHONY: test-noise-erasure
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

test-local-sync-mobile-harness:  ## Test the fail-closed mobile scenario orchestrator without starting devices.
	python3 tools/local-sync-tests/mobile_scenario_test.py

test-local-sync-android-lan-discovery: android-emulator-provision android-scenario-boundary-check test-local-sync-mobile-harness  ## Prove shipping Android NSD against a real CLI on shared emulator Wi-Fi.
	HIDLINS_ANDROID_STRICT=$(HIDLINS_ANDROID_STRICT) python3 tools/android-native/lan_discovery.py

test-local-sync-mobile-scenarios-ios: release test-local-sync-mobile-harness  ## Run expensive iPhone/iPad client scenarios against a CLI authority.
	HIDLINS_MOBILE_SYNC_SCENARIOS=1 $(MAKE) --no-print-directory app-test-ios-integration

# Provision AVDs separately, matching app-test-android-emulator. This focused
# gate avoids repeating native/UI journeys when diagnosing real LAN scenarios.
.PHONY: test-local-sync-android-lan-scenarios
test-local-sync-android-lan-scenarios: app-build-android test-local-sync-mobile-harness  ## Run only strict Android shared-LAN CLI-authority scenarios on provisioned AVDs.
	HIDLINS_ANDROID_STRICT=1 python3 tools/android-native/android_mobile_scenario.py

test-local-sync-mobile-scenarios-android: release android-emulator-provision android-scenario-boundary-check test-local-sync-mobile-harness  ## Run expensive Android client scenarios against a CLI authority.
	HIDLINS_MOBILE_SYNC_SCENARIOS=1 $(MAKE) --no-print-directory app-test-android-emulator

test-local-sync-mobile-scenarios: test-local-sync-mobile-harness  ## Run the isolated expensive iOS+Android simulator client scenario gate (macOS host).
	@if [[ "$(UNAME_S)" != Darwin ]]; then \
		echo "error: the aggregate iOS+Android scenario target requires a macOS host" >&2; exit 1; \
	fi
	$(MAKE) --no-print-directory test-local-sync-mobile-scenarios-ios
	$(MAKE) --no-print-directory test-local-sync-mobile-scenarios-android

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
.PHONY: test-native-helper-features
test-native-helper-features:  ## Verify native helper feature unification and unsupported-host rejection.
	$(MAKE) --no-print-directory -f tools/dev/native-helper-tests.mk test

.PHONY: test-logind-fixture
test-logind-fixture:  ## Verify prepared and standalone logind regression entry modes on Linux.
	$(MAKE) --no-print-directory -f tools/dev/logind-fixture-tests.mk test

.PHONY: test-dbus-maintenance
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

.PHONY: test-tui-formatting test-tui-formatting-host
test-tui-formatting-host:  ## Verify native fixture invocation and real rustfmt rejection without Linux-specific paths.
	$(MAKE) --no-print-directory -f tools/dev/tui-format-fixture-host-tests.mk test

test-tui-formatting: test-tui-formatting-host  ## Verify formatting of TUI include! test modules with isolated negative controls.
	python3 tools/test_tui_formatting.py

.PHONY: test-tui-terminal
test-tui-terminal:  ## Verify real TUI signal/quit cleanup on owned controlling terminals.
	$(CARGO_COMPILE) test -p hidlins-tui --test terminal_signal_regressions --offline --locked

.PHONY: test-tui-terminal-backend
test-tui-terminal-backend:  ## Verify actual macOS controlling-TTY and Linux terminal feature boundaries.
	python3 tools/dev/test_tui_terminal_backend.py -- $(CARGO)

.PHONY: test-tui-os-events
test-tui-os-events:  ## Verify TUI native lock/sleep lifetime with owned processes and a private bus.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked native_lock_regressions

.PHONY: test-tui-readonly
test-tui-readonly:  ## Verify read-only TUI shared locking, writer exclusion and query behavior.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked readonly_lock_regressions

.PHONY: test-tui-password-memory
test-tui-password-memory:  ## Verify password input erases allocations before growth and release.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked password_memory_regressions

.PHONY: test-tui-note-memory
test-tui-note-memory:  ## Verify secure-note editor, view and history allocations erase before release.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked note_memory_regressions

.PHONY: test-tui-edit-validation
test-tui-edit-validation:  ## Verify Edit rejects field collisions, failed generation and implicit disclosure.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked edit_validation_regressions

.PHONY: test-tui-configuration
test-tui-configuration:  ## Verify effective keyboard dispatch, counted jumps and content-safe warnings.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked configuration_regressions

.PHONY: test-tui-state
test-tui-state:  ## Verify UI-state merge-on-write, advisory locking and error preservation.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked ui_state_regressions

.PHONY: test-tui-peers
test-tui-peers:  ## Verify selected peer visibility and matching persisted revocation.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked peers_visibility_regressions

.PHONY: test-tui-render-memory
test-tui-render-memory:  ## Verify first-party secret render allocations erase before release.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked render_memory_regressions

.PHONY: test-tui-framework-memory test-ratatui-memory-patches bench-tui-render
test-tui-framework-memory:  ## Verify Ratatui symbol and screen-buffer erasure across allocation lifetimes.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked framework_memory_regressions -- --test-threads=1

test-ratatui-memory-patches:  ## Verify maintained Ratatui patch guards, checksums and idempotence.
	python3 tools/dev/test_ratatui_memory_patches.py

bench-tui-render:  ## Measure serial release redraw/unchanged-frame work against the existing 20-FPS budget.
	python3 tools/dev/test_ratatui_memory_patches.py --render-benchmark $(CARGO_COMPILE) test -p hidlins-tui --lib --release --offline --locked framework_memory_regressions::render_workload_performance_baseline -- --exact --ignored --nocapture --test-threads=1

.PHONY: test-tui-output bench-tui-output
test-tui-output:  ## Verify owned output erasure, batching, partial writes, errors and panic cleanup.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked terminal_output -- --test-threads=1

bench-tui-output:  ## Measure real redirected stdout rendering without recording synthetic payloads.
	python3 -c 'import subprocess,sys; subprocess.run(sys.argv[1:], stdout=subprocess.DEVNULL, check=True)' $(CARGO_COMPILE) test -p hidlins-tui --lib --release --offline --locked terminal_output::tests::render_ -- --ignored --nocapture --test-threads=1

.PHONY: test-tui-detail-scroll
test-tui-detail-scroll:  ## Verify wrapped detail reachability and resize bounds on Secrets and pinned tabs.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked detail_scrolling_regressions

.PHONY: test-tui-history-scroll
test-tui-history-scroll:  ## Verify newest-first history, detail paging, resize and unchanged version navigation.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked history_scrolling_regressions

.PHONY: test-tui-edit-viewport test-tui-paragraph-position test-ratatui-geometry-patch
test-tui-edit-viewport:  ## Verify editor focus, exact wrapped caret, errors, masking and resize.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked edit_viewport_regressions

test-tui-paragraph-position:  ## Compare exact span positions with real Paragraph rendering.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --offline --locked paragraph_position_regressions

test-ratatui-geometry-patch:  ## Verify exact maintained renderer-query replay and drift rejection.
	python3 tools/dev/test_ratatui_geometry_patch.py

.PHONY: bench-tui-edit
bench-tui-edit:  ## Measure complete editor focus/caret/error rendering against the existing frame budget.
	$(CARGO_COMPILE) test -p hidlins-tui --lib --release --offline --locked paragraph_position_regressions::editor_render_budget -- --exact --ignored --nocapture --test-threads=1

.PHONY: test-tui-reserved-bindings
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

# `check` stays runnable from a clean clone with nothing but rustup. The
# Flutter/codegen gates live in `app-check` and run under `verify`, matching how
# the keepassxc-dependent interop targets are already scoped — a contributor
# making a Rust-only change must not be blocked on installing a 1 GB SDK, and
# CLAUDE.md/T2.3 commit to exactly that ("generated code is committed so
# contributors without the Flutter/frb toolchain can still build Rust-only
# changes"). Note api-gen-check REWRITES the generated files as a side effect,
# which is another reason it does not belong in the quick loop.
check: test-headless-stdin ci-toolchain-check build-policy-check fmt-check test-tui-formatting lint build test check-macos check-feature-gates  ## warning policy + fmt-check + lint + build + test + cross/feature-gate checks (Rust-only; no Flutter needed).

# pub-vendor-check runs right after app-deps, BEFORE anything executes
# vendored Dart (flutter test, build_runner): tampered cache bytes should
# fail the integrity gate without ever having run.
app-check: flutter-version-check telemetry-check app-deps pub-vendor-check pub-production-check cargokit-policy-check app-analyze app-fmt-check app-brand-check app-test app-test-bridge api-gen-check  ## Flutter-side gates (requires the Flutter SDK + Rust toolchain).

verify: test-tui-terminal-backend test-macos-manual-routing check asset-license-check app-check acceptance-evidence-check test-ignored doc deny audit interop interop-entry interop-sync test-local-sync-security test-local-sync-discovery test-local-sync-integration fuzz-local-sync-corpus ncsa-boundary-check s3-removal-check boundary-check  ## Full verification gate (Rust + Flutter + interop + acceptance evidence).

asset-license-check:  ## Check the exact EFF asset exception, notices and unchanged code-license policy.
	python3 -m unittest discover -s tools/dev -p 'test_asset_licenses.py'
	python3 tools/dev/asset_license_check.py --spec-root "$(SPEC_POLICY_ROOT)"

interop:  ## Run vault-core KeePassXC interop shell tests (requires keepassxc-cli).
	$(CARGO_COMPILE) build -p hidlins-core --bin hidlins-test-driver --offline --locked
	HIDLINS_TEST_DRIVER="$(CARGO_ARTIFACT_DIR)/debug/hidlins-test-driver" sh tools/interop-tests/us_090_rust_to_kpxc.sh
	HIDLINS_TEST_DRIVER="$(CARGO_ARTIFACT_DIR)/debug/hidlins-test-driver" sh tools/interop-tests/us_091_kpxc_to_rust.sh
	HIDLINS_TEST_DRIVER="$(CARGO_ARTIFACT_DIR)/debug/hidlins-test-driver" sh tools/interop-tests/us_092_round_trip.sh

interop-app: app-deps  ## Run compiled Flutter-bridge KeePassXC round-trip and merge-history interop.
	@command -v keepassxc-cli >/dev/null 2>&1 || { echo "error: keepassxc-cli is required" >&2; exit 1; }
	$(CARGO_COMPILE) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/real_bridge_keepassxc_integration_test.dart
	$(MAKE) --no-print-directory interop-sync

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

.PHONY: test-nested-completions
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

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
CARGO_FLAGS    := --workspace --offline --locked
CLIPPY_FLAGS   := --workspace --all-targets --offline --locked -- -D warnings

# Host OS — used to pick the platform-appropriate OS-event source test
# (logind on Linux, IOKit on macOS); the wrong-platform feature won't
# even resolve (`logind`→`zbus` is Linux-target-only; `iokit`→`objc2`
# is macOS-target-only), so `test-os-events` must not invoke both.
UNAME_S        := $(shell uname -s)

# `make` with no args shows the help screen — friendlier for first-run.
# Devs who know what they want type `make build`, `make test`, `make check`.
.DEFAULT_GOAL := help

.PHONY: help toolchain build test test-ignored test-all test-update-snapshots test-clipboard test-os-events \
        test-sigv4 minio-up minio-down minio-native-up minio-native-down test-s3-integration interop-sync \
        fmt fmt-check lint lint-fix check-feature-gates check-android-guards check-android build-android ndk-preflight \
        check verify interop interop-entry bench bench-search bench-search-gate bench-search-gate-ci \
        vendor deny audit doc clean completions completions-check run-tui \
        snapshots-check \
        api-gen api-gen-check app-build-linux app-build-macos app-analyze app-fmt app-fmt-check \
        app-test app-goldens-update app-test-bridge app-test-integration app-test-integration-minio test-minio-managed \
        interop-app app-deps app-run boundary-check pub-vendor pub-vendor-check \
        flutter-version-check telemetry-check app-check check-macos test-merge-properties

help:  ## Show this help.
	@awk 'BEGIN {FS = ":.*##"; print "Usage: make <target>\n\nTargets:"} /^[a-zA-Z_-]+:.*##/ {printf "  %-15s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# ---------------------------------------------------------------------------
# One-time environment setup — run this first on a fresh machine.
# ---------------------------------------------------------------------------

toolchain:  ## Install all dev tooling (Rust, keepassxc-cli, MinIO mc, cargo-deny/audit). Run first on a new machine.
	# Cross-platform bootstrap (Linux + macOS). Idempotent — re-running
	# skips already-installed tools. Tool versions are pinned to match CI
	# (.github/workflows/ci.yml). After this, `make check` works offline.
	# Invoked directly (not via `sh`) so its bash shebang is honoured —
	# the script uses `set -o pipefail`, which POSIX `sh`/dash lacks.
	tools/dev/install-toolchain.sh

# ---------------------------------------------------------------------------
# Core developer loop — these match CI exactly.
# ---------------------------------------------------------------------------

build:  ## Build the workspace (offline, vendored).
	$(CARGO) build $(CARGO_FLAGS)

run-tui:  ## Launch the interactive terminal UI (hidlins-tui).
	$(CARGO) run -p hidlins-tui --offline --locked

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

FRB_RUST_OUT   := crates/hidlins-api/src/frb_generated.rs
FRB_DART_OUT   := app/lib/src/bridge/

api-gen:  ## Re-generate flutter_rust_bridge bindings (codegen tool required).
	@command -v flutter_rust_bridge_codegen >/dev/null 2>&1 || { \
		echo "error: flutter_rust_bridge_codegen not installed. Run \`make toolchain\`." >&2; exit 1; }
	@command -v cargo-expand >/dev/null 2>&1 || { \
		echo "error: cargo-expand not installed (frb codegen needs it). Run \`make toolchain\`." >&2; exit 1; }
	flutter_rust_bridge_codegen generate --config-file flutter_rust_bridge.yaml
	# Canonicalize generated Dart with the same formatter enforced by
	# app-fmt-check. FRB's formatter and the pinned Dart SDK can otherwise
	# disagree on line wrapping, making api-gen-check and app-fmt-check
	# mutually exclusive.
	dart format $(FRB_DART_OUT)

api-gen-check:  ## Re-generate frb bindings and fail if output drifted (CI gate). NOTE: rewrites the generated files.
	@# Content-hash the generated tree, regenerate, compare. Deliberately NOT
	@# `git status --porcelain`: that conflates "drifted" with "not yet
	@# committed" (untracked files report as `??`), so it fails on a branch
	@# where the bindings are new — and, worse, would pass silently if the
	@# generated paths were ever gitignored. Hashing measures the thing the
	@# gate is actually about: does codegen still produce the committed bytes?
	@before="$$(find $(FRB_RUST_OUT) $(FRB_DART_OUT) -type f 2>/dev/null | sort | xargs sha256sum 2>/dev/null | sha256sum)"; \
	$(MAKE) --no-print-directory api-gen || { \
		echo "error: api-gen failed — the hash comparison below would be vacuous" >&2; exit 1; }; \
	test -s $(FRB_RUST_OUT) || { echo "error: codegen produced no $(FRB_RUST_OUT)" >&2; exit 1; }; \
	after="$$(find $(FRB_RUST_OUT) $(FRB_DART_OUT) -type f 2>/dev/null | sort | xargs sha256sum 2>/dev/null | sha256sum)"; \
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
app-build-linux: app-deps  ## Build the Flutter desktop app for Linux.
	cd app && flutter build linux --no-pub

app-build-macos: app-deps  ## Build the Flutter desktop app for macOS (requires macOS host).
	cd app && flutter build macos --no-pub

app-analyze:  ## Run Dart static analysis (CI gate).
	cd app && dart analyze --fatal-infos

app-fmt:  ## Format Dart code.
	cd app && dart format lib/ test/ test_bridge/

app-fmt-check:  ## Verify Dart formatting (CI gate).
	cd app && dart format --output=none --set-exit-if-changed lib/ test/ test_bridge/

app-test: app-deps  ## Run Flutter widget/unit tests.
	@if [ -z "$$(find app/test -name '*_test.dart' 2>/dev/null)" ]; then \
		echo "error: no Flutter tests found (app/test/*_test.dart). Add at least a probe test." >&2; \
		exit 1; \
	fi
	cd app && flutter test --no-pub

app-goldens-update: app-deps  ## Regenerate Flutter golden files (requires HIDLINS_UPDATE_GOLDENS=1).
	@if [ "$${HIDLINS_UPDATE_GOLDENS}" != "1" ]; then \
		echo "error: set HIDLINS_UPDATE_GOLDENS=1 to confirm golden regeneration." >&2; \
		echo "       Review the diff before committing." >&2; exit 1; fi
	@if [ "$(UNAME_S)" != "Linux" ]; then \
		echo "error: committed Flutter goldens must be regenerated on Linux." >&2; \
		echo "       Use the pinned Linux Flutter environment used by CI." >&2; exit 1; fi
	cd app && flutter test --update-goldens --no-pub test/goldens_test.dart

# The cdylib the smoke test loads. `flutter test` runs on the host VM, so
# this is always the host target dir; only the extension is platform-bound.
ifeq ($(UNAME_S),Darwin)
HIDLINS_API_LIB := $(CURDIR)/target/debug/libhidlins_api.dylib
else
HIDLINS_API_LIB := $(CURDIR)/target/debug/libhidlins_api.so
endif

# The bridge smoke tests live in app/test_bridge/ (not app/test/) so plain
# `make app-test` stays runnable without a Rust build. Headless: the test
# opens the cdylib directly instead of resolving it from an app bundle, so
# it exercises library loading, SSE codecs, opaque receivers, typed errors,
# and the lock-event stream without a display or device.
app-test-bridge: app-deps  ## Build the hidlins-api cdylib and run the headless Dart↔Rust bridge smoke tests.
	$(CARGO) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB=$(HIDLINS_API_LIB) flutter test --no-pub \
		test_bridge/bridge_smoke_test.dart

HIDLINS_API_TEST_DRIVER := $(CURDIR)/target/debug/api-test-driver

app-test-integration: app-deps  ## Run real-bridge lifecycle, auto-lock, and 5k-search integration tests.
	$(CARGO) build -p hidlins-api --features test-fixtures --offline --locked
	@set -euo pipefail; \
	idle_dir="$$(mktemp -d "$${TMPDIR:-/tmp}/hidlins-app-idle.XXXXXX")"; \
	perf_dir="$$(mktemp -d "$${TMPDIR:-/tmp}/hidlins-app-perf.XXXXXX")"; \
	cleanup() { rm -rf -- "$$idle_dir" "$$perf_dir"; }; \
	trap cleanup EXIT INT TERM; \
	printf '%s\n' test-password | $(HIDLINS_API_TEST_DRIVER) create-vault "$$idle_dir" idle >/dev/null; \
	$(HIDLINS_API_TEST_DRIVER) set-lock-timeout "$$idle_dir" idle 1 >/dev/null; \
	printf '%s\n' test-password | $(HIDLINS_API_TEST_DRIVER) create-vault "$$perf_dir" performance >/dev/null; \
	printf '%s\n' test-password | $(HIDLINS_API_TEST_DRIVER) seed-search-corpus "$$perf_dir" performance 5000 >/dev/null; \
	cd app && HIDLINS_API_LIB=$(HIDLINS_API_LIB) \
		HIDLINS_APP_IDLE_STATE_DIR="$$idle_dir" HIDLINS_APP_PERF_STATE_DIR="$$perf_dir" \
		flutter test --no-pub test_bridge/integration/lifecycle_test.dart

app-test-integration-minio: app-deps  ## Run real-bridge two-session sync/conflict tests (run `make minio-up` first).
	@if [ ! -f tools/sync-tests/fixtures/.minio-env ]; then \
		echo "error: MinIO not running — run \`make minio-up\` first." >&2; exit 1; fi
	$(CARGO) build -p hidlins-api --features test-fixtures --offline --locked
	. tools/sync-tests/fixtures/.minio-env && cd app && \
		HIDLINS_API_LIB=$(HIDLINS_API_LIB) \
		flutter test --no-pub test_bridge/integration/sync_test.dart
	. tools/sync-tests/fixtures/.minio-env && cd app && \
		HIDLINS_API_LIB=$(HIDLINS_API_LIB) \
		flutter test --no-pub test_bridge/integration/sync_conflict_test.dart

test-minio-managed:  ## Start MinIO if needed, run all Rust + app live-wire suites, then stop only fixtures this target owns.
	@set -euo pipefail; \
	already_running=0; \
	for engine in docker podman; do \
		if command -v "$$engine" >/dev/null 2>&1 && \
			[ "$$($$engine inspect -f '{{.State.Running}}' hidlins-minio 2>/dev/null)" = "true" ]; then \
			already_running=1; break; \
		fi; \
	done; \
	cleanup() { \
		if [ "$$already_running" -eq 0 ]; then $(MAKE) --no-print-directory minio-down; fi; \
	}; \
	trap cleanup EXIT INT TERM; \
	$(MAKE) --no-print-directory minio-up; \
	$(MAKE) --no-print-directory test-s3-integration; \
	$(MAKE) --no-print-directory app-test-integration-minio

app-deps:  ## Install pub dependencies (offline from the vendored cache).
	@log="$$(mktemp)"; trap 'rm -f "$$log"' EXIT; \
	cd app && dart pub get --offline --enforce-lockfile 2>&1 | tee "$$log"; \
	status=$${PIPESTATUS[0]}; \
	if grep -q "doesn't match contents" "$$log"; then \
		echo "error: vendored pub cache content-hash mismatch — app/vendor-pub/hosted-hashes/ is missing or stale." >&2; \
		echo "       \`dart pub get\` reports this but exits 0, so it must be gated here." >&2; \
		exit 1; \
	fi; \
	exit $$status

app-run: app-deps  ## Launch the Flutter app on the default device.
	cd app && flutter run --no-pub

pub-vendor:  ## Re-vendor pub dependencies into app/vendor-pub/. REQUIRES NETWORK.
	cd app && dart pub get
	tools/dev/pub-vendor-hash.sh generate
	@echo "Vendored pub cache updated. Commit app/vendor-pub/ (incl. hosted-hashes/),"
	@echo "app/pubspec.lock, and tools/dev/pub-vendor.SHA256SUMS."

pub-vendor-check:  ## Verify both vendored pub caches are up to date and unmodified (CI gate).
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
		cat app/pubspec.lock 2>/dev/null | sha256sum; \
		find app/vendor-pub/hosted app/rust_builder/cargokit/build_tool/vendor-pub/hosted \
			-maxdepth 2 -mindepth 2 -type d 2>/dev/null | sort | sha256sum; \
	}; \
	before="$$(sig)"; \
	$(MAKE) --no-print-directory app-deps >/dev/null || { \
		echo "error: app-deps failed — the drift comparison below would be vacuous" >&2; exit 1; }; \
	after="$$(sig)"; \
	if [ "$$before" != "$$after" ]; then \
		echo "error: pub vendor cache drifted. Run \`make pub-vendor\` and commit the result." >&2; \
		git --no-pager diff -- app/pubspec.lock >&2 || true; \
		exit 1; \
	fi; \
	echo "  OK: vendored pub caches match the lockfile"

flutter-version-check:  ## Assert the installed Flutter matches .flutter-version (CI gate).
	tools/dev/flutter-version-check.sh --strict

telemetry-check:  ## Assert Dart/Flutter telemetry is disabled — NFR-013 (CI gate).
	tools/dev/telemetry-check.sh

boundary-check:  ## Verify the API boundary is enforced (CI gate).
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
	$(CARGO) check --target aarch64-apple-darwin --offline --locked -p hidlins-security --features iokit
	$(CARGO) check --target x86_64-apple-darwin  --offline --locked -p hidlins-security --features iokit

check-feature-gates: check-android-guards  ## Type-check feature-gated suites/mobile stubs and enforce Android API-floor parity.
	# Compile-only drift gate. The display-dependent clipboard suites
	# (`clipboard-tests`) and the OS-event helper binaries are excluded
	# from every runtime CI sweep by design — but without this gate they
	# are never even *compiled* in CI, so an API refactor of
	# `hidlins_security::Clipboard` or the CLI's `--copy` path could
	# silently break them. `cargo check --tests` compiles the gated test
	# targets without running them (no display needed).
	$(CARGO) check -p hidlins-security --offline --locked --features clipboard-tests --tests
	$(CARGO) check -p hidlins-cli --offline --locked --features clipboard-tests --tests
	$(CARGO) check -p hidlins-security --offline --locked --no-default-features
	$(CARGO) check -p hidlins-api --offline --locked --features test-fixtures --tests
	# Mobile-cfg parity gate (flutter-app P6): compiles the
	# cfg(not(feature = "desktop")) counterpart stubs against the committed
	# desktop-shaped frb glue on the HOST target — no NDK needed — so
	# signature drift between the desktop doors and their mobile twins
	# fails every contributor's `make check`, not just the NDK-gated CI
	# android job. Deliberately lib-only (no --tests): the self dev-dep
	# keeps default features, so --tests would re-enable `desktop` via
	# feature unification and check nothing.
	$(CARGO) check -p hidlins-api --offline --locked --no-default-features
ifeq ($(UNAME_S),Darwin)
	# iokit compiles natively here; logind's zbus tree is Linux-only.
	$(CARGO) check -p hidlins-security --offline --locked --features test-binaries,iokit --tests
else
	$(CARGO) check -p hidlins-security --offline --locked --features test-binaries,logind --tests
endif

# ---------------------------------------------------------------------------
# Android cross-compilation (flutter-app P6 spike T6.1; productionized in P8).
#
# Cargo's `config.toml` `linker` key is a static path and cannot expand
# `$ANDROID_NDK_HOME` (design D-7 as amended / review A9), so the NDK
# linker + C toolchain env vars are exported HERE, where host-OS path
# resolution is possible. Cargokit does its own NDK resolution for the
# Gradle-driven app build (T8.1); these targets cover the Rust-only gate.
#
# API level 29 matches the D-10 Android floor: the NDK's clang wrappers
# encode the minimum API in their filename (aarch64-linux-android29-clang),
# and `ring`'s build script compiles its C/asm sources with the same
# wrapper via the `CC_<triple>` / `AR_<triple>` env vars the `cc` crate
# reads. The NDK prebuilt host tag is darwin-x86_64 on macOS even for
# Apple Silicon hosts (the binaries are universal); there is no
# linux-arm64 host NDK, hence only the two tags below.
# ---------------------------------------------------------------------------

# API level 29 = the design D-10 Android floor. Keep in lockstep with
# `minSdk` in app/android/app/build.gradle.kts — the NDK clang wrapper
# name embeds this minimum, so a floor change that updates only one side
# ships a cdylib built against a different minimum than the APK declares.
ANDROID_API_LEVEL := 29
ifeq ($(UNAME_S),Darwin)
NDK_HOST_TAG      := darwin-x86_64
else
NDK_HOST_TAG      := linux-x86_64
endif
NDK_BIN           := $(ANDROID_NDK_HOME)/toolchains/llvm/prebuilt/$(NDK_HOST_TAG)/bin
NDK_CC_AARCH64    := $(NDK_BIN)/aarch64-linux-android$(ANDROID_API_LEVEL)-clang
NDK_CC_X86_64     := $(NDK_BIN)/x86_64-linux-android$(ANDROID_API_LEVEL)-clang

# Target-scoped exports: only the android targets see these, and only for
# these recipes — no other make target's environment is polluted. The
# underscore forms (CC_aarch64_linux_android) are the `cc` crate's
# documented per-target override names; CARGO_TARGET_* is cargo's own
# linker override, equivalent to a config.toml `linker` key but computed
# at make time so $ANDROID_NDK_HOME expansion works.
check-android build-android: export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER := $(NDK_CC_AARCH64)
check-android build-android: export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER  := $(NDK_CC_X86_64)
check-android build-android: export CC_aarch64_linux_android := $(NDK_CC_AARCH64)
check-android build-android: export CC_x86_64_linux_android  := $(NDK_CC_X86_64)
check-android build-android: export AR_aarch64_linux_android := $(NDK_BIN)/llvm-ar
check-android build-android: export AR_x86_64_linux_android  := $(NDK_BIN)/llvm-ar

# `--no-default-features` is load-bearing: hidlins-api's default `desktop`
# feature forwards to hidlins-security's desktop stack (arboard,
# signal-hook, OS event sources), none of which exist for Android. The
# mobile build compiles only the OS-agnostic security pieces plus the
# cfg(target_os = "android") JNI module.
ANDROID_CARGO_FLAGS := -p hidlins-api --no-default-features --offline --locked

check-android-guards:  ## Test Android guard logic and enforce Makefile/Gradle API-floor lockstep (NDK-free).
	tools/dev/android-build-guards.sh self-test
	tools/dev/android-build-guards.sh api-floor app/android/app/build.gradle.kts $(ANDROID_API_LEVEL)

ifdef ANDROID_NDK_HOME
check-android: ndk-preflight  ## Type-check hidlins-api + deps for both Android triples (needs $ANDROID_NDK_HOME; ring's build.rs compiles C).
	$(CARGO) check $(ANDROID_CARGO_FLAGS) --target aarch64-linux-android
	$(CARGO) check $(ANDROID_CARGO_FLAGS) --target x86_64-linux-android

build-android: ndk-preflight  ## Build hidlins-api (cdylib) for both Android triples — the full ring compile+link gate.
	$(CARGO) build $(ANDROID_CARGO_FLAGS) --target aarch64-linux-android
	$(CARGO) build $(ANDROID_CARGO_FLAGS) --target x86_64-linux-android
	# The JNI export is referenced by nothing on the Rust side (Kotlin
	# resolves it by name at runtime), so only a symbol-table assertion
	# catches an accidental rename/removal of the Kotlin-facing ABI.
	@for so in "$${CARGO_TARGET_DIR:-target}"/aarch64-linux-android/debug/libhidlins_api.so \
	           "$${CARGO_TARGET_DIR:-target}"/x86_64-linux-android/debug/libhidlins_api.so; do \
		tools/dev/android-build-guards.sh jni-symbol "$(NDK_BIN)/llvm-nm" "$$so"; \
	done
	@echo "  OK: JNI verifier-init export present in both Android cdylibs"
	# Single-copy invariant: hidlins-api's direct rustls-platform-verifier
	# dep must be the SAME instance ureq links — the JNI init writes a
	# process-global inside the crate, and a semver split (e.g. a future
	# ureq bump to 0.7) would compile cleanly while the sync stack reads a
	# second, uninitialized global. cargo-deny only WARNS on duplicate
	# versions, so this is the hard gate.
	@count="$$($(CARGO) tree -p hidlins-api --no-default-features --offline --locked \
		--target aarch64-linux-android -i rustls-platform-verifier 2>/dev/null \
		| grep -c '^rustls-platform-verifier v')"; \
	if [ "$$count" != "1" ]; then \
		echo "error: expected exactly one rustls-platform-verifier in the Android graph, found $$count." >&2; \
		echo "       A version split leaves the TLS verifier global uninitialized on Android." >&2; \
		exit 1; \
	fi
	@echo "  OK: single rustls-platform-verifier instance in the Android graph"

# Internal helper (deliberately not in `make help`): fail fast with an
# actionable message when ANDROID_NDK_HOME does not point at a usable
# NDK, instead of a raw clang/linker error from deep inside cargo.
ndk-preflight:
ifneq ($(UNAME_S),Darwin)
	@if [ "$$(uname -m)" != "x86_64" ]; then \
		echo "error: no NDK host toolchain exists for linux-$$(uname -m)." >&2; \
		echo "       Cross-compile from an x86_64 Linux or a macOS host." >&2; \
		exit 1; \
	fi
endif
	@for cc in "$(NDK_CC_AARCH64)" "$(NDK_CC_X86_64)"; do \
		test -x "$$cc" || { \
			echo "error: NDK clang not found at $$cc" >&2; \
			echo "       ANDROID_NDK_HOME=$(ANDROID_NDK_HOME) does not look like a usable NDK root (r26+ expected)." >&2; \
			exit 1; }; \
	done
else
# Graceful skip so desktop-only contributors are never blocked by a
# missing NDK (Phase-1 doc acceptance item). CI sets
# HIDLINS_ANDROID_STRICT=1 so a runner-image change that drops the NDK
# fails loudly instead of silently skipping the gate. Any non-empty
# value counts as strict — an exact-literal match would fail open on
# "true"/"yes".
check-android build-android:
	@if [ -n "$(strip $(HIDLINS_ANDROID_STRICT))" ]; then \
		echo "error: ANDROID_NDK_HOME is not set but HIDLINS_ANDROID_STRICT is (CI must not skip the Android gate)." >&2; \
		exit 1; \
	fi
	@echo "skipping $@: ANDROID_NDK_HOME is not set (Android NDK required)."
	@echo "  Install the NDK (Android Studio SDK Manager, or \`sdkmanager 'ndk;<version>'\`)"
	@echo "  and export ANDROID_NDK_HOME=<sdk>/ndk/<version>, then re-run."
endif

test:  ## Run default-parallel tests (offline, vendored).
	$(CARGO) test $(CARGO_FLAGS)

test-ignored:  ## Run #[ignore]d tests serially (env-mutating + signal-handler tests, etc).
	# `--features test-binaries` enables hidlins-security's `sigstop_helper`
	# test binary, needed by `tests/us_052_sigstop_lock.rs`. Other workspace
	# members don't define the feature; cargo silently no-ops on them.
	# NOT enabled here: `minio-tests` (live-wire S3 cases needing `make
	# minio-up`) and `clipboard-tests` (real-display cases) — both stay
	# `cfg`/feature-gated out of this blanket sweep and run only via their
	# dedicated targets (`test-s3-integration`, `test-clipboard`).
	# Build vault_holder (hidlins-core test binary) — needed by
	# hidlins-api's contended_vault_surfaces_holder_pid test.
	$(CARGO) build -p hidlins-core $(CARGO_FLAGS) --bin vault_holder
	$(CARGO) test $(CARGO_FLAGS) --features test-binaries -- --ignored --test-threads=1

test-all: test test-ignored  ## Run both default and #[ignore]d tests.

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
	$(CARGO) test -p hidlins-security --offline --locked --features clipboard-tests --test us_053_clipboard_autoclear -- --ignored --test-threads=1
	$(CARGO) test -p hidlins-cli --offline --locked --features clipboard-tests --test cli_clipboard_handoff -- --ignored --test-threads=1

test-sigv4:  ## Run the AWS SigV4 published test-vector corpus runner (s3-sync T2.2; fast CI gate).
	# Runs ONLY `tests/sigv4_aws_test_vectors.rs`, the corpus runner that
	# asserts our hand-rolled SigV4 signer (crates/hidlins-sync/src/s3/signer.rs)
	# matches the AWS-published expected outputs across ~15 applicable
	# test vectors. Fast (<1s); independent of the broader unit-test
	# suite so a SigV4 encoding bug produces a well-isolated failure
	# signal. The corpus is vendored at
	# crates/hidlins-sync/tests/data/aws_sigv4_vectors/ — see the
	# AWS_SIGV4_VECTORS_SOURCE.md doc there for the upstream provenance.
	$(CARGO) test -p hidlins-sync --offline --locked --test sigv4_aws_test_vectors

# Heavier merge-engine property sweep. The properties run at 256 cases under
# the blanket `make test`; this target raises the count for a deeper sweep
# (PRD §11 Risk #2 / impl-plan §8). Override the iteration count via
# PROPTEST_CASES (e.g. `make test-merge-properties PROPTEST_CASES=10000`).
PROPTEST_CASES ?= 4096
test-merge-properties:  ## Run the merge-engine property tests with a heavier case count (PROPTEST_CASES, default 4096).
	PROPTEST_CASES=$(PROPTEST_CASES) $(CARGO) test -p hidlins-sync --offline --locked --test merge_property_tests

# ---------------------------------------------------------------------------
# S3 sync live-wire integration (s3-sync T6.1/T6.2) — needs Docker + mc.
# The MINIO-* tests are #[ignore]-gated so the default `make test` skips
# them; this is the only path that runs them. See
# tools/sync-tests/README.md for the local + CI workflow.
# ---------------------------------------------------------------------------

minio-up:  ## Start the pinned MinIO container for s3-sync integration tests (requires Docker/Podman).
	tools/sync-tests/fixtures/start_minio.sh

minio-down:  ## Stop + remove the MinIO container started by `make minio-up`.
	tools/sync-tests/fixtures/stop_minio.sh

minio-native-up:  ## Start pinned native MinIO binaries (macOS CI fallback; requires minio + mc on PATH).
	tools/sync-tests/fixtures/start_minio_native.sh

minio-native-down:  ## Stop + clean the native MinIO fixture started by `make minio-native-up`.
	tools/sync-tests/fixtures/stop_minio_native.sh

test-s3-integration:  ## Run the #[ignore]-gated MinIO live-wire tests (run `make minio-up` first).
	# Sources the endpoint + test credentials start_minio.sh wrote, then
	# runs ONLY the minio_integration test binary's #[ignore]-gated cases.
	# `--features minio-tests` compiles that binary in the first place: it is
	# `#![cfg(feature = "minio-tests")]`-gated so the blanket `make test-ignored`
	# sweep (non-MinIO `vault-core` CI job) never builds or runs these live-wire
	# cases. This target — the MinIO-provisioned `integration-s3` job — is the
	# only path that turns them on.
	# Serial (`--test-threads=1`): the cases share one container, and
	# `make_bucket.sh` reuses a single `mc` alias, so concurrent bucket
	# setup could race. Each case still uses a uniquely-suffixed bucket.
	@if [ ! -f tools/sync-tests/fixtures/.minio-env ]; then \
		echo "error: MinIO not running — run \`make minio-up\` first." >&2; \
		exit 1; \
	fi
	. tools/sync-tests/fixtures/.minio-env && \
		$(CARGO) test -p hidlins-sync --offline --locked --features minio-tests --test minio_integration -- --ignored --test-threads=1
	# The CLI's own live-wire happy-path (spawns the built binary against the
	# same MinIO): `cli-sync-wiring` T3.2's `#![cfg(feature = "minio-tests")]`
	# + `#[ignore]` case. Kept under this one target so all live-wire S3 cases
	# run via a single command (per plan §7.4).
	. tools/sync-tests/fixtures/.minio-env && \
		$(CARGO) test -p hidlins-cli --offline --locked --features minio-tests --test cli_sync_minio -- --ignored --test-threads=1

test-os-events:  ## Run hidlins-security OS-event integration tests (Phase 5: logind on Linux, IOKit on macOS).
	# Picks the host-appropriate source test. Both files are
	# `#[ignore]`d so the default `make test` skips them.
	#
	# Linux: `--features test-binaries,logind` enables the
	# `logind_helper` binary and compiles `os_events/logind.rs` + its
	# `zbus` dep tree. Requires a logind-enabled host + `dbus-send` +
	# `busctl` to drive the signals from outside the helper.
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
	$(CARGO) test -p hidlins-security --offline --locked --features iokit --lib os_events::macos -- --ignored --test-threads=1
	$(CARGO) test -p hidlins-security --offline --locked --features test-binaries,iokit --test us_052_post_mvp_iokit -- --ignored --test-threads=1
else
	$(CARGO) test -p hidlins-security --offline --locked --features test-binaries,logind --test us_052_post_mvp_logind -- --ignored --test-threads=1
endif

fmt:  ## Auto-format the workspace.
	$(CARGO) fmt --all

fmt-check:  ## Verify formatting without modifying files (CI gate).
	$(CARGO) fmt --all --check

lint:  ## Run clippy with `-D warnings` (CI gate).
	$(CARGO) clippy $(CLIPPY_FLAGS)

lint-fix:  ## Apply clippy auto-fixes where safe.
	$(CARGO) clippy --workspace --all-targets --offline --locked --fix --allow-dirty

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
check: fmt-check lint build test check-macos check-feature-gates  ## fmt-check + lint + build + test + cross/feature-gate checks (Rust-only; no Flutter needed).

# pub-vendor-check runs right after app-deps, BEFORE anything executes
# vendored Dart (flutter test, build_runner): tampered cache bytes should
# fail the integrity gate without ever having run.
app-check: flutter-version-check telemetry-check app-deps pub-vendor-check app-analyze app-fmt-check app-test app-test-bridge app-test-integration api-gen-check  ## Flutter-side gates (requires the Flutter SDK + Rust toolchain).

# build-android (not check-android): the JNI-export symbol assertion and
# the single-verifier-instance gate live in the build recipe, and they are
# the only guards on the Kotlin-facing ABI — the full gate must run them.
# Still skips gracefully on machines without an NDK.
verify: check build-android app-check test-ignored doc deny audit interop interop-entry interop-sync interop-app test-minio-managed boundary-check  ## Full verification gate (Rust + Flutter + interop + managed MinIO; Android build skips without an NDK).

interop:  ## Run vault-core KeePassXC interop shell tests (requires keepassxc-cli).
	$(CARGO) build -p hidlins-core --bin hidlins-test-driver --offline --locked
	HIDLINS_TEST_DRIVER=target/debug/hidlins-test-driver sh tools/interop-tests/us_090_rust_to_kpxc.sh
	HIDLINS_TEST_DRIVER=target/debug/hidlins-test-driver sh tools/interop-tests/us_091_kpxc_to_rust.sh
	HIDLINS_TEST_DRIVER=target/debug/hidlins-test-driver sh tools/interop-tests/us_092_round_trip.sh

interop-sync:  ## Run the s3-sync KeePassXC merge-interop test (US-044; requires keepassxc-cli).
	# Builds the test-only merge driver (gated behind `test-helpers` via
	# required-features, so `make build` never compiles it), then runs the
	# shell harness that opens the merged vault in keepassxc-cli and asserts
	# the collision loser survives as a history entry (NFR-009 / FR-043).
	$(CARGO) build -p hidlins-sync --bin merge-interop-driver --features test-helpers --offline --locked
	HIDLINS_SYNC_MERGE_DRIVER=target/debug/merge-interop-driver sh tools/interop-tests/sync_us-044.sh

interop-entry:  ## Run entry-management KeePassXC interop shell tests (requires keepassxc-cli; oathtool optional).
	$(CARGO) build -p hidlins-core --bin hidlins-test-driver --offline --locked
	@for script in tools/interop-tests/entry_us-010.sh \
	               tools/interop-tests/entry_us-012.sh \
	               tools/interop-tests/entry_us-013.sh \
	               tools/interop-tests/entry_us-014.sh \
	               tools/interop-tests/entry_us-016.sh \
	               tools/interop-tests/entry_us-018.sh; do \
		echo "==> $$script"; \
		HIDLINS_TEST_DRIVER=target/debug/hidlins-test-driver "$$script" || exit $$?; \
	done

interop-app:  ## Run desktop app/API KDBX round-trip against KeePassXC 2.7.12+.
	$(CARGO) build -p hidlins-api --features test-fixtures --bin api-test-driver --offline --locked
	HIDLINS_API_TEST_DRIVER=$(HIDLINS_API_TEST_DRIVER) tools/interop-tests/app_us-09x.sh

bench:  ## Run informational benchmarks.
	$(CARGO) bench -p hidlins-core --bench vault_open --offline --locked

bench-search:  ## Run the informational entry-search benchmark.
	$(CARGO) bench -p hidlins-core --bench bench_search --offline --locked

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
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps --offline

# ---------------------------------------------------------------------------
# Shell completions (FR-064) — re-generated by `make completions`, checked
# into shell-completions/ for packaging + the CI drift gate.
# ---------------------------------------------------------------------------

completions:  ## Re-generate shell-completions/hidlins.{bash,fish} + _hidlins.
	$(CARGO) run --example gen_completions -p hidlins-cli --offline --locked

completions-check:  ## Re-generate completions and fail the build if shell-completions/ drifted (CI gate).
	@before="$$(find shell-completions -type f 2>/dev/null | sort | xargs sha256sum 2>/dev/null | sha256sum)"; \
	$(MAKE) --no-print-directory completions || { \
		echo "error: completion generation failed — the hash comparison below would be vacuous" >&2; exit 1; }; \
	test -s shell-completions/hidlins.bash && \
	test -s shell-completions/_hidlins && \
	test -s shell-completions/hidlins.fish || { \
		echo "error: completion generation did not produce every expected Hidlins completion file" >&2; exit 1; }; \
	after="$$(find shell-completions -type f 2>/dev/null | sort | xargs sha256sum 2>/dev/null | sha256sum)"; \
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
	CARGO_NET_OFFLINE=false $(CARGO) vendor

# ---------------------------------------------------------------------------
# Housekeeping.
# ---------------------------------------------------------------------------

clean:  ## Remove build artifacts (target/).
	$(CARGO) clean

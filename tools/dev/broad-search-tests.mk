# Included after the canonical Makefile; uses its configured Cargo artifact roots.
.PHONY: test-broad-search

test-broad-search: app-deps ncsa-boundary-precheck
	$(MAKE) --no-print-directory -f Makefile test CARGO_FLAGS='-p hidlins-core --test us_011_uuid_stable --test us_014_edit_preserves_history --test us_017_delete_entry --offline --locked'
	$(CARGO_COMPILE) build -p hidlins-api --bin api-test-driver --features test-fixtures --offline --locked
	$(CARGO_COMPILE) build -p hidlins-api --release --offline --locked
	@set -e; fixture="$$(mktemp -d)"; \
	trap 'rm -rf "$$fixture"' EXIT; \
	printf '%s\n' 'performance-integration-master-marker' | \
		"$(CARGO_ARTIFACT_DIR)/debug/api-test-driver" create-search-corpus "$$fixture" performance 5000; \
	cd app; \
	HIDLINS_API_LIB="$(HIDLINS_API_RELEASE_LIB)" \
	HIDLINS_PERF_STATE_DIR="$$fixture" \
	HIDLINS_FLUTTER_VERSION="$$(cat ../.flutter-version)" \
	HIDLINS_GIT_REVISION="$$(git -C .. rev-parse HEAD)" \
	flutter test --reporter expanded --no-pub test_bridge/broad_search_performance_test.dart

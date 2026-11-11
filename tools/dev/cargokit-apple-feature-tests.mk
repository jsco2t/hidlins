.PHONY: characterize test bootstrap

CARGOKIT_APPLE_TOOL_DIR ?= $(CURDIR)/.codex-tmp/cargokit-apple-feature-tests
CARGOKIT_APPLE_DART ?= dart

bootstrap:
	@CARGOKIT_TOOL_TEMP_DIR="$(CARGOKIT_APPLE_TOOL_DIR)" app/rust_builder/cargokit/run_build_tool.sh --help > /dev/null

characterize test: bootstrap
	$(CARGOKIT_APPLE_DART) --packages="$(CARGOKIT_APPLE_TOOL_DIR)/.dart_tool/package_config.json" tools/dev/cargokit_apple_features_test.dart "$(CURDIR)" $@

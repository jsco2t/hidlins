.PHONY: test

CARGOKIT_FEATURE_TOOL_DIR ?= $(CURDIR)/.codex-tmp/cargokit-feature-tests
CARGOKIT_FEATURE_DART ?= dart

test:
	@CARGOKIT_TOOL_TEMP_DIR="$(CARGOKIT_FEATURE_TOOL_DIR)" app/rust_builder/cargokit/run_build_tool.sh --help > /dev/null
	$(CARGOKIT_FEATURE_DART) --packages="$(CARGOKIT_FEATURE_TOOL_DIR)/.dart_tool/package_config.json" tools/dev/cargokit_linux_features_test.dart "$(CURDIR)"

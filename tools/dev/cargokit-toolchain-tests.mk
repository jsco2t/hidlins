.PHONY: test
CARGOKIT_POLICY_TOOL_DIR ?= $(CURDIR)/.codex-tmp/cargokit-toolchain-tests

test:
	@CARGOKIT_TOOL_TEMP_DIR="$(CARGOKIT_POLICY_TOOL_DIR)" app/rust_builder/cargokit/run_build_tool.sh --help > /dev/null
	dart --packages="$(CARGOKIT_POLICY_TOOL_DIR)/.dart_tool/package_config.json" tools/dev/cargokit_toolchain_test.dart "$(CURDIR)"

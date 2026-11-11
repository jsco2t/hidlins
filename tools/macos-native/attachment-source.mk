.PHONY: test

test:
	$(MAKE) --no-print-directory app-deps
	bash tools/macos-native/run_attachment_tests.sh

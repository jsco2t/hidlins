# Run both prepared-parent and standalone ignored-child entry modes.
.PHONY: test

test:
	$(MAKE) --no-print-directory -f Makefile test CARGO_FLAGS='-p hidlins-security --features logind --test logind_startup_shutdown_regressions --offline --locked -- --include-ignored --test-threads=1'

.PHONY: test
test:
	$(MAKE) --no-print-directory -f Makefile test CARGO_FLAGS='-p hidlins-sync --test sync_fixture_paths_regressions --offline --locked'

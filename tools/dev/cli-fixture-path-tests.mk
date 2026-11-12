.PHONY: test
test:
	$(MAKE) --no-print-directory -f Makefile test CARGO_FLAGS='-p hidlins-cli --test cli_fixture_paths_regressions --offline --locked'

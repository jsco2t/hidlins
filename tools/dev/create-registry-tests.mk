.PHONY: test

test:
	$(MAKE) --no-print-directory -f Makefile test CARGO_FLAGS='-p hidlins-api --test create_registry_commit_regressions --offline --locked'

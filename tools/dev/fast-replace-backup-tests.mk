.PHONY: test
test:
	$(MAKE) --no-print-directory test CARGO_FLAGS='-p hidlins-sync --offline --locked --test us_041_fast_replace_open'

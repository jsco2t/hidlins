.PHONY: test
test:
	$(MAKE) --no-print-directory test CARGO_FLAGS='-p hidlins-sync --offline --locked --test preserved_contract_characterization'

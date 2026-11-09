.PHONY: test core api

test: core api

core:
	$(MAKE) --no-print-directory -f Makefile test CARGO_FLAGS='-p hidlins-core --test reserved_custom_field_regressions --offline --locked'

api:
	$(MAKE) --no-print-directory -f Makefile test CARGO_FLAGS='-p hidlins-api --test reserved_custom_field_regressions --offline --locked'

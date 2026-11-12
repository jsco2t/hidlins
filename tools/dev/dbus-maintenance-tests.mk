# Add to the canonical Makefile, retaining configured artifact/cache guards.
.PHONY: test-dbus-characterization

test-dbus-characterization:
	python3 -B tools/dev/test_zbus_maintenance.py
	$(MAKE) --no-print-directory -f Makefile test-os-events test-cli-foreground-lock test-tui-os-events
	$(MAKE) --no-print-directory -f Makefile test CARGO_FLAGS='-p hidlins-core --offline --locked -- --include-ignored --test-threads=1'

.PHONY: test
test:
	# Repetition exposes the native connect/accept scheduling race without sleeps.
	python3 -c 'import subprocess,sys; [subprocess.run(sys.argv[1:], check=True) for _ in range(20)]' $(MAKE) --no-print-directory -f Makefile test CARGO_FLAGS='-p hidlins-sync --features desktop-discovery --lib --offline --locked listener_recovery_regressions'

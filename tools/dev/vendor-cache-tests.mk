.PHONY: test backend

test:
	python3 -B tools/dev/test_vendor_cache.py

# RustBuilder integration is checked by test; backend is independently runnable
# while the preserved existing Cargokit command mocks await their amendment.
backend:
	python3 -B tools/dev/test_vendor_cache.py \
	  CacheTests.test_make_real_library_macro_and_host_build_dependency \
	  CacheTests.test_raw_checksum_tamper_cannot_hide_behind_warm_artifact \
	  CacheTests.test_vendor_symlink_read_failure_stops_before_compile \
	  CacheTests.test_cleanup_failure_keeps_old_generation_and_retries \
	  CacheTests.test_failed_first_party_compile_preserves_warm_dependency_retry \
	  CacheTests.test_stamp_publication_and_partial_cleanup_fail_closed \
	  CacheTests.test_fuzz_runner_forwards_guarded_explicit_target_to_real_cargo \
	  CacheTests.test_dart_kernel_cache_uses_bytes_with_restored_metadata

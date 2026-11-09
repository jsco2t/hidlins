.PHONY: test

test:
	python3 -B tools/android-native/test_release_build_cleanup.py
	python3 -B tools/android-native/test_process_liveness_observer.py

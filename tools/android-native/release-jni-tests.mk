.PHONY: build test self-test

# Canonical Flutter packaging is required: calling assembleRelease directly
# can omit the Dart AOT payload when Flutter's generated inputs are cold.
build:
	$(MAKE) -f Makefile app-build-android CARGO_TARGET_DIR="$(CURDIR)/target"

test:
	python3 -B tools/android-native/release_jni.py test

self-test:
	python3 -B tools/android-native/test_release_jni.py

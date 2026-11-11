include Makefile
.NOTPARALLEL:
.PHONY: host self-test runtime

host: app-deps
	$(CARGO) build -p hidlins-api --offline --locked
	cd app && HIDLINS_API_LIB="$(HIDLINS_API_LIB)" flutter test --no-pub test_bridge/startup_discovery_eligibility_regression_test.dart test/data/bridge_repositories_test.dart

self-test:
	python3 -B tools/android-native/test_startup_discovery.py

runtime:
	python3 -B tools/android-native/startup_discovery.py

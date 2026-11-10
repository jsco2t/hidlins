.PHONY: test
test:
	cd app && flutter test --no-pub --dart-define=INTEGRATION_TEST_SHOULD_REPORT_RESULTS_TO_NATIVE=false test/ios_simulator_semantics_regression_test.dart

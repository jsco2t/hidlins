.PHONY: test-native-publication

# Avoid desktop self-dev-dependency unification. The test-only module is
# present only in this actual native harness. Serialize this filtered suite;
# older native fixtures can clear the entire pending registry.
test-native-publication: build-api-native-test-harness
	@"$(API_NATIVE_UNIT_TEST_BIN)" --list native_publication_regressions | grep -Fq 'lock_waits_for_the_started_native_receiver: test'
	"$(API_NATIVE_UNIT_TEST_BIN)" native_publication_regressions --test-threads=1

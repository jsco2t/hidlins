// Keep the implementation under integration_test/ for discoverability while
// running it with the headless Flutter test runner. Invoking a file directly
// from Flutter's reserved integration_test/ directory would require the
// integration_test SDK package and its webdriver dependency, even though this
// suite opens the native library directly and needs no device driver.
import '../integration_test/real_bridge_lifecycle_test.dart' as suite;

void main() => suite.main();

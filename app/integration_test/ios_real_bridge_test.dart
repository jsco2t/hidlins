import 'package:integration_test/integration_test.dart';

import 'real_bridge_lifecycle_test.dart' as suite;

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  suite.main();
}

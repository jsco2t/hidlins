import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

import '../integration_test/ios_simulator_ui_test.dart' as shipping;

void main() {
  final binding = IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  binding.platformDispatcher.semanticsEnabledTestValue = false;
  var platformRequestScheduled = false;
  void acknowledgePlatformRequest() {
    if (binding.semanticsEnabled && !platformRequestScheduled) {
      platformRequestScheduled = true;
      // Model the real iOS response arriving after framework enablement.
      scheduleMicrotask(() {
        binding.platformDispatcher.semanticsEnabledTestValue = true;
      });
    }
  }

  binding.addSemanticsEnabledListener(acknowledgePlatformRequest);
  tearDownAll(() {
    binding.removeSemanticsEnabledListener(acknowledgePlatformRequest);
    expect(
      binding.debugOutstandingSemanticsHandles,
      1,
      reason:
          'Suite ownership must be released; only platform interest remains',
    );
    binding.platformDispatcher.clearSemanticsEnabledTestValue();
  });

  shipping.main();
  setUpAll(() {
    expect(
      binding.platformDispatcher.semanticsEnabled,
      isTrue,
      reason: 'Platform startup must complete before widget test baselines',
    );
    expect(
      binding.debugOutstandingSemanticsHandles,
      2,
      reason: 'Suite and platform own distinct semantics handles',
    );
  });
}

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/ui/startup_warning_banner.dart';

import 'helpers/feature_test_helpers.dart';

void main() {
  testWidgets('surfaces degraded OS lock detection without raw errors', (
    tester,
  ) async {
    final harness = TestHarness();
    harness.session.startupWarningMessages = [
      'OS lock detection degraded (IOKit): sensitive internal detail',
    ];
    addTearDown(harness.dispose);

    await tester.pumpFeatureWithHarness(
      const StartupWarningBanner(child: Text('Workspace')),
      harness,
    );
    await tester.pumpAndSettle();

    expect(find.text('Security integration degraded'), findsOneWidget);
    expect(
      find.text(
        'Automatic OS screen-lock detection is unavailable. '
        'Idle auto-lock remains active.',
      ),
      findsOneWidget,
    );
    expect(find.textContaining('sensitive internal detail'), findsNothing);
    expect(find.text('Workspace'), findsOneWidget);
  });

  testWidgets('does not reserve banner space when startup is healthy', (
    tester,
  ) async {
    final harness = TestHarness();
    addTearDown(harness.dispose);

    await tester.pumpFeatureWithHarness(
      const StartupWarningBanner(child: Text('Workspace')),
      harness,
    );
    await tester.pumpAndSettle();

    expect(find.text('Security integration degraded'), findsNothing);
    expect(find.text('Workspace'), findsOneWidget);
  });
}

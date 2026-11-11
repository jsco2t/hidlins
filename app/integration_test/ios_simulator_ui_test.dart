import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';

import '../test/helpers/feature_test_helpers.dart';

void main() {
  final binding = IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    // iOS acknowledges framework semantics asynchronously. Establish both
    // suite and platform ownership before testWidgets records its leak baseline.
    final semantics = binding.ensureSemantics();
    addTearDown(semantics.dispose);
    final deadline = DateTime.now().add(const Duration(seconds: 5));
    while (!binding.platformDispatcher.semanticsEnabled) {
      if (DateTime.now().isAfter(deadline)) {
        throw TestFailure('iOS semantics startup did not complete');
      }
      await Future<void>.delayed(const Duration(milliseconds: 10));
    }
  });

  testWidgets('shipping iOS surface reaches every alpha destination', (
    tester,
  ) async {
    final harness = TestHarness();
    registerTestVault(harness);
    harness.session.currentLockState = LockEvent.unlocked;
    addTearDown(harness.dispose);

    await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
    await tester.pumpAndSettle();

    final logicalWidth =
        tester.view.physicalSize.width / tester.view.devicePixelRatio;
    if (logicalWidth < 600) {
      expect(find.byType(NavigationBar), findsOneWidget);
    } else {
      expect(find.byType(NavigationRail), findsOneWidget);
    }

    for (final destination in const [
      'Search',
      'Generator',
      'Sync',
      'Settings',
      'Entries',
    ]) {
      await tester.tap(find.text(destination).last);
      await tester.pumpAndSettle();
      expect(find.text(destination), findsWidgets);
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets('shipping iOS first-run surface exposes all acquisition paths', (
    tester,
  ) async {
    final harness = TestHarness();
    addTearDown(harness.dispose);

    await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
    await tester.pumpAndSettle();

    expect(find.text('Create a new vault'), findsOneWidget);
    expect(find.text('Import a .kdbx file'), findsOneWidget);
    expect(find.text('Import paired vault'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_list.dart';

import '../test/helpers/feature_test_helpers.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('shipping Android surface reaches every alpha destination', (
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

    await tester.tap(find.text('GitHub'));
    await tester.pumpAndSettle();
    expect(find.text('Development account'), findsOneWidget);
    if (logicalWidth < 600) {
      await tester.binding.handlePopRoute();
      await tester.pumpAndSettle();
      expect(find.text('Development account'), findsNothing);
    } else {
      expect(find.text('Development account'), findsOneWidget);
    }
    expect(find.byType(EntryList), findsOneWidget);
  }, timeout: const Timeout(Duration(minutes: 1)));

  testWidgets('shipping Android surface survives rotation and keyboard input', (
    tester,
  ) async {
    final harness = TestHarness();
    registerTestVault(harness);
    addTearDown(harness.dispose);

    await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
    await tester.pumpAndSettle();
    await tester.tap(find.byType(TextField));
    await tester.enterText(find.byType(TextField), 'non-secret-input');
    await tester.pump(const Duration(milliseconds: 500));
    expect(
      tester.widget<EditableText>(find.byType(EditableText)).obscureText,
      isTrue,
    );
    expect(tester.takeException(), isNull);

    final original = tester.view.physicalSize;
    tester.view.physicalSize = Size(original.height, original.width);
    addTearDown(tester.view.resetPhysicalSize);
    await tester.pump(const Duration(milliseconds: 500));
    expect(find.text('Vault locked'), findsOneWidget);
    expect(find.byType(TextField), findsOneWidget);
    expect(tester.takeException(), isNull);
  }, timeout: const Timeout(Duration(minutes: 1)));

  testWidgets('shipping Android first-run exposes every acquisition path', (
    tester,
  ) async {
    final harness = TestHarness();
    addTearDown(harness.dispose);

    await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
    await tester.pumpAndSettle();

    expect(find.text('Create a new vault'), findsOneWidget);
    expect(find.text('Import a .kdbx file'), findsOneWidget);
    expect(find.text('Connect to an existing vault via sync'), findsOneWidget);
    expect(tester.takeException(), isNull);
  }, timeout: const Timeout(Duration(minutes: 1)));
}

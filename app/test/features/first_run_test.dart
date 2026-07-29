import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/features/vaults/first_run_page.dart';
import '../helpers/feature_test_helpers.dart';

void main() {
  group('FirstRunPage', () {
    testWidgets('shows three paths', (tester) async {
      final harness = await tester.pumpFeature(FirstRunPage(onChoice: (_) {}));
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Welcome to Hidlins'), findsOneWidget);
      expect(find.text('Create a new vault'), findsOneWidget);
      expect(
        find.text('Connect to an existing vault via sync'),
        findsOneWidget,
      );
      expect(find.text('Import a .kdbx file'), findsOneWidget);
    });

    testWidgets('create choice fires callback', (tester) async {
      FirstRunChoice? chosen;
      final harness = await tester.pumpFeature(
        FirstRunPage(onChoice: (c) => chosen = c),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Create a new vault'));
      expect(chosen, FirstRunChoice.create);
    });

    testWidgets('connect choice fires callback', (tester) async {
      FirstRunChoice? chosen;
      final harness = await tester.pumpFeature(
        FirstRunPage(onChoice: (c) => chosen = c),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Connect to an existing vault via sync'));
      expect(chosen, FirstRunChoice.connectSync);
    });

    testWidgets('import choice fires callback', (tester) async {
      FirstRunChoice? chosen;
      final harness = await tester.pumpFeature(
        FirstRunPage(onChoice: (c) => chosen = c),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Import a .kdbx file'));
      expect(chosen, FirstRunChoice.import_);
    });
  });
}

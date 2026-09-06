import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/providers/lock_state_provider.dart';

import 'helpers/feature_test_helpers.dart';

void main() {
  group('router redirect', () {
    testWidgets('redirects to /lock when lock stream emits locked', (
      tester,
    ) async {
      final harness = TestHarness();
      registerTestVault(harness);
      addTearDown(harness.dispose);
      await tester.pumpWidget(
        HidlinsApp(
          overrides: [
            ...harness.overrides,
            lockStateProvider.overrideWith(
              (_) => Stream.value(LockEvent.locked),
            ),
          ],
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Vault locked'), findsOneWidget);
    });

    testWidgets('shows entries when unlocked', (tester) async {
      final harness = TestHarness();
      registerTestVault(harness);
      addTearDown(harness.dispose);
      await tester.pumpWidget(
        HidlinsApp(
          overrides: [
            ...harness.overrides,
            lockStateProvider.overrideWith(
              (_) => Stream.value(LockEvent.unlocked),
            ),
          ],
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('Entries'), findsWidgets);
    });

    testWidgets('keyboard-only find, open, copy, and lock flow', (
      tester,
    ) async {
      final harness = TestHarness();
      registerTestVault(harness);
      harness.session.currentLockState = LockEvent.unlocked;
      harness.search.results = [
        SearchHit(entry: harness.entries.tree.entries.first, matches: const []),
      ];
      addTearDown(harness.dispose);
      await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
      await tester.pumpAndSettle();

      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyF);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pumpAndSettle();
      expect(find.text('Search entries…'), findsOneWidget);

      await tester.enterText(find.byType(EditableText).first, 'github');
      await tester.pump(const Duration(milliseconds: 301));
      await tester.pump();
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();
      expect(find.text('GitHub'), findsWidgets);

      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyC);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pump();
      expect(harness.secrets.lastCopyUuid, 'entry-1');

      await tester.sendKeyEvent(LogicalKeyboardKey.keyL);
      await tester.pumpAndSettle();
      expect(harness.session.lockNowCalled, isTrue);
      expect(find.text('Vault locked'), findsOneWidget);
    });

    testWidgets('late sync completion cannot restore a locked workspace', (
      tester,
    ) async {
      final harness = TestHarness();
      registerTestVault(harness);
      harness.session.currentLockState = LockEvent.unlocked;
      addTearDown(harness.dispose);
      await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
      await tester.pumpAndSettle();
      expect(find.text('GitHub'), findsOneWidget);

      harness.session.emitLockState(LockEvent.locked);
      await tester.pumpAndSettle();
      expect(find.text('Vault locked'), findsOneWidget);
      expect(find.text('GitHub'), findsNothing);

      harness.sync.syncController.add(
        const bridge.SyncEvent.done(bridge.SyncOutcomeDto.fastReplaced()),
      );
      await tester.pump(const Duration(milliseconds: 1));
      await tester.pump();

      expect(find.text('Vault locked'), findsOneWidget);
      expect(find.text('GitHub'), findsNothing);
    });
  });
}

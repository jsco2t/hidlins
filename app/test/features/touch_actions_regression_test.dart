import 'package:app/src/app.dart';
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_edit.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../helpers/feature_test_helpers.dart';

Future<TestHarness> _mount(
  WidgetTester tester,
  double width, {
  bool empty = false,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = Size(width, 1000);
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final h = TestHarness();
  registerTestVault(h);
  h.session.currentLockState = LockEvent.unlocked;
  if (empty) {
    h.entries.tree = VaultTree(root: h.entries.tree.root, entries: const []);
  }
  await tester.pumpWidget(HidlinsApp(overrides: h.overrides));
  await tester.pumpAndSettle();
  return h;
}

Future<void> _close(WidgetTester tester, TestHarness h) async {
  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pump();
  h.dispose();
  await tester.pump();
}

void main() {
  for (final width in [390.0, 900.0]) {
    testWidgets(
      'touch New entry works in empty ${width == 390 ? 'phone' : 'tablet'} vault',
      (tester) async {
        final h = await _mount(tester, width, empty: true);
        try {
          final newEntry = find.widgetWithText(FilledButton, 'New entry');
          expect(newEntry, findsOneWidget);
          await tester.tap(newEntry);
          await tester.pumpAndSettle();
          expect(find.byType(EntryEditDialog), findsOneWidget);
          await tester.enterText(
            find.widgetWithText(TextFormField, 'Title'),
            'touch-created',
          );
          await tester.tap(find.widgetWithText(FilledButton, 'Save'));
          await tester.pumpAndSettle();
          expect(h.entries.createEntryCalled, isTrue);
          expect(h.entries.lastCreateGroup, h.entries.tree.root.uuid);
          expect(h.entries.lastDraft?.title, 'touch-created');
        } finally {
          await _close(tester, h);
        }
      },
    );

    testWidgets(
      'touch Lock is reachable on ${width == 390 ? 'phone' : 'tablet'}',
      (tester) async {
        final h = await _mount(tester, width);
        try {
          expect(find.byTooltip('Lock'), findsOneWidget);
          await tester.tap(find.byTooltip('Lock'));
          await tester.pumpAndSettle();
          expect(h.session.lockNowCalled, isTrue);
          expect(find.byType(UnlockScreen), findsOneWidget);
        } finally {
          await _close(tester, h);
        }
      },
    );

    testWidgets(
      'touch long-press Delete supports cancel and confirmed delete on ${width == 390 ? 'phone' : 'tablet'}',
      (tester) async {
        final h = await _mount(tester, width);
        try {
          final entry = h.entries.tree.entries.first;
          await tester.longPress(find.text(entry.title).first);
          await tester.pumpAndSettle();
          final delete = find.widgetWithText(MenuItemButton, 'Delete');
          expect(delete, findsOneWidget);
          await tester.tap(delete);
          await tester.pumpAndSettle();
          await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
          await tester.pumpAndSettle();
          expect(h.entries.deleteEntryCalled, isFalse);
          await tester.longPress(find.text(entry.title).first);
          await tester.pumpAndSettle();
          await tester.tap(find.widgetWithText(MenuItemButton, 'Delete'));
          await tester.pumpAndSettle();
          await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
          await tester.pumpAndSettle();
          expect(h.entries.lastDeleteUuid, entry.uuid);
        } finally {
          await _close(tester, h);
        }
      },
    );
  }

  testWidgets('touch create is disabled while native sync owns mutations', (
    tester,
  ) async {
    final h = await _mount(tester, 390, empty: true);
    try {
      h.sync.syncController.add(
        const bridge.SyncEvent.started(automatic: false),
      );
      await tester.pump();
      await tester.pump();
      final button = find.widgetWithText(FilledButton, 'New entry');
      expect(button, findsOneWidget);
      expect(tester.widget<FilledButton>(button).onPressed, isNull);
      expect(h.entries.createEntryCalled, isFalse);
      h.sync.syncController.add(
        const bridge.SyncEvent.done(bridge.SyncOutcomeDto.alreadyInSync()),
      );
      await tester.pumpAndSettle();
      expect(tester.widget<FilledButton>(button).onPressed, isNotNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('existing desktop keyboard New entry and Lock remain usable', (
    tester,
  ) async {
    final h = await _mount(tester, 1280, empty: true);
    try {
      await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
      await tester.pumpAndSettle();
      expect(find.byType(EntryEditDialog), findsOneWidget);
      await tester.tap(find.byTooltip('Back'));
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.keyL);
      await tester.pumpAndSettle();
      expect(h.session.lockNowCalled, isTrue);
      expect(find.byType(UnlockScreen), findsOneWidget);
    } finally {
      await _close(tester, h);
    }
  });
}

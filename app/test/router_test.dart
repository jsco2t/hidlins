import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/error.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/entries/entry_edit.dart';
import 'package:app/src/router.dart';
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/providers/lock_state_provider.dart';

import 'helpers/feature_test_helpers.dart';

void main() {
  Future<TestHarness> openWorkspace(WidgetTester tester) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(1200, 800);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final h = TestHarness()..session.currentLockState = LockEvent.unlocked;
    registerTestVault(h);
    addTearDown(h.dispose);
    await tester.pumpWidget(HidlinsApp(overrides: h.overrides));
    await tester.pumpAndSettle();
    return h;
  }

  testWidgets('sync results are presented outside the Sync page', (
    tester,
  ) async {
    final h = await openWorkspace(tester);
    h.sync.syncController.add(
      const bridge.SyncEvent.done(bridge.SyncOutcomeDto.fastReplaced()),
    );
    await tester.pumpAndSettle();
    expect(find.text('Remote changes downloaded'), findsOneWidget);
    expect(find.byType(SnackBar), findsOneWidget);
  });

  testWidgets(
    'an off-page conflict has one dialog across navigation and clears on lock',
    (tester) async {
      final h = await openWorkspace(tester);
      final container = ProviderScope.containerOf(
        tester.element(find.byType(EntriesPage)),
      );
      h.sync.syncController.add(
        const bridge.SyncEvent.failed(
          HidlinsApiError.syncConflictUnresolvable(
            backupPath: '/vaults/conflict.kdbx.bak',
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsOneWidget);
      expect(find.textContaining('/vaults/conflict.kdbx.bak'), findsOneWidget);
      container.read(routerProvider).go('/sync');
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsOneWidget);
      expect(find.textContaining('/vaults/conflict.kdbx.bak'), findsOneWidget);
      h.session.emitLockState(LockEvent.locked);
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsNothing);
      expect(find.textContaining('/vaults/conflict.kdbx.bak'), findsNothing);
    },
  );

  testWidgets('offline startup feedback preserves the local workspace', (
    tester,
  ) async {
    final h = await openWorkspace(tester);
    h.sync.syncController.add(const bridge.SyncEvent.started(automatic: true));
    h.sync.syncController.add(
      const bridge.SyncEvent.failed(HidlinsApiError.syncOffline()),
    );
    await tester.pumpAndSettle();
    expect(find.text('GitHub'), findsOneWidget);
    expect(find.byType(MaterialBanner), findsOneWidget);
    await tester.tap(find.text('Close'));
    await tester.pumpAndSettle();
    expect(find.byType(MaterialBanner), findsNothing);
    h.session.emitLockState(LockEvent.locked);
    await tester.pumpAndSettle();
    expect(find.byType(MaterialBanner), findsNothing);
  });

  testWidgets('copy failure feedback does not expose exception contents', (
    tester,
  ) async {
    final h = await openWorkspace(tester);
    h.secrets.copyError = Exception('copy-secret-canary');
    await tester.tapAt(
      tester.getCenter(find.text('GitHub')),
      buttons: kSecondaryMouseButton,
    );
    await tester.pumpAndSettle();
    await tester.tap(
      find.descendant(
        of: find.byType(MenuItemButton),
        matching: find.text('Password'),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('Copy failed'), findsOneWidget);
    expect(find.textContaining('copy-secret-canary'), findsNothing);
  });

  testWidgets('a delayed edit lookup cannot open after sync starts', (
    tester,
  ) async {
    final h = await openWorkspace(tester);
    final original = await h.entries.entryDetail('entry-1');
    await tester.tapAt(
      tester.getCenter(find.text('GitHub')),
      buttons: kSecondaryMouseButton,
    );
    await tester.pumpAndSettle();
    final gate = h.entries.detailCompleter = Completer<EntryDetail>();
    await tester.tap(find.text('Edit').last);
    await tester.pump();
    h.sync.syncController.add(const bridge.SyncEvent.started(automatic: true));
    await tester.pump();
    gate.complete(original);
    await tester.pump(const Duration(milliseconds: 300));
    expect(find.byType(EntryEditDialog), findsNothing);
  });

  testWidgets('delete confirmation cannot mutate after sync starts', (
    tester,
  ) async {
    final h = await openWorkspace(tester);
    await tester.tapAt(
      tester.getCenter(find.text('GitHub')),
      buttons: kSecondaryMouseButton,
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('Delete').last);
    await tester.pumpAndSettle();
    h.sync.syncController.add(const bridge.SyncEvent.started(automatic: true));
    await tester.pump();
    await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
    await tester.pump(const Duration(milliseconds: 300));
    expect(h.entries.deleteEntryCalled, isFalse);
  });

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

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_edit.dart';
import 'package:app/src/ui/shortcuts.dart';
import 'helpers/feature_test_helpers.dart';

void main() {
  testWidgets('Ctrl-F fires the search intent', (tester) async {
    final counts = _ShortcutCounts();
    final probe = _ShortcutProbe(counts: counts);
    await tester.pumpWidget(probe);

    await _pressModified(
      tester,
      LogicalKeyboardKey.controlLeft,
      LogicalKeyboardKey.keyF,
    );

    expect(counts.search, 1);
  });

  testWidgets('Command-F fires the search intent', (tester) async {
    final counts = _ShortcutCounts();
    final probe = _ShortcutProbe(counts: counts);
    await tester.pumpWidget(probe);

    await _pressModified(
      tester,
      LogicalKeyboardKey.metaLeft,
      LogicalKeyboardKey.keyF,
    );

    expect(counts.search, 1);
  });

  testWidgets('N fires the new-entry intent', (tester) async {
    final counts = _ShortcutCounts();
    final probe = _ShortcutProbe(counts: counts);
    await tester.pumpWidget(probe);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyN);

    expect(counts.newEntry, 1);
  });

  testWidgets('L fires the lock intent', (tester) async {
    final counts = _ShortcutCounts();
    final probe = _ShortcutProbe(counts: counts);
    await tester.pumpWidget(probe);

    await tester.sendKeyEvent(LogicalKeyboardKey.keyL);

    expect(counts.lock, 1);
  });

  testWidgets('Ctrl-C fires copy-password-on-selection', (tester) async {
    final counts = _ShortcutCounts();
    final probe = _ShortcutProbe(counts: counts);
    await tester.pumpWidget(probe);

    await _pressModified(
      tester,
      LogicalKeyboardKey.controlLeft,
      LogicalKeyboardKey.keyC,
    );

    expect(counts.copy, 1);
  });

  testWidgets('Escape fires dismiss', (tester) async {
    final counts = _ShortcutCounts();
    final probe = _ShortcutProbe(counts: counts);
    await tester.pumpWidget(probe);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);

    expect(counts.dismiss, 1);
  });

  testWidgets('plain-letter shortcuts yield to editable text', (tester) async {
    final counts = _ShortcutCounts();
    final probe = _ShortcutProbe(counts: counts, showTextField: true);
    await tester.pumpWidget(probe);
    await tester.tap(find.byType(TextField));
    await tester.pump();

    await tester.sendKeyEvent(LogicalKeyboardKey.keyL);

    expect(counts.lock, 0);
  });

  group('real app wiring', () {
    testWidgets('Ctrl-F opens and focuses the search surface', (tester) async {
      final harness = await _pumpRealApp(tester);
      addTearDown(harness.dispose);

      await _pressModified(
        tester,
        LogicalKeyboardKey.controlLeft,
        LogicalKeyboardKey.keyF,
      );
      await tester.pumpAndSettle();

      expect(find.byType(SearchBar), findsOneWidget);
      expect(
        tester
            .widget<EditableText>(find.byType(EditableText))
            .focusNode
            .hasFocus,
        isTrue,
      );
    });

    testWidgets('N opens create-entry for the selected root group', (
      tester,
    ) async {
      final harness = await _pumpRealApp(tester);
      addTearDown(harness.dispose);

      await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
      await tester.pumpAndSettle();

      expect(find.byType(EntryEditDialog), findsOneWidget);
    });

    testWidgets('L locks the active session and returns to lock screen', (
      tester,
    ) async {
      final harness = await _pumpRealApp(tester);
      addTearDown(harness.dispose);

      await tester.sendKeyEvent(LogicalKeyboardKey.keyL);
      await tester.pumpAndSettle();

      expect(harness.session.lockNowCalled, isTrue);
      expect(find.text('Vault locked'), findsOneWidget);
    });

    for (final modifier in [
      LogicalKeyboardKey.controlLeft,
      LogicalKeyboardKey.metaLeft,
    ]) {
      testWidgets('${modifier.keyLabel}-C copies the selected password', (
        tester,
      ) async {
        final harness = await _pumpRealApp(tester);
        addTearDown(harness.dispose);
        await tester.tap(find.text('GitHub').first);
        await tester.pump();

        await _pressModified(tester, modifier, LogicalKeyboardKey.keyC);
        await tester.pump();

        expect(harness.secrets.copyCalled, isTrue);
        expect(harness.secrets.lastCopyUuid, 'entry-1');
        expect(harness.secrets.lastCopyField, CopyField.password);
      });
    }

    testWidgets('Escape dismisses an actual create-entry route', (
      tester,
    ) async {
      final harness = await _pumpRealApp(tester);
      addTearDown(harness.dispose);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
      await tester.pumpAndSettle();
      expect(find.byType(EntryEditDialog), findsOneWidget);

      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();

      expect(find.byType(EntryEditDialog), findsNothing);
    });
  });
}

Future<TestHarness> _pumpRealApp(WidgetTester tester) async {
  final harness = TestHarness();
  harness.session.currentLockState = LockEvent.unlocked;
  tester.view.physicalSize = const Size(1400, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
  await tester.pumpAndSettle();
  expect(find.text('Entries'), findsWidgets);
  return harness;
}

Future<void> _pressModified(
  WidgetTester tester,
  LogicalKeyboardKey modifier,
  LogicalKeyboardKey key,
) async {
  await tester.sendKeyDownEvent(modifier);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(modifier);
}

class _ShortcutProbe extends StatelessWidget {
  const _ShortcutProbe({required this.counts, this.showTextField = false});

  final _ShortcutCounts counts;
  final bool showTextField;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: HidlinsShortcuts(
        onSearch: () => counts.search++,
        onLock: () => counts.lock++,
        onDismiss: () => counts.dismiss++,
        child: Actions(
          actions: {
            NewEntryIntent: NonEditingCallbackAction<NewEntryIntent>(
              onInvoke: (_) {
                counts.newEntry++;
                return null;
              },
            ),
            CopySelectedPasswordIntent:
                NonEditingCallbackAction<CopySelectedPasswordIntent>(
                  onInvoke: (_) {
                    counts.copy++;
                    return null;
                  },
                ),
          },
          child: Scaffold(
            body: Focus(
              autofocus: true,
              child: showTextField
                  ? const TextField()
                  : const Text('shortcut probe'),
            ),
          ),
        ),
      ),
    );
  }
}

class _ShortcutCounts {
  int search = 0;
  int newEntry = 0;
  int lock = 0;
  int copy = 0;
  int dismiss = 0;
}

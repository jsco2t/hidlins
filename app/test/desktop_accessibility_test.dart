import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/generator/generator_page.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/features/settings/settings_page.dart';

import 'helpers/feature_test_helpers.dart';

void main() {
  testWidgets('desktop alpha screens support 200 percent text scaling', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(800, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final harness = TestHarness();
    harness.session.vaults = const [
      VaultSummary(
        name: 'Personal',
        path: '/vaults/personal.kdbx',
        hasKeyfile: false,
        hasSync: true,
      ),
    ];
    addTearDown(harness.dispose);

    for (final screen in const <Widget>[
      UnlockScreen(),
      EntriesPage(initialUuid: 'entry-1'),
      GeneratorPage(),
      SettingsPage(),
    ]) {
      await tester.pumpFeatureWithHarness(
        MediaQuery(
          data: const MediaQueryData(
            size: Size(800, 900),
            textScaler: TextScaler.linear(2),
            disableAnimations: true,
          ),
          child: screen,
        ),
        harness,
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));
      expect(
        tester.takeException(),
        isNull,
        reason: '${screen.runtimeType} must not overflow at 200% scaling',
      );
    }
  });

  testWidgets('unlock controls expose labels, tooltips, and keyboard order', (
    tester,
  ) async {
    final harness = TestHarness();
    harness.session.vaults = const [
      VaultSummary(
        name: 'Personal',
        path: '/vaults/personal.kdbx',
        hasKeyfile: false,
        hasSync: false,
      ),
    ];
    addTearDown(harness.dispose);
    final semantics = tester.ensureSemantics();

    await tester.pumpFeatureWithHarness(const UnlockScreen(), harness);
    await tester.pumpAndSettle();

    expect(find.bySemanticsLabel(RegExp('Master password')), findsWidgets);
    expect(
      tester
          .widgetList<IconButton>(find.byType(IconButton))
          .map((button) => button.tooltip),
      contains('Reveal'),
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    expect(
      _focusIsWithin(find.byType(DropdownButtonFormField<String>)),
      isTrue,
    );

    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    expect(_focusIsWithin(find.byType(TextField)), isTrue);
    semantics.dispose();
  });
}

bool _focusIsWithin(Finder finder) {
  final focusedContext = FocusManager.instance.primaryFocus?.context;
  if (focusedContext is! Element) return false;
  final targets = finder.evaluate().toSet();
  if (targets.contains(focusedContext)) return true;
  var found = false;
  focusedContext.visitAncestorElements((ancestor) {
    if (targets.contains(ancestor)) found = true;
    return !found;
  });
  return found;
}

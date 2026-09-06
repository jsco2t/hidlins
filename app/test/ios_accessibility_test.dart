import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/entries/entry_detail.dart';
import 'package:app/src/features/generator/generator_page.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/features/settings/settings_page.dart';

import 'helpers/feature_test_helpers.dart';

void main() {
  setUp(() {
    debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
    addTearDown(() => debugDefaultTargetPlatformOverride = null);
  });

  testWidgets(
    'iPhone and iPad layouts keep every alpha destination reachable',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(390, 844);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);

      final harness = TestHarness();
      registerTestVault(harness);
      harness.session.currentLockState = LockEvent.unlocked;
      addTearDown(harness.dispose);

      await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
      await tester.pumpAndSettle();

      expect(find.byType(NavigationBar), findsOneWidget);
      expect(
        tester.takeException(),
        isNull,
        reason: 'compact entries route must lay out without overflow',
      );
      for (final destination in <(String, int)>[
        ('Search', 1),
        ('Generator', 2),
        ('Sync', 3),
        ('Settings', 4),
        ('Entries', 0),
      ]) {
        await tester.tap(find.text(destination.$1).last);
        await tester.pumpAndSettle();
        expect(
          tester
              .widget<NavigationBar>(find.byType(NavigationBar))
              .selectedIndex,
          destination.$2,
        );
        expect(
          tester.takeException(),
          isNull,
          reason: '${destination.$1} compact route must not overflow',
        );
      }

      tester.view.physicalSize = const Size(1024, 1366);
      await tester.pumpAndSettle();
      expect(find.byType(NavigationRail), findsOneWidget);
      expect(find.byType(NavigationBar), findsNothing);
      expect(
        tester.takeException(),
        isNull,
        reason: 'portrait iPad route must not overflow',
      );

      tester.view.physicalSize = const Size(1366, 1024);
      await tester.pumpAndSettle();
      expect(find.byType(NavigationRail), findsOneWidget);
      expect(tester.takeException(), isNull);
      debugDefaultTargetPlatformOverride = null;
    },
  );

  testWidgets('iOS alpha screens support 200 percent text scaling', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(430, 932);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final harness = TestHarness();
    registerTestVault(harness);
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
            size: Size(430, 932),
            textScaler: TextScaler.linear(2),
            disableAnimations: true,
          ),
          child: screen,
        ),
        harness,
      );
      await tester.pump(const Duration(milliseconds: 300));
      expect(
        tester.takeException(),
        isNull,
        reason: '${screen.runtimeType} must not overflow at 200% scaling',
      );
    }
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('iOS semantics expose a heading and conceal unrevealed secrets', (
    tester,
  ) async {
    final harness = TestHarness();
    addTearDown(harness.dispose);
    final semantics = tester.ensureSemantics();

    await tester.pumpFeatureWithHarness(
      const EntryDetailPane(uuid: 'entry-1'),
      harness,
    );
    await tester.pumpAndSettle();

    expect(tester.getSemantics(find.text('GitHub')).headingLevel, 1);
    expect(find.bySemanticsLabel('revealed-secret'), findsNothing);
    expect(
      find.bySemanticsLabel(RegExp(r'•{4,}')),
      findsNothing,
      reason: 'concealed values must not be announced as bullet strings',
    );
    semantics.dispose();
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('iOS controls have labels and 44 point tap targets', (
    tester,
  ) async {
    try {
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final harness = TestHarness();
      registerTestVault(harness);
      harness.session.currentLockState = LockEvent.unlocked;
      addTearDown(harness.dispose);

      for (final size in const [Size(430, 932), Size(1024, 1366)]) {
        tester.view.physicalSize = size;
        await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
        await tester.pumpAndSettle();

        await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
        await expectLater(tester, meetsGuideline(iOSTapTargetGuideline));
      }
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });

  testWidgets('iOS external keyboard focus follows the unlock form', (
    tester,
  ) async {
    try {
      final harness = TestHarness();
      registerTestVault(harness);
      addTearDown(harness.dispose);

      await tester.pumpFeatureWithHarness(const UnlockScreen(), harness);
      await tester.pumpAndSettle();

      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(
        _focusIsWithin(find.byType(DropdownButtonFormField<String>)),
        isTrue,
      );

      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      expect(_focusIsWithin(find.byType(TextField)), isTrue);
    } finally {
      debugDefaultTargetPlatformOverride = null;
    }
  });

  testWidgets('compact entry detail uses the native back affordance', (
    tester,
  ) async {
    tester.view.physicalSize = const Size(390, 844);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final harness = TestHarness();
    addTearDown(harness.dispose);
    await tester.pumpFeatureWithHarness(const EntriesPage(), harness);
    await tester.pumpAndSettle();

    await tester.tap(find.text('GitHub'));
    await tester.pumpAndSettle();
    expect(find.byType(BackButton), findsOneWidget);
    expect(find.text('Development account'), findsOneWidget);

    await tester.tap(find.byType(BackButton));
    await tester.pumpAndSettle();
    expect(find.text('Bank of Test'), findsOneWidget);
    debugDefaultTargetPlatformOverride = null;
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

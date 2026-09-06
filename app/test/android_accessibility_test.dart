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
    debugDefaultTargetPlatformOverride = TargetPlatform.android;
    addTearDown(() => debugDefaultTargetPlatformOverride = null);
  });

  testWidgets(
    'Android phone and tablet keep every alpha destination reachable',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(412, 915);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);

      final harness = TestHarness();
      registerTestVault(harness);
      harness.session.currentLockState = LockEvent.unlocked;
      addTearDown(harness.dispose);

      await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
      await tester.pumpAndSettle();
      expect(find.byType(NavigationBar), findsOneWidget);

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

      tester.view.physicalSize = const Size(1280, 800);
      await tester.pumpAndSettle();
      expect(find.byType(NavigationRail), findsOneWidget);
      expect(find.byType(NavigationBar), findsNothing);
      expect(tester.takeException(), isNull);
      debugDefaultTargetPlatformOverride = null;
    },
  );

  testWidgets('Android alpha screens support 200 percent text and IME resize', (
    tester,
  ) async {
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
            size: Size(412, 500),
            textScaler: TextScaler.linear(2),
            viewInsets: EdgeInsets.only(bottom: 250),
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
        reason: '${screen.runtimeType} must not overflow with scaling and IME',
      );
    }
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets(
    'Android semantics conceal secrets and controls meet guidelines',
    (tester) async {
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(412, 915);
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
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
      await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
      await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
      semantics.dispose();
      debugDefaultTargetPlatformOverride = null;
    },
  );

  testWidgets('Android hardware keyboard focus follows the unlock form', (
    tester,
  ) async {
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
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('Android system back returns compact detail to its entry list', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(412, 915);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final harness = TestHarness();
    addTearDown(harness.dispose);

    await tester.pumpFeatureWithHarness(const EntriesPage(), harness);
    await tester.pumpAndSettle();
    await tester.tap(find.text('GitHub'));
    await tester.pumpAndSettle();
    expect(find.text('Development account'), findsOneWidget);

    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(find.text('Bank of Test'), findsOneWidget);
    expect(find.text('Development account'), findsNothing);
    debugDefaultTargetPlatformOverride = null;
  });

  testWidgets('Android routed system back returns detail to its entry list', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(412, 915);
    addTearDown(tester.view.resetDevicePixelRatio);
    addTearDown(tester.view.resetPhysicalSize);
    final harness = TestHarness();
    registerTestVault(harness);
    harness.session.currentLockState = LockEvent.unlocked;
    addTearDown(harness.dispose);

    await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
    await tester.pumpAndSettle();
    await tester.tap(find.text('GitHub'));
    await tester.pumpAndSettle();
    expect(find.text('Development account'), findsOneWidget);

    await tester.binding.handlePopRoute();
    await tester.pumpAndSettle();
    expect(find.text('Bank of Test'), findsOneWidget);
    expect(find.text('Development account'), findsNothing);
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

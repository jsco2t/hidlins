import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/generator/generator_page.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/features/settings/settings_page.dart';
import 'helpers/feature_test_helpers.dart';

void main() {
  for (final surface in <String, Widget Function(TestHarness)>{
    'unlock': (_) => const UnlockScreen(),
    'entries': (_) => const EntriesPage(initialUuid: 'entry-1'),
    'generator': (_) => const GeneratorPage(),
    'settings': (_) => const SettingsPage(),
  }.entries) {
    testWidgets('${surface.key} has labeled desktop tap targets', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(1200, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final semantics = tester.ensureSemantics();

      final harness = TestHarness();
      harness.session.currentLockState = LockEvent.unlocked;
      harness.session.vaults = const [
        VaultSummary(
          name: 'Personal',
          path: '/vaults/personal.kdbx',
          hasKeyfile: false,
          hasSync: false,
        ),
      ];
      await tester.pumpFeature(surface.value(harness), testHarness: harness);
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      try {
        await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
        await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
      } finally {
        semantics.dispose();
      }
    });
  }
}

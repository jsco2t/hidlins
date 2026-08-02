import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/generator/generator_page.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/features/settings/settings_page.dart';
import 'helpers/feature_test_helpers.dart';
import 'helpers/golden.dart';

void main() {
  const sizes = <String, Size>{
    'compact': goldenSizeCompact,
    'medium': goldenSizeMedium,
    'expanded': goldenSizeExpanded,
  };
  const brightnesses = <String, Brightness>{
    'light': Brightness.light,
    'dark': Brightness.dark,
  };

  for (final brightness in brightnesses.entries) {
    for (final size in sizes.entries) {
      testWidgets('lock ${brightness.key} ${size.key}', (tester) async {
        final harness = _goldenHarness(locked: true);
        addTearDown(harness.dispose);
        await expectGolden(
          tester,
          ProviderScope(
            overrides: harness.overrides,
            child: const UnlockScreen(),
          ),
          name: 'lock_${brightness.key}_${size.key}',
          size: size.value,
          brightness: brightness.value,
        );
      });

      testWidgets('list detail ${brightness.key} ${size.key}', (tester) async {
        final harness = _goldenHarness();
        addTearDown(harness.dispose);
        await expectGolden(
          tester,
          ProviderScope(
            overrides: harness.overrides,
            child: const EntriesPage(initialUuid: 'entry-1'),
          ),
          name: 'list_detail_${brightness.key}_${size.key}',
          size: size.value,
          brightness: brightness.value,
        );
      });

      testWidgets('generator ${brightness.key} ${size.key}', (tester) async {
        final harness = _goldenHarness();
        addTearDown(harness.dispose);
        await expectGolden(
          tester,
          ProviderScope(
            overrides: harness.overrides,
            child: const GeneratorPage(),
          ),
          name: 'generator_${brightness.key}_${size.key}',
          size: size.value,
          brightness: brightness.value,
        );
      });

      testWidgets('settings ${brightness.key} ${size.key}', (tester) async {
        final harness = _goldenHarness();
        addTearDown(harness.dispose);
        await expectGolden(
          tester,
          ProviderScope(
            overrides: harness.overrides,
            child: const SettingsPage(),
          ),
          name: 'settings_${brightness.key}_${size.key}',
          size: size.value,
          brightness: brightness.value,
        );
      });
    }
  }
}

TestHarness _goldenHarness({bool locked = false}) {
  final harness = TestHarness();
  harness.session.currentLockState = locked
      ? LockEvent.locked
      : LockEvent.unlocked;
  harness.session.vaults = const [
    VaultSummary(
      name: 'Personal',
      path: '/vaults/personal.kdbx',
      hasKeyfile: false,
      hasSync: true,
    ),
  ];
  harness.totp.codes['entry-1'] = const TotpCode(
    code: '123456',
    remainingSecs: 17,
    period: 30,
  );
  harness.sync.status = const SyncStatusDto(
    configured: true,
    inFlight: false,
    lastOutcome: SyncOutcomeDto.alreadyInSync,
  );
  harness.prefs.prefs = const UiPrefs(themeMode: 'system');
  return harness;
}

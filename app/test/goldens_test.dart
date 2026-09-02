import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/generator/generator_page.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/features/settings/settings_page.dart';
import 'package:app/src/features/sync/sync_page.dart';
import 'package:app/src/features/vaults/first_run_page.dart';
import 'package:app/src/ui/adaptive_scaffold.dart';

import 'helpers/feature_test_helpers.dart';
import 'helpers/golden.dart';

enum _AlphaScene { lock, entries, generator, settings }

void main() {
  final layouts = <(String, Size)>[
    ('compact', goldenSizeCompact),
    ('medium', goldenSizeMedium),
    ('expanded', goldenSizeExpanded),
  ];
  final themes = <(String, Brightness)>[
    ('light', Brightness.light),
    ('dark', Brightness.dark),
  ];

  group('24-screen desktop alpha matrix', () {
    for (final scene in _AlphaScene.values) {
      for (final (themeName, brightness) in themes) {
        for (final (layoutName, size) in layouts) {
          testWidgets('${scene.name} $themeName $layoutName', (tester) async {
            final harness = TestHarness();
            addTearDown(harness.dispose);
            harness.session.vaults = const [
              VaultSummary(
                name: 'Personal',
                path: '/vaults/personal.kdbx',
                hasKeyfile: false,
                hasSync: true,
              ),
            ];

            await expectGolden(
              tester,
              ProviderScope(overrides: harness.overrides, child: _scene(scene)),
              name: '${scene.name}_${themeName}_$layoutName',
              size: size,
              brightness: brightness,
            );
          });
        }
      }
    }
  });

  group('additional brand and sync states', () {
    testWidgets('branded first run light compact', (tester) async {
      await expectGolden(
        tester,
        FirstRunPage(onChoice: (_) {}),
        name: 'first_run_light_compact',
        size: goldenSizeCompact,
      );
    });

    for (final variant in <(String, SyncStatusDto, Size, Brightness)>[
      (
        'sync_config_light_compact',
        const SyncStatusDto(configured: false, inFlight: false),
        goldenSizeCompact,
        Brightness.light,
      ),
      (
        'sync_progress_dark_medium',
        const SyncStatusDto(configured: true, inFlight: true),
        goldenSizeMedium,
        Brightness.dark,
      ),
      (
        'sync_result_light_expanded',
        const SyncStatusDto(
          configured: true,
          inFlight: false,
          lastOutcome: SyncOutcomeDto.merged,
        ),
        goldenSizeExpanded,
        Brightness.light,
      ),
    ]) {
      testWidgets(variant.$1, (tester) async {
        final harness = TestHarness()..sync.status = variant.$2;
        addTearDown(harness.dispose);
        await expectGolden(
          tester,
          ProviderScope(overrides: harness.overrides, child: const SyncPage()),
          name: variant.$1,
          size: variant.$3,
          brightness: variant.$4,
          settle: variant.$1 != 'sync_progress_dark_medium',
        );
      });
    }
  });
}

Widget _scene(_AlphaScene scene) {
  return switch (scene) {
    _AlphaScene.lock => const UnlockScreen(),
    _AlphaScene.entries => const AdaptiveScaffold(
      selectedIndex: 0,
      onDestinationSelected: _ignoreDestination,
      body: EntriesPage(initialUuid: 'entry-1'),
    ),
    _AlphaScene.generator => const AdaptiveScaffold(
      selectedIndex: 2,
      onDestinationSelected: _ignoreDestination,
      body: GeneratorPage(),
    ),
    _AlphaScene.settings => const AdaptiveScaffold(
      selectedIndex: 4,
      onDestinationSelected: _ignoreDestination,
      body: SettingsPage(),
    ),
  };
}

void _ignoreDestination(int _) {}

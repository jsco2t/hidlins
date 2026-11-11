import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/providers/lock_state_provider.dart';

import 'helpers/feature_test_helpers.dart';

void main() {
  test('first-party Dart stays on one standalone Material type system', () {
    final forbiddenImports = [
      'package:flutter/'
          'material.dart',
      'package:cupertino_'
          'ui/',
    ];

    for (final root in ['lib', 'test', 'test_bridge']) {
      for (final entity in Directory(root).listSync(recursive: true)) {
        if (entity is! File || !entity.path.endsWith('.dart')) continue;
        final contents = entity.readAsStringSync();
        for (final forbiddenImport in forbiddenImports) {
          expect(
            contents,
            isNot(contains(forbiddenImport)),
            reason: '${entity.path} imports $forbiddenImport',
          );
        }
      }
    }
  });

  test('desktop alpha user-facing strings flow through localization', () {
    final sourceByPath = <String, String>{
      'lib/src/features/entries/group_tree.dart': 'All entries',
      'lib/src/features/entries/entry_edit.dart': "'Required'",
      'lib/src/router.dart': 'Page Not Found',
    };

    for (final entry in sourceByPath.entries) {
      final source = File(entry.key).readAsStringSync();
      expect(
        source,
        isNot(contains(entry.value)),
        reason: '${entry.key} must use AppLocalizations',
      );
    }
    expect(
      File('lib/src/router.dart').readAsStringSync(),
      isNot(contains('state.error?.toString()')),
      reason: 'router errors must not expose raw internal details',
    );
  });

  testWidgets(
    'standalone Material shell preserves router theme and localization',
    (tester) async {
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

      expect(find.byType(MaterialApp), findsOneWidget);

      final lockedContext = tester.element(find.text('Vault locked'));
      expect(Theme.of(lockedContext).colorScheme.primary, isNotNull);
      expect(MaterialLocalizations.of(lockedContext).okButtonLabel, isNotEmpty);

      final navigators = tester.widgetList<Navigator>(find.byType(Navigator));
      expect(navigators, isNotEmpty);
      for (final navigator in navigators) {
        expect(
          navigator.pages,
          everyElement(isA<MaterialPage<dynamic>>()),
          reason:
              'go_router must not fall back to legacy or no-transition pages',
        );
      }
    },
  );
}

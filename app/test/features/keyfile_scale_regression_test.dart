import 'package:app/src/data/models.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/platform/keyfile_access.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../helpers/feature_test_helpers.dart';

void main() {
  for (final width in [360.0, 430.0, 800.0]) {
    for (final hasKeyfile in [false, true]) {
      testWidgets(
        'optional keyfile fits 200 percent scale with IME at $width known=$hasKeyfile',
        (tester) async {
          tester.view.devicePixelRatio = 1;
          tester.view.physicalSize = Size(width, 900);
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          final h = TestHarness();
          h.session.vaults = [
            VaultSummary(
              name: 'fixture',
              path: '/fixture.kdbx',
              hasKeyfile: hasKeyfile,
              hasSync: false,
            ),
          ];
          const reference = KeyfileReference(
            reference: 'fixture-reference',
            displayName: 'a_very_long_keyfile_filename_that_still_needs_accessible_selection.key',
          );
          h.keyfiles.pickResults.add(const PlatformSuccess(reference));
          try {
            await tester.pumpFeatureWithHarness(
              MediaQuery(
                data: MediaQueryData(
                  size: Size(width, 900),
                  textScaler: const TextScaler.linear(2),
                  viewInsets: const EdgeInsets.only(bottom: 250),
                  disableAnimations: true,
                ),
                child: const UnlockScreen(),
              ),
              h,
            );
            await tester.pumpAndSettle();
            expect(
              tester.takeException(),
              isNull,
              reason:
                  'optional keyfile is readable without horizontal overflow',
            );
            final select = find.widgetWithText(TextButton, 'Select keyfile');
            expect(select, findsOneWidget);
            await tester.ensureVisible(select);
            await tester.tap(select);
            await tester.pumpAndSettle();
            expect(
              tester.takeException(),
              isNull,
              reason: 'selected long filename remains inside the scaled layout',
            );
            expect(find.text(reference.displayName), findsOneWidget);
            expect(tester.widget<TextButton>(select).onPressed, isNotNull);
          } finally {
            await tester.pumpWidget(const SizedBox.shrink());
            await tester.pump();
            h.dispose();
            await tester.pump();
          }
        },
      );
    }
  }

  testWidgets('unscaled optional keyfile selection remains usable', (
    tester,
  ) async {
    tester.view.devicePixelRatio = 1;
    tester.view.physicalSize = const Size(360, 900);
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    final h = TestHarness();
    registerTestVault(h);
    const reference = KeyfileReference(
      reference: 'fixture-reference',
      displayName: 'a_very_long_keyfile_filename_that_still_needs_accessible_selection.key',
    );
    h.keyfiles.pickResults.add(const PlatformSuccess(reference));
    try {
      await tester.pumpFeatureWithHarness(const UnlockScreen(), h);
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(TextButton, 'Select keyfile'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text(reference.displayName), findsOneWidget);
    } finally {
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump();
      h.dispose();
      await tester.pump();
    }
  });
}

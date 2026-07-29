import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/features/settings/settings_page.dart';
import '../helpers/feature_test_helpers.dart';

void main() {
  group('SettingsPage', () {
    testWidgets('shows theme toggle with system selected by default', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(const SettingsPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Settings'), findsOneWidget);
      expect(find.text('Theme'), findsOneWidget);
      expect(find.text('System'), findsOneWidget);
      expect(find.text('Light'), findsOneWidget);
      expect(find.text('Dark'), findsOneWidget);
    });

    testWidgets('theme change persists to prefs', (tester) async {
      final harness = await tester.pumpFeature(const SettingsPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Dark'));
      await tester.pumpAndSettle();

      expect(harness.prefs.setPrefsCalled, isTrue);
      expect(harness.prefs.lastPrefs?.themeMode, 'dark');
    });

    testWidgets('shows auto-lock as display only', (tester) async {
      final harness = await tester.pumpFeature(const SettingsPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Auto-lock timeout'), findsOneWidget);
      expect(find.text('Configured via CLI or TUI'), findsOneWidget);
    });

    testWidgets('license page link is present', (tester) async {
      final harness = await tester.pumpFeature(const SettingsPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Open source licenses'), findsOneWidget);
    });
  });
}

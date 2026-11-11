import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/features/settings/settings_page.dart';

import '../helpers/feature_test_helpers.dart';

void main() {
  testWidgets('license page includes the exact EFF asset notice once', (
    tester,
  ) async {
    final harness = await tester.pumpFeature(const SettingsPage());
    addTearDown(harness.dispose);
    await tester.pumpAndSettle();

    for (var visit = 0; visit < 2; visit++) {
      await tester.tap(find.text('Open source licenses'));
      await tester.pumpAndSettle();
      final entries = await tester.runAsync(
        () => LicenseRegistry.licenses.toList(),
      );
      final eff = entries!
          .where((entry) => entry.packages.contains('EFF Large Wordlist'))
          .toList();
      expect(eff, hasLength(1));
      final notice = eff.single.paragraphs
          .map((paragraph) => paragraph.text)
          .join('\n');
      for (final required in [
        'Electronic Frontier Foundation',
        'Joseph Bonneau',
        'CC-BY-4.0',
        'https://creativecommons.org/licenses/by/4.0/',
        'https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt',
        'unmodified',
      ]) {
        expect(notice, contains(required));
      }
      await tester.pageBack();
      await tester.pumpAndSettle();
    }
  });
}

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/app.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/search/search_page.dart';
import '../helpers/feature_test_helpers.dart';

void main() {
  group('SearchPage', () {
    testWidgets('shows search bar and mode toggle', (tester) async {
      final harness = await tester.pumpFeature(const SearchPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.byType(SearchBar), findsOneWidget);
      expect(find.text('Contains'), findsOneWidget);
      expect(find.text('Fuzzy'), findsOneWidget);
      expect(find.text('Wildcard'), findsOneWidget);
    });

    testWidgets('queries repo on text input after debounce', (tester) async {
      final harness = await tester.pumpFeature(const SearchPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.enterText(find.byType(SearchBar), 'test query');
      await tester.pump(const Duration(milliseconds: 350));
      await tester.pumpAndSettle();

      expect(harness.search.searchCalled, isTrue);
      expect(harness.search.lastOptions?.query, 'test query');
      expect(harness.search.lastOptions?.mode, SearchModeDto.substring);
    });

    testWidgets('displays results with highlights', (tester) async {
      final harness = await tester.pumpFeature(const SearchPage());
      addTearDown(harness.dispose);

      harness.search.results = [
        SearchHit(
          entry: EntrySummary(
            uuid: 'e1',
            title: 'GitHub',
            username: 'octocat',
            url: '',
            kind: EntryKindDto.credential,
            hasTotp: false,
            hasAttachments: false,
            isExpired: false,
            groupUuid: 'root',
            tags: const [],
          ),
          matches: [
            const SearchFieldMatchDto(
              field: MatchedFieldDto.title,
              ranges: [(0, 3)],
            ),
          ],
        ),
      ];

      await tester.pumpAndSettle();
      await tester.enterText(find.byType(SearchBar), 'git');
      await tester.pump(const Duration(milliseconds: 350));
      await tester.pumpAndSettle();

      expect(find.text('octocat'), findsOneWidget);
    });

    testWidgets('shows empty state when no results', (tester) async {
      final harness = await tester.pumpFeature(const SearchPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.enterText(find.byType(SearchBar), 'nonexistent');
      await tester.pump(const Duration(milliseconds: 350));
      await tester.pumpAndSettle();

      expect(find.text('No results found'), findsOneWidget);
    });

    testWidgets('passes mode to search options', (tester) async {
      final harness = await tester.pumpFeature(const SearchPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Fuzzy'));
      await tester.pumpAndSettle();

      await tester.enterText(find.byType(SearchBar), 'test');
      await tester.pump(const Duration(milliseconds: 350));
      await tester.pumpAndSettle();

      expect(harness.search.lastOptions?.mode, SearchModeDto.fuzzy);
    });

    testWidgets('tapping a result routes to the actual entry detail', (
      tester,
    ) async {
      final harness = TestHarness();
      addTearDown(harness.dispose);
      harness.session.currentLockState = LockEvent.unlocked;
      harness.search.results = [
        SearchHit(
          entry: EntrySummary(
            uuid: 'entry-1',
            title: 'GitHub',
            username: 'octocat',
            url: '',
            kind: EntryKindDto.credential,
            hasTotp: false,
            hasAttachments: false,
            isExpired: false,
            groupUuid: 'root-uuid',
            tags: const [],
          ),
          matches: const [],
        ),
      ];
      tester.view.physicalSize = const Size(1400, 900);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
      await tester.pumpAndSettle();

      await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.keyF);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(SearchBar), 'git');
      await tester.pump(const Duration(milliseconds: 350));
      await tester.pumpAndSettle();
      await tester.tap(find.text('octocat'));
      await tester.pumpAndSettle();

      expect(harness.entries.entryDetailCalls, greaterThan(0));
      expect(find.text('https://github.com'), findsOneWidget);
    });
  });
}

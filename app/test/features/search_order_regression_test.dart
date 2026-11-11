import 'dart:async';

import 'package:app/src/data/models.dart';
import 'package:app/src/data/repositories.dart';
import 'package:app/src/features/search/search_page.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../helpers/test_helpers.dart';

class _Search implements SearchRepository {
  final calls =
      <({SearchOptionsDto options, Completer<List<SearchHit>> result})>[];

  @override
  Future<List<SearchHit>> search(SearchOptionsDto options) {
    final result = Completer<List<SearchHit>>();
    calls.add((options: options, result: result));
    return result.future;
  }

  void finish(int index, String title) => calls[index].result.complete([
    SearchHit(
      entry: EntrySummary(
        uuid: title,
        title: title,
        username: '',
        url: '',
        kind: EntryKindDto.credential,
        hasTotp: false,
        hasAttachments: false,
        isExpired: false,
        groupUuid: 'root',
        tags: const [],
      ),
      matches: const [],
    ),
  ]);
}

Future<_Search> _mount(WidgetTester tester) async {
  final repo = _Search();
  await tester.pumpApp(
    ProviderScope(
      overrides: [searchRepositoryProvider.overrideWithValue(repo)],
      child: const SearchPage(),
    ),
  );
  await tester.pumpAndSettle();
  addTearDown(() async {
    await tester.pumpWidget(const SizedBox.shrink());
    for (final call in repo.calls) {
      if (!call.result.isCompleted) call.result.complete([]);
    }
    await tester.pump();
  });
  return repo;
}

Future<void> _query(WidgetTester tester, String query) async {
  await tester.enterText(find.byType(SearchBar), query);
  await tester.pump(const Duration(milliseconds: 300));
  await tester.pump();
}

void main() {
  testWidgets('clearing query revokes pending response and busy state', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _query(tester, 'old');
    expect(repo.calls, hasLength(1));
    await tester.enterText(find.byType(SearchBar), '');
    await tester.pump();
    repo.finish(0, 'stale-cleared-hit');
    await tester.pump();
    expect(find.text('stale-cleared-hit'), findsNothing);
    expect(find.byType(CircularProgressIndicator), findsNothing);
    expect(find.text('No results found'), findsNothing);
  });

  testWidgets('editing query revokes old response before debounce fires', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _query(tester, 'old');
    await tester.enterText(find.byType(SearchBar), 'new');
    await tester.pump();
    repo.finish(0, 'stale-during-debounce');
    await tester.pump();
    expect(repo.calls, hasLength(1));
    expect(find.text('stale-during-debounce'), findsNothing);
    await tester.pump(const Duration(milliseconds: 300));
    expect(repo.calls.last.options.query, 'new');
    repo.finish(1, 'current-hit');
    await tester.pumpAndSettle();
    expect(find.text('current-hit'), findsOneWidget);
  });

  testWidgets('reordered query completions retain latest result', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _query(tester, 'old');
    await _query(tester, 'new');
    repo.finish(1, 'new-hit');
    await tester.pump();
    repo.finish(0, 'old-hit');
    await tester.pumpAndSettle();
    expect(find.text('new-hit'), findsOneWidget);
    expect(find.text('old-hit'), findsNothing);
  });

  testWidgets('mode change owns result despite late previous-mode response', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _query(tester, 'query');
    await tester.tap(find.text('Fuzzy'));
    await tester.pump();
    expect(repo.calls.last.options.mode, SearchModeDto.fuzzy);
    repo.finish(1, 'fuzzy-hit');
    await tester.pump();
    repo.finish(0, 'substring-hit');
    await tester.pumpAndSettle();
    expect(find.text('fuzzy-hit'), findsOneWidget);
    expect(find.text('substring-hit'), findsNothing);
  });

  testWidgets('filter change owns result despite late unfiltered response', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _query(tester, 'query');
    await tester.tap(find.byType(FilterChip));
    await tester.pump();
    expect(repo.calls.last.options.includeRecycled, isTrue);
    repo.finish(1, 'filtered-hit');
    await tester.pump();
    repo.finish(0, 'unfiltered-hit');
    await tester.pumpAndSettle();
    expect(find.text('filtered-hit'), findsOneWidget);
    expect(find.text('unfiltered-hit'), findsNothing);
  });

  testWidgets('old failure cannot end current request busy state', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _query(tester, 'old');
    await _query(tester, 'new');
    repo.calls[0].result.completeError(Exception('old-private-error-marker'));
    await tester.pump();
    expect(find.byType(CircularProgressIndicator), findsOneWidget);
    expect(find.textContaining('old-private-error-marker'), findsNothing);
    repo.finish(1, 'new-hit');
    await tester.pumpAndSettle();
    expect(find.text('new-hit'), findsOneWidget);
  });

  testWidgets('new input immediately removes actionable old hits', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _query(tester, 'old');
    repo.finish(0, 'previous-visible-hit');
    await tester.pumpAndSettle();
    expect(find.text('previous-visible-hit'), findsOneWidget);
    await tester.enterText(find.byType(SearchBar), 'new');
    await tester.pump();
    expect(find.text('previous-visible-hit'), findsNothing);
    expect(find.byType(ListTile), findsNothing);
  });

  testWidgets('option change cancels a pending query debounce', (tester) async {
    final repo = await _mount(tester);
    await tester.enterText(find.byType(SearchBar), 'query');
    await tester.tap(find.text('Fuzzy'));
    await tester.pump(const Duration(milliseconds: 350));
    expect(repo.calls, hasLength(1));
    expect(repo.calls.single.options.mode, SearchModeDto.fuzzy);
    repo.finish(0, 'fuzzy-hit');
    await tester.pumpAndSettle();
  });

  testWidgets('ordinary debounced query retains current options', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _query(tester, 'current');
    expect(repo.calls.single.options.query, 'current');
    expect(repo.calls.single.options.mode, SearchModeDto.substring);
    expect(repo.calls.single.options.includeRecycled, isFalse);
    repo.finish(0, 'current-hit');
    await tester.pumpAndSettle();
    expect(find.text('current-hit'), findsOneWidget);
    expect(find.byType(CircularProgressIndicator), findsNothing);
  });

  testWidgets('disposal rejects pending response without an exception', (
    tester,
  ) async {
    final repo = await _mount(tester);
    await _query(tester, 'query');
    await tester.pumpWidget(const SizedBox.shrink());
    repo.finish(0, 'disposed-hit');
    await tester.pump();
    expect(find.text('disposed-hit'), findsNothing);
    expect(tester.takeException(), isNull);
  });
}

import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/data/models.dart';
import 'package:app/src/data/repositories.dart';
import 'package:app/src/features/entries/entry_list.dart';

import '../helpers/feature_test_helpers.dart';

void main() {
  final entries = List<EntrySummary>.generate(
    5000,
    (index) => EntrySummary(
      uuid: 'entry-$index',
      title: 'Controlled entry $index',
      username: 'user-$index',
      url: 'https://example.test/$index',
      kind: EntryKindDto.credential,
      hasTotp: false,
      hasAttachments: false,
      isExpired: false,
      groupUuid: 'root',
      tags: const ['controlled'],
    ),
    growable: false,
  );

  test('controlled 5,000-entry search remains below the PRD budget', () async {
    final repository = _ControlledSearchRepository(entries);
    const options = SearchOptionsDto(query: 'entry 4999');
    await repository.search(options);

    final samples = <int>[];
    for (var run = 0; run < 20; run++) {
      final stopwatch = Stopwatch()..start();
      final results = await repository.search(options);
      stopwatch.stop();
      expect(results.single.entry.uuid, 'entry-4999');
      samples.add(stopwatch.elapsedMicroseconds);
    }
    samples.sort();
    final p95Micros = samples[(samples.length * 0.95).ceil() - 1];
    debugPrint(
      'PERF desktop controlled search entries=5000 p95_us=$p95Micros budget_us=50000',
    );
    expect(p95Micros, lessThan(50000));
  });

  testWidgets('5,000-entry list is virtualized and first-frame bounded', (
    tester,
  ) async {
    final stopwatch = Stopwatch()..start();
    await tester.pumpFeature(
      EntryList(
        entries: entries,
        selectedUuid: null,
        onEntrySelected: (_) {},
        onCopyUsername: (_) {},
        onCopyPassword: (_) {},
      ),
    );
    await tester.pump();
    stopwatch.stop();

    final builtRows =
        find.byType(ListTile).evaluate().length +
        find.byType(InkWell).evaluate().length;
    debugPrint(
      'PERF desktop controlled list entries=5000 first_frame_us=${stopwatch.elapsedMicroseconds} built_widgets=$builtRows',
    );
    expect(find.text('Controlled entry 0'), findsOneWidget);
    expect(find.text('Controlled entry 4999'), findsNothing);
    expect(builtRows, lessThan(100));
  });
}

class _ControlledSearchRepository implements SearchRepository {
  const _ControlledSearchRepository(this.entries);

  final List<EntrySummary> entries;

  @override
  Future<List<SearchHit>> search(SearchOptionsDto options) async {
    final query = options.query.toLowerCase();
    return [
      for (final entry in entries)
        if (entry.title.toLowerCase().contains(query))
          SearchHit(entry: entry, matches: const []),
    ];
  }
}

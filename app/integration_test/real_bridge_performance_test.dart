import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/frb_generated.dart';

const _master = 'performance-integration-master-marker';
const _searchBudgetMicros = 50000;
const _listBudgetMicros = 2000000;

String _requiredEnv(String name) {
  final value = Platform.environment[name];
  if (value == null || value.isEmpty) {
    fail('$name is required by the Make target');
  }
  return value;
}

int _percentile95(List<int> values) {
  final sorted = [...values]..sort();
  return sorted[((sorted.length - 1) * 0.95).round()];
}

void main() {
  setUpAll(() async {
    final libraryPath = _requiredEnv('HIDLINS_API_LIB');
    await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));
  });

  test('5,000 entries stay within the compiled bridge budget', () async {
    final session = await api.initApp(
      cfg: AppInitConfig(stateDir: _requiredEnv('HIDLINS_PERF_STATE_DIR')),
    );
    try {
      await session.unlock(name: 'performance', masterPassword: _master);

      final listTimer = Stopwatch()..start();
      final tree = await session.vaultTree();
      listTimer.stop();
      expect(tree.entries, hasLength(5000));

      const options = SearchOptionsDto(
        query: 'Entry-04999',
        mode: SearchModeDto.substring,
        scope: SearchScopeDto.all(),
        includeRecycled: false,
      );
      await session.search(opts: options);
      final samples = <int>[];
      for (var index = 0; index < 20; index++) {
        final timer = Stopwatch()..start();
        final hits = await session.search(opts: options);
        timer.stop();
        expect(hits, hasLength(1));
        samples.add(timer.elapsedMicroseconds);
      }
      final p95 = _percentile95(samples);
      final evidence = <String, Object>{
        'schema': 1,
        'entries': tree.entries.length,
        'search_p95_us': p95,
        'search_budget_us': _searchBudgetMicros,
        'list_us': listTimer.elapsedMicroseconds,
        'list_budget_us': _listBudgetMicros,
        'os': Platform.operatingSystem,
        'os_version': Platform.operatingSystemVersion,
        'abi': Abi.current().toString(),
        'flutter': _requiredEnv('HIDLINS_FLUTTER_VERSION'),
        'revision': _requiredEnv('HIDLINS_GIT_REVISION'),
        'profile': 'debug-native-cdylib',
      };
      // Stable prefix lets the Make target and CI retain machine-readable
      // evidence without treating the rest of Flutter's reporter as JSON.
      // ignore: avoid_print
      print('HIDLINS_PERF_JSON:${jsonEncode(evidence)}');
      expect(p95, lessThan(_searchBudgetMicros));
      expect(listTimer.elapsedMicroseconds, lessThan(_listBudgetMicros));
    } finally {
      await session.shutdown();
    }
  }, timeout: const Timeout(Duration(minutes: 2)));
}

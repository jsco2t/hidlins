import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/frb_generated.dart';

const _master = 'process-coexistence-master-marker';

Future<int> _runCli(List<String> arguments, String stdin) async {
  final process = await Process.start(
    Platform.environment['HIDLINS_CLI_BIN']!,
    arguments,
  );
  final stdoutDone = process.stdout.transform(utf8.decoder).join();
  final stderrDone = process.stderr.transform(utf8.decoder).join();
  process.stdin.write(stdin);
  await process.stdin.close();
  final exitCode = await process.exitCode.timeout(const Duration(seconds: 20));
  await Future.wait([stdoutDone, stderrDone]);
  return exitCode;
}

void main() {
  setUpAll(() async {
    final libraryPath = Platform.environment['HIDLINS_API_LIB'];
    final cliPath = Platform.environment['HIDLINS_CLI_BIN'];
    expect(libraryPath, isNotNull);
    expect(cliPath, isNotNull);
    expect(File(cliPath!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath!));
  });

  test('CLI contention and supported external refresh are deterministic', () async {
    final fixture = Directory.systemTemp.createTempSync(
      'hidlins-app-process-coexistence-',
    );
    final session = await api.initApp(
      cfg: AppInitConfig(stateDir: fixture.path),
    );
    try {
      await session.createVault(
        name: 'coexistence',
        masterPassword: _master,
        confirmedNoRecovery: true,
      );
      final tree = await session.unlock(
        name: 'coexistence',
        masterPassword: _master,
      );
      final entry = await session.createEntry(
        group: tree.root.uuid,
        draft: const EntryDraftDto(
          kind: EntryKindDto.credential,
          title: 'Before external edit',
          tags: [],
          customFields: [],
        ),
      );
      final cliArgs = [
        '--registry',
        '${fixture.path}/vaults.toml',
        'entry',
        'edit',
        '--vault',
        'coexistence',
        '--uuid',
        entry,
        '--title',
        'After external edit',
      ];

      // The bridge owns the same exclusive sidecar lock used by CLI and TUI.
      // A competing CLI write must fail closed with the stable lock exit code.
      expect(await _runCli(cliArgs, '$_master\n'), 2);
      expect(
        (await session.entryDetail(uuid: entry)).title,
        'Before external edit',
      );

      // The supported refresh path is lock/re-open: after the bridge releases
      // the lock, the external writer succeeds and the next unlock re-reads
      // the KDBX instead of presenting stale in-memory data.
      await session.lockNow();
      expect(await _runCli(cliArgs, '$_master\n'), 0);
      await session.unlock(name: 'coexistence', masterPassword: _master);
      expect(
        (await session.entryDetail(uuid: entry)).title,
        'After external edit',
      );
    } finally {
      await session.shutdown();
      fixture.deleteSync(recursive: true);
    }
  }, timeout: const Timeout(Duration(minutes: 2)));
}

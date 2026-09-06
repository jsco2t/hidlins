import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/frb_generated.dart';

const _master = 'keepassxc-integration-master-marker';
const _originalSecret = 'keepassxc-original-secret-marker';
const _externalSecret = 'keepassxc-external-secret-marker';

Future<int> _keepassxcAdd(String vaultPath) async {
  final process = await Process.start('keepassxc-cli', [
    'add',
    '-q',
    '-u',
    'external-user',
    '-p',
    vaultPath,
    'KeePassXC external entry',
  ]);
  final stdoutDone = process.stdout.transform(utf8.decoder).join();
  final stderrDone = process.stderr.transform(utf8.decoder).join();
  process.stdin
    ..writeln(_master)
    ..writeln(_externalSecret)
    ..writeln(_externalSecret);
  await process.stdin.close();
  try {
    final exitCode = await process.exitCode.timeout(
      const Duration(seconds: 30),
    );
    await Future.wait([stdoutDone, stderrDone]);
    return exitCode;
  } on TimeoutException {
    process.kill();
    await Future.wait([stdoutDone, stderrDone]);
    rethrow;
  }
}

void main() {
  setUpAll(() async {
    final libraryPath = Platform.environment['HIDLINS_API_LIB'];
    expect(libraryPath, isNotNull);
    await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath!));
  });

  test(
    'KeePassXC save preserves bridge-created data and is reloaded',
    () async {
      final fixture = Directory.systemTemp.createTempSync(
        'hidlins-app-keepassxc-',
      );
      var session = await api.initApp(
        cfg: AppInitConfig(stateDir: fixture.path),
      );

      try {
        final summary = await session.createVault(
          name: 'interop',
          masterPassword: _master,
          confirmedNoRecovery: true,
        );
        final tree = await session.unlock(
          name: summary.name,
          masterPassword: _master,
        );
        final original = await session.createEntry(
          group: tree.root.uuid,
          draft: const EntryDraftDto(
            kind: EntryKindDto.credential,
            title: 'Bridge original entry',
            username: 'bridge-user',
            password: _originalSecret,
            url: 'https://interop.invalid',
            notes: 'must survive a KeePassXC save',
            tags: ['interop'],
            customFields: [
              CustomFieldInputDto(
                name: 'origin',
                value: 'flutter-bridge',
                protected: false,
              ),
            ],
          ),
        );
        await session.shutdown();

        expect(
          await _keepassxcAdd(summary.path),
          0,
          reason: 'keepassxc-cli add failed; captured output is intentionally scrubbed',
        );

        session = await api.initApp(cfg: AppInitConfig(stateDir: fixture.path));
        final reopened = await session.unlock(
          name: summary.name,
          masterPassword: _master,
        );
        expect(reopened.entries, hasLength(2));
        final originalDetail = await session.entryDetail(uuid: original);
        expect(originalDetail.title, 'Bridge original entry');
        expect(originalDetail.username, 'bridge-user');
        expect(originalDetail.url, 'https://interop.invalid');
        expect(originalDetail.notes, 'must survive a KeePassXC save');
        expect(originalDetail.tags, ['interop']);
        expect(
          await session.revealField(
            uuid: original,
            field: const RevealField.password(),
          ),
          _originalSecret,
        );

        final externalHits = await session.search(
          opts: const SearchOptionsDto(
            query: 'KeePassXC external entry',
            mode: SearchModeDto.substring,
            scope: SearchScopeDto.all(),
            includeRecycled: false,
          ),
        );
        expect(externalHits, hasLength(1));
        expect(
          await session.revealField(
            uuid: externalHits.single.entry.uuid,
            field: const RevealField.password(),
          ),
          _externalSecret,
        );
      } finally {
        await session.shutdown();
        fixture.deleteSync(recursive: true);
      }
    },
    timeout: const Timeout(Duration(minutes: 2)),
  );
}

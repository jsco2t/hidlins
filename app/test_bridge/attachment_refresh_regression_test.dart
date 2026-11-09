import 'dart:io';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-attachment-refresh');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  test(
    'real attachment commits refresh cached detail without manual invalidation',
    () async {
      final directory = Directory.systemTemp.createTempSync(
        'hidlins-detail-refresh-',
      );
      final source = File('${directory.path}/sample.txt')
        ..writeAsStringSync('fixture');
      final session = await api.initApp(
        cfg: bridge.AppInitConfig(stateDir: directory.path),
      );
      ProviderContainer? container;
      try {
        await session.createVault(
          name: 'fixture',
          masterPassword: 'fixture-master-marker',
          confirmedNoRecovery: true,
        );
        final tree = await session.unlock(
          name: 'fixture',
          masterPassword: 'fixture-master-marker',
        );
        final repo = BridgeEntryRepository(session);
        final uuid = await repo.createEntry(
          tree.root.uuid,
          const EntryDraftDto(
            kind: EntryKindDto.credential,
            title: 'Refresh fixture',
          ),
        );
        final c = container = ProviderContainer(
          overrides: [
            entryRepositoryProvider.overrideWithValue(repo),
            syncRepositoryProvider.overrideWithValue(
              BridgeSyncRepository(session),
            ),
          ],
        );
        expect(
          (await c.read(entryDetailProvider(uuid).future)).attachments,
          isEmpty,
        );
        await repo.addAttachment(uuid, source.path);
        expect((await repo.entryDetail(uuid)).attachments.map((a) => a.name), [
          'sample.txt',
        ]);
        expect(
          (await c.read(entryDetailProvider(uuid).future)).attachments
              .map((a) => a.name),
          ['sample.txt'],
        );
        await repo.removeAttachment(uuid, 'sample.txt');
        expect(
          (await c.read(entryDetailProvider(uuid).future)).attachments,
          isEmpty,
        );
        // The committed KDBX, rather than only the invalidated cache, agrees.
        await session.lockNow();
        await session.unlock(
          name: 'fixture',
          masterPassword: 'fixture-master-marker',
        );
        expect((await repo.entryDetail(uuid)).attachments, isEmpty);
      } finally {
        container?.dispose();
        await session.shutdown();
        directory.deleteSync(recursive: true);
      }
    },
  );
}

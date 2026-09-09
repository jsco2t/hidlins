import 'dart:async';
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/error.dart';
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/platform/rust_library.dart';

const _master = 'integration-master-marker';
const _rotatedMaster = 'integration-rotated-marker';
const _entrySecret = 'integration-entry-secret-marker';

void main() {
  setUpAll(() async {
    if (Platform.isIOS || Platform.isAndroid) {
      await initializeBundledRustLibrary();
      return;
    }
    final libraryPath = Platform.environment['HIDLINS_API_LIB'];
    expect(
      libraryPath,
      isNotNull,
      reason: 'HIDLINS_API_LIB is set by make app-test-integration',
    );
    expect(File(libraryPath!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));
  });

  test('real bridge covers vault lifecycle and representative CRUD', () async {
    final fixture = Directory.systemTemp.createTempSync(
      'hidlins-real-bridge-lifecycle-',
    );
    final attachment = File('${fixture.path}/attachment.txt')
      ..writeAsStringSync('non-secret attachment fixture');
    final session = await api.initApp(
      cfg: AppInitConfig(stateDir: fixture.path),
    );

    try {
      final created = await session.createVault(
        name: 'desktop-alpha',
        masterPassword: _master,
        confirmedNoRecovery: true,
      );
      expect(File(created.path).existsSync(), isTrue);

      final tree = await session.unlock(
        name: created.name,
        masterPassword: _master,
      );
      final root = tree.root.uuid;

      final credential = await session.createEntry(
        group: root,
        draft: const EntryDraftDto(
          kind: EntryKindDto.credential,
          title: 'Bridge credential',
          username: 'bridge-user',
          password: _entrySecret,
          url: 'https://example.invalid',
          notes: 'fixture note',
          tags: ['integration'],
          customFields: [
            CustomFieldInputDto(
              name: 'account',
              value: 'desktop',
              protected: false,
            ),
          ],
        ),
      );
      final note = await session.createEntry(
        group: root,
        draft: const EntryDraftDto(
          kind: EntryKindDto.secureNote,
          title: 'Bridge secure note',
          notes: 'stored only inside KDBX',
          tags: [],
          customFields: [],
        ),
      );
      final totp = await session.createEntry(
        group: root,
        draft: const EntryDraftDto(
          kind: EntryKindDto.totp,
          title: 'Bridge TOTP',
          tags: [],
          customFields: [],
          totpUri: 'otpauth://totp/Hidlins:bridge?secret=JBSWY3DPEHPK3PXP&issuer=Hidlins',
        ),
      );

      if (Platform.isIOS || Platform.isAndroid) {
        await expectLater(
          session.addAttachment(uuid: credential, sourcePath: attachment.path),
          throwsA(isA<HidlinsApiError_UnsupportedPlatform>()),
        );
      } else {
        await session.addAttachment(
          uuid: credential,
          sourcePath: attachment.path,
        );
      }
      await session.updateEntry(
        uuid: credential,
        edit: const EntryEditDto(title: 'Bridge credential updated'),
      );

      final credentialDetail = await session.entryDetail(uuid: credential);
      expect(credentialDetail.title, 'Bridge credential updated');
      expect(credentialDetail.hasPassword, isTrue);
      expect(
        credentialDetail.attachments,
        Platform.isIOS || Platform.isAndroid ? isEmpty : hasLength(1),
      );
      expect(await session.entryHistory(uuid: credential), isNotEmpty);
      expect(
        (await session.entryDetail(uuid: note)).kind,
        EntryKindDto.secureNote,
      );
      expect((await session.entryDetail(uuid: totp)).kind, EntryKindDto.totp);
      expect(session.totpNow(uuid: totp).code, hasLength(6));

      final hits = await session.search(
        opts: const SearchOptionsDto(
          query: 'Bridge',
          mode: SearchModeDto.substring,
          scope: SearchScopeDto.all(),
          includeRecycled: false,
        ),
      );
      expect(
        hits.map((hit) => hit.entry.uuid),
        containsAll([credential, note, totp]),
      );

      await session.changeMasterPassword(
        current: _master,
        newPassword: _rotatedMaster,
      );
      await session.lockNow();
      await expectLater(
        session.vaultTree(),
        throwsA(isA<HidlinsApiError_VaultLocked>()),
      );
      final reopened = await session.unlock(
        name: created.name,
        masterPassword: _rotatedMaster,
      );
      expect(reopened.entries, hasLength(3));
    } finally {
      await session.shutdown();
      fixture.deleteSync(recursive: true);
    }
  }, timeout: const Timeout(Duration(minutes: 2)));

  test('real bridge registers an existing KDBX without rewriting it', () async {
    final sourceDir = Directory.systemTemp.createTempSync(
      'hidlins-real-bridge-import-source-',
    );
    final importDir = Directory.systemTemp.createTempSync(
      'hidlins-real-bridge-import-target-',
    );
    final source = await api.initApp(
      cfg: AppInitConfig(stateDir: sourceDir.path),
    );
    final target = await api.initApp(
      cfg: AppInitConfig(stateDir: importDir.path),
    );

    try {
      final summary = await source.createVault(
        name: 'source',
        masterPassword: _master,
        confirmedNoRecovery: true,
      );
      final before = await File(summary.path).readAsBytes();
      await source.shutdown();

      final external = File('${importDir.path}/external.kdbx');
      await File(summary.path).copy(external.path);
      final registered = await target.registerExistingVault(
        name: 'imported',
        kdbxPath: external.path,
      );
      expect(registered.path, external.path);
      expect(await external.readAsBytes(), before);
      expect(
        (await target.unlock(
          name: 'imported',
          masterPassword: _master,
        )).root.uuid,
        isNotEmpty,
      );
    } finally {
      await source.shutdown();
      await target.shutdown();
      sourceDir.deleteSync(recursive: true);
      importDir.deleteSync(recursive: true);
    }
  }, timeout: const Timeout(Duration(minutes: 2)));

  test(
    'real bridge completes local pair/import, sync, and peer control',
    () async {
      final serverDir = Directory.systemTemp.createTempSync(
        'hidlins-real-bridge-local-server-',
      );
      final clientDir = Directory.systemTemp.createTempSync(
        'hidlins-real-bridge-local-client-',
      );
      final server = await api.initApp(
        cfg: AppInitConfig(stateDir: serverDir.path),
      );
      final client = await api.initApp(
        cfg: AppInitConfig(stateDir: clientDir.path),
      );
      final serverPrompt = Completer<PairingPromptDto>();
      final startupSync = Completer<SyncEvent>();
      final subscription = server.syncEvents().listen((event) {
        if (event case SyncEvent_PairingRequested(:final field0)) {
          if (!serverPrompt.isCompleted) serverPrompt.complete(field0);
        }
      });
      final clientSubscription = client.syncEvents().listen((event) {
        if ((event is SyncEvent_Done || event is SyncEvent_Failed) &&
            !startupSync.isCompleted) {
          startupSync.complete(event);
        }
      });

      try {
        await server.createVault(
          name: 'local-shared',
          masterPassword: _master,
          confirmedNoRecovery: true,
        );
        await server.unlock(name: 'local-shared', masterPassword: _master);
        await server.configureLocalSync(role: LocalSyncRoleDto.server);

        final candidates = await server.localServerEndpoints();
        expect(candidates, isNotEmpty);
        expect(candidates.every((candidate) => candidate.port > 0), isTrue);
        final requested = candidates.firstWhere(
          (candidate) => candidate.address == '127.0.0.1',
          orElse: () => candidates.first,
        );
        final endpoint = await server.startSyncServer(endpoint: requested);
        await server.openPairingWindow();
        expect((await server.localSyncStatus()).serverRunning, isTrue);
        expect((await server.localSyncStatus()).pairingOpen, isTrue);

        final clientSide = await client.beginPairImport(
          name: 'local-shared',
          masterPassword: _master,
          candidates: [endpoint],
        );
        final serverSide = await serverPrompt.future.timeout(
          const Duration(seconds: 10),
        );
        expect(clientSide.sas, serverSide.sas);
        expect(clientSide.toString(), isNot(contains(clientSide.sas)));

        await server.confirmPairing(
          transactionHandle: serverSide.transactionHandle,
          accepted: true,
          peerDisplayName: 'Desktop client',
        );
        final imported = await client.confirmPairing(
          transactionHandle: clientSide.transactionHandle,
          accepted: true,
          peerDisplayName: 'Desktop authority',
        );
        expect(imported?.name, 'local-shared');
        expect(File(imported!.path).existsSync(), isTrue);

        await client.setDiscoveryCandidates(
          permission: DiscoveryPermissionDto.granted,
          candidates: [endpoint],
        );
        await client.unlock(name: 'local-shared', masterPassword: _master);
        expect(
          await startupSync.future.timeout(const Duration(seconds: 15)),
          isA<SyncEvent_Done>(),
        );
        await client.setDiscoveryCandidates(
          permission: DiscoveryPermissionDto.granted,
          candidates: [
            LocalEndpointDto(
              address: endpoint.address,
              port: 9,
              scopeId: endpoint.scopeId,
            ),
          ],
        );
        await client.setDiscoveryCandidates(
          permission: DiscoveryPermissionDto.granted,
          candidates: [endpoint],
        );
        final discovery = await client.localDiscoveryStatus();
        expect(discovery.candidates, [endpoint]);
        expect(await client.syncNow(), isA<SyncOutcomeDto_AlreadyInSync>());

        var peers = await server.listSyncPeers();
        expect(peers, hasLength(1));
        await server.renameSyncPeer(
          peerId: peers.single.peerId,
          displayName: 'Renamed desktop client',
        );
        peers = await server.listSyncPeers();
        expect(peers.single.displayName, 'Renamed desktop client');
        await server.revokeSyncPeer(peerId: peers.single.peerId);
        expect((await server.listSyncPeers()).single.revoked, isTrue);
        expect((await server.localSyncStatus()).activePeerCount, BigInt.zero);
      } finally {
        await client.shutdown();
        await server.stopSyncServer();
        await server.shutdown();
        await subscription.cancel();
        await clientSubscription.cancel();
        serverDir.deleteSync(recursive: true);
        clientDir.deleteSync(recursive: true);
      }
    },
    skip: Platform.isIOS || Platform.isAndroid
        ? 'mobile client scenarios use a separate CLI authority'
        : false,
    timeout: const Timeout(Duration(minutes: 2)),
  );

  test('per-vault idle timeout auto-locks through the real ticker', () async {
    final fixture = Directory.systemTemp.createTempSync(
      'hidlins-real-bridge-autolock-',
    );
    var session = await api.initApp(cfg: AppInitConfig(stateDir: fixture.path));

    try {
      await session.createVault(
        name: 'auto-lock',
        masterPassword: _master,
        confirmedNoRecovery: true,
      );
      await session.shutdown();

      final registry = File('${fixture.path}/vaults.toml');
      final original = registry.readAsStringSync();
      registry.writeAsStringSync(
        '$original\n[vault.lock]\nidle_timeout_seconds = 1\n',
        flush: true,
      );

      session = await api.initApp(cfg: AppInitConfig(stateDir: fixture.path));
      await session.unlock(name: 'auto-lock', masterPassword: _master);
      final deadline = DateTime.now().add(const Duration(seconds: 8));
      var observedLocked = false;
      while (DateTime.now().isBefore(deadline)) {
        try {
          await session.vaultTree();
        } on HidlinsApiError_VaultLocked {
          observedLocked = true;
          break;
        }
        await Future<void>.delayed(const Duration(milliseconds: 100));
      }
      expect(observedLocked, isTrue, reason: 'real ticker did not auto-lock');
    } finally {
      await session.shutdown();
      fixture.deleteSync(recursive: true);
    }
  }, timeout: const Timeout(Duration(minutes: 2)));
}

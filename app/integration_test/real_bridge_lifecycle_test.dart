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

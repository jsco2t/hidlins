import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/error.dart';
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/platform/rust_library.dart';

const _master = 'minio-integration-master-marker';
const _newMaster = 'minio-integration-new-master-marker';
const _entrySecret = 'minio-integration-entry-secret-marker';

const _deviceConfig = <String, String>{
  'HIDLINS_TEST_S3_BUCKET': String.fromEnvironment('HIDLINS_TEST_S3_BUCKET'),
  'HIDLINS_TEST_S3_KEY': String.fromEnvironment('HIDLINS_TEST_S3_KEY'),
  'HIDLINS_TEST_S3_REGION': String.fromEnvironment('HIDLINS_TEST_S3_REGION'),
  'HIDLINS_TEST_S3_ENDPOINT': String.fromEnvironment(
    'HIDLINS_TEST_S3_ENDPOINT',
  ),
  'HIDLINS_TEST_S3_PATH_STYLE': String.fromEnvironment(
    'HIDLINS_TEST_S3_PATH_STYLE',
  ),
  'HIDLINS_TEST_S3_ACCESS_KEY': String.fromEnvironment(
    'HIDLINS_TEST_S3_ACCESS_KEY',
  ),
  'HIDLINS_TEST_S3_SECRET_KEY': String.fromEnvironment(
    'HIDLINS_TEST_S3_SECRET_KEY',
  ),
};

String _requiredConfig(String deviceName, String desktopName) {
  final deviceValue = _deviceConfig[deviceName];
  final value = deviceValue?.isNotEmpty == true
      ? deviceValue
      : Platform.environment[desktopName];
  if (value == null || value.isEmpty) {
    fail(
      '$deviceName is required; run through a repository-owned integration target',
    );
  }
  return value;
}

S3ConfigDto _config() => S3ConfigDto(
  bucket: _requiredConfig('HIDLINS_TEST_S3_BUCKET', 'HIDLINS_APP_MINIO_BUCKET'),
  key: _deviceConfig['HIDLINS_TEST_S3_KEY']?.isNotEmpty == true
      ? _deviceConfig['HIDLINS_TEST_S3_KEY']!
      : 'desktop-alpha.kdbx',
  region: _requiredConfig('HIDLINS_TEST_S3_REGION', 'HIDLINS_MINIO_REGION'),
  endpoint: _requiredConfig(
    'HIDLINS_TEST_S3_ENDPOINT',
    'HIDLINS_MINIO_ENDPOINT',
  ),
  pathStyle: _deviceConfig['HIDLINS_TEST_S3_PATH_STYLE']?.isNotEmpty == true
      ? _deviceConfig['HIDLINS_TEST_S3_PATH_STYLE'] == 'true'
      : true,
  accessKeyId: _requiredConfig(
    'HIDLINS_TEST_S3_ACCESS_KEY',
    'HIDLINS_MINIO_ACCESS_KEY',
  ),
  secretAccessKey: _requiredConfig(
    'HIDLINS_TEST_S3_SECRET_KEY',
    'HIDLINS_MINIO_SECRET_KEY',
  ),
);

S3ConfigDto _offlineConfig() => S3ConfigDto(
  bucket: _config().bucket,
  key: _config().key,
  region: _config().region,
  endpoint: 'http://127.0.0.1:1',
  pathStyle: _config().pathStyle,
  accessKeyId: _config().accessKeyId,
  secretAccessKey: _config().secretAccessKey,
);

EntryDraftDto _credential(String title) => EntryDraftDto(
  kind: EntryKindDto.credential,
  title: title,
  username: 'integration-user',
  password: _entrySecret,
  tags: const ['minio'],
  customFields: const [],
);

Future<void> _waitForFreshSecond() async {
  while (DateTime.now().millisecond > 100) {
    await Future<void>.delayed(const Duration(milliseconds: 10));
  }
}

void main() {
  setUpAll(() async {
    if (Platform.isIOS || Platform.isAndroid) {
      await initializeBundledRustLibrary();
      return;
    }
    final libraryPath = _requiredConfig(
      'HIDLINS_TEST_S3_LIBRARY',
      'HIDLINS_API_LIB',
    );
    expect(File(libraryPath).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(libraryPath));
  });

  test(
    'two real bridge sessions sync, merge, recover, and bootstrap',
    () async {
      final aDir = Directory.systemTemp.createTempSync('hidlins-minio-app-a-');
      final bDir = Directory.systemTemp.createTempSync('hidlins-minio-app-b-');
      final wrongDir = Directory.systemTemp.createTempSync(
        'hidlins-minio-bootstrap-wrong-',
      );
      final rollbackDir = Directory.systemTemp.createTempSync(
        'hidlins-minio-bootstrap-rollback-',
      );
      final recoveryDir = Directory.systemTemp.createTempSync(
        'hidlins-minio-recovery-',
      );
      final sessions = <api.AppSession>[];

      try {
        final a = await api.initApp(cfg: AppInitConfig(stateDir: aDir.path));
        sessions.add(a);
        final created = await a.createVault(
          name: 'alpha-a',
          masterPassword: _master,
          confirmedNoRecovery: true,
        );
        final initialTree = await a.unlock(
          name: created.name,
          masterPassword: _master,
        );
        final sharedId = await a.createEntry(
          group: initialTree.root.uuid,
          draft: _credential('Shared base'),
        );
        await a.configureSync(cfg: _config());
        await a.syncNow();

        final wrong = await api.initApp(
          cfg: AppInitConfig(stateDir: wrongDir.path),
        );
        sessions.add(wrong);
        await expectLater(
          wrong.bootstrapVaultFromRemote(
            name: 'wrong-password',
            cfg: _config(),
            masterPassword: 'wrong-bootstrap-password',
          ),
          throwsA(isA<HidlinsApiError_AuthenticationFailed>()),
        );
        expect(wrongDir.listSync().whereType<File>(), isEmpty);
        expect(await wrong.listVaults(), isEmpty);

        final rollback = await api.initApp(
          cfg: AppInitConfig(stateDir: rollbackDir.path),
        );
        sessions.add(rollback);
        Directory('${rollbackDir.path}/vaults.toml').createSync();
        await expectLater(
          rollback.bootstrapVaultFromRemote(
            name: 'rollback',
            cfg: _config(),
            masterPassword: _master,
          ),
          throwsA(isA<HidlinsApiError>()),
        );
        expect(File('${rollbackDir.path}/rollback.kdbx').existsSync(), isFalse);
        expect(await rollback.listVaults(), isEmpty);

        final b = await api.initApp(cfg: AppInitConfig(stateDir: bDir.path));
        sessions.add(b);
        final bootstrapped = await b.bootstrapVaultFromRemote(
          name: 'alpha-b',
          cfg: _config(),
          masterPassword: _master,
        );
        expect(bootstrapped.hasSync, isTrue);
        final bTree = await b.unlock(
          name: bootstrapped.name,
          masterPassword: _master,
        );
        expect(bTree.entries.map((entry) => entry.uuid), contains(sharedId));

        await a.configureSync(cfg: _offlineConfig());
        await b.configureSync(cfg: _offlineConfig());
        final aOnly = await a.createEntry(
          group: initialTree.root.uuid,
          draft: _credential('Offline A'),
        );
        final bOnly = await b.createEntry(
          group: bTree.root.uuid,
          draft: _credential('Offline B'),
        );
        HidlinsApiError? networkError;
        try {
          await a.syncNow();
        } on HidlinsApiError catch (error) {
          networkError = error;
        }
        expect(networkError, isNotNull);
        final renderedError = networkError.toString();
        expect(renderedError, isNot(contains(_entrySecret)));
        expect(
          renderedError,
          isNot(
            contains(
              _requiredConfig(
                'HIDLINS_TEST_S3_SECRET_KEY',
                'HIDLINS_MINIO_SECRET_KEY',
              ),
            ),
          ),
        );
        expect((await a.entryDetail(uuid: aOnly)).title, 'Offline A');

        await a.configureSync(cfg: _config());
        await b.configureSync(cfg: _config());
        await a.syncNow();
        await b.syncNow();
        await a.syncNow();
        expect(
          (await a.vaultTree()).entries.map((entry) => entry.uuid),
          containsAll([aOnly, bOnly]),
        );
        expect(
          (await b.vaultTree()).entries.map((entry) => entry.uuid),
          containsAll([aOnly, bOnly]),
        );

        await a.configureSync(cfg: _offlineConfig());
        await b.configureSync(cfg: _offlineConfig());
        await a.updateEntry(
          uuid: sharedId,
          edit: const EntryEditDto(title: 'Earlier A value'),
        );
        await Future<void>.delayed(const Duration(milliseconds: 1200));
        await b.updateEntry(
          uuid: sharedId,
          edit: const EntryEditDto(title: 'Later B value'),
        );
        await a.configureSync(cfg: _config());
        await b.configureSync(cfg: _config());
        await a.syncNow();
        await b.syncNow();
        expect((await b.entryDetail(uuid: sharedId)).title, 'Later B value');
        expect(
          (await b.entryHistory(uuid: sharedId)).map((entry) => entry.title),
          contains('Earlier A value'),
        );
        await a.syncNow();
        expect((await a.entryDetail(uuid: sharedId)).title, 'Later B value');

        await a.configureSync(cfg: _offlineConfig());
        await b.configureSync(cfg: _offlineConfig());
        await _waitForFreshSecond();
        await Future.wait([
          a.updateEntry(
            uuid: sharedId,
            edit: const EntryEditDto(title: 'Unresolvable A side'),
          ),
          b.updateEntry(
            uuid: sharedId,
            edit: const EntryEditDto(title: 'Unresolvable B side'),
          ),
        ]);
        await a.configureSync(cfg: _config());
        await b.configureSync(cfg: _config());
        await a.syncNow();
        HidlinsApiError_SyncConflictUnresolvable? conflict;
        try {
          await b.syncNow();
        } on HidlinsApiError_SyncConflictUnresolvable catch (error) {
          conflict = error;
        }
        expect(
          conflict,
          isNotNull,
          reason:
              'same-second divergence must not be mislabeled as a normal merge',
        );
        final backup = File(conflict!.backupPath);
        expect(backup.existsSync(), isTrue);
        expect(
          (await a.entryDetail(uuid: sharedId)).title,
          'Unresolvable A side',
        );

        final recovery = await api.initApp(
          cfg: AppInitConfig(stateDir: recoveryDir.path),
        );
        sessions.add(recovery);
        await recovery.registerExistingVault(
          name: 'pre-conflict-b',
          kdbxPath: backup.path,
        );
        await recovery.unlock(name: 'pre-conflict-b', masterPassword: _master);
        expect(
          (await recovery.entryDetail(uuid: sharedId)).title,
          'Unresolvable B side',
        );

        final registry = File('${aDir.path}/vaults.toml');
        final beforePasswordChange = registry.readAsStringSync();
        await a.changeMasterPassword(current: _master, newPassword: _newMaster);
        final afterPasswordChange = registry.readAsStringSync();
        expect(afterPasswordChange, isNot(equals(beforePasswordChange)));
        expect(
          afterPasswordChange,
          isNot(
            contains(
              _requiredConfig(
                'HIDLINS_TEST_S3_SECRET_KEY',
                'HIDLINS_MINIO_SECRET_KEY',
              ),
            ),
          ),
        );
        expect(afterPasswordChange, contains('secret_access_key_encrypted'));
        await a.syncNow();
        await a.lockNow();
        await a.unlock(name: 'alpha-a', masterPassword: _newMaster);

        expect(
          a.reportLifecycleState(state: LifecycleStateDto.paused),
          LockEvent.unlocked,
        );
        expect(
          a.reportLifecycleState(state: LifecycleStateDto.resumed),
          LockEvent.unlocked,
        );
        expect(
          a.reportLifecycleState(state: LifecycleStateDto.detached),
          LockEvent.locked,
        );
        await expectLater(
          a.entryDetail(uuid: sharedId),
          throwsA(isA<HidlinsApiError_VaultLocked>()),
        );
        expect(
          a.reportLifecycleState(state: LifecycleStateDto.resumed),
          LockEvent.locked,
        );
        await a.unlock(name: 'alpha-a', masterPassword: _newMaster);
        await a.syncNow();
      } finally {
        for (final session in sessions.reversed) {
          await session.shutdown();
        }
        for (final directory in [
          aDir,
          bDir,
          wrongDir,
          rollbackDir,
          recoveryDir,
        ]) {
          if (directory.existsSync()) directory.deleteSync(recursive: true);
        }
      }
    },
    timeout: const Timeout(Duration(minutes: 6)),
  );
}

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;

import 'real_bridge_harness.dart';

bridge.EntryDraftDto testCredential(String title) => bridge.EntryDraftDto(
  kind: bridge.EntryKindDto.credential,
  title: title,
  username: 'user',
  password: 'test-only-secret',
  tags: const [],
  customFields: const [],
);

class SyncHarness {
  SyncHarness({
    required this.a,
    required this.b,
    required this.aDir,
    required this.bDir,
    required this.aRoot,
    required this.bRoot,
    required this.config,
  });

  static const _name = 'desktop-sync';
  static const _password = 'test-password';

  final api.AppSession a;
  final api.AppSession b;
  final Directory aDir;
  final Directory bDir;
  final String aRoot;
  final String bRoot;
  final bridge.S3ConfigDto config;

  static Future<SyncHarness> create(String label) async {
    final config = bridge.S3ConfigDto(
      bucket: _requiredEnv('HIDLINS_MINIO_BUCKET'),
      key:
          'app-integration-$label-${DateTime.now().microsecondsSinceEpoch}.kdbx',
      region: _requiredEnv('HIDLINS_MINIO_REGION'),
      endpoint: _requiredEnv('HIDLINS_MINIO_ENDPOINT'),
      pathStyle: true,
      accessKeyId: _requiredEnv('HIDLINS_MINIO_ACCESS_KEY'),
      secretAccessKey: _requiredEnv('HIDLINS_MINIO_SECRET_KEY'),
    );

    Directory? aDir;
    Directory? bDir;
    api.AppSession? a;
    api.AppSession? b;
    try {
      aDir = await createTestVault(name: _name, password: _password);
      bDir = Directory.systemTemp.createTempSync('hidlins-app-sync-b-');
      a = await api.initApp(cfg: bridge.AppInitConfig(stateDir: aDir.path));
      final aTree = await a.unlock(name: _name, masterPassword: _password);
      await a.configureSync(cfg: config);
      await a.syncNow();

      b = await api.initApp(cfg: bridge.AppInitConfig(stateDir: bDir.path));
      await b.bootstrapVaultFromRemote(
        name: _name,
        cfg: config,
        masterPassword: _password,
      );
      final bTree = await b.unlock(name: _name, masterPassword: _password);
      return SyncHarness(
        a: a,
        b: b,
        aDir: aDir,
        bDir: bDir,
        aRoot: aTree.root.uuid,
        bRoot: bTree.root.uuid,
        config: config,
      );
    } catch (_) {
      await _cleanupSessionAndDirectory(b, bDir);
      await _cleanupSessionAndDirectory(a, aDir);
      rethrow;
    }
  }

  Future<void> pauseAutoSync() async {
    await a.clearSyncConfig(name: _name);
    await b.clearSyncConfig(name: _name);
  }

  Future<void> resumeSync() async {
    await a.configureSync(cfg: config);
    await b.configureSync(cfg: config);
  }

  Future<void> dispose() async {
    Object? firstError;
    StackTrace? firstStackTrace;

    Future<void> attempt(Future<void> Function() action) async {
      try {
        await action();
      } on Object catch (error, stackTrace) {
        firstError ??= error;
        firstStackTrace ??= stackTrace;
      }
    }

    await attempt(a.shutdown);
    await attempt(b.shutdown);
    await attempt(() async {
      if (aDir.existsSync()) aDir.deleteSync(recursive: true);
    });
    await attempt(() async {
      if (bDir.existsSync()) bDir.deleteSync(recursive: true);
    });

    if (firstError case final error?) {
      Error.throwWithStackTrace(error, firstStackTrace!);
    }
  }
}

Future<void> _cleanupSessionAndDirectory(
  api.AppSession? session,
  Directory? directory,
) async {
  if (session != null) {
    try {
      await session.shutdown();
    } on Object {
      // Preserve the setup failure; cleanup continues for every resource.
    }
  }
  if (directory != null && directory.existsSync()) {
    try {
      directory.deleteSync(recursive: true);
    } on Object {
      // Preserve the setup failure; there is no useful harness to return.
    }
  }
}

String _requiredEnv(String name) {
  final value = Platform.environment[name];
  if (value == null || value.isEmpty) {
    fail('$name is unset; run via `make app-test-integration-minio`');
  }
  return value;
}

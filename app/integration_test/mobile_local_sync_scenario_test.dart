import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:integration_test/integration_test.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart';
import 'package:app/src/bridge/error.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/models.dart'
    show DiscoveryKind, LocalDiscoveryPermission, LocalDiscoveryStatus;
import 'package:app/src/platform/rust_library.dart';

const _phase = String.fromEnvironment('HIDLINS_SCENARIO_PHASE');
const _controlHost = String.fromEnvironment('HIDLINS_SCENARIO_CONTROL_HOST');
const _controlPort = int.fromEnvironment('HIDLINS_SCENARIO_CONTROL_PORT');
const _token = String.fromEnvironment('HIDLINS_SCENARIO_TOKEN');
const _master = 'integration-master-marker';
const _vaultName = 'authority';
const _authorityEntry = 'authority-origin';
const _clientEntry = 'mobile-origin';

Directory _scenarioStateDirectory() => Platform.isAndroid
    ? Directory(
        '${Directory.systemTemp.parent.path}/files/hidlins-mobile-local-sync-v1',
      )
    : Directory('${Directory.systemTemp.path}/hidlins-mobile-local-sync-v1');

Future<Map<String, dynamic>> _control(
  String path, {
  String method = 'GET',
  Map<String, String>? body,
}) async {
  final client = HttpClient()..connectionTimeout = const Duration(seconds: 10);
  try {
    final uri = Uri(
      scheme: 'http',
      host: _controlHost,
      port: _controlPort,
      path: path,
    );
    final request = method == 'POST'
        ? await client.postUrl(uri).timeout(const Duration(seconds: 10))
        : await client.getUrl(uri).timeout(const Duration(seconds: 10));
    request.headers.set('X-Hidlins-Scenario', _token);
    if (body != null) {
      final encoded = utf8.encode(jsonEncode(body));
      request.headers.contentType = ContentType.json;
      request.contentLength = encoded.length;
      request.add(encoded);
    }
    final response = await request.close().timeout(const Duration(seconds: 60));
    final payload = jsonDecode(await utf8.decoder.bind(response).join())
        .cast<String, dynamic>();
    expect(response.statusCode, HttpStatus.ok);
    expect(payload['ok'], isTrue);
    return payload;
  } finally {
    client.close(force: true);
  }
}

Future<void> _uploadRestartSnapshot(Directory state) async {
  final registry = File('${state.path}/vaults.toml');
  final vault = File('${state.path}/$_vaultName.kdbx');
  expect(registry.existsSync(), isTrue);
  expect(vault.existsSync(), isTrue);
  await _control(
    '/snapshot',
    method: 'POST',
    body: {
      'registry': base64Encode(await registry.readAsBytes()),
      'vault': base64Encode(await vault.readAsBytes()),
    },
  );
}

Future<void> _restoreRestartSnapshot(Directory state) async {
  final payload = await _control('/snapshot');
  state.createSync(recursive: true);
  final vault = File('${state.path}/$_vaultName.kdbx');
  await vault.writeAsBytes(
    base64Decode(payload['vault'] as String),
    flush: true,
  );
  final original = utf8.decode(base64Decode(payload['registry'] as String));
  final replacement = 'path = "${vault.path}"';
  final restored = original.replaceFirst(
    RegExp(r'^path = ".*"$', multiLine: true),
    replacement,
  );
  expect(restored, isNot(original));
  await File('${state.path}/vaults.toml').writeAsString(restored, flush: true);
}

Future<void> _expectMobileServerRejected(api.AppSession session) async {
  await expectLater(
    session.localServerEndpoints(),
    throwsA(isA<HidlinsApiError_UnsupportedPlatform>()),
  );
  await expectLater(
    session.startSyncServer(
      endpoint: const LocalEndpointDto(
        address: '127.0.0.1',
        port: 1,
        scopeId: 0,
      ),
    ),
    throwsA(isA<HidlinsApiError_UnsupportedPlatform>()),
  );
}

Future<SyncEvent> _unlockAndWaitForStartup(
  api.AppSession session,
  BridgeSyncRepository sync,
) async {
  final completed = Completer<SyncEvent>();
  final subscription = session.syncEvents().listen((event) {
    if (!completed.isCompleted &&
        (event is SyncEvent_Done || event is SyncEvent_Failed)) {
      completed.complete(event);
    }
  });
  try {
    // ignore: avoid_print
    print('HIDLINS_STARTUP_CHECKPOINT=before-unlock');
    await session
        .unlock(name: _vaultName, masterPassword: _master)
        .timeout(const Duration(seconds: 30));
    // ignore: avoid_print
    print('HIDLINS_STARTUP_CHECKPOINT=after-unlock');
    // A newly joined emulator can briefly retain a stale private DNS-SD
    // answer. Repeat the shipping platform discovery until one of its own
    // candidates is reachable, then exercise the Rust startup-sync operation.
    // No address is supplied by the host or selected outside discovery.
    await _discoverAndReport(sync, DiscoveryKind.trusted);
    await session.startStartupSync().timeout(const Duration(seconds: 15));
    // ignore: avoid_print
    print('HIDLINS_STARTUP_CHECKPOINT=after-start-request');
    await _reportRustDiscovery(session);
    return await completed.future.timeout(const Duration(seconds: 30));
  } finally {
    await subscription.cancel();
  }
}

Future<void> _reportRustDiscovery(api.AppSession session) async {
  final checked = await session.localDiscoveryStatus();
  expect(checked.permission, DiscoveryPermissionDto.granted);
  expect(checked.candidates, isNotEmpty);
  for (final candidate in checked.candidates) {
    // The repository's shipping startup path performed native discovery and
    // passed these routes through Rust policy. Reporting afterward avoids a
    // second discovery pass that could replace a fresh short-lived result.
    // ignore: avoid_print
    print(
      'HIDLINS_DISCOVERY_CANDIDATE='
      '${candidate.address}:${candidate.port}:${candidate.scopeId}',
    );
  }
}

Future<bool> _hasTitle(api.AppSession session, String title) async {
  final matches = await session.search(
    opts: SearchOptionsDto(
      query: title,
      mode: SearchModeDto.substring,
      scope: const SearchScopeDto.all(),
      includeRecycled: false,
    ),
  );
  return matches.any((match) => match.entry.title == title);
}

Future<void> _discoverAndReport(
  BridgeSyncRepository sync,
  DiscoveryKind kind,
) async {
  final deadline = DateTime.now().add(const Duration(seconds: 60));
  LocalDiscoveryStatus? discovered;
  while (DateTime.now().isBefore(deadline)) {
    final status = await sync.discover(kind);
    expect(status.permission, LocalDiscoveryPermission.granted);
    if (status.candidates.isNotEmpty &&
        await _waitForReachableDiscoveredCandidate(status, deadline)) {
      discovered = status;
      break;
    }
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
  expect(discovered, isNotNull);
  final ready = discovered!;
  for (final candidate in ready.candidates) {
    // Routing metadata comes exclusively from the shipping platform discovery
    // adapter. The host verifies every marker against the authority emulator's
    // independently observed wlan0 route.
    // ignore: avoid_print
    print(
      'HIDLINS_DISCOVERY_CANDIDATE='
      '${candidate.address}:${candidate.port}:${candidate.scopeId}',
    );
  }
}

Future<bool> _waitForReachableDiscoveredCandidate(
  LocalDiscoveryStatus discovered,
  DateTime overallDeadline,
) async {
  final candidateDeadline = DateTime.now().add(const Duration(seconds: 10));
  while (DateTime.now().isBefore(candidateDeadline) &&
      DateTime.now().isBefore(overallDeadline)) {
    for (final candidate in discovered.candidates) {
      try {
        final socket = await Socket.connect(
          candidate.address,
          candidate.port,
          timeout: const Duration(seconds: 2),
        );
        socket.destroy();
        return true;
      } on SocketException {
        // Android can retain a stale private DNS-SD answer after the client
        // rejoins the emulator Wi-Fi network. Retry briefly for neighbor-table
        // readiness, then require a later native scan to refresh the route.
        // The test never fabricates a candidate.
      }
    }
    await Future<void>.delayed(const Duration(milliseconds: 250));
  }
  return false;
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    expect(Platform.isIOS || Platform.isAndroid, isTrue);
    expect(['discovery', 'pair', 'restart'], contains(_phase));
    if (_phase != 'discovery') {
      expect(_controlPort, greaterThan(0));
      expect(_token, isNotEmpty);
    }
    await initializeBundledRustLibrary();
  });

  test('mobile client completes the separate CLI authority scenario', () async {
    final state = _scenarioStateDirectory();
    if (_phase == 'pair' && state.existsSync()) {
      state.deleteSync(recursive: true);
    }
    if (_phase == 'restart') {
      if (state.existsSync()) state.deleteSync(recursive: true);
      await _restoreRestartSnapshot(state);
    }
    final session = await api.initApp(cfg: AppInitConfig(stateDir: state.path));
    final sync = BridgeSyncRepository(session, useNativeDiscovery: true);
    var shutdown = false;
    try {
      await _expectMobileServerRejected(session);
      if (_phase == 'discovery') {
        // The host runner grants the same runtime permission a user approves
        // after this direct-launch test process is alive. No network route is
        // provided during this bounded readiness interval.
        await Future<void>.delayed(const Duration(seconds: 2));
        await _discoverAndReport(sync, DiscoveryKind.pairing);
        final checked = await session.localDiscoveryStatus();
        expect(checked.permission, DiscoveryPermissionDto.granted);
        expect(checked.candidates, isNotEmpty);
        final route = checked.candidates.first;
        for (final candidate in checked.candidates) {
          // This marker contains routing metadata only. The host harness checks
          // it against the authority emulator's independently observed wlan0
          // address and listener; no route is supplied to this application.
          // ignore: avoid_print
          print(
            'HIDLINS_DISCOVERY_CANDIDATE='
            '${candidate.address}:${candidate.port}:${candidate.scopeId}',
          );
        }
        final socket = await Socket.connect(
          route.address,
          route.port,
          timeout: const Duration(seconds: 5),
        );
        socket.destroy();
        // ignore: avoid_print
        print('HIDLINS_DISCOVERY_CLI_TCP_CONNECTED=true');
        final rejected = await sync.beginPairImport(
          name: _vaultName,
          masterPassword: _master,
        );
        expect(
          await sync.confirmPairing(
            transactionHandle: rejected.transactionHandle,
            accepted: false,
            peerDisplayName: 'Discovery smoke client',
          ),
          isNull,
        );
        // This is emitted only after a real Noise XX connection to the route
        // selected by native discovery. Rejecting SAS prevents trust or vault
        // transfer while proving the listener is the actual CLI authority.
        // ignore: avoid_print
        print('HIDLINS_DISCOVERY_CLI_NOISE_CONNECTED=true');
        return;
      }
      if (_phase == 'pair') {
        await _discoverAndReport(sync, DiscoveryKind.pairing);
        final rejected = await sync.beginPairImport(
          name: _vaultName,
          masterPassword: _master,
        );
        expect(rejected.toString(), isNot(contains(rejected.sas)));
        expect(
          await sync.confirmPairing(
            transactionHandle: rejected.transactionHandle,
            accepted: false,
            peerDisplayName: 'Rejected mobile client',
          ),
          isNull,
        );
        await _control('/assert-no-peer');

        await _discoverAndReport(sync, DiscoveryKind.pairing);
        final accepted = await sync.beginPairImport(
          name: _vaultName,
          masterPassword: _master,
        );
        final imported = await sync.confirmPairing(
          transactionHandle: accepted.transactionHandle,
          accepted: true,
          peerDisplayName: 'Simulator client',
        );
        expect(imported?.name, _vaultName);
        await _control('/service/trusted');
        expect(
          await _unlockAndWaitForStartup(session, sync),
          isA<SyncEvent_Done>(),
        );

        await _control('/authority-add');
        // Follow the same path as SyncController.syncNow(): every manual
        // request refreshes platform discovery before entering the Rust sync
        // operation. This also waits for a restarted authority discovered by
        // NSD to be reachable without supplying an endpoint from the harness.
        await _discoverAndReport(sync, DiscoveryKind.trusted);
        await session.syncNow();
        expect(await _hasTitle(session, _authorityEntry), isTrue);

        final root = (await session.vaultTree()).root.uuid;
        await session.createEntry(
          group: root,
          draft: const EntryDraftDto(
            kind: EntryKindDto.credential,
            title: _clientEntry,
            tags: [],
            customFields: [],
          ),
        );
        await Future<void>.delayed(const Duration(seconds: 2));
        await _control('/assert-client-absent');
        await _discoverAndReport(sync, DiscoveryKind.trusted);
        await session.syncNow();
        await _control('/assert-client-present');
        await session.shutdown();
        shutdown = true;
        await _uploadRestartSnapshot(state);
      } else {
        expect(
          await _unlockAndWaitForStartup(session, sync),
          isA<SyncEvent_Done>(),
        );
        final peers = await session.listSyncPeers();
        expect(peers, hasLength(1));
        expect(peers.single.revoked, isFalse);
        expect(await _hasTitle(session, _authorityEntry), isTrue);
        expect(await _hasTitle(session, _clientEntry), isTrue);

        await _control('/restart');
        await _discoverAndReport(sync, DiscoveryKind.trusted);
        await session.syncNow();
        await _control('/revoke');
        await _discoverAndReport(sync, DiscoveryKind.trusted);
        await expectLater(
          session.syncNow(),
          throwsA(
            anyOf(
              isA<HidlinsApiError_SyncAuthFailed>(),
              isA<HidlinsApiError_SyncRevoked>(),
              isA<HidlinsApiError_SyncOffline>(),
            ),
          ),
        );
      }
    } finally {
      if (!shutdown) await session.shutdown();
      if (_phase == 'restart' && state.existsSync()) {
        state.deleteSync(recursive: true);
      }
    }
  }, timeout: const Timeout(Duration(minutes: 6)));
}

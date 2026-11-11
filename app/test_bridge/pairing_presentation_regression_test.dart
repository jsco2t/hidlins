import 'dart:async';
import 'dart:io';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/sync/sync_controller.dart';
import 'package:app/src/features/sync/sync_page.dart';
import 'package:app/src/providers/lock_state_provider.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../test/fakes/fake_repositories.dart';
import '../test/helpers/test_helpers.dart';

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-pairing-presentation');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });
  testWidgets(
    'real inbound pairing received off-page is shown and confirms only its matching code',
    (tester) async {
      final directory = Directory.systemTemp.createTempSync('hidlins-prompt-');
      final server = (await tester.runAsync(
        () => api.initApp(
          cfg: bridge.AppInitConfig(stateDir: '${directory.path}/server'),
        ),
      ))!;
      final client = (await tester.runAsync(
        () => api.initApp(
          cfg: bridge.AppInitConfig(stateDir: '${directory.path}/client'),
        ),
      ))!;
      final session = FakeSessionRepository()
        ..currentLockState = LockEvent.unlocked;
      ProviderContainer? container;
      try {
        final clientPrompt = (await tester.runAsync(() async {
          for (final native in [server, client]) {
            await native.createVault(
              name: 'fixture',
              masterPassword: 'presentation-fixture-marker',
              confirmedNoRecovery: true,
            );
            await native.unlock(
              name: 'fixture',
              masterPassword: 'presentation-fixture-marker',
            );
          }
          await server.configureLocalSync(role: bridge.LocalSyncRoleDto.server);
          await client.configureLocalSync(role: bridge.LocalSyncRoleDto.client);
          container = ProviderContainer(
            overrides: [
              sessionRepositoryProvider.overrideWithValue(session),
              syncRepositoryProvider.overrideWithValue(
                BridgeSyncRepository(server),
              ),
            ],
          );
          await container!.read(lockStateProvider.future);
          await container!.read(syncControllerProvider.future);
          final probe = await ServerSocket.bind(
            InternetAddress.loopbackIPv4,
            0,
          );
          final endpoint = bridge.LocalEndpointDto(
            address: '127.0.0.1',
            port: probe.port,
            scopeId: 0,
          );
          await probe.close();
          await server.startSyncServer(endpoint: endpoint);
          await server.openPairingWindow();
          await client.setDiscoveryCandidates(
            permission: bridge.DiscoveryPermissionDto.granted,
            candidates: [endpoint],
          );
          final prompt = await client.beginPairing().timeout(
            const Duration(seconds: 10),
          );
          final elapsed = Stopwatch()..start;
          while (container!.read(syncControllerProvider).valueOrNull?.prompt ==
                  null &&
              elapsed.elapsed < const Duration(seconds: 10)) {
            await Future<void>.delayed(const Duration(milliseconds: 1));
          }
          return prompt;
        }))!;
        final pending = container!
            .read(syncControllerProvider)
            .requireValue
            .prompt;
        expect(
          pending,
          isNotNull,
          reason: 'actual Rust event reached off-page controller',
        );
        expect(
          pending!.sas,
          clientPrompt.sas,
          reason: 'both endpoints see the same transcript code',
        );
        final navigator = GlobalKey<NavigatorState>();
        await tester.pumpApp(
          UncontrolledProviderScope(
            container: container!,
            child: Navigator(
              key: navigator,
              onGenerateRoute: (_) => MaterialPageRoute<void>(
                builder: (_) => const Scaffold(body: Text('entries-marker')),
              ),
            ),
          ),
        );
        await tester.pump();
        unawaited(
          navigator.currentState!.push(
            MaterialPageRoute<void>(
              builder: (_) => const SyncPage(desktopOverride: true),
            ),
          ),
        );
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 350));
        await tester.pump();
        final sas = find.byKey(const ValueKey('pairing-sas'));
        expect(sas, findsOneWidget);
        expect(tester.widget<SelectableText>(sas).data, clientPrompt.sas);
        await tester.tap(find.text('Codes match'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 350));
        await tester.runAsync(
          () => client.confirmPairing(
            transactionHandle: clientPrompt.transactionHandle,
            accepted: true,
            peerDisplayName: 'server-fixture',
          ),
        );
        // The UI confirmation/refresh completes in the widget zone while the
        // actual Noise pairing and persistence complete on native workers.
        final elapsed = Stopwatch()..start;
        var paired = false;
        while (!paired && elapsed.elapsed < const Duration(seconds: 10)) {
          await tester.pump();
          paired = (await tester.runAsync(
            () async =>
                (await server.syncStatus()).activePeerCount == BigInt.one,
          ))!;
          if (!paired) {
            await tester.runAsync(
              () => Future<void>.delayed(const Duration(milliseconds: 1)),
            );
          }
        }
        expect(paired, isTrue);
        expect((await tester.runAsync(client.syncStatus))!.paired, isTrue);
        expect(tester.takeException(), isNull);
      } finally {
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump();
        await tester.runAsync(() async {
          await server.cancelSync();
          await client.cancelSync();
        });
        await tester.pump();
        container?.dispose();
        session.dispose();
        await tester.runAsync(() async {
          await server.shutdown();
          await client.shutdown();
        });
        await tester.pump();
        directory.deleteSync(recursive: true);
      }
    },
  );
}

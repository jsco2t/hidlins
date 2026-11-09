import 'dart:async';
import 'dart:io';

import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/vaults/connect_sync_dialog.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';
import 'package:app/src/platform/local_discovery.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

class _Discovery implements LocalDiscoveryCapability {
  final entered = Completer<void>();
  final pending = Completer<PlatformResult<LocalDiscoveryStatus>>();
  int stops = 0;
  @override
  Future<PlatformResult<LocalDiscoveryStatus>> discover(DiscoveryKind kind) {
    entered.complete();
    return pending.future;
  }

  @override
  Future<PlatformResult<PlatformUnit>> stop() async {
    stops++;
    return const PlatformSuccess(platformUnit);
  }

  @override
  Future<PlatformResult<LocalDiscoveryStatus>> permissionStatus() async =>
      const PlatformSuccess(
        LocalDiscoveryStatus(permission: LocalDiscoveryPermission.granted),
      );
  @override
  Future<PlatformResult<PlatformUnit>> openSettings() async =>
      const PlatformSuccess(platformUnit);
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-import-lifetime');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  testWidgets('disposed locked import safely drains real bridge discovery', (
    tester,
  ) async {
    final directory = Directory.systemTemp.createTempSync('hidlins-import-');
    final native = (await tester.runAsync(
      () => api.initApp(cfg: bridge.AppInitConfig(stateDir: directory.path)),
    ))!;
    final discovery = (await tester.runAsync(() async => _Discovery()))!;
    final container = ProviderContainer(
      overrides: [
        syncRepositoryProvider.overrideWithValue(
          BridgeSyncRepository(
            native,
            useNativeDiscovery: true,
            nativeDiscovery: discovery,
          ),
        ),
      ],
    );
    Future<Object?>? operation;
    try {
      // No unlocked fake/native mismatch: import admission intentionally starts
      // with the real Rust session locked and discovery never dials a peer.
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            localizationsDelegates: hidlinsLocalizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: const ConnectSyncDialog(mobileOverride: false),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Vault name'),
        'bridge-fixture',
      );
      await tester.enterText(
        find.widgetWithText(TextFormField, 'Master password'),
        'bridge-fixture-marker',
      );
      final callback = tester
          .widget<FilledButton>(
            find.widgetWithText(FilledButton, 'Import paired vault'),
          )
          .onPressed!;
      await tester.runAsync(() async {
        operation = (Function.apply(callback, const []) as Future<void>)
            .then<Object?>(
              (_) => null,
              onError: (Object error, StackTrace stack) => error,
            );
        await discovery.entered.future;
      });
      await tester.pumpWidget(const SizedBox.shrink());
      Object? error;
      var finished = false;
      unawaited(
        operation!.then((value) {
          error = value;
          finished = true;
        }),
      );
      await tester.runAsync(() async {
        discovery.pending.complete(
          const PlatformSuccess(
            LocalDiscoveryStatus(permission: LocalDiscoveryPermission.granted),
          ),
        );
      });
      // Native completion and the cleanup queued by widget disposal use
      // different scheduler zones. Let both run until the callback settles.
      final elapsed = Stopwatch()..start;
      while (!finished && elapsed.elapsed < const Duration(seconds: 10)) {
        await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 1)),
        );
        await tester.pump();
      }
      expect(finished, isTrue, reason: 'native callback and cleanup settled');
      expect(error, isNull, reason: 'no disposed password-controller access');
      expect(discovery.stops, 1);
      expect(tester.takeException(), isNull);
    } finally {
      await tester.pumpWidget(const SizedBox.shrink());
      if (!discovery.pending.isCompleted) {
        discovery.pending.complete(const PlatformCanceled());
      }
      if (operation != null) await tester.runAsync(() => operation!);
      container.dispose();
      await tester.runAsync(native.shutdown);
      directory.deleteSync(recursive: true);
    }
  });
}

import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/error.dart';
import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/sync/sync_page.dart';

import '../helpers/feature_test_helpers.dart';

void main() {
  group('SyncPage', () {
    testWidgets('pairs with bilateral SAS and exposes no server on mobile', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(
        const SyncPage(desktopOverride: false),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Configure as sync server'), findsNothing);
      await tester.tap(find.text('Pair this vault'));
      await tester.pumpAndSettle();
      expect(find.text('Allow local network access'), findsOneWidget);
      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(harness.sync.configureCalled, isTrue);
      expect(harness.sync.configuredRole, LocalSyncRole.client);
      expect(find.text('Compare this code on both devices'), findsOneWidget);
      expect(find.text('123 456'), findsOneWidget);
      expect(
        find.bySemanticsLabel('Compare this code on both devices: 123 456'),
        findsOneWidget,
      );
      await tester.tap(find.text('Codes match'));
      await tester.pumpAndSettle();
      expect(find.text('123 456'), findsNothing);
    });

    testWidgets('desktop server controls are explicit and pairing is bounded', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(
        const SyncPage(desktopOverride: true),
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      expect(find.text('Configure as sync server'), findsOneWidget);
      await tester.tap(find.text('Configure as sync server'));
      await tester.pumpAndSettle();
      expect(harness.sync.startServerCalled, isTrue);
      expect(find.text('Stop sync server'), findsOneWidget);
      expect(find.text('Allow pairing for 3 minutes'), findsOneWidget);

      await tester.tap(find.text('Allow pairing for 3 minutes'));
      await tester.pump();
      expect(harness.sync.openPairingWindowCalled, isTrue);
      expect(find.byKey(const ValueKey('pairing-countdown')), findsOneWidget);
      expect(find.textContaining('Pairing open:'), findsOneWidget);

      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump();
    });

    testWidgets('permission denial keeps mobile offline and links settings', (
      tester,
    ) async {
      final harness = TestHarness();
      harness.sync.discoverError = const PlatformOperationFailure(
        capability: 'local network',
        state: 'denied',
      );
      await tester.pumpFeatureWithHarness(
        const SyncPage(desktopOverride: false),
        harness,
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Pair this vault'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();
      expect(
        find.textContaining('Local network access is denied'),
        findsOneWidget,
      );
      expect(find.text('Retry discovery'), findsOneWidget);
      expect(find.text('Open system settings'), findsOneWidget);
      await tester.tap(find.text('Open system settings'));
      await tester.pump();
      expect(harness.sync.openDiscoverySettingsCalled, isTrue);
    });

    testWidgets('peer rename and revocation remain reachable', (tester) async {
      final harness = TestHarness();
      harness.sync.status = const SyncStatusDto(
        configured: true,
        inFlight: false,
        role: LocalSyncRole.server,
        activePeerCount: 1,
        serverEnabled: true,
        serverRunning: true,
      );
      harness.sync.peers = const [
        SyncPeer(peerId: 'peer-0', displayName: 'Phone', revoked: false),
      ];
      await tester.pumpFeatureWithHarness(
        const SyncPage(desktopOverride: true),
        harness,
      );
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.byTooltip('Rename'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField).last, 'Tablet');
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
      expect(find.text('Tablet'), findsOneWidget);

      await tester.tap(find.byTooltip('Revoke'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(FilledButton, 'Revoke'));
      await tester.pumpAndSettle();
      expect(harness.sync.peers.single.revoked, isTrue);
    });

    testWidgets('mobile sync layout supports phone, tablet, and large text', (
      tester,
    ) async {
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetDevicePixelRatio);
      addTearDown(tester.view.resetPhysicalSize);
      final harness = TestHarness();
      harness.sync.status = const SyncStatusDto(
        configured: true,
        inFlight: false,
        role: LocalSyncRole.client,
        paired: true,
      );
      addTearDown(harness.dispose);

      for (final size in const [Size(390, 844), Size(1024, 1366)]) {
        tester.view.physicalSize = size;
        await tester.pumpFeatureWithHarness(
          MediaQuery(
            data: MediaQueryData(
              size: size,
              textScaler: const TextScaler.linear(2),
            ),
            child: const SyncPage(desktopOverride: false),
          ),
          harness,
        );
        await tester.pumpAndSettle();
        await tester.scrollUntilVisible(
          find.text('Sync now'),
          100,
          scrollable: find.byType(Scrollable).first,
        );
        expect(find.text('Sync now'), findsOneWidget);
        expect(find.text('Start sync server'), findsNothing);
        expect(tester.takeException(), isNull);
      }
    });

    testWidgets('renders progress and every successful outcome', (
      tester,
    ) async {
      final harness = TestHarness();
      harness.sync.status = const SyncStatusDto(
        configured: true,
        inFlight: false,
        role: LocalSyncRole.client,
        paired: true,
      );
      harness.sync.syncNowCompleter = Completer<void>();
      await tester.pumpFeatureWithHarness(const SyncPage(), harness);
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Sync now'));
      await tester.pump();
      expect(
        find.text('Looking for Hidlins devices on your local network…'),
        findsWidgets,
      );
      harness.sync.syncNowCompleter!.complete();
      await tester.pump();

      for (final outcome in <bridge.SyncOutcomeDto>[
        const bridge.SyncOutcomeDto.alreadyInSync(),
        const bridge.SyncOutcomeDto.pushed(isFirstSeed: true),
        const bridge.SyncOutcomeDto.fastReplaced(),
        bridge.SyncOutcomeDto.merged(
          entriesAdded: BigInt.one,
          entriesModified: BigInt.two,
          entriesRemoved: BigInt.zero,
        ),
        const bridge.SyncOutcomeDto.unknown(),
      ]) {
        harness.sync.syncController.add(
          const bridge.SyncEvent.started(automatic: false),
        );
        await tester.pump(const Duration(milliseconds: 1));
        harness.sync.syncController.add(bridge.SyncEvent.done(outcome));
        await tester.pump(const Duration(milliseconds: 1));
        await tester.pump();
        expect(find.byType(SnackBar), findsOneWidget);
        tester
            .state<ScaffoldMessengerState>(find.byType(ScaffoldMessenger))
            .hideCurrentSnackBar();
        await tester.pumpAndSettle();
      }
    });

    testWidgets('keeps cached entries readable while sync disables mutations', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(1200, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final harness = TestHarness();
      harness.sync.status = const SyncStatusDto(
        configured: true,
        inFlight: true,
        role: LocalSyncRole.client,
        paired: true,
      );
      await tester.pumpFeatureWithHarness(const EntriesPage(), harness);
      addTearDown(harness.dispose);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));

      expect(find.text('Syncing…'), findsOneWidget);
      expect(find.text('GitHub'), findsOneWidget);

      await tester.tapAt(
        tester.getCenter(find.text('GitHub')),
        buttons: kSecondaryMouseButton,
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 300));

      expect(find.text('Username'), findsWidgets);
      expect(find.text('Edit'), findsNothing);
      expect(find.text('Delete'), findsNothing);
    });

    testWidgets('shows unresolvable conflict in a non-dismissible dialog', (
      tester,
    ) async {
      final harness = TestHarness();
      harness.sync.status = const SyncStatusDto(
        configured: true,
        inFlight: false,
        role: LocalSyncRole.client,
        paired: true,
      );
      await tester.pumpFeatureWithHarness(const SyncPage(), harness);
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      harness.sync.syncController.add(
        const bridge.SyncEvent.failed(
          HidlinsApiError.syncConflictUnresolvable(
            backupPath: '/vaults/main.kdbx.bak',
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Sync conflict needs attention'), findsWidgets);
      expect(find.textContaining('/vaults/main.kdbx.bak'), findsOneWidget);
      await tester.tapAt(const Offset(5, 5));
      await tester.pump();
      expect(find.text('Sync conflict needs attention'), findsWidgets);
    });

    testWidgets('renders reachable, authentication, and generic failures', (
      tester,
    ) async {
      final harness = TestHarness();
      harness.sync.status = const SyncStatusDto(
        configured: true,
        inFlight: false,
        role: LocalSyncRole.client,
        paired: true,
      );
      await tester.pumpFeatureWithHarness(const SyncPage(), harness);
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      for (final error in const <HidlinsApiError>[
        HidlinsApiError.syncOffline(),
        HidlinsApiError.syncAuthFailed(),
        HidlinsApiError.internal(context: 'redacted'),
      ]) {
        harness.sync.syncController.add(bridge.SyncEvent.failed(error));
        await tester.pump(const Duration(milliseconds: 1));
        expect(find.byKey(const ValueKey('sync-error')), findsOneWidget);
      }
    });
  });
}

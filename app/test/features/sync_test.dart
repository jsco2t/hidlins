import 'package:flutter/gestures.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/error.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/sync/sync_page.dart';

import '../helpers/feature_test_helpers.dart';

void main() {
  group('SyncPage', () {
    testWidgets('configures sync without retaining the secret in the UI', (
      tester,
    ) async {
      final harness = await tester.pumpFeature(const SyncPage());
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.enterText(find.bySemanticsLabel('S3 Bucket'), 'vaults');
      await tester.enterText(find.bySemanticsLabel('Object key'), 'main.kdbx');
      await tester.enterText(find.bySemanticsLabel('Region'), 'us-east-1');
      await tester.enterText(find.bySemanticsLabel('Access key ID'), 'access');
      await tester.enterText(
        find.bySemanticsLabel('Secret access key'),
        'do-not-retain',
      );
      await tester.tap(find.text('Save sync configuration'));
      await tester.pumpAndSettle();

      expect(harness.sync.configureCalled, isTrue);
      expect(harness.sync.lastConfig?.secretAccessKey, 'do-not-retain');
      expect(find.text('do-not-retain'), findsNothing);
      expect(find.text('Sync now'), findsOneWidget);
    });

    testWidgets('renders progress and every successful outcome', (
      tester,
    ) async {
      final harness = TestHarness();
      harness.sync.status = const SyncStatusDto(
        configured: true,
        inFlight: false,
      );
      await tester.pumpFeatureWithHarness(const SyncPage(), harness);
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      await tester.tap(find.text('Sync now'));
      await tester.pump();
      expect(find.text('Syncing…'), findsWidgets);

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
        harness.sync.syncController.add(const bridge.SyncEvent.started());
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
      );
      await tester.pumpFeatureWithHarness(const SyncPage(), harness);
      addTearDown(harness.dispose);
      await tester.pumpAndSettle();

      for (final error in const <HidlinsApiError>[
        HidlinsApiError.syncRemoteUnreachable(endpoint: 'https://minio'),
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

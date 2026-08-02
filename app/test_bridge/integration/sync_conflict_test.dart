import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/error.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/data/repositories.dart';
import 'package:app/src/features/sync/sync_event_listener.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/providers/repository_providers.dart';

import 'real_bridge_harness.dart';
import 'sync_harness.dart';

void main() {
  setUpAll(initializeRealBridge);

  testWidgets(
    'real unresolvable sync error opens a dialog with the real backup path',
    (tester) async {
      final harness = (await tester.runAsync(
        () => SyncHarness.create('conflict'),
      ))!;
      addTearDown(() => tester.runAsync(harness.dispose));

      final conflict = (await tester.runAsync(() async {
        final uuid = await harness.a.createEntry(
          group: harness.aRoot,
          draft: testCredential('Shared entry'),
        );
        await harness.a.syncNow();
        await harness.b.syncNow();
        await harness.pauseAutoSync();

        // KDBX timestamps have one-second resolution. Build and *verify* the
        // irreconcilable production fixture before syncing. Starting early in
        // a wall-clock second and retrying protects the test from landing on
        // opposite sides of a second boundary under asymmetric CI load.
        int? aModified;
        int? bModified;
        for (var attempt = 0; attempt < 5; attempt++) {
          final millisecond = DateTime.now().millisecond;
          if (millisecond > 400) {
            await Future<void>.delayed(
              Duration(milliseconds: 1100 - millisecond),
            );
          }
          await Future.wait([
            harness.a.updateEntry(
              uuid: uuid,
              edit: bridge.EntryEditDto(username: 'desktop-a-$attempt'),
            ),
            harness.b.updateEntry(
              uuid: uuid,
              edit: bridge.EntryEditDto(username: 'desktop-b-$attempt'),
            ),
          ]);
          final details = await Future.wait([
            harness.a.entryDetail(uuid: uuid),
            harness.b.entryDetail(uuid: uuid),
          ]);
          aModified = details[0].lastModificationTime;
          bModified = details[1].lastModificationTime;
          if (aModified == bModified) break;
        }
        expect(
          aModified,
          bModified,
          reason: 'fixture setup must produce equal KDBX modification times',
        );
        await harness.resumeSync();
        await harness.b.syncNow();

        Object? caught;
        try {
          await harness.a.syncNow();
        } on Object catch (error) {
          caught = error;
        }
        expect(caught, isA<HidlinsApiError_SyncConflictUnresolvable>());
        final conflict = caught! as HidlinsApiError_SyncConflictUnresolvable;
        expect(conflict.backupPath, isNotEmpty);
        expect(File(conflict.backupPath).existsSync(), isTrue);
        return conflict;
      }))!;

      final repository = _EventSyncRepository();
      addTearDown(repository.dispose);
      await tester.pumpWidget(
        ProviderScope(
          overrides: [syncRepositoryProvider.overrideWithValue(repository)],
          child: const MaterialApp(
            localizationsDelegates: [
              AppLocalizations.delegate,
              GlobalMaterialLocalizations.delegate,
              GlobalWidgetsLocalizations.delegate,
              GlobalCupertinoLocalizations.delegate,
            ],
            supportedLocales: AppLocalizations.supportedLocales,
            home: Scaffold(body: SyncEventListener(child: SizedBox.expand())),
          ),
        ),
      );
      await tester.pump();
      repository.events.add(bridge.SyncEvent.failed(conflict));
      await tester.pumpAndSettle();

      expect(find.text('Sync needs attention'), findsOneWidget);
      expect(find.text(conflict.backupPath), findsOneWidget);
      expect(find.textContaining('Both sides were preserved'), findsOneWidget);
      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
    },
    timeout: const Timeout(Duration(minutes: 2)),
  );
}

class _EventSyncRepository implements SyncRepository {
  final events = StreamController<bridge.SyncEvent>.broadcast();

  void dispose() => events.close();

  @override
  Stream<bridge.SyncEvent> syncEvents() => events.stream;

  @override
  Stream<ClipboardEvent> clipboardEvents() => const Stream.empty();

  @override
  Future<void> clearSyncConfig(String name) async {}

  @override
  Future<void> configureSync(S3ConfigDto config) async {}

  @override
  Future<void> syncNow() async {}

  @override
  Future<SyncStatusDto> syncStatus() async =>
      const SyncStatusDto(configured: true, inFlight: false);
}

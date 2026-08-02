import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/dto.dart' as bridge;

import 'real_bridge_harness.dart';
import 'sync_harness.dart';

void main() {
  setUpAll(initializeRealBridge);

  test(
    'two real app sessions merge disjoint edits through MinIO',
    () async {
      final harness = await SyncHarness.create('merge');
      addTearDown(harness.dispose);
      await harness.pauseAutoSync();

      final aUuid = await harness.a.createEntry(
        group: harness.aRoot,
        draft: testCredential('From desktop A'),
      );
      final bUuid = await harness.b.createEntry(
        group: harness.bRoot,
        draft: testCredential('From desktop B'),
      );
      final sharedUuid = await harness.a.createEntry(
        group: harness.aRoot,
        draft: testCredential('Shared collision entry'),
      );
      await harness.resumeSync();
      await harness.a.syncNow();
      final outcome = await harness.b.syncNow();
      expect(outcome, isA<bridge.SyncOutcomeDto_Merged>());
      await harness.a.syncNow();

      final aTree = await harness.a.vaultTree();
      expect(
        aTree.entries.map((entry) => entry.uuid),
        containsAll([aUuid, bUuid]),
      );

      await harness.pauseAutoSync();
      await harness.a.updateEntry(
        uuid: sharedUuid,
        edit: const bridge.EntryEditDto(username: 'desktop-a-loser'),
      );
      await Future<void>.delayed(const Duration(milliseconds: 1200));
      await harness.b.updateEntry(
        uuid: sharedUuid,
        edit: const bridge.EntryEditDto(username: 'desktop-b-winner'),
      );
      final modified = await Future.wait([
        harness.a.entryDetail(uuid: sharedUuid),
        harness.b.entryDetail(uuid: sharedUuid),
      ]);
      expect(
        modified[0].lastModificationTime,
        lessThan(modified[1].lastModificationTime!),
        reason: 'the collision fixture must have an unambiguous later winner',
      );

      await harness.resumeSync();
      await harness.a.syncNow();
      final collisionOutcome = await harness.b.syncNow();
      expect(collisionOutcome, isA<bridge.SyncOutcomeDto_Merged>());
      await harness.a.syncNow();

      final merged = await harness.a.entryDetail(uuid: sharedUuid);
      expect(merged.username, 'desktop-b-winner');
      final history = await harness.a.entryHistory(uuid: sharedUuid);
      expect(
        history.map((snapshot) => snapshot.username),
        contains('desktop-a-loser'),
        reason: 'the collision loser must remain recoverable in KDBX history',
      );
    },
    timeout: const Timeout(Duration(minutes: 2)),
  );
}

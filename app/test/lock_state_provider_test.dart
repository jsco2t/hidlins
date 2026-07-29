import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/dto.dart';
import 'package:app/src/providers/lock_state_provider.dart';
import 'fakes/fake_repositories.dart';

void main() {
  group('failClosedLockEvents', () {
    test('emits locked when the source stream closes', () async {
      final repo = FakeSessionRepository()
        ..currentLockState = LockEvent.unlocked;
      final events = <LockEvent>[];
      final subscription = failClosedLockEvents(repo).listen(events.add);
      addTearDown(subscription.cancel);

      await Future<void>.delayed(Duration.zero);
      expect(events, [LockEvent.unlocked]);

      await repo.lockController.close();
      await Future<void>.delayed(Duration.zero);

      expect(events.last, LockEvent.locked);
    });

    test('emits locked when the dropped-event counter increases', () async {
      final repo = FakeSessionRepository()
        ..currentLockState = LockEvent.unlocked;
      final events = <LockEvent>[];
      final subscription = failClosedLockEvents(repo).listen(events.add);
      addTearDown(repo.dispose);
      addTearDown(subscription.cancel);

      await Future<void>.delayed(Duration.zero);
      expect(events, [LockEvent.unlocked]);

      repo.droppedLockEventCount = BigInt.one;
      await Future<void>.delayed(const Duration(milliseconds: 300));

      expect(events, contains(LockEvent.locked));
    });

    test('a failing dropped-event check locks once while retrying', () async {
      final repo = FakeSessionRepository()
        ..currentLockState = LockEvent.unlocked;
      final events = <LockEvent>[];
      final subscription = failClosedLockEvents(repo).listen(events.add);
      addTearDown(repo.dispose);
      addTearDown(subscription.cancel);

      await Future<void>.delayed(Duration.zero);
      repo.droppedLockEventsError = StateError('bridge unavailable');
      await Future<void>.delayed(const Duration(milliseconds: 600));

      expect(events, [LockEvent.unlocked, LockEvent.locked]);
    });
  });
}

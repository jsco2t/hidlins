import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/models.dart';
import 'package:app/src/data/repositories.dart';
import 'package:app/src/providers/providers.dart';
import '../fakes/fake_repositories.dart';

void main() {
  group('vaultTreeProvider', () {
    test('refetches after sync-done event', () async {
      final entryRepo = FakeEntryRepository();
      final syncRepo = FakeSyncRepository();

      final container = ProviderContainer(
        overrides: [
          entryRepositoryProvider.overrideWithValue(entryRepo),
          syncRepositoryProvider.overrideWithValue(syncRepo),
        ],
      );
      addTearDown(container.dispose);

      final sub = container.listen(
        vaultTreeProvider,
        (_, _) {},
        fireImmediately: true,
      );

      await container.read(vaultTreeProvider.future);
      final initialTree = sub.read().valueOrNull;
      expect(initialTree, isNotNull);

      final updatedTree = VaultTree(
        root: GroupNode(
          uuid: 'root',
          name: 'Root',
          children: const [],
          entryCount: BigInt.zero,
        ),
        entries: const [],
      );
      entryRepo.tree = updatedTree;

      syncRepo.syncController.add(
        SyncEvent.done(
          bridge.SyncOutcomeDto.merged(
            entriesAdded: BigInt.zero,
            entriesModified: BigInt.zero,
            entriesRemoved: BigInt.zero,
          ),
        ),
      );

      await Future<void>.delayed(Duration.zero);
      await container.read(vaultTreeProvider.future);
      final refreshedTree = sub.read().valueOrNull;
      expect(refreshedTree, isNotNull);
      expect(refreshedTree!.entries, isEmpty);
    });

    test('does not refetch on sync-started event', () async {
      final entryRepo = FakeEntryRepository();
      final syncRepo = FakeSyncRepository();

      final container = ProviderContainer(
        overrides: [
          entryRepositoryProvider.overrideWithValue(entryRepo),
          syncRepositoryProvider.overrideWithValue(syncRepo),
        ],
      );
      addTearDown(container.dispose);

      await container.read(vaultTreeProvider.future);
      final initialEntries = (await container.read(
        vaultTreeProvider.future,
      )).entries;

      entryRepo.tree = VaultTree(
        root: GroupNode(
          uuid: 'root',
          name: 'Root',
          children: const [],
          entryCount: BigInt.zero,
        ),
        entries: const [],
      );

      syncRepo.syncController.add(const SyncEvent.started());
      await Future<void>.delayed(Duration.zero);
      await container.read(vaultTreeProvider.future);
      final afterStarted = (await container.read(
        vaultTreeProvider.future,
      )).entries;
      expect(afterStarted.length, initialEntries.length);
    });

    test('refetches vault-scoped data after unlock', () async {
      final sessionRepo = FakeSessionRepository()
        ..currentLockState = LockEvent.unlocked;
      final entryRepo = FakeEntryRepository();
      final syncRepo = FakeSyncRepository();
      final container = ProviderContainer(
        overrides: [
          sessionRepositoryProvider.overrideWithValue(sessionRepo),
          entryRepositoryProvider.overrideWithValue(entryRepo),
          syncRepositoryProvider.overrideWithValue(syncRepo),
        ],
      );
      addTearDown(container.dispose);
      addTearDown(sessionRepo.dispose);
      addTearDown(syncRepo.dispose);

      container.read(vaultCacheInvalidationProvider);
      final initial = await container.read(vaultTreeProvider.future);
      expect(initial.entries, isNotEmpty);

      sessionRepo.emitLockState(LockEvent.locked);
      await Future<void>.delayed(Duration.zero);

      entryRepo.tree = VaultTree(
        root: GroupNode(
          uuid: 'new-root',
          name: 'New Root',
          children: const [],
          entryCount: BigInt.zero,
        ),
        entries: const [],
      );
      sessionRepo.emitLockState(LockEvent.unlocked);
      await Future<void>.delayed(Duration.zero);

      final refreshed = await container.read(vaultTreeProvider.future);
      expect(refreshed.root.uuid, 'new-root');
      expect(refreshed.entries, isEmpty);
    });
  });

  group('prefsProvider', () {
    test('queues pane-width update while preferences are loading', () async {
      final repo = _DelayedPrefsRepository();
      final container = ProviderContainer(
        overrides: [prefsRepositoryProvider.overrideWithValue(repo)],
      );
      addTearDown(container.dispose);

      container.read(prefsProvider);
      final update = container
          .read(prefsProvider.notifier)
          .setListPaneWidth(410);
      repo.initial.complete(
        const UiPrefs(
          themeMode: 'dark',
          windowX: 10,
          windowY: 20,
          windowWidth: 1200,
          windowHeight: 800,
        ),
      );
      await update;

      expect(repo.lastPrefs?.listPaneWidth, 410);
      expect(repo.lastPrefs?.windowX, 10);
      expect(repo.lastPrefs?.windowHeight, 800);
    });
  });
}

final class _DelayedPrefsRepository implements PrefsRepository {
  final initial = Completer<UiPrefs>();
  UiPrefs? lastPrefs;

  @override
  Future<UiPrefs> getPrefs() => initial.future;

  @override
  Future<void> setPrefs(UiPrefs prefs) async {
    lastPrefs = prefs;
  }
}

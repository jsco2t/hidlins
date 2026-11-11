import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/models.dart';
import 'lock_state_provider.dart';
import 'repository_providers.dart';

export 'repository_providers.dart';

final _entryMutationRevisionProvider =
    NotifierProvider<_EntryMutationRevision, int>(_EntryMutationRevision.new);

class _EntryMutationRevision extends Notifier<int> {
  @override
  int build() {
    final repo = ref.watch(entryRepositoryProvider);
    void onMutation() => state++;
    repo.onMutation = onMutation;
    ref.onDispose(() {
      if (identical(repo.onMutation, onMutation)) repo.onMutation = null;
    });
    return 0;
  }
}

final vaultTreeProvider = AsyncNotifierProvider<VaultTreeNotifier, VaultTree>(
  VaultTreeNotifier.new,
);

class VaultTreeNotifier extends AsyncNotifier<VaultTree> {
  @override
  Future<VaultTree> build() async {
    ref.watch(_entryMutationRevisionProvider);
    final repo = ref.watch(entryRepositoryProvider);
    ref.listen(syncEventsProvider, (_, next) {
      final event = next.valueOrNull;
      if (event is SyncEvent_Done) {
        invalidate();
      }
    });
    return repo.vaultTree();
  }

  void invalidate() {
    ref.invalidateSelf();
  }
}

final entryDetailProvider = FutureProvider.family<EntryDetail, String>((
  ref,
  uuid,
) {
  ref.watch(_entryMutationRevisionProvider);
  final repo = ref.watch(entryRepositoryProvider);
  ref.listen(syncEventsProvider, (_, next) {
    if (next.valueOrNull is SyncEvent_Done) ref.invalidateSelf();
  });
  return repo.entryDetail(uuid);
});

final vaultListProvider = FutureProvider<List<VaultSummary>>((ref) {
  final repo = ref.watch(sessionRepositoryProvider);
  return repo.listVaults();
});

final startupWarningsProvider = FutureProvider<List<String>>((ref) {
  return ref.watch(sessionRepositoryProvider).startupWarnings();
});

final syncEventsProvider = StreamProvider<SyncEvent>((ref) {
  final repo = ref.watch(syncRepositoryProvider);
  return repo.syncEvents();
});

final selectedEntryProvider = NotifierProvider<SelectedEntryNotifier, String?>(
  SelectedEntryNotifier.new,
);

class SelectedEntryNotifier extends Notifier<String?> {
  @override
  String? build() => null;

  void select(String? uuid) => state = uuid;
}

final prefsProvider = AsyncNotifierProvider<PrefsNotifier, UiPrefs>(
  PrefsNotifier.new,
);

class PrefsNotifier extends AsyncNotifier<UiPrefs> {
  @override
  Future<UiPrefs> build() async {
    final repo = ref.watch(prefsRepositoryProvider);
    return repo.getPrefs();
  }

  Future<void> setPrefs(UiPrefs prefs) async {
    final repo = ref.read(prefsRepositoryProvider);
    await repo.setPrefs(prefs);
    state = AsyncValue.data(prefs);
  }

  Future<void> setListPaneWidth(double width) async {
    try {
      final current = state.valueOrNull ?? await future;
      await setPrefs(current.copyWith(listPaneWidth: width));
    } catch (error, stackTrace) {
      state = AsyncValue.error(error, stackTrace);
    }
  }
}

final vaultCacheInvalidationProvider = Provider<void>((ref) {
  ref.listen(lockStateProvider, (_, next) {
    if (next.valueOrNull != null) {
      ref.invalidate(vaultTreeProvider);
      ref.invalidate(entryDetailProvider);
    }
  });
});

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/models.dart';
import 'lock_state_provider.dart';
import 'repository_providers.dart';

export 'repository_providers.dart';

final vaultTreeProvider = AsyncNotifierProvider<VaultTreeNotifier, VaultTree>(
  VaultTreeNotifier.new,
);

class VaultTreeNotifier extends AsyncNotifier<VaultTree> {
  @override
  Future<VaultTree> build() async {
    final repo = ref.watch(entryRepositoryProvider);
    repo.onMutation = invalidate;
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
  final repo = ref.watch(entryRepositoryProvider);
  return repo.entryDetail(uuid);
});

final vaultListProvider = FutureProvider<List<VaultSummary>>((ref) {
  final repo = ref.watch(sessionRepositoryProvider);
  return repo.listVaults();
});

final syncEventsProvider = StreamProvider<SyncEvent>((ref) {
  final repo = ref.watch(syncRepositoryProvider);
  return repo.syncEvents();
});

final clipboardEventsProvider = StreamProvider<ClipboardEvent>((ref) {
  final repo = ref.watch(syncRepositoryProvider);
  return repo.clipboardEvents();
});

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

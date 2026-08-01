import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../bridge/dto.dart' as bridge;
import '../data/failures.dart';
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
  ref.listen(syncEventsProvider, (_, next) {
    if (next.valueOrNull is SyncEvent_Done) {
      ref.invalidateSelf();
    }
  });
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

final syncUiStateProvider =
    AsyncNotifierProvider<SyncUiStateNotifier, SyncUiState>(
      SyncUiStateNotifier.new,
    );

class SyncUiState {
  const SyncUiState({
    required this.configured,
    required this.inFlight,
    this.lastOutcome,
    this.lastFailure,
  });

  final bool configured;
  final bool inFlight;
  final SyncOutcomeDto? lastOutcome;
  final AppFailure? lastFailure;

  SyncUiState copyWith({
    bool? configured,
    bool? inFlight,
    SyncOutcomeDto? lastOutcome,
    AppFailure? lastFailure,
    bool clearFailure = false,
  }) {
    return SyncUiState(
      configured: configured ?? this.configured,
      inFlight: inFlight ?? this.inFlight,
      lastOutcome: lastOutcome ?? this.lastOutcome,
      lastFailure: clearFailure ? null : lastFailure ?? this.lastFailure,
    );
  }
}

class SyncUiStateNotifier extends AsyncNotifier<SyncUiState> {
  static const _lockedState = SyncUiState(configured: false, inFlight: false);

  SyncEvent? _pendingEvent;
  bool _locked = false;
  int _sessionGeneration = 0;

  @override
  Future<SyncUiState> build() async {
    final buildGeneration = ++_sessionGeneration;
    ref.listen(lockStateProvider, (_, next) {
      final event = next.valueOrNull;
      if (event == LockEvent.locked) {
        _locked = true;
        _sessionGeneration++;
        _pendingEvent = null;
        state = const AsyncValue.data(_lockedState);
      } else if (event == LockEvent.unlocked) {
        final wasLocked = _locked;
        _locked = false;
        if (wasLocked) ref.invalidateSelf();
      }
    }, fireImmediately: true);
    ref.listen(syncEventsProvider, (_, next) {
      final event = next.valueOrNull;
      if (event != null) _onEvent(event);
    });
    var loaded = await _loadStatus();
    if (_locked || buildGeneration != _sessionGeneration) {
      return _lockedState;
    }
    if (_pendingEvent case final event?) {
      loaded = _reduce(loaded, event);
      _pendingEvent = null;
    }
    return loaded;
  }

  Future<void> syncNow() async {
    final current = state.valueOrNull ?? await future;
    if (current.inFlight) return;
    final sessionGeneration = _sessionGeneration;
    state = AsyncValue.data(
      current.copyWith(inFlight: true, clearFailure: true),
    );
    try {
      await ref.read(syncRepositoryProvider).syncNow();
      final loaded = await _loadStatus();
      if (_isCurrentSession(sessionGeneration)) {
        state = AsyncValue.data(loaded);
      }
    } on AppFailure catch (error) {
      if (_isCurrentSession(sessionGeneration)) {
        state = AsyncValue.data(
          error is VaultIsBusy
              ? current.copyWith(inFlight: true, clearFailure: true)
              : current.copyWith(inFlight: false, lastFailure: error),
        );
      }
      rethrow;
    } on Object catch (error, stackTrace) {
      if (_isCurrentSession(sessionGeneration)) {
        state = AsyncValue.data(
          current.copyWith(
            inFlight: false,
            lastFailure: const InternalFailure('sync failed'),
          ),
        );
      }
      Error.throwWithStackTrace(error, stackTrace);
    }
  }

  Future<void> configureSync(S3ConfigDto config) async {
    final current = state.valueOrNull ?? await future;
    final sessionGeneration = _sessionGeneration;
    try {
      await ref.read(syncRepositoryProvider).configureSync(config);
      final loaded = await _loadStatus();
      if (_isCurrentSession(sessionGeneration)) {
        state = AsyncValue.data(
          loaded.copyWith(configured: true, clearFailure: true),
        );
        unawaited(_syncAfterConfigure());
      }
    } on AppFailure catch (error) {
      if (_isCurrentSession(sessionGeneration)) {
        state = AsyncValue.data(current.copyWith(lastFailure: error));
      }
      rethrow;
    }
  }

  Future<SyncUiState> _loadStatus() async {
    final status = await ref.read(syncRepositoryProvider).syncStatus();
    return SyncUiState(
      configured: status.configured,
      inFlight: status.inFlight,
      lastOutcome: status.lastOutcome,
    );
  }

  void _onEvent(SyncEvent event) {
    if (_locked) return;
    final current = state.valueOrNull;
    if (current == null) {
      _pendingEvent = event;
      return;
    }
    state = AsyncValue.data(_reduce(current, event));
  }

  bool _isCurrentSession(int sessionGeneration) {
    return !_locked && sessionGeneration == _sessionGeneration;
  }

  Future<void> _syncAfterConfigure() async {
    try {
      await syncNow();
    } on Object {
      // Terminal sync events own user-facing failure presentation.
    }
  }

  SyncUiState _reduce(SyncUiState current, SyncEvent event) {
    return switch (event) {
      SyncEvent_Started() || SyncEvent_Activity() => current.copyWith(
        inFlight: true,
        clearFailure: true,
      ),
      SyncEvent_Done(:final field0) => current.copyWith(
        configured: true,
        inFlight: false,
        lastOutcome: _syncOutcomeFromBridge(field0),
        clearFailure: true,
      ),
      SyncEvent_Failed(:final field0) => current.copyWith(
        inFlight: false,
        lastFailure: mapApiError(field0),
      ),
    };
  }
}

final unlockSyncWarningProvider =
    StateNotifierProvider<UnlockSyncWarningNotifier, bool>(
      (_) => UnlockSyncWarningNotifier(),
    );

class UnlockSyncWarningNotifier extends StateNotifier<bool> {
  UnlockSyncWarningNotifier() : super(false);

  void show() => state = true;
  void clear() => state = false;
}

SyncOutcomeDto _syncOutcomeFromBridge(bridge.SyncOutcomeDto value) {
  return switch (value) {
    bridge.SyncOutcomeDto_AlreadyInSync() => SyncOutcomeDto.alreadyInSync,
    bridge.SyncOutcomeDto_Pushed() => SyncOutcomeDto.pushed,
    bridge.SyncOutcomeDto_FastReplaced() => SyncOutcomeDto.fastReplaced,
    bridge.SyncOutcomeDto_Merged() => SyncOutcomeDto.merged,
    bridge.SyncOutcomeDto_Unknown() => SyncOutcomeDto.unknown,
  };
}

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
    if (next.valueOrNull == LockEvent.locked) {
      ref.read(unlockSyncWarningProvider.notifier).clear();
    }
  });
});

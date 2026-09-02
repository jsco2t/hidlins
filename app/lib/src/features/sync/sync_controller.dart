import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/dto.dart' as bridge;
import '../../data/failures.dart';
import '../../data/models.dart';
import '../../providers/providers.dart';

class SyncViewState {
  const SyncViewState({
    required this.configured,
    required this.inFlight,
    this.lastOutcome,
    this.failure,
  });

  final bool configured;
  final bool inFlight;
  final SyncOutcomeDto? lastOutcome;
  final AppFailure? failure;

  SyncViewState copyWith({
    bool? configured,
    bool? inFlight,
    Object? lastOutcome = _unset,
    Object? failure = _unset,
  }) {
    return SyncViewState(
      configured: configured ?? this.configured,
      inFlight: inFlight ?? this.inFlight,
      lastOutcome: lastOutcome == _unset
          ? this.lastOutcome
          : lastOutcome as SyncOutcomeDto?,
      failure: failure == _unset ? this.failure : failure as AppFailure?,
    );
  }
}

const _unset = Object();

final syncControllerProvider =
    AsyncNotifierProvider<SyncController, SyncViewState>(SyncController.new);

class SyncController extends AsyncNotifier<SyncViewState> {
  @override
  Future<SyncViewState> build() async {
    ref.listen(syncEventsProvider, (_, next) {
      final event = next.valueOrNull;
      if (event != null) _applyEvent(event);
    });
    final status = await ref.watch(syncRepositoryProvider).syncStatus();
    return SyncViewState(
      configured: status.configured,
      inFlight: status.inFlight,
      lastOutcome: status.lastOutcome,
    );
  }

  Future<void> configure(S3ConfigDto config) async {
    final current = state.valueOrNull;
    if (current != null) {
      state = AsyncValue.data(current.copyWith(failure: null));
    }
    try {
      await ref.read(syncRepositoryProvider).configureSync(config);
      state = AsyncValue.data(
        (state.valueOrNull ??
                const SyncViewState(configured: false, inFlight: false))
            .copyWith(configured: true, failure: null),
      );
    } on AppFailure catch (failure) {
      _setFailure(failure);
    } on Exception {
      _setFailure(const InternalFailure('sync configuration failed'));
    }
  }

  Future<void> syncNow() async {
    final current = state.valueOrNull;
    if (current == null || current.inFlight) return;
    state = AsyncValue.data(current.copyWith(inFlight: true, failure: null));
    try {
      await ref.read(syncRepositoryProvider).syncNow();
    } on AppFailure catch (failure) {
      _setFailure(failure);
    } on Exception {
      _setFailure(const InternalFailure('sync failed'));
    }
  }

  void clearFailure() {
    final current = state.valueOrNull;
    if (current != null) {
      state = AsyncValue.data(current.copyWith(failure: null));
    }
  }

  void _applyEvent(bridge.SyncEvent event) {
    final current = state.valueOrNull;
    if (current == null) return;
    state = AsyncValue.data(switch (event) {
      bridge.SyncEvent_Started() || bridge.SyncEvent_Activity() =>
        current.copyWith(inFlight: true, failure: null),
      bridge.SyncEvent_Done(:final field0) => current.copyWith(
        inFlight: false,
        lastOutcome: _outcome(field0),
        failure: null,
      ),
      bridge.SyncEvent_Failed(:final field0) => current.copyWith(
        inFlight: false,
        failure: mapApiError(field0),
      ),
    });
  }

  void _setFailure(AppFailure failure) {
    final current = state.valueOrNull;
    state = AsyncValue.data(
      (current ?? const SyncViewState(configured: false, inFlight: false))
          .copyWith(inFlight: false, failure: failure),
    );
  }

  SyncOutcomeDto _outcome(bridge.SyncOutcomeDto outcome) {
    return switch (outcome) {
      bridge.SyncOutcomeDto_AlreadyInSync() => SyncOutcomeDto.alreadyInSync,
      bridge.SyncOutcomeDto_Pushed() => SyncOutcomeDto.pushed,
      bridge.SyncOutcomeDto_FastReplaced() => SyncOutcomeDto.fastReplaced,
      bridge.SyncOutcomeDto_Merged() => SyncOutcomeDto.merged,
      bridge.SyncOutcomeDto_Unknown() => SyncOutcomeDto.unknown,
    };
  }
}

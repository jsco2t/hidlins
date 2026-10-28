import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/dto.dart' as bridge;
import '../../data/failures.dart';
import '../../data/models.dart';
import '../../data/repositories.dart';
import '../../providers/providers.dart';
import '../../providers/lock_state_provider.dart';

class SyncViewState {
  const SyncViewState({
    required this.status,
    this.discovery = const LocalDiscoveryStatus(
      permission: LocalDiscoveryPermission.notDetermined,
    ),
    this.peers = const [],
    this.prompt,
    this.pairingSecondsRemaining = 0,
    this.working = false,
    this.failure,
    this.terminalRevision = 0,
  });

  final SyncStatusDto status;
  final LocalDiscoveryStatus discovery;
  final List<SyncPeer> peers;
  final PairingPrompt? prompt;
  final int pairingSecondsRemaining;
  final bool working;
  final AppFailure? failure;
  final int terminalRevision;

  bool get configured => status.configured;
  bool get inFlight => status.inFlight;
  SyncOutcomeDto? get lastOutcome => status.lastOutcome;

  SyncViewState copyWith({
    SyncStatusDto? status,
    LocalDiscoveryStatus? discovery,
    List<SyncPeer>? peers,
    Object? prompt = _unset,
    int? pairingSecondsRemaining,
    bool? working,
    Object? failure = _unset,
    int? terminalRevision,
  }) => SyncViewState(
    status: status ?? this.status,
    discovery: discovery ?? this.discovery,
    peers: peers ?? this.peers,
    prompt: prompt == _unset ? this.prompt : prompt as PairingPrompt?,
    pairingSecondsRemaining:
        pairingSecondsRemaining ?? this.pairingSecondsRemaining,
    working: working ?? this.working,
    failure: failure == _unset ? this.failure : failure as AppFailure?,
    terminalRevision: terminalRevision ?? this.terminalRevision,
  );
}

const _unset = Object();

final syncControllerProvider =
    AsyncNotifierProvider<SyncController, SyncViewState>(SyncController.new);

class SyncController extends AsyncNotifier<SyncViewState> {
  Timer? _pairingTimer;
  SyncRepository? _repository;
  int _generation = 0;
  int _eventRevision = 0;
  int _terminalRevision = 0;
  bool _locked = false;

  static const _empty = SyncViewState(
    status: SyncStatusDto(configured: false, inFlight: false),
  );

  bool _isCurrent(int generation) => generation == _generation && !_locked;

  @override
  Future<SyncViewState> build() async {
    _locked = ref.read(lockStateProvider).valueOrNull == LockEvent.locked;
    ref.listen(lockStateProvider, (_, next) {
      final event = next.valueOrNull;
      if (event == null) return;
      final locked = event == LockEvent.locked;
      if (locked == _locked) return;
      _locked = locked;
      _generation++;
      _pairingTimer?.cancel();
      if (locked) {
        state = const AsyncValue.data(_empty);
      } else {
        ref.invalidateSelf();
      }
    });
    final generation = ++_generation;
    var loading = true;
    final pending = <bridge.SyncEvent>[];
    final repository = ref.watch(syncRepositoryProvider);
    _repository = repository;
    ref.onDispose(() {
      _pairingTimer?.cancel();
      if (_generation == generation) _generation++;
      unawaited(repository.stopDiscovery());
    });
    ref.listen(syncEventsProvider, (_, next) {
      final event = next.valueOrNull;
      if (event == null || !_isCurrent(generation)) return;
      _eventRevision++;
      if (loading) {
        pending.add(event);
      } else {
        _applyEvent(event);
      }
    });
    if (_locked) return _empty;
    final status = await repository.syncStatus();
    if (!_isCurrent(generation)) return _empty;
    final peers = status.configured
        ? await repository.listPeers().onError((_, _) => const <SyncPeer>[])
        : const <SyncPeer>[];
    if (!_isCurrent(generation)) return _empty;
    var loaded = SyncViewState(status: status, peers: peers);
    for (final event in pending) {
      loaded = _reduceEvent(loaded, event);
    }
    loading = false;
    return loaded;
  }

  Future<void> discover(DiscoveryKind kind) async {
    await _run((generation) async {
      final discovery = await _repo.discover(kind);
      _replace(discovery: discovery, generation: generation);
    });
  }

  Future<void> useManualEndpoint(String value) async {
    await _run((generation) async {
      final endpoint = _parseEndpoint(value);
      await _repo.setManualEndpoint(endpoint);
      _replace(
        generation: generation,
        discovery: LocalDiscoveryStatus(
          permission: LocalDiscoveryPermission.granted,
          candidates: [endpoint],
        ),
      );
    });
  }

  Future<void> pairThisVault() async {
    await _run((generation) async {
      final repository = _repo;
      if (!_value.status.configured) {
        await repository.configureLocalSync(LocalSyncRole.client);
      }
      if (!_isCurrent(generation)) return;
      final discovered = await repository.discover(DiscoveryKind.pairing);
      if (!_isCurrent(generation)) return;
      _replace(discovery: discovered, generation: generation);
      _requireDiscovery(discovered);
      final prompt = await repository.beginPairing();
      _replace(prompt: prompt, generation: generation);
    });
  }

  Future<void> beginPairImport({
    required String name,
    required String masterPassword,
  }) async {
    await _run((generation) async {
      final repository = _repo;
      final discovered = await repository.discover(DiscoveryKind.pairing);
      if (!_isCurrent(generation)) return;
      _replace(discovery: discovered, generation: generation);
      _requireDiscovery(discovered);
      _replace(
        generation: generation,
        prompt: await repository.beginPairImport(
          name: name,
          masterPassword: masterPassword,
        ),
      );
    });
  }

  Future<VaultSummary?> confirmPairing({
    required bool accepted,
    String peerName = 'Nearby Hidlins',
  }) async {
    final generation = _generation;
    if (!_isCurrent(generation)) return null;
    final prompt = _value.prompt;
    if (prompt == null) return null;
    _replace(prompt: null, failure: null, working: true);
    try {
      final imported = await _repo.confirmPairing(
        transactionHandle: prompt.transactionHandle,
        accepted: accepted,
        peerDisplayName: peerName.trim().isEmpty
            ? 'Nearby Hidlins'
            : peerName.trim(),
      );
      if (!_isCurrent(generation)) return null;
      await refresh(expectedGeneration: generation);
      return _isCurrent(generation) ? imported : null;
    } on AppFailure catch (failure) {
      _setFailure(failure, generation: generation);
      return null;
    } on Exception {
      _setFailure(
        const InternalFailure('pairing failed'),
        generation: generation,
      );
      return null;
    }
  }

  Future<void> startServer() async {
    await _run((generation) async {
      final repository = _repo;
      if (!_value.status.configured) {
        await repository.configureLocalSync(LocalSyncRole.server);
      }
      if (!_isCurrent(generation)) return;
      final endpoints = await repository.serverEndpoints();
      if (!_isCurrent(generation)) return;
      if (endpoints.isEmpty) throw const SyncUnreachable();
      await repository.startServer(endpoints.first);
      await refresh(expectedGeneration: generation);
    });
  }

  Future<void> stopServer() async {
    await _run((generation) async {
      await _repo.stopServer();
      if (!_isCurrent(generation)) return;
      _pairingTimer?.cancel();
      await refresh(expectedGeneration: generation);
    });
  }

  Future<void> openPairingWindow() async {
    await _run((generation) async {
      await _repo.openPairingWindow();
      if (!_isCurrent(generation)) return;
      _pairingTimer?.cancel();
      _replace(pairingSecondsRemaining: 180);
      _pairingTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
        if (!_isCurrent(generation)) {
          timer.cancel();
          return;
        }
        final remaining = _value.pairingSecondsRemaining - 1;
        if (remaining <= 0) {
          timer.cancel();
          _replace(pairingSecondsRemaining: 0);
          unawaited(_repo.closePairingWindow());
        } else {
          _replace(pairingSecondsRemaining: remaining);
        }
      });
      await refresh(preserveCountdown: true, expectedGeneration: generation);
    });
  }

  Future<void> syncNow() async {
    final generation = _generation;
    if (!_isCurrent(generation)) return;
    if (_value.inFlight) return;
    _replace(working: true, failure: null);
    try {
      await _repo.discover(DiscoveryKind.trusted);
      if (!_isCurrent(generation)) return;
      await _repo.syncNow();
    } on AppFailure catch (failure) {
      _setFailure(failure, generation: generation);
    } on Exception {
      _setFailure(const InternalFailure('sync failed'), generation: generation);
    }
  }

  Future<void> renamePeer(String peerId, String displayName) async {
    await _run((generation) async {
      await _repo.renamePeer(peerId, displayName);
      await refresh(expectedGeneration: generation);
    });
  }

  Future<void> revokePeer(String peerId) async {
    await _run((generation) async {
      await _repo.revokePeer(peerId);
      await refresh(expectedGeneration: generation);
    });
  }

  Future<void> openDiscoverySettings() => _repo.openDiscoverySettings();

  Future<void> cancelForegroundOperations({bool updateState = true}) async {
    final generation = _generation;
    _pairingTimer?.cancel();
    final prompt = _value.prompt;
    if (updateState) {
      _replace(prompt: null, pairingSecondsRemaining: 0, working: false);
    }
    if (prompt != null) {
      await _repo
          .confirmPairing(
            transactionHandle: prompt.transactionHandle,
            accepted: false,
            peerDisplayName: '',
          )
          .onError((_, _) => null);
    }
    if (!_isCurrent(generation)) return;
    await _repo.cancelSync();
    if (!_isCurrent(generation)) return;
    await _repo.stopDiscovery();
  }

  Future<void> refresh({
    bool preserveCountdown = false,
    int? expectedGeneration,
  }) async {
    final generation = expectedGeneration ?? _generation;
    if (!_isCurrent(generation)) return;
    final eventRevision = _eventRevision;
    final repository = _repo;
    final status = await repository.syncStatus();
    if (!_isCurrent(generation)) return;
    final peers = status.configured
        ? await repository.listPeers().onError((_, _) => const <SyncPeer>[])
        : const <SyncPeer>[];
    _replace(
      generation: generation,
      status: eventRevision == _eventRevision ? status : null,
      peers: peers,
      working: false,
      pairingSecondsRemaining: preserveCountdown
          ? _value.pairingSecondsRemaining
          : status.pairingOpen
          ? _value.pairingSecondsRemaining
          : 0,
    );
  }

  void clearFailure() => _replace(failure: null);

  Future<void> _run(Future<void> Function(int generation) action) async {
    final generation = _generation;
    if (!_isCurrent(generation)) return;
    _replace(working: true, failure: null);
    try {
      await action(generation);
      _replace(working: false, generation: generation);
    } on AppFailure catch (failure) {
      _setFailure(failure, generation: generation);
    } on FormatException catch (error) {
      _setFailure(
        InvalidInputFailure(field: 'endpoint', reason: error.message),
        generation: generation,
      );
    } on Exception {
      _setFailure(
        const InternalFailure('local sync operation failed'),
        generation: generation,
      );
    }
  }

  void _applyEvent(bridge.SyncEvent event) {
    final current = state.valueOrNull;
    if (current == null) return;
    state = AsyncValue.data(_reduceEvent(current, event));
    if (event is bridge.SyncEvent_ServerStarted ||
        event is bridge.SyncEvent_ServerStopped) {
      unawaited(refresh());
    }
  }

  SyncViewState _reduceEvent(SyncViewState current, bridge.SyncEvent event) {
    switch (event) {
      case bridge.SyncEvent_Started() || bridge.SyncEvent_Activity():
        return current.copyWith(
          status: _statusCopy(current.status, inFlight: true),
          working: false,
          failure: null,
        );
      case bridge.SyncEvent_Done(:final field0):
        return current.copyWith(
          terminalRevision: ++_terminalRevision,
          status: _statusCopy(
            current.status,
            inFlight: false,
            lastOutcome: _outcome(field0),
          ),
          working: false,
          failure: null,
        );
      case bridge.SyncEvent_Failed(:final field0):
        return current.copyWith(
          terminalRevision: ++_terminalRevision,
          status: _statusCopy(current.status, inFlight: false),
          working: false,
          failure: mapApiError(field0),
        );
      case bridge.SyncEvent_PairingRequested(:final field0):
        return current.copyWith(
          prompt: PairingPrompt(
            transactionHandle: field0.transactionHandle,
            sas: field0.sas,
          ),
          working: false,
        );
      case bridge.SyncEvent_ServerStarted() || bridge.SyncEvent_ServerStopped():
        return current;
    }
  }

  SyncViewState get _value =>
      state.valueOrNull ??
      const SyncViewState(
        status: SyncStatusDto(configured: false, inFlight: false),
      );

  SyncRepository get _repo {
    final repository = _repository;
    if (repository != null) return repository;
    final initialized = ref.read(syncRepositoryProvider);
    _repository = initialized;
    return initialized;
  }

  void _replace({
    int? generation,
    SyncStatusDto? status,
    LocalDiscoveryStatus? discovery,
    List<SyncPeer>? peers,
    Object? prompt = _unset,
    int? pairingSecondsRemaining,
    bool? working,
    Object? failure = _unset,
  }) {
    if (_locked || (generation != null && !_isCurrent(generation))) return;
    state = AsyncValue.data(
      _value.copyWith(
        status: status,
        discovery: discovery,
        peers: peers,
        prompt: prompt,
        pairingSecondsRemaining: pairingSecondsRemaining,
        working: working,
        failure: failure,
      ),
    );
  }

  void _setFailure(AppFailure failure, {int? generation}) => _replace(
    generation: generation,
    status: _statusCopy(_value.status, inFlight: false),
    working: false,
    failure: failure,
  );

  static void _requireDiscovery(LocalDiscoveryStatus status) {
    switch (status.permission) {
      case LocalDiscoveryPermission.denied:
        throw const PlatformOperationFailure(
          capability: 'local network',
          state: 'denied',
        );
      case LocalDiscoveryPermission.restricted:
        throw const PlatformOperationFailure(
          capability: 'local network',
          state: 'restricted',
        );
      case LocalDiscoveryPermission.unavailable:
        throw const UnsupportedPlatformFailure('local network discovery');
      case LocalDiscoveryPermission.notDetermined ||
          LocalDiscoveryPermission.granted:
        if (status.candidates.isEmpty) throw const SyncUnreachable();
    }
  }

  static LocalEndpoint _parseEndpoint(String input) {
    final value = input.trim();
    if (value.isEmpty) throw const FormatException('enter an IP and port');
    if (value.startsWith('[')) {
      final close = value.lastIndexOf(']:');
      if (close < 0) throw const FormatException('use [IPv6%scope]:port');
      final host = value.substring(1, close);
      final scopeSeparator = host.lastIndexOf('%');
      final address = scopeSeparator < 0
          ? host
          : host.substring(0, scopeSeparator);
      final scope = scopeSeparator < 0
          ? 0
          : int.tryParse(host.substring(scopeSeparator + 1));
      final port = int.tryParse(value.substring(close + 2));
      if (scope == null || port == null) {
        throw const FormatException('use [IPv6%scope]:port');
      }
      return LocalEndpoint(address: address, port: port, scopeId: scope);
    }
    final separator = value.lastIndexOf(':');
    if (separator < 1) throw const FormatException('use IP:port');
    final port = int.tryParse(value.substring(separator + 1));
    if (port == null) throw const FormatException('port must be a number');
    return LocalEndpoint(address: value.substring(0, separator), port: port);
  }

  static SyncStatusDto _statusCopy(
    SyncStatusDto value, {
    bool? inFlight,
    SyncOutcomeDto? lastOutcome,
  }) => SyncStatusDto(
    configured: value.configured,
    inFlight: inFlight ?? value.inFlight,
    lastOutcome: lastOutcome ?? value.lastOutcome,
    role: value.role,
    paired: value.paired,
    activePeerCount: value.activePeerCount,
    serverEnabled: value.serverEnabled,
    serverRunning: value.serverRunning,
    pairingOpen: value.pairingOpen,
  );

  static SyncOutcomeDto _outcome(bridge.SyncOutcomeDto outcome) =>
      switch (outcome) {
        bridge.SyncOutcomeDto_AlreadyInSync() => SyncOutcomeDto.alreadyInSync,
        bridge.SyncOutcomeDto_Pushed() => SyncOutcomeDto.pushed,
        bridge.SyncOutcomeDto_FastReplaced() => SyncOutcomeDto.fastReplaced,
        bridge.SyncOutcomeDto_Merged() => SyncOutcomeDto.merged,
        bridge.SyncOutcomeDto_Unknown() => SyncOutcomeDto.unknown,
      };
}

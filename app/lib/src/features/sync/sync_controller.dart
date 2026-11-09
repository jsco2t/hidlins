import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/dto.dart' as bridge;
import '../../data/failures.dart';
import '../../data/models.dart';
import '../../data/repositories.dart';
import '../../providers/providers.dart';
import '../../providers/lock_state_provider.dart';
import 'foreground_lifecycle.dart';

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

typedef _ForegroundToken = ({int session, int foreground});

final syncControllerProvider =
    AsyncNotifierProvider<SyncController, SyncViewState>(SyncController.new);

class SyncController extends AsyncNotifier<SyncViewState> {
  Timer? _pairingTimer;
  SyncRepository? _repository;
  int _generation = 0;
  int _foregroundEpoch = 0;
  late SyncForegroundLifecycle _foregroundLifecycle;
  Object _foregroundOwner = Object();
  LocalEndpoint? _manualEndpoint;
  int _eventRevision = 0;
  int _terminalRevision = 0;
  bool _locked = false;

  static const _empty = SyncViewState(
    status: SyncStatusDto(configured: false, inFlight: false),
  );

  bool _isCurrent(int generation) => generation == _generation && !_locked;

  _ForegroundToken get _foregroundToken =>
      (session: _generation, foreground: _foregroundEpoch);

  bool _isForegroundCurrent(_ForegroundToken token) =>
      _isCurrent(token.session) && token.foreground == _foregroundEpoch;

  @override
  Future<SyncViewState> build() async {
    final lifecycle = _foregroundLifecycle = ref.read(
      syncForegroundLifecycleProvider,
    );
    final owner = _foregroundOwner = Object();
    _manualEndpoint = null;
    _locked = ref.read(lockStateProvider).valueOrNull == LockEvent.locked;
    ref.listen(lockStateProvider, (_, next) {
      final event = next.valueOrNull;
      if (event == null) return;
      final locked = event == LockEvent.locked;
      if (locked == _locked) return;
      if (locked) unawaited(lifecycle.releaseOwner(owner));
      if (locked) _manualEndpoint = null;
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
      unawaited(lifecycle.releaseOwner(owner));
      if (_generation == generation) _generation++;
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
    await _run((token, lease) async {
      final discovery = await lease.track(_repo.discover(kind));
      _replace(discovery: discovery, foreground: token);
    });
  }

  Future<void> useManualEndpoint(String value) async {
    await _run((token, lease) async {
      final endpoint = _parseEndpoint(value);
      await lease.track(_repo.setManualEndpoint(endpoint));
      if (!_isForegroundCurrent(token)) return;
      _manualEndpoint = endpoint;
      _replace(
        foreground: token,
        discovery: LocalDiscoveryStatus(
          permission: LocalDiscoveryPermission.granted,
          candidates: [endpoint],
        ),
      );
    });
  }

  Future<void> pairThisVault() async {
    await _run((token, lease) async {
      final repository = _repo;
      if (!_value.status.configured) {
        await lease.track(repository.configureLocalSync(LocalSyncRole.client));
      }
      if (!_isForegroundCurrent(token)) return;
      final discovered = await _operationRoute(
        repository,
        lease,
        DiscoveryKind.pairing,
      );
      if (!_isForegroundCurrent(token)) return;
      _replace(discovery: discovered, foreground: token);
      _requireDiscovery(discovered);
      final prompt = await _ownedPrompt(
        lease,
        token,
        repository,
        repository.beginPairing(),
      );
      if (!_isForegroundCurrent(token)) return;
      _replace(prompt: prompt, foreground: token);
    });
  }

  Future<void> beginPairImport({
    required String name,
    required String masterPassword,
  }) async {
    await _run((token, lease) async {
      final repository = _repo;
      final discovered = await _operationRoute(
        repository,
        lease,
        DiscoveryKind.pairing,
      );
      if (!_isForegroundCurrent(token)) return;
      _replace(discovery: discovered, foreground: token);
      _requireDiscovery(discovered);
      final prompt = await _ownedPrompt(
        lease,
        token,
        repository,
        repository.beginPairImport(name: name, masterPassword: masterPassword),
      );
      if (!_isForegroundCurrent(token)) return;
      _replace(foreground: token, prompt: prompt);
    });
  }

  Future<VaultSummary?> confirmPairing({
    required bool accepted,
    String peerName = 'Nearby Hidlins',
    String? transactionHandle,
  }) async {
    if (!_isCurrent(_generation)) return null;
    final prompt = _value.prompt;
    if (prompt == null ||
        (transactionHandle != null &&
            prompt.transactionHandle != transactionHandle)) {
      return null;
    }
    final lease = _acquireForeground();
    final token = _foregroundToken;
    final repository = _repo;
    await lease.ready;
    if (!_isForegroundCurrent(token) ||
        _value.prompt?.transactionHandle != prompt.transactionHandle) {
      return null;
    }
    _replace(prompt: null, failure: null, working: true, foreground: token);
    try {
      final imported = await lease.track(
        repository.confirmPairing(
          transactionHandle: prompt.transactionHandle,
          accepted: accepted,
          peerDisplayName: peerName.trim().isEmpty
              ? 'Nearby Hidlins'
              : peerName.trim(),
        ),
      );
      if (!_isForegroundCurrent(token)) return null;
      await refresh(
        expectedGeneration: token.session,
        expectedForegroundEpoch: token.foreground,
      );
      return _isForegroundCurrent(token) ? imported : null;
    } on AppFailure catch (failure) {
      _setFailure(failure, foreground: token);
      return null;
    } on Exception {
      _setFailure(const InternalFailure('pairing failed'), foreground: token);
      return null;
    }
  }

  Future<void> startServer() async {
    await _run((token, lease) async {
      final repository = _repo;
      if (!_value.status.configured) {
        await lease.track(repository.configureLocalSync(LocalSyncRole.server));
      }
      if (!_isForegroundCurrent(token)) return;
      final endpoints = await lease.track(repository.serverEndpoints());
      if (!_isForegroundCurrent(token)) return;
      if (endpoints.isEmpty) throw const SyncUnreachable();
      await lease.track(repository.startServer(endpoints.first));
      await refresh(
        expectedGeneration: token.session,
        expectedForegroundEpoch: token.foreground,
      );
    });
  }

  Future<void> stopServer() async {
    await _run((token, lease) async {
      await lease.track(_repo.stopServer());
      if (!_isForegroundCurrent(token)) return;
      _pairingTimer?.cancel();
      await refresh(
        expectedGeneration: token.session,
        expectedForegroundEpoch: token.foreground,
      );
    });
  }

  Future<void> openPairingWindow() async {
    await _run((token, lease) async {
      await lease.track(_repo.openPairingWindow());
      if (!_isForegroundCurrent(token)) return;
      _pairingTimer?.cancel();
      _replace(pairingSecondsRemaining: 180);
      _pairingTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
        if (!_isCurrent(token.session)) {
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
      await refresh(
        preserveCountdown: true,
        expectedGeneration: token.session,
        expectedForegroundEpoch: token.foreground,
      );
    });
  }

  Future<void> syncNow() async {
    if (!_isCurrent(_generation) || _value.inFlight) return;
    final lease = _acquireForeground();
    final token = _foregroundToken;
    await lease.ready;
    if (!_isForegroundCurrent(token) || _value.inFlight) return;
    final repository = _repo;
    _replace(working: true, failure: null, foreground: token);
    try {
      await _operationRoute(repository, lease, DiscoveryKind.trusted);
      if (!_isForegroundCurrent(token)) return;
      await lease.track(repository.syncNow());
    } on AppFailure catch (failure) {
      _setFailure(failure, foreground: token);
    } on Exception {
      _setFailure(const InternalFailure('sync failed'), foreground: token);
    }
  }

  Future<void> renamePeer(String peerId, String displayName) async {
    await _run((token, lease) async {
      await lease.track(_repo.renamePeer(peerId, displayName));
      await refresh(
        expectedGeneration: token.session,
        expectedForegroundEpoch: token.foreground,
      );
    });
  }

  Future<void> revokePeer(String peerId) async {
    await _run((token, lease) async {
      await lease.track(_repo.revokePeer(peerId));
      await refresh(
        expectedGeneration: token.session,
        expectedForegroundEpoch: token.foreground,
      );
    });
  }

  Future<void> openDiscoverySettings() => _repo.openDiscoverySettings();

  Future<void> _rejectPairing(
    SyncRepository repository,
    PairingPrompt prompt,
  ) async {
    await repository
        .confirmPairing(
          transactionHandle: prompt.transactionHandle,
          accepted: false,
          peerDisplayName: '',
        )
        .onError((_, _) => null);
  }

  SyncForegroundLease _acquireForeground() {
    final repository = _repo;
    return _foregroundLifecycle.acquire(
      _foregroundOwner,
      () => _prepareCleanup(repository),
    );
  }

  Future<LocalDiscoveryStatus> _operationRoute(
    SyncRepository repository,
    SyncForegroundLease lease,
    DiscoveryKind kind,
  ) async {
    final manual = _manualEndpoint;
    if (manual == null) return lease.track(repository.discover(kind));
    // Discovery may update its candidate list independently. A user-selected
    // route remains separate and is admitted by Rust again before this effect.
    await lease.track(repository.setManualEndpoint(manual));
    return LocalDiscoveryStatus(
      permission: LocalDiscoveryPermission.granted,
      candidates: [manual],
    );
  }

  Future<PairingPrompt> _ownedPrompt(
    SyncForegroundLease lease,
    _ForegroundToken token,
    SyncRepository repository,
    Future<PairingPrompt> operation,
  ) => lease.track(
    operation.then((prompt) async {
      if (!_isForegroundCurrent(token)) {
        await _rejectPairing(repository, prompt);
      }
      return prompt;
    }),
  );

  Future<void> cancelForegroundOperations({bool updateState = true}) =>
      _foregroundLifecycle.cancel(
        _foregroundOwner,
        () =>
            _prepareCleanup(_repo, updateState: updateState, cancelTimer: true),
      );

  ForegroundCleanup _prepareCleanup(
    SyncRepository repository, {
    bool updateState = true,
    bool cancelTimer = false,
  }) {
    final token = (session: _generation, foreground: ++_foregroundEpoch);
    final prompt = _value.prompt;
    if (cancelTimer) _pairingTimer?.cancel();
    if (updateState) {
      _replace(
        prompt: null,
        pairingSecondsRemaining: cancelTimer ? 0 : null,
        working: false,
      );
    }
    return (mayCancelNative) async {
      try {
        try {
          if (prompt != null) await _rejectPairing(repository, prompt);
          if (mayCancelNative && _isCurrent(token.session)) {
            await repository.cancelSync();
          }
        } finally {
          if (mayCancelNative) await repository.stopDiscovery();
        }
      } on AppFailure catch (failure) {
        _setFailure(failure, foreground: token);
      } catch (_) {
        _setFailure(
          const InternalFailure('local sync cancellation failed'),
          foreground: token,
        );
      } finally {
        if (_isForegroundCurrent(token)) {
          final clearPrompt =
              _value.prompt?.transactionHandle == prompt?.transactionHandle;
          _replace(
            foreground: token,
            prompt: clearPrompt ? null : _unset,
            pairingSecondsRemaining: cancelTimer ? 0 : null,
            working: false,
          );
        }
      }
    };
  }

  Future<void> refresh({
    bool preserveCountdown = false,
    int? expectedGeneration,
    int? expectedForegroundEpoch,
  }) async {
    final generation = expectedGeneration ?? _generation;
    bool isCurrent() =>
        _isCurrent(generation) &&
        (expectedForegroundEpoch == null ||
            expectedForegroundEpoch == _foregroundEpoch);
    if (!isCurrent()) return;
    final eventRevision = _eventRevision;
    final repository = _repo;
    final status = await repository.syncStatus();
    if (!isCurrent()) return;
    final peers = status.configured
        ? await repository.listPeers().onError((_, _) => const <SyncPeer>[])
        : const <SyncPeer>[];
    if (!isCurrent()) return;
    _replace(
      generation: generation,
      foreground: expectedForegroundEpoch == null
          ? null
          : (session: generation, foreground: expectedForegroundEpoch),
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

  Future<void> _run(
    Future<void> Function(_ForegroundToken token, SyncForegroundLease lease)
    action,
  ) async {
    if (!_isCurrent(_generation)) return;
    final lease = _acquireForeground();
    final token = _foregroundToken;
    await lease.ready;
    if (!_isForegroundCurrent(token)) return;
    _replace(working: true, failure: null, foreground: token);
    try {
      await action(token, lease);
      _replace(working: false, foreground: token);
    } on AppFailure catch (failure) {
      _setFailure(failure, foreground: token);
    } on FormatException catch (error) {
      _setFailure(
        InvalidInputFailure(field: 'endpoint', reason: error.message),
        foreground: token,
      );
    } on Exception {
      _setFailure(
        const InternalFailure('local sync operation failed'),
        foreground: token,
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
    _ForegroundToken? foreground,
    SyncStatusDto? status,
    LocalDiscoveryStatus? discovery,
    List<SyncPeer>? peers,
    Object? prompt = _unset,
    int? pairingSecondsRemaining,
    bool? working,
    Object? failure = _unset,
  }) {
    if (_locked ||
        (generation != null && !_isCurrent(generation)) ||
        (foreground != null && !_isForegroundCurrent(foreground))) {
      return;
    }
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

  void _setFailure(
    AppFailure failure, {
    int? generation,
    _ForegroundToken? foreground,
  }) => _replace(
    generation: generation,
    foreground: foreground,
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

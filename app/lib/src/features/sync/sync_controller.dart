import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../bridge/dto.dart' as bridge;
import '../../data/failures.dart';
import '../../data/models.dart';
import '../../data/repositories.dart';
import '../../providers/providers.dart';

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
  });

  final SyncStatusDto status;
  final LocalDiscoveryStatus discovery;
  final List<SyncPeer> peers;
  final PairingPrompt? prompt;
  final int pairingSecondsRemaining;
  final bool working;
  final AppFailure? failure;

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
  }) => SyncViewState(
    status: status ?? this.status,
    discovery: discovery ?? this.discovery,
    peers: peers ?? this.peers,
    prompt: prompt == _unset ? this.prompt : prompt as PairingPrompt?,
    pairingSecondsRemaining:
        pairingSecondsRemaining ?? this.pairingSecondsRemaining,
    working: working ?? this.working,
    failure: failure == _unset ? this.failure : failure as AppFailure?,
  );
}

const _unset = Object();

final syncControllerProvider =
    AsyncNotifierProvider<SyncController, SyncViewState>(SyncController.new);

class SyncController extends AsyncNotifier<SyncViewState> {
  Timer? _pairingTimer;
  SyncRepository? _repository;

  @override
  Future<SyncViewState> build() async {
    final repository = ref.watch(syncRepositoryProvider);
    _repository = repository;
    ref.onDispose(() {
      _pairingTimer?.cancel();
      unawaited(repository.stopDiscovery());
    });
    ref.listen(syncEventsProvider, (_, next) {
      final event = next.valueOrNull;
      if (event != null) _applyEvent(event);
    });
    final status = await repository.syncStatus();
    final peers = status.configured
        ? await repository.listPeers().onError((_, _) => const <SyncPeer>[])
        : const <SyncPeer>[];
    return SyncViewState(status: status, peers: peers);
  }

  Future<void> discover(DiscoveryKind kind) async {
    await _run(() async {
      final discovery = await _repo.discover(kind);
      _replace(discovery: discovery);
    });
  }

  Future<void> useManualEndpoint(String value) async {
    await _run(() async {
      final endpoint = _parseEndpoint(value);
      await _repo.setManualEndpoint(endpoint);
      _replace(
        discovery: LocalDiscoveryStatus(
          permission: LocalDiscoveryPermission.granted,
          candidates: [endpoint],
        ),
      );
    });
  }

  Future<void> pairThisVault() async {
    await _run(() async {
      final repository = _repo;
      if (!_value.status.configured) {
        await repository.configureLocalSync(LocalSyncRole.client);
      }
      final discovered = await repository.discover(DiscoveryKind.pairing);
      _replace(discovery: discovered);
      _requireDiscovery(discovered);
      _replace(prompt: await repository.beginPairing());
    });
  }

  Future<void> beginPairImport({
    required String name,
    required String masterPassword,
  }) async {
    await _run(() async {
      final repository = _repo;
      final discovered = await repository.discover(DiscoveryKind.pairing);
      _replace(discovery: discovered);
      _requireDiscovery(discovered);
      _replace(
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
      await refresh();
      return imported;
    } on AppFailure catch (failure) {
      _setFailure(failure);
      return null;
    } on Exception {
      _setFailure(const InternalFailure('pairing failed'));
      return null;
    }
  }

  Future<void> startServer() async {
    await _run(() async {
      final repository = _repo;
      if (!_value.status.configured) {
        await repository.configureLocalSync(LocalSyncRole.server);
      }
      final endpoints = await repository.serverEndpoints();
      if (endpoints.isEmpty) throw const SyncUnreachable();
      await repository.startServer(endpoints.first);
      await refresh();
    });
  }

  Future<void> stopServer() async {
    await _run(() async {
      await _repo.stopServer();
      _pairingTimer?.cancel();
      await refresh();
    });
  }

  Future<void> openPairingWindow() async {
    await _run(() async {
      await _repo.openPairingWindow();
      _pairingTimer?.cancel();
      _replace(pairingSecondsRemaining: 180);
      _pairingTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
        final remaining = _value.pairingSecondsRemaining - 1;
        if (remaining <= 0) {
          timer.cancel();
          _replace(pairingSecondsRemaining: 0);
          unawaited(_repo.closePairingWindow());
        } else {
          _replace(pairingSecondsRemaining: remaining);
        }
      });
      await refresh(preserveCountdown: true);
    });
  }

  Future<void> syncNow() async {
    if (_value.inFlight) return;
    _replace(working: true, failure: null);
    try {
      await _repo.discover(DiscoveryKind.trusted);
      await _repo.syncNow();
    } on AppFailure catch (failure) {
      _setFailure(failure);
    } on Exception {
      _setFailure(const InternalFailure('sync failed'));
    }
  }

  Future<void> renamePeer(String peerId, String displayName) async {
    await _run(() async {
      await _repo.renamePeer(peerId, displayName);
      await refresh();
    });
  }

  Future<void> revokePeer(String peerId) async {
    await _run(() async {
      await _repo.revokePeer(peerId);
      await refresh();
    });
  }

  Future<void> openDiscoverySettings() => _repo.openDiscoverySettings();

  Future<void> cancelForegroundOperations({bool updateState = true}) async {
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
    await _repo.cancelSync();
    await _repo.stopDiscovery();
  }

  Future<void> refresh({bool preserveCountdown = false}) async {
    final repository = _repo;
    final status = await repository.syncStatus();
    final peers = status.configured
        ? await repository.listPeers().onError((_, _) => const <SyncPeer>[])
        : const <SyncPeer>[];
    _replace(
      status: status,
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

  Future<void> _run(Future<void> Function() action) async {
    _replace(working: true, failure: null);
    try {
      await action();
      _replace(working: false);
    } on AppFailure catch (failure) {
      _setFailure(failure);
    } on FormatException catch (error) {
      _setFailure(
        InvalidInputFailure(field: 'endpoint', reason: error.message),
      );
    } on Exception {
      _setFailure(const InternalFailure('local sync operation failed'));
    }
  }

  void _applyEvent(bridge.SyncEvent event) {
    final current = state.valueOrNull;
    if (current == null) return;
    switch (event) {
      case bridge.SyncEvent_Started() || bridge.SyncEvent_Activity():
        _replace(
          status: _statusCopy(current.status, inFlight: true),
          working: false,
          failure: null,
        );
      case bridge.SyncEvent_Done(:final field0):
        _replace(
          status: _statusCopy(
            current.status,
            inFlight: false,
            lastOutcome: _outcome(field0),
          ),
          working: false,
          failure: null,
        );
      case bridge.SyncEvent_Failed(:final field0):
        _setFailure(mapApiError(field0));
      case bridge.SyncEvent_PairingRequested(:final field0):
        _replace(
          prompt: PairingPrompt(
            transactionHandle: field0.transactionHandle,
            sas: field0.sas,
          ),
          working: false,
        );
      case bridge.SyncEvent_ServerStarted() || bridge.SyncEvent_ServerStopped():
        unawaited(refresh());
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
    SyncStatusDto? status,
    LocalDiscoveryStatus? discovery,
    List<SyncPeer>? peers,
    Object? prompt = _unset,
    int? pairingSecondsRemaining,
    bool? working,
    Object? failure = _unset,
  }) {
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

  void _setFailure(AppFailure failure) => _replace(
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

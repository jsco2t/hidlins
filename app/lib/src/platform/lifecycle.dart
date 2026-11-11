import 'dart:async';

import 'package:flutter/services.dart';

import '../bridge/dto.dart' show LockEvent;
import 'platform_channel.dart';
import 'platform_result.dart';

enum AppLifecycleSignal { resumed, inactive, hidden, paused, detached }

abstract interface class LifecycleCapability {
  Future<PlatformResult<PlatformUnit>> report(AppLifecycleSignal state);
}

final class MethodChannelLifecycle implements LifecycleCapability {
  const MethodChannelLifecycle([
    this._client = const FlutterPlatformChannelClient(),
  ]);

  static const channelName = 'app.hidlins/lifecycle';
  final PlatformChannelClient _client;

  @override
  Future<PlatformResult<PlatformUnit>> report(AppLifecycleSignal state) {
    return invokePlatform(
      client: _client,
      channel: channelName,
      method: 'reportState',
      arguments: {'state': state.name},
      decode: (_) => platformUnit,
    );
  }

  Future<PlatformResult<PlatformUnit>> acknowledge(
    AppLifecycleSignal state,
    LockEvent lockState, {
    String? observationToken,
  }) {
    return invokePlatform(
      client: _client,
      channel: channelName,
      method: 'reportState',
      arguments: {
        'state': state.name,
        'acknowledged': true,
        'lockState': lockState.name,
        'observationToken': ?observationToken,
      },
      decode: (_) => platformUnit,
    );
  }
}

/// Receives report-only native lifecycle observations on iOS. The callback
/// must forward each raw value to Rust before acknowledging it; neither this
/// adapter nor native code owns lock policy.
final class NativeLifecycleEvents {
  NativeLifecycleEvents([MethodChannel? channel])
    : _channel =
          channel ?? const MethodChannel(MethodChannelLifecycle.channelName);

  final MethodChannel _channel;
  Timer? _bootstrapRetry;

  void listen(
    Future<void> Function(AppLifecycleSignal state, {String? observationToken})
    onSignal,
  ) {
    // A first native event may predate Dart registration. Request a replay
    // independently of Rust or frame completion; a blocked report cannot
    // prevent recovery of the actual native observation.
    _bootstrapRetry?.cancel();
    _bootstrapRetry = Timer.periodic(const Duration(milliseconds: 500), (_) {
      unawaited(_requestCurrentObservation());
    });
    _channel.setMethodCallHandler((call) async {
      if (call.method != 'reportState' || call.arguments is! Map) return;
      final arguments = call.arguments as Map<Object?, Object?>;
      final rawState = arguments['state'];
      if (rawState is! String) return;
      final state = AppLifecycleSignal.values.where((value) {
        return value.name == rawState;
      }).firstOrNull;
      if (state == null) return;
      final rawToken = arguments['observationToken'];
      final token =
          rawToken is String && rawToken.isNotEmpty && rawToken.length <= 128
          ? rawToken
          : null;
      if (token != null) _bootstrapRetry?.cancel();
      await onSignal(state, observationToken: token);
    });
  }

  Future<void> _requestCurrentObservation() async {
    try {
      final result = await _channel.invokeMethod<Object?>('reportState', {
        'requestObservation': true,
      });
      if (result is Map &&
          (result['status'] == 'unsupported' ||
              result['status'] == 'failure')) {
        _bootstrapRetry?.cancel();
      }
    } on MissingPluginException {
      _bootstrapRetry?.cancel();
    } on PlatformException {
      _bootstrapRetry?.cancel();
    }
  }

  void dispose() {
    _bootstrapRetry?.cancel();
    _channel.setMethodCallHandler(null);
  }
}

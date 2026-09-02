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
    LockEvent lockState,
  ) {
    return invokePlatform(
      client: _client,
      channel: channelName,
      method: 'reportState',
      arguments: {
        'state': state.name,
        'acknowledged': true,
        'lockState': lockState.name,
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

  void listen(Future<void> Function(AppLifecycleSignal state) onSignal) {
    _channel.setMethodCallHandler((call) async {
      if (call.method != 'reportState' || call.arguments is! Map) return;
      final arguments = call.arguments as Map<Object?, Object?>;
      final rawState = arguments['state'];
      if (rawState is! String) return;
      final state = AppLifecycleSignal.values.where((value) {
        return value.name == rawState;
      }).firstOrNull;
      if (state != null) await onSignal(state);
    });
  }

  void dispose() {
    _channel.setMethodCallHandler(null);
  }
}

import 'package:flutter/services.dart';

import 'platform_result.dart';

abstract interface class PlatformChannelClient {
  Future<Object?> invoke(String channel, String method, [Object? arguments]);
}

final class FlutterPlatformChannelClient implements PlatformChannelClient {
  const FlutterPlatformChannelClient();

  @override
  Future<Object?> invoke(
    String channel,
    String method, [
    Object? arguments,
  ]) async {
    return await MethodChannel(channel)
        .invokeMethod<Object?>(method, arguments);
  }
}

Future<PlatformResult<T>> invokePlatform<T>({
  required PlatformChannelClient client,
  required String channel,
  required String method,
  Object? arguments,
  required T Function(Object? value) decode,
}) async {
  try {
    final raw = await client.invoke(channel, method, arguments);
    if (raw is! Map<Object?, Object?>) {
      return PlatformFailure<T>('invalid-native-result');
    }
    final PlatformResult<T> result = switch (raw['status']) {
      'success' => PlatformSuccess(decode(raw['value'])),
      'canceled' => PlatformCanceled<T>(),
      'denied' => PlatformDenied<T>(),
      'stale' => PlatformStale<T>(),
      'unsupported' => PlatformUnsupported<T>(),
      'failure' => PlatformFailure<T>(_safeCode(raw['code'])),
      _ => PlatformFailure<T>('invalid-native-status'),
    };
    return result;
  } on MissingPluginException {
    return PlatformUnsupported<T>();
  } on PlatformException catch (error) {
    return PlatformFailure<T>(_safeCode(error.code));
  } on Object {
    return PlatformFailure<T>('platform-dispatch-failed');
  }
}

String _safeCode(Object? value) {
  final code = value is String ? value : 'platform-error';
  return RegExp(r'^[a-zA-Z0-9._-]{1,64}$').hasMatch(code)
      ? code
      : 'platform-error';
}

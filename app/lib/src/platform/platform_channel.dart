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
      return const PlatformFailure('invalid-native-result');
    }
    final PlatformResult<T> result = switch (raw['status']) {
      'success' => PlatformSuccess(decode(raw['value'])),
      'canceled' => const PlatformCanceled(),
      'denied' => const PlatformDenied(),
      'stale' => const PlatformStale(),
      'unsupported' => const PlatformUnsupported(),
      'failure' => PlatformFailure(_safeCode(raw['code'])),
      _ => const PlatformFailure('invalid-native-status'),
    };
    return result;
  } on MissingPluginException {
    return const PlatformUnsupported();
  } on PlatformException catch (error) {
    return PlatformFailure(_safeCode(error.code));
  } on Object {
    return const PlatformFailure('platform-dispatch-failed');
  }
}

String _safeCode(Object? value) {
  final code = value is String ? value : 'platform-error';
  return RegExp(r'^[a-zA-Z0-9._-]{1,64}$').hasMatch(code)
      ? code
      : 'platform-error';
}

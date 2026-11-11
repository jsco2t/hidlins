import 'platform_channel.dart';
import 'platform_result.dart';

abstract interface class AppPathsCapability {
  Future<PlatformResult<String>> applicationSupportPath();
}

final class MethodChannelAppPaths implements AppPathsCapability {
  const MethodChannelAppPaths([
    this._client = const FlutterPlatformChannelClient(),
  ]);

  static const channelName = 'app.hidlins/paths';
  final PlatformChannelClient _client;

  @override
  Future<PlatformResult<String>> applicationSupportPath() {
    return invokePlatform(
      client: _client,
      channel: channelName,
      method: 'applicationSupportPath',
      decode: (value) {
        if (value case final String path when path.isNotEmpty) return path;
        throw const FormatException('invalid application-support path');
      },
    );
  }
}

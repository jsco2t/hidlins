import 'platform_channel.dart';
import 'platform_result.dart';

final class PreparedClipboardTransfer {
  const PreparedClipboardTransfer({
    required this.id,
    required this.expiresAfterSeconds,
  });

  final String id;
  final int expiresAfterSeconds;
}

abstract interface class SecureClipboardCapability {
  Future<PlatformResult<PlatformUnit>> copyPrepared(
    PreparedClipboardTransfer transfer,
  );
}

final class MethodChannelSecureClipboard implements SecureClipboardCapability {
  const MethodChannelSecureClipboard([
    this._client = const FlutterPlatformChannelClient(),
  ]);

  static const channelName = 'app.hidlins/clipboard';
  final PlatformChannelClient _client;

  @override
  Future<PlatformResult<PlatformUnit>> copyPrepared(
    PreparedClipboardTransfer transfer,
  ) {
    return invokePlatform(
      client: _client,
      channel: channelName,
      method: 'copySecret',
      arguments: {
        'transferId': transfer.id,
        'ttlSeconds': transfer.expiresAfterSeconds,
      },
      decode: (_) => platformUnit,
    );
  }
}

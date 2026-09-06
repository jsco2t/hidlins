import 'platform_channel.dart';
import 'platform_result.dart';

final class KeyfileReference {
  const KeyfileReference({required this.reference, required this.displayName});

  final String reference;
  final String displayName;
}

abstract interface class KeyfileAccessCapability {
  Future<PlatformResult<KeyfileReference>> pickReference();
  Future<PlatformResult<String>> resolve(KeyfileReference reference);
  Future<PlatformResult<PlatformUnit>> release(KeyfileReference reference);
}

final class MethodChannelKeyfileAccess implements KeyfileAccessCapability {
  const MethodChannelKeyfileAccess([
    this._client = const FlutterPlatformChannelClient(),
  ]);

  static const channelName = 'app.hidlins/keyfile';
  final PlatformChannelClient _client;

  @override
  Future<PlatformResult<KeyfileReference>> pickReference() {
    return invokePlatform(
      client: _client,
      channel: channelName,
      method: 'pickReference',
      decode: (value) {
        if (value
            case {
              'reference': final String reference,
              'displayName': final String name,
            }
            when reference.isNotEmpty && name.isNotEmpty) {
          return KeyfileReference(reference: reference, displayName: name);
        }
        throw const FormatException('invalid native reference');
      },
    );
  }

  @override
  Future<PlatformResult<String>> resolve(KeyfileReference reference) {
    return invokePlatform(
      client: _client,
      channel: channelName,
      method: 'resolveReference',
      arguments: {'reference': reference.reference},
      decode: (value) {
        if (value case final String path when path.isNotEmpty) return path;
        throw const FormatException('invalid native path');
      },
    );
  }

  @override
  Future<PlatformResult<PlatformUnit>> release(KeyfileReference reference) {
    return invokePlatform(
      client: _client,
      channel: channelName,
      method: 'releaseReference',
      arguments: {'reference': reference.reference},
      decode: (_) => platformUnit,
    );
  }
}

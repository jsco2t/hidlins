import '../data/models.dart';
import 'platform_channel.dart';
import 'platform_result.dart';

abstract interface class LocalDiscoveryCapability {
  Future<PlatformResult<LocalDiscoveryStatus>> permissionStatus();
  Future<PlatformResult<LocalDiscoveryStatus>> discover(DiscoveryKind kind);
  Future<PlatformResult<PlatformUnit>> openSettings();
  Future<PlatformResult<PlatformUnit>> stop();
}

final class MethodChannelLocalDiscovery implements LocalDiscoveryCapability {
  const MethodChannelLocalDiscovery([
    this._client = const FlutterPlatformChannelClient(),
  ]);

  static const channelName = 'app.hidlins/local_discovery';
  static const maxCandidates = 8;
  final PlatformChannelClient _client;

  @override
  Future<PlatformResult<LocalDiscoveryStatus>> permissionStatus() =>
      invokePlatform(
        client: _client,
        channel: channelName,
        method: 'permissionStatus',
        decode: _decodeStatus,
      );

  @override
  Future<PlatformResult<LocalDiscoveryStatus>> discover(DiscoveryKind kind) =>
      invokePlatform(
        client: _client,
        channel: channelName,
        method: 'discover',
        arguments: {'kind': kind.name, 'timeoutMs': 1200},
        decode: _decodeStatus,
      );

  @override
  Future<PlatformResult<PlatformUnit>> openSettings() => invokePlatform(
    client: _client,
    channel: channelName,
    method: 'openSettings',
    decode: (_) => platformUnit,
  );

  @override
  Future<PlatformResult<PlatformUnit>> stop() => invokePlatform(
    client: _client,
    channel: channelName,
    method: 'stop',
    decode: (_) => platformUnit,
  );

  static LocalDiscoveryStatus _decodeStatus(Object? raw) {
    if (raw is! Map<Object?, Object?>) {
      throw const FormatException('invalid discovery result');
    }
    final permission = switch (raw['permission']) {
      'notDetermined' => LocalDiscoveryPermission.notDetermined,
      'granted' => LocalDiscoveryPermission.granted,
      'denied' => LocalDiscoveryPermission.denied,
      'restricted' => LocalDiscoveryPermission.restricted,
      'unavailable' => LocalDiscoveryPermission.unavailable,
      _ => throw const FormatException('invalid discovery permission'),
    };
    final candidates = raw['candidates'];
    if (candidates is! List || candidates.length > maxCandidates) {
      throw const FormatException('invalid discovery candidates');
    }
    return LocalDiscoveryStatus(
      permission: permission,
      candidates: candidates.map(_decodeEndpoint).toList(growable: false),
    );
  }

  static LocalEndpoint _decodeEndpoint(Object? raw) {
    if (raw is! Map<Object?, Object?> ||
        raw.length != 3 ||
        !raw.containsKey('address') ||
        !raw.containsKey('port') ||
        !raw.containsKey('scopeId') ||
        raw['address'] is! String ||
        raw['port'] is! int ||
        raw['scopeId'] is! int) {
      throw const FormatException('invalid discovery endpoint');
    }
    final address = raw['address'] as String;
    final port = raw['port'] as int;
    final scopeId = raw['scopeId'] as int;
    if (address.isEmpty ||
        address.length > 64 ||
        port < 1 ||
        port > 65535 ||
        scopeId < 0) {
      throw const FormatException('invalid discovery endpoint');
    }
    return LocalEndpoint(address: address, port: port, scopeId: scopeId);
  }
}

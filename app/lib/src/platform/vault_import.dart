import 'platform_channel.dart';
import 'platform_result.dart';

final class ImportedVault {
  const ImportedVault({
    required this.sourceReference,
    required this.displayName,
  });

  final String sourceReference;
  final String displayName;
}

abstract interface class VaultImportCapability {
  Future<PlatformResult<ImportedVault>> pickVault();
}

final class MethodChannelVaultImport implements VaultImportCapability {
  const MethodChannelVaultImport([
    this._client = const FlutterPlatformChannelClient(),
  ]);

  static const channelName = 'app.hidlins/vault_import';
  final PlatformChannelClient _client;

  @override
  Future<PlatformResult<ImportedVault>> pickVault() {
    return invokePlatform(
      client: _client,
      channel: channelName,
      method: 'pickVault',
      decode: (value) {
        if (value
            case {
              'sourceReference': final String source,
              'displayName': final String name,
            }
            when source.isNotEmpty && name.isNotEmpty) {
          return ImportedVault(sourceReference: source, displayName: name);
        }
        throw const FormatException('invalid imported-vault result');
      },
    );
  }
}

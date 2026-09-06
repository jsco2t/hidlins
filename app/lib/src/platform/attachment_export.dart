import 'platform_channel.dart';
import 'platform_result.dart';

abstract interface class AttachmentExportCapability {
  Future<PlatformResult<String>> chooseDestination(String suggestedName);
}

final class MethodChannelAttachmentExport
    implements AttachmentExportCapability {
  const MethodChannelAttachmentExport([
    this._client = const FlutterPlatformChannelClient(),
  ]);

  static const channelName = 'app.hidlins/attachment_export';
  final PlatformChannelClient _client;

  @override
  Future<PlatformResult<String>> chooseDestination(String suggestedName) {
    return invokePlatform(
      client: _client,
      channel: channelName,
      method: 'chooseDestination',
      arguments: {
        'suggestedName': sanitizeAttachmentSuggestedName(suggestedName),
      },
      decode: (value) {
        if (value case final String path when path.isNotEmpty) return path;
        throw const FormatException('invalid attachment destination');
      },
    );
  }
}

String sanitizeAttachmentSuggestedName(String value) {
  final output = StringBuffer();
  for (final rune in value.runes) {
    final isControl = rune <= 0x1f || (rune >= 0x7f && rune <= 0x9f);
    final isSeparator = rune == 0x2f || rune == 0x5c;
    if (!isControl && !isSeparator) output.writeCharCode(rune);
  }
  final sanitized = output.toString().trim();
  if (sanitized.isEmpty || sanitized == '.' || sanitized == '..') {
    return 'attachment';
  }
  return sanitized;
}

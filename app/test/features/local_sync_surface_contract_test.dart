import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

void main() {
  test('Flutter and mobile surfaces are local-sync-only', () {
    final active = <String>[
      'lib/src/data/models.dart',
      'lib/src/data/repositories.dart',
      'lib/src/data/bridge_repositories.dart',
      'lib/src/features/sync/sync_controller.dart',
      'lib/src/features/sync/sync_page.dart',
      'lib/src/features/vaults/connect_sync_dialog.dart',
      'l10n/app_en.arb',
    ].map((path) => File(path).readAsStringSync()).join('\n');

    for (final required in [
      'Pair this vault',
      'Import paired vault',
      'Compare this code',
      'Sync now',
      'Manage peers',
      'Allow pairing for 3 minutes',
      'Use an IP address manually',
    ]) {
      expect(active, contains(required), reason: required);
    }

    final channels = File('lib/src/platform/channels.json').readAsStringSync();
    expect(channels, contains('local_discovery'));
    expect(channels, contains('permissionStatus'));
    expect(channels, contains('discover'));
    expect(channels, contains('openSettings'));

    final ios = File('ios/Runner/Info.plist').readAsStringSync();
    expect(ios, contains('NSLocalNetworkUsageDescription'));
    expect(ios, contains('_hidlins-sync._tcp'));
    expect(ios, contains('_hidlins-pair._tcp'));

    final android = File('android/app/src/main/AndroidManifest.xml')
        .readAsStringSync();
    expect(android, contains('ACCESS_NETWORK_STATE'));
    expect(android, contains('CHANGE_WIFI_MULTICAST_STATE'));
    expect(android, contains('NEARBY_WIFI_DEVICES'));
    expect(android, isNot(contains('<service')));
  });
}

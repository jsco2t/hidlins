import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:app/src/bridge/dto.dart' show LockEvent;
import 'package:app/src/platform/app_paths.dart';
import 'package:app/src/platform/keyfile_access.dart';
import 'package:app/src/platform/lifecycle.dart';
import 'package:app/src/platform/platform_channel.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/platform/secure_clipboard.dart';
import 'package:app/src/platform/vault_import.dart';

import '../fakes/fake_platform_capabilities.dart';

final class _RecordingClient implements PlatformChannelClient {
  _RecordingClient({this.result, this.error});

  Object? result;
  Object? error;
  String? channel;
  String? method;
  Object? arguments;

  @override
  Future<Object?> invoke(
    String channel,
    String method, [
    Object? arguments,
  ]) async {
    this.channel = channel;
    this.method = method;
    this.arguments = arguments;
    if (error case final Object value) throw value;
    return result;
  }
}

String _kind(PlatformResult<Object?> result) => switch (result) {
  PlatformSuccess<Object?>() => 'success',
  PlatformCanceled<Object?>() => 'canceled',
  PlatformDenied<Object?>() => 'denied',
  PlatformStale<Object?>() => 'stale',
  PlatformUnsupported<Object?>() => 'unsupported',
  PlatformFailure<Object?>() => 'failure',
};

void main() {
  test(
    'lifecycle adapter reports one raw transition and owns no lock policy',
    () async {
      final client = _RecordingClient(result: {'status': 'success'});
      final service = MethodChannelLifecycle(client);

      final result = await service.report(AppLifecycleSignal.paused);

      expect(result, isA<PlatformSuccess<PlatformUnit>>());
      expect(client.channel, 'app.hidlins/lifecycle');
      expect(client.method, 'reportState');
      expect(client.arguments, {'state': 'paused'});
    },
  );

  test('iOS lifecycle acknowledgement carries Rust-known lock state', () async {
    final client = _RecordingClient(result: {'status': 'success'});
    final service = MethodChannelLifecycle(client);

    final result = await service.acknowledge(
      AppLifecycleSignal.resumed,
      LockEvent.locked,
    );

    expect(result, isA<PlatformSuccess<PlatformUnit>>());
    expect(client.arguments, {
      'state': 'resumed',
      'acknowledged': true,
      'lockState': 'locked',
    });
  });

  test(
    'clipboard adapter sends only the one-shot ticket, never plaintext',
    () async {
      final client = _RecordingClient(result: {'status': 'success'});
      final service = MethodChannelSecureClipboard(client);

      final result = await service.copyPrepared(
        const PreparedClipboardTransfer(
          id: 'hct-ticket',
          expiresAfterSeconds: 30,
        ),
      );

      expect(result, isA<PlatformSuccess<PlatformUnit>>());
      expect(client.channel, 'app.hidlins/clipboard');
      expect(client.method, 'copySecret');
      expect(client.arguments, {'transferId': 'hct-ticket', 'ttlSeconds': 30});
      expect(client.arguments.toString(), isNot(contains('password')));
      expect(client.arguments.toString(), isNot(contains('secretValue')));
    },
  );

  test(
    'paths, import, and keyfile adapters decode successful values',
    () async {
      final pathsClient = _RecordingClient(
        result: {'status': 'success', 'value': '/app/support'},
      );
      final importClient = _RecordingClient(
        result: {
          'status': 'success',
          'value': {
            'sourceReference': 'provider://vault',
            'displayName': 'vault.kdbx',
          },
        },
      );
      final keyfileClient = _RecordingClient(
        result: {
          'status': 'success',
          'value': {'reference': 'bookmark-1', 'displayName': 'vault.key'},
        },
      );

      final path = await MethodChannelAppPaths(pathsClient)
          .applicationSupportPath();
      final imported = await MethodChannelVaultImport(importClient).pickVault();
      final keyfile = await MethodChannelKeyfileAccess(keyfileClient)
          .pickReference();

      expect((path as PlatformSuccess<String>).value, '/app/support');
      expect(
        (imported as PlatformSuccess<ImportedVault>).value.displayName,
        'vault.kdbx',
      );
      expect(
        (keyfile as PlatformSuccess<KeyfileReference>).value.reference,
        'bookmark-1',
      );
    },
  );

  test('every documented native result state maps explicitly', () async {
    for (final status in [
      'canceled',
      'denied',
      'stale',
      'unsupported',
      'failure',
    ]) {
      final client = _RecordingClient(
        result: {'status': status, 'code': 'native-$status'},
      );
      final result = await MethodChannelAppPaths(client)
          .applicationSupportPath();
      expect(_kind(result), status == 'failure' ? 'failure' : status);
    }
  });

  test('missing plugin and native exceptions fail closed', () async {
    final missing = await MethodChannelAppPaths(
      _RecordingClient(error: MissingPluginException()),
    ).applicationSupportPath();
    final failed = await MethodChannelAppPaths(
      _RecordingClient(error: PlatformException(code: 'native-path-error')),
    ).applicationSupportPath();

    expect(missing, isA<PlatformUnsupported<String>>());
    expect((failed as PlatformFailure<String>).code, 'native-path-error');
  });

  test('keyfile stale access and release are explicit', () async {
    final staleClient = _RecordingClient(result: {'status': 'stale'});
    final service = MethodChannelKeyfileAccess(staleClient);
    final reference = const KeyfileReference(
      reference: 'bookmark-1',
      displayName: 'vault.key',
    );

    expect(await service.resolve(reference), isA<PlatformStale<String>>());
    staleClient.result = {'status': 'denied'};
    expect(
      await service.release(reference),
      isA<PlatformDenied<PlatformUnit>>(),
    );
  });

  test('focused fakes script races and every explicit result state', () async {
    final lifecycle = FakeLifecycleCapability([
      const PlatformSuccess(platformUnit),
      const PlatformFailure('native-race'),
    ]);
    await lifecycle.report(AppLifecycleSignal.paused);
    expect(
      await lifecycle.report(AppLifecycleSignal.resumed),
      isA<PlatformFailure<PlatformUnit>>(),
    );
    expect(lifecycle.reports, [
      AppLifecycleSignal.paused,
      AppLifecycleSignal.resumed,
    ]);

    final paths = FakeAppPathsCapability([
      const PlatformSuccess('/support'),
      const PlatformCanceled(),
      const PlatformDenied(),
      const PlatformStale(),
      const PlatformUnsupported(),
      const PlatformFailure('native-error'),
    ]);
    final kinds = <String>[];
    for (var index = 0; index < 6; index += 1) {
      kinds.add(_kind(await paths.applicationSupportPath()));
    }
    expect(kinds, [
      'success',
      'canceled',
      'denied',
      'stale',
      'unsupported',
      'failure',
    ]);

    final clipboard = FakeSecureClipboardCapability([
      const PlatformUnsupported(),
    ]);
    expect(
      await clipboard.copyPrepared(
        const PreparedClipboardTransfer(id: 'ticket', expiresAfterSeconds: 30),
      ),
      isA<PlatformUnsupported<PlatformUnit>>(),
    );
    expect(clipboard.copies.single.id, 'ticket');

    final importer = FakeVaultImportCapability([const PlatformCanceled()]);
    expect(await importer.pickVault(), isA<PlatformCanceled<ImportedVault>>());

    final keyfiles = FakeKeyfileAccessCapability(
      pickResults: [const PlatformDenied()],
      resolveResults: [const PlatformStale()],
      releaseResults: [const PlatformFailure('release-failed')],
    );
    const reference = KeyfileReference(reference: 'ref', displayName: 'key');
    expect(
      await keyfiles.pickReference(),
      isA<PlatformDenied<KeyfileReference>>(),
    );
    expect(await keyfiles.resolve(reference), isA<PlatformStale<String>>());
    expect(
      await keyfiles.release(reference),
      isA<PlatformFailure<PlatformUnit>>(),
    );
  });
}

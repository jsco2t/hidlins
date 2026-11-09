import 'dart:collection';
import 'dart:io';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/frb_generated.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/platform/attachment_export.dart';
import 'package:app/src/platform/platform_channel.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

class _Client implements PlatformChannelClient {
  _Client(Iterable<Object?> sources, this.destination)
    : sources = Queue.of(sources);
  final Queue<Object?> sources;
  final String destination;
  final calls = <(String, String, Object?)>[];

  int get sourceCalls => calls.where((c) => c.$2 == 'chooseSource').length;

  @override
  Future<Object?> invoke(
    String channel,
    String method, [
    Object? arguments,
  ]) async {
    calls.add((channel, method, arguments));
    if (method == 'chooseSource') return sources.removeFirst();
    if (method == 'chooseDestination') {
      return {'status': 'success', 'value': destination};
    }
    throw StateError('unexpected fixture method');
  }
}

Future<void> _drive(WidgetTester tester, bool Function() done) async {
  final elapsed = Stopwatch()..start();
  while (!done() && elapsed.elapsed < const Duration(seconds: 10)) {
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 1)),
    );
    await tester.pump(const Duration(milliseconds: 20));
  }
  expect(done(), isTrue, reason: 'native/UI operation completed');
}

Future<void> _tapAttach(WidgetTester tester) async {
  final action = find.text('Attach file');
  expect(action, findsOneWidget);
  await tester.ensureVisible(action);
  await tester.tap(action);
  await tester.pump();
}

void main() {
  setUpAll(() async {
    final path = Platform.environment['HIDLINS_API_LIB'];
    expect(path, isNotNull, reason: 'run make app-test-attachment-source');
    expect(File(path!).existsSync(), isTrue);
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
  });

  testWidgets(
    'shipping source path stays native through attach and export roundtrip',
    (tester) async {
      debugDefaultTargetPlatformOverride = TargetPlatform.linux;
      addTearDown(() => debugDefaultTargetPlatformOverride = null);
      tester.view.devicePixelRatio = 1;
      tester.view.physicalSize = const Size(1280, 1100);
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);
      final directory = Directory.systemTemp.createTempSync('hidlins-source-');
      final source = File('${directory.path}/payload.bin');
      final bytes = List<int>.generate(16384, (i) => (i * 31) % 256);
      source.writeAsBytesSync(bytes);
      final large = File('${directory.path}/oversize.bin');
      final sparse = large.openSync(mode: FileMode.write);
      sparse.truncateSync(100 * 1024 * 1024 + 1);
      sparse.closeSync();
      final output = File('${directory.path}/saved.bin');
      final client = _Client([
        {'status': 'success', 'value': source.path},
        const {'status': 'canceled'},
        {'status': 'success', 'value': large.path},
      ], output.path);
      final session = (await tester.runAsync(
        () => api.initApp(cfg: bridge.AppInitConfig(stateDir: directory.path)),
      ))!;
      final repo = BridgeEntryRepository(session);
      var notifications = 0;
      try {
        await tester.runAsync(() async {
          await session.createVault(
            name: 'fixture',
            masterPassword: 'attachment-master-fixture',
            confirmedNoRecovery: true,
          );
          await session.unlock(
            name: 'fixture',
            masterPassword: 'attachment-master-fixture',
          );
        });
        final root = (await tester.runAsync(session.vaultTree))!.root.uuid;
        final uuid = (await tester.runAsync(
          () => session.createEntry(
            group: root,
            draft: const bridge.EntryDraftDto(
              kind: bridge.EntryKindDto.credential,
              title: 'prior subject',
              password: 'attachment-secret-marker',
              notes: 'retained notes',
              tags: [],
              customFields: [],
            ),
          ),
        ))!;
        await tester.runAsync(
          () => session.updateEntry(
            uuid: uuid,
            edit: const bridge.EntryEditDto(title: 'subject'),
          ),
        );
        final unrelated = (await tester.runAsync(
          () => session.createEntry(
            group: root,
            draft: const bridge.EntryDraftDto(
              kind: bridge.EntryKindDto.secureNote,
              title: 'unrelated',
              notes: 'unrelated notes',
              tags: [],
              customFields: [],
            ),
          ),
        ))!;
        await tester.pumpWidget(
          HidlinsApp(
            overrides: [
              appSessionProvider.overrideWithValue(session),
              entryRepositoryProvider.overrideWithValue(repo),
              attachmentExportCapabilityProvider.overrideWithValue(
                MethodChannelAttachmentExport(client),
              ),
            ],
          ),
        );
        await _drive(tester, () => find.text('subject').evaluate().isNotEmpty);
        final notify = repo.onMutation!;
        repo.onMutation = () {
          notifications++;
          notify();
        };
        await tester.tap(find.text('subject').first);
        await _drive(
          tester,
          () => find.byTooltip('Edit').evaluate().isNotEmpty,
        );
        await tester.pump(const Duration(milliseconds: 350));
        await _tapAttach(tester);
        expect(
          client.sourceCalls,
          1,
          reason: 'shipping detail invokes source selection',
        );
        expect(client.calls.single, (
          'app.hidlins/attachment_export',
          'chooseSource',
          null,
        ));
        await _drive(
          tester,
          () =>
              notifications == 1 &&
              find.text('payload.bin').evaluate().isNotEmpty,
        );
        final detail = (await tester.runAsync(
          () => session.entryDetail(uuid: uuid),
        ))!;
        expect(detail.uuid, uuid);
        expect(detail.notes, 'retained notes');
        expect(detail.attachments.single.name, 'payload.bin');
        expect(detail.attachments.single.sizeBytes, BigInt.from(bytes.length));
        expect(
          (await tester.runAsync(() => session.entryHistory(uuid: uuid)))!
              .single
              .title,
          'prior subject',
        );

        final export = find.byTooltip('Save payload.bin as…');
        expect(export, findsOneWidget);
        await tester.ensureVisible(export);
        await tester.tap(export);
        await _drive(tester, output.existsSync);
        expect(output.readAsBytesSync(), bytes);
        expect(
          (client.calls.last.$1, client.calls.last.$2),
          ('app.hidlins/attachment_export', 'chooseDestination'),
        );
        expect(client.calls.last.$3, {'suggestedName': 'payload.bin'});
        expect(notifications, 1, reason: 'export does not mutate the vault');

        await _tapAttach(tester);
        expect(client.sourceCalls, 2);
        await tester.pumpAndSettle();
        expect(notifications, 1);
        expect(find.text('Could not choose attachment'), findsNothing);
        final vault = File(
          (await tester.runAsync(session.listVaults))!.single.path,
        );
        final beforeCap = vault.readAsBytesSync();
        await _tapAttach(tester);
        expect(client.sourceCalls, 3);
        await _drive(
          tester,
          () => find.text('File too large').evaluate().isNotEmpty,
        );
        expect(notifications, 1);
        expect(vault.readAsBytesSync(), beforeCap);
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: uuid)))!
              .attachments,
          hasLength(1),
        );
        expect(
          (await tester.runAsync(() => session.entryDetail(uuid: unrelated)))!
              .notes,
          'unrelated notes',
        );
        expect(find.text('attachment-secret-marker'), findsNothing);
        expect(find.textContaining(directory.path), findsNothing);

        await tester.runAsync(session.lockNow);
        await tester.runAsync(
          () => session.unlock(
            name: 'fixture',
            masterPassword: 'attachment-master-fixture',
          ),
        );
        final reopened = (await tester.runAsync(
          () => session.entryDetail(uuid: uuid),
        ))!;
        expect(reopened.attachments.single.name, 'payload.bin');
        expect(reopened.notes, 'retained notes');
        expect(
          (await tester.runAsync(() => session.entryHistory(uuid: uuid)))!
              .single
              .title,
          'prior subject',
        );
        expect(
          await tester.runAsync(
            () => session.revealField(
              uuid: uuid,
              field: const bridge.RevealField.password(),
            ),
          ),
          'attachment-secret-marker',
        );
        final secondOutput = File('${directory.path}/reopened.bin');
        await tester.runAsync(
          () => session.saveAttachmentTo(
            uuid: uuid,
            key: 'payload.bin',
            destPath: secondOutput.path,
          ),
        );
        expect(secondOutput.readAsBytesSync(), bytes);
        expect(tester.takeException(), isNull);
      } finally {
        repo.onMutation = null;
        await tester.pumpWidget(const SizedBox.shrink());
        await tester.pump();
        await tester.runAsync(session.shutdown);
        await tester.pump();
        directory.deleteSync(recursive: true);
        debugDefaultTargetPlatformOverride = null;
      }
    },
  );
}

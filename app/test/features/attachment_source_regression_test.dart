import 'dart:async';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_detail.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/platform/attachment_export.dart';
import 'package:app/src/platform/platform_channel.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_repositories.dart';
import '../helpers/feature_test_helpers.dart';

// The existing save-only operation lets the protocol fixture run before the
// additive source method exists. The inherent method takes precedence later.
extension ExistingAttachmentSource on MethodChannelAttachmentExport {
  Future<PlatformResult<String>> chooseSource() =>
      chooseDestination('attachment');
}

class _Client implements PlatformChannelClient {
  Object? result = const {'status': 'success', 'value': '/chosen/report.bin'};
  Object? error;
  Completer<Object?>? gate;
  final calls = <(String, String, Object?)>[];

  @override
  Future<Object?> invoke(
    String channel,
    String method, [
    Object? arguments,
  ]) async {
    calls.add((channel, method, arguments));
    if (error case final error?) throw error;
    if (gate case final gate?) return gate.future;
    return result;
  }
}

EntryDetail _detail(String uuid, List<AttachmentMeta> attachments) =>
    EntryDetail(
      uuid: uuid,
      title: uuid == 'entry-1' ? 'subject' : 'unrelated',
      username: '',
      hasPassword: false,
      url: '',
      notes: 'retained notes',
      kind: EntryKindDto.secureNote,
      tags: const [],
      customFields: const [],
      attachments: attachments,
    );

class _Entries extends FakeEntryRepository {
  final added = <(String, String)>[];
  _Entries() {
    tree = VaultTree(
      root: GroupNode(
        uuid: 'root-uuid',
        name: 'Root',
        entryCount: BigInt.two,
        children: const [],
      ),
      entries: [
        for (final uuid in ['entry-1', 'entry-2'])
          EntrySummary(
            uuid: uuid,
            title: uuid == 'entry-1' ? 'subject' : 'unrelated',
            username: '',
            url: '',
            kind: EntryKindDto.secureNote,
            hasTotp: false,
            hasAttachments: false,
            isExpired: false,
            groupUuid: 'root-uuid',
            tags: const [],
          ),
      ],
    );
    details['entry-1'] = _detail('entry-1', const []);
    details['entry-2'] = _detail('entry-2', const []);
  }

  @override
  Future<void> addAttachment(String uuid, String sourcePath) async {
    added.add((uuid, sourcePath));
    if (attachmentError case final error?) throw error;
    details[uuid] = _detail(uuid, [
      ...details[uuid]!.attachments,
      AttachmentMeta(name: sourcePath.split('/').last, sizeBytes: 7),
    ]);
    onMutation?.call();
  }
}

Future<TestHarness> _mount(
  WidgetTester tester,
  _Entries entries,
  _Client client, {
  TestHarness? harness,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(1280, 1100);
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
  final h = harness ?? TestHarness();
  h.session.currentLockState = LockEvent.unlocked;
  registerTestVault(h);
  await tester.pumpWidget(
    HidlinsApp(
      overrides: [
        sessionRepositoryProvider.overrideWithValue(h.session),
        entryRepositoryProvider.overrideWithValue(entries),
        secretsRepositoryProvider.overrideWithValue(h.secrets),
        searchRepositoryProvider.overrideWithValue(h.search),
        totpRepositoryProvider.overrideWithValue(h.totp),
        generatorRepositoryProvider.overrideWithValue(h.generator),
        syncRepositoryProvider.overrideWithValue(h.sync),
        prefsRepositoryProvider.overrideWithValue(h.prefs),
        keyfileAccessCapabilityProvider.overrideWithValue(h.keyfiles),
        vaultImportCapabilityProvider.overrideWithValue(h.vaultImport),
        attachmentExportCapabilityProvider.overrideWithValue(
          MethodChannelAttachmentExport(client),
        ),
      ],
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.text('subject').first);
  await tester.pumpAndSettle();
  return h;
}

Future<void> _close(WidgetTester tester, TestHarness h) async {
  await tester.pumpWidget(const SizedBox.shrink());
  await tester.pump();
  h.dispose();
  await tester.pump();
}

TextButton _attachButton(WidgetTester tester) => tester.widget<TextButton>(
  find
      .ancestor(
        of: find.text('Attach file'),
        matching: find.byWidgetPredicate((w) => w is TextButton),
      )
      .first,
);

Future<void> _attach(WidgetTester tester) async {
  final action = find.text('Attach file');
  expect(action, findsOneWidget);
  await tester.ensureVisible(action);
  await tester.tap(action);
  await tester.pumpAndSettle();
}

void main() {
  test(
    'source dispatch uses the approved method without payload arguments',
    () async {
      final client = _Client();
      final result = await MethodChannelAttachmentExport(client).chooseSource();
      expect((result as PlatformSuccess<String>).value, '/chosen/report.bin');
      expect(client.calls.single, (
        'app.hidlins/attachment_export',
        'chooseSource',
        null,
      ));
    },
  );

  test(
    'source adapter preserves cancellation and rejects malformed values',
    () async {
      for (final fixture in <(Object?, Type)>[
        (const {'status': 'canceled'}, PlatformCanceled<String>),
        (const {'status': 'denied'}, PlatformDenied<String>),
        (const {'status': 'stale'}, PlatformStale<String>),
        (const {'status': 'unsupported'}, PlatformUnsupported<String>),
        (const {'status': 'success', 'value': ''}, PlatformFailure<String>),
        (
          const {
            'status': 'success',
            'value': <int>[1, 2],
          },
          PlatformFailure<String>,
        ),
        ('malformed', PlatformFailure<String>),
      ]) {
        final client = _Client()..result = fixture.$1;
        final result = await MethodChannelAttachmentExport(client)
            .chooseSource();
        expect(result.runtimeType, fixture.$2);
      }
      final missing = _Client()..error = MissingPluginException();
      expect(
        await MethodChannelAttachmentExport(missing).chooseSource(),
        isA<PlatformUnsupported<String>>(),
      );
      final unsafe = _Client()
        ..result = const {'status': 'failure', 'code': 'raw-secret\nmarker'};
      expect(
        (await MethodChannelAttachmentExport(unsafe).chooseSource()
                as PlatformFailure<String>)
            .code,
        'platform-error',
      );
    },
  );

  testWidgets('shipping default Attach chooses a source and refreshes detail', (
    tester,
  ) async {
    final entries = _Entries();
    final client = _Client();
    final h = await _mount(tester, entries, client);
    try {
      await _attach(tester);
      expect(client.calls, hasLength(1));
      expect(client.calls.single, (
        'app.hidlins/attachment_export',
        'chooseSource',
        null,
      ));
      expect(entries.added.single, ('entry-1', '/chosen/report.bin'));
      expect(find.text('report.bin'), findsOneWidget);
      expect(entries.details['entry-2']!.attachments, isEmpty);
      expect(find.textContaining('/chosen'), findsNothing);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('source cancellation is silent and nonmutating', (tester) async {
    final entries = _Entries();
    final client = _Client()..result = const {'status': 'canceled'};
    final h = await _mount(tester, entries, client);
    try {
      await _attach(tester);
      expect(client.calls, hasLength(1));
      expect(entries.added, isEmpty);
      expect(find.text('Could not choose attachment'), findsNothing);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  for (final status in ['denied', 'unsupported', 'failure']) {
    testWidgets('source $status reports safely without mutating', (
      tester,
    ) async {
      final entries = _Entries();
      final client = _Client()
        ..result = {'status': status, 'code': 'raw-marker'};
      final h = await _mount(tester, entries, client);
      try {
        await _attach(tester);
        expect(client.calls, hasLength(1));
        expect(entries.added, isEmpty);
        expect(find.text('Could not choose attachment'), findsOneWidget);
        expect(find.textContaining('raw-marker'), findsNothing);
        expect(tester.takeException(), isNull);
      } finally {
        await _close(tester, h);
      }
    });
  }

  testWidgets('duplicate callbacks cannot open concurrent source pickers', (
    tester,
  ) async {
    final gate = Completer<Object?>();
    final client = _Client()..gate = gate;
    final entries = _Entries();
    final h = await _mount(tester, entries, client);
    try {
      final callback = _attachButton(tester).onPressed!;
      callback();
      callback();
      await tester.pump();
      expect(client.calls, hasLength(1));
      expect(_attachButton(tester).onPressed, isNull);
      gate.complete(const {'status': 'success', 'value': '/chosen/report.bin'});
      await tester.pumpAndSettle();
      expect(entries.added, [('entry-1', '/chosen/report.bin')]);
      expect(tester.takeException(), isNull);
    } finally {
      if (!gate.isCompleted) gate.complete(const {'status': 'canceled'});
      await tester.pump();
      await _close(tester, h);
    }
  });

  for (final boundary in ['sync', 'lock', 'selection']) {
    testWidgets('late source cannot attach after $boundary changes', (
      tester,
    ) async {
      final gate = Completer<Object?>();
      final client = _Client()..gate = gate;
      final entries = _Entries();
      final h = await _mount(tester, entries, client);
      try {
        _attachButton(tester).onPressed!();
        await tester.pump();
        expect(client.calls, hasLength(1));
        if (boundary == 'sync') {
          h.sync.syncController.add(
            const bridge.SyncEvent.done(bridge.SyncOutcomeDto.alreadyInSync()),
          );
          await tester.pump();
          await tester.pump();
        } else if (boundary == 'lock') {
          h.session.lockController.add(LockEvent.locked);
          await tester.pump();
          await tester.pump();
        } else {
          await tester.tap(find.text('unrelated').first);
          await tester.pumpAndSettle();
        }
        gate.complete(const {
          'status': 'success',
          'value': '/chosen/report.bin',
        });
        await tester.pumpAndSettle();
        expect(entries.added, isEmpty);
        expect(entries.details['entry-1']!.attachments, isEmpty);
        expect(entries.details['entry-2']!.attachments, isEmpty);
        if (boundary == 'lock') {
          expect(find.byType(UnlockScreen), findsOneWidget);
        }
        expect(tester.takeException(), isNull);
      } finally {
        if (!gate.isCompleted) gate.complete(const {'status': 'canceled'});
        await tester.pump();
        await _close(tester, h);
      }
    });
  }

  testWidgets('captured Attach cannot dispatch after sync starts', (
    tester,
  ) async {
    final entries = _Entries();
    final client = _Client();
    final h = await _mount(tester, entries, client);
    try {
      final callback = _attachButton(tester).onPressed!;
      h.sync.syncController.add(
        const bridge.SyncEvent.started(automatic: false),
      );
      await tester.pump();
      await tester.pump();
      callback();
      await tester.pump(const Duration(milliseconds: 400));
      await tester.pump();
      expect(client.calls, isEmpty);
      expect(entries.added, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('unknown sync status keeps Attach disabled', (tester) async {
    final gate = Completer<SyncStatusDto>();
    final pending = TestHarness()..sync.statusCompleter = gate;
    final client = _Client();
    final entries = _Entries();
    final h = await _mount(tester, entries, client, harness: pending);
    try {
      expect(_attachButton(tester).onPressed, isNull);
      expect(client.calls, isEmpty);
      expect(entries.added, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      gate.complete(const SyncStatusDto(configured: false, inFlight: false));
      await tester.pump();
      await _close(tester, h);
    }
  });

  testWidgets('native size-cap rejection keeps the designed safe error', (
    tester,
  ) async {
    final entries = _Entries()
      ..attachmentError = const InvalidInputFailure(
        field: 'attachment',
        reason: 'raw-marker',
      );
    final client = _Client();
    final h = await _mount(tester, entries, client);
    try {
      await _attach(tester);
      expect(client.calls, hasLength(1));
      expect(entries.details['entry-1']!.attachments, isEmpty);
      expect(find.text('File too large'), findsOneWidget);
      expect(find.textContaining('raw-marker'), findsNothing);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });

  testWidgets('explicit legacy picker callback remains supported', (
    tester,
  ) async {
    final h = TestHarness()..session.currentLockState = LockEvent.unlocked;
    h.entries.details['entry-1'] = _detail('entry-1', const []);
    try {
      await tester.pumpFeatureWithHarness(
        Scaffold(
          body: EntryDetailPane(
            uuid: 'entry-1',
            onPickFile: () async => '/chosen/legacy.bin',
          ),
        ),
        h,
      );
      await tester.pumpAndSettle();
      await _attach(tester);
      expect(h.entries.addAttachmentCalled, isTrue);
      expect(h.entries.lastAttachmentPath, '/chosen/legacy.bin');
      expect(h.attachmentExport.suggestedNames, isEmpty);
      expect(tester.takeException(), isNull);
    } finally {
      await _close(tester, h);
    }
  });
}

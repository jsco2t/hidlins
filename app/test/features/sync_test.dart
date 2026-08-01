import 'dart:async';
import 'dart:ui' show PointerDeviceKind;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/app.dart';
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/error.dart';
import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entry_detail.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/features/sync/sync_event_listener.dart';
import 'package:app/src/features/sync/sync_page.dart';
import 'package:app/src/features/sync/sync_status_button.dart';
import 'package:app/src/features/vaults/connect_sync_dialog.dart';
import 'package:app/src/ui/widgets/secret_text.dart';
import '../helpers/feature_test_helpers.dart';

void main() {
  group('SyncPage status', () {
    testWidgets('shows idle status when sync is configured', (tester) async {
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false);
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        const Scaffold(body: SyncPage()),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      expect(find.text('Sync idle'), findsOneWidget);
      final button = tester.widget<FilledButton>(
        find.byKey(const Key('sync-now-button')),
      );
      expect(button.onPressed, isNotNull);
    });

    testWidgets('shows syncing status and disables sync-now', (tester) async {
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: true);
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        const Scaffold(body: SyncPage()),
        testHarness: harness,
      );
      await tester.pump();

      expect(find.text('Syncing…'), findsOneWidget);
      final button = tester.widget<FilledButton>(
        find.byKey(const Key('sync-now-button')),
      );
      expect(button.onPressed, isNull);
      final secretField = tester.widget<TextFormField>(
        find.byKey(const Key('sync-secret-key-field')),
      );
      expect(secretField.enabled, isFalse);
      final saveButton = tester.widget<FilledButton>(
        find.byKey(const Key('save-sync-config')),
      );
      expect(saveButton.onPressed, isNull);
    });

    testWidgets('shows failure status after a failed event', (tester) async {
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false);
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        const Scaffold(body: SyncPage()),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      harness.sync.syncController.add(
        const SyncEvent.failed(HidlinsApiError.syncAuthFailed()),
      );
      await tester.pump();
      await tester.pump();

      expect(find.text('Sync failed'), findsOneWidget);
    });

    testWidgets('keeps an event that arrives while status is loading', (
      tester,
    ) async {
      final statusCompleter = Completer<SyncStatusDto>();
      final harness = _unlockedHarness()
        ..sync.statusCompleter = statusCompleter;
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        const Scaffold(body: SyncPage()),
        testHarness: harness,
      );
      await tester.pump();

      harness.sync.syncController.add(const SyncEvent.started());
      await tester.pump();
      statusCompleter.complete(
        const SyncStatusDto(configured: true, inFlight: false),
      );
      await tester.pump();
      await tester.pump();

      expect(find.text('Syncing…'), findsOneWidget);
    });

    testWidgets('reloads status between unlocked vault sessions', (
      tester,
    ) async {
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false);
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        const Scaffold(body: SyncPage()),
        testHarness: harness,
      );
      await tester.pumpAndSettle();
      expect(find.text('Sync idle'), findsOneWidget);

      harness.session.emitLockState(LockEvent.locked);
      await tester.pump();
      harness.sync.status = const SyncStatusDto(
        configured: false,
        inFlight: false,
      );
      harness.session.emitLockState(LockEvent.unlocked);
      await tester.pump();
      await tester.pump();
      expect(find.text('Sync is not configured'), findsOneWidget);

      harness.session.emitLockState(LockEvent.locked);
      await tester.pump();
      harness.sync.status = const SyncStatusDto(
        configured: true,
        inFlight: false,
      );
      harness.session.emitLockState(LockEvent.unlocked);
      await tester.pump();
      await tester.pump();
      expect(find.text('Sync idle'), findsOneWidget);
    });
  });

  group('SyncStatusButton', () {
    testWidgets('starts sync for a configured vault', (tester) async {
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false);
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        Scaffold(
          appBar: AppBar(actions: [SyncStatusButton(onNotConfigured: () {})]),
        ),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      expect(find.byIcon(Icons.cloud_done_outlined), findsOneWidget);
      expect(find.byTooltip('Sync now'), findsOneWidget);
      await tester.tap(find.byKey(const Key('sync-status-button')));
      await tester.pump();

      expect(harness.sync.syncNowCalled, isTrue);
    });

    testWidgets('stays disabled until an in-flight sync completes', (
      tester,
    ) async {
      final completer = Completer<void>();
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false)
        ..sync.syncNowCompleter = completer;
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        Scaffold(
          appBar: AppBar(actions: [SyncStatusButton(onNotConfigured: () {})]),
        ),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('sync-status-button')));
      await tester.pump();
      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('sync-status-button')))
            .onPressed,
        isNull,
      );
      expect(find.byType(CircularProgressIndicator), findsOneWidget);

      completer.complete();
      await tester.pump();
      await tester.pump();
      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('sync-status-button')))
            .onPressed,
        isNotNull,
      );
    });

    testWidgets('ignores a second trigger while sync is already running', (
      tester,
    ) async {
      final completer = Completer<void>();
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false)
        ..sync.syncNowCompleter = completer;
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        Scaffold(
          appBar: AppBar(actions: [SyncStatusButton(onNotConfigured: () {})]),
        ),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('sync-status-button')));
      await tester.tap(find.byKey(const Key('sync-status-button')));

      expect(harness.sync.syncNowCallCount, 1);
      completer.complete();
      await tester.pump();
      await tester.pump();
    });

    testWidgets('keeps in-flight state when Rust reports an active sync', (
      tester,
    ) async {
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false)
        ..sync.syncNowError = const VaultIsBusy();
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        Scaffold(
          appBar: AppBar(actions: [SyncStatusButton(onNotConfigured: () {})]),
        ),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('sync-status-button')));
      await tester.pump();
      await tester.pump();

      final button = tester.widget<IconButton>(
        find.byKey(const Key('sync-status-button')),
      );
      expect(button.onPressed, isNull);
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });

    testWidgets('shows a failure affordance', (tester) async {
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false);
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        Scaffold(
          appBar: AppBar(actions: [SyncStatusButton(onNotConfigured: () {})]),
        ),
        testHarness: harness,
      );
      await tester.pumpAndSettle();
      harness.sync.syncController.add(
        const SyncEvent.failed(HidlinsApiError.syncAuthFailed()),
      );
      await tester.pump();
      await tester.pump();

      expect(find.byIcon(Icons.sync_problem), findsOneWidget);
      expect(find.byTooltip('Sync failed'), findsOneWidget);
    });

    testWidgets('opens configuration when sync is not configured', (
      tester,
    ) async {
      var opened = false;
      final harness = _unlockedHarness();
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        Scaffold(
          appBar: AppBar(
            actions: [SyncStatusButton(onNotConfigured: () => opened = true)],
          ),
        ),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      expect(find.byIcon(Icons.cloud_off_outlined), findsOneWidget);
      expect(find.byTooltip('Sync is not configured'), findsOneWidget);
      await tester.tap(find.byKey(const Key('sync-status-button')));
      expect(opened, isTrue);
    });

    testWidgets('recovers from an unexpected repository exception', (
      tester,
    ) async {
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false)
        ..sync.syncNowError = StateError('bridge runtime failure');
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        Scaffold(
          appBar: AppBar(actions: [SyncStatusButton(onNotConfigured: () {})]),
        ),
        testHarness: harness,
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('sync-status-button')));
      await tester.pump();
      await tester.pump();

      final button = tester.widget<IconButton>(
        find.byKey(const Key('sync-status-button')),
      );
      expect(button.onPressed, isNotNull);
      expect(find.byIcon(Icons.sync_problem), findsOneWidget);
      expect(
        find.text('Sync failed. Your local vault is still available.'),
        findsOneWidget,
      );
    });
  });

  group('Sync outcome presentation', () {
    final cases = <(bridge.SyncOutcomeDto, String)>[
      (const bridge.SyncOutcomeDto.alreadyInSync(), 'Vault is already in sync'),
      (
        const bridge.SyncOutcomeDto.pushed(isFirstSeed: false),
        'Local changes uploaded',
      ),
      (const bridge.SyncOutcomeDto.fastReplaced(), 'Vault updated from remote'),
      (
        bridge.SyncOutcomeDto.merged(
          entriesAdded: BigInt.one,
          entriesModified: BigInt.one,
          entriesRemoved: BigInt.zero,
        ),
        'Local and remote changes merged',
      ),
      (const bridge.SyncOutcomeDto.unknown(), 'Sync completed'),
    ];

    for (final (outcome, message) in cases) {
      testWidgets('shows snackbar for $message', (tester) async {
        final harness = TestHarness();
        addTearDown(harness.dispose);
        await tester.pumpFeature(
          const SyncEventListener(child: Scaffold(body: SizedBox())),
          testHarness: harness,
        );
        await tester.pump();

        harness.sync.syncController.add(SyncEvent.done(outcome));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 250));

        expect(find.text(message), findsOneWidget);
      });
    }

    testWidgets('conflict dialog preserves both sides and shows backup path', (
      tester,
    ) async {
      final harness = TestHarness();
      addTearDown(harness.dispose);
      await tester.pumpFeature(
        const SyncEventListener(child: Scaffold(body: SizedBox())),
        testHarness: harness,
      );
      await tester.pump();

      harness.sync.syncController.add(
        const SyncEvent.failed(
          HidlinsApiError.syncConflictUnresolvable(
            backupPath: '/vaults/main.kdbx.bak',
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Sync needs attention'), findsOneWidget);
      expect(find.textContaining('Both sides were preserved'), findsOneWidget);
      expect(find.text('/vaults/main.kdbx.bak'), findsOneWidget);

      await tester.tapAt(const Offset(4, 4));
      await tester.pump();
      expect(find.text('Sync needs attention'), findsOneWidget);

      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
      expect(find.text('Sync needs attention'), findsNothing);
    });

    testWidgets('manual conflict shows one dialog and no generic snackbar', (
      tester,
    ) async {
      const error = HidlinsApiError.syncConflictUnresolvable(
        backupPath: '/vaults/manual.kdbx.bak',
      );
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false)
        ..sync.syncNowEvent = const SyncEvent.failed(error)
        ..sync.syncNowError = const SyncConflict('/vaults/manual.kdbx.bak');
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        SyncEventListener(
          child: Scaffold(
            appBar: AppBar(actions: [SyncStatusButton(onNotConfigured: () {})]),
          ),
        ),
        testHarness: harness,
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('sync-status-button')));
      await tester.pumpAndSettle();

      expect(find.text('Sync needs attention'), findsOneWidget);
      expect(
        find.text('Sync failed. Your local vault is still available.'),
        findsNothing,
      );
      expect(find.byType(AlertDialog), findsOneWidget);

      await tester.tap(find.text('Close'));
      await tester.pumpAndSettle();
    });

    testWidgets('lock-mid-sync shows at most one failure message', (
      tester,
    ) async {
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false)
        ..sync.syncNowEvent = const SyncEvent.failed(
          HidlinsApiError.vaultLocked(),
        )
        ..sync.syncNowError = const VaultIsLocked();
      addTearDown(harness.dispose);

      await tester.pumpFeature(
        SyncEventListener(
          child: Scaffold(
            appBar: AppBar(actions: [SyncStatusButton(onNotConfigured: () {})]),
          ),
        ),
        testHarness: harness,
      );
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('sync-status-button')));
      await tester.pumpAndSettle();

      expect(
        find.text('Sync failed. Your local vault is still available.'),
        findsOneWidget,
      );
    });
  });

  group('Sync configuration', () {
    testWidgets('saves configuration and clears the secret field', (
      tester,
    ) async {
      final syncCompleter = Completer<void>();
      final harness = _unlockedHarness()..sync.syncNowCompleter = syncCompleter;
      addTearDown(harness.dispose);
      await tester.pumpFeature(
        const Scaffold(body: SyncPage()),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      final secretBeforeSave = tester.widget<EditableText>(
        find.descendant(
          of: find.byKey(const Key('sync-secret-key-field')),
          matching: find.byType(EditableText),
        ),
      );
      expect(secretBeforeSave.obscureText, isTrue);
      expect(secretBeforeSave.autocorrect, isFalse);
      expect(secretBeforeSave.enableSuggestions, isFalse);
      await _fillSyncConfiguration(tester, secret: 'top-secret-key');
      await tester.ensureVisible(find.byKey(const Key('save-sync-config')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('save-sync-config')));
      await tester.pump();
      await tester.pump();

      expect(harness.sync.configureCalled, isTrue);
      expect(harness.sync.syncNowCalled, isTrue);
      expect(harness.sync.lastConfig?.bucket, 'sync-bucket');
      expect(harness.sync.lastConfig?.key, 'vault.kdbx');
      expect(harness.sync.lastConfig?.region, 'us-east-1');
      expect(harness.sync.lastConfig?.endpoint, 'http://minio.settings:9000');
      expect(harness.sync.lastConfig?.pathStyle, isTrue);
      expect(harness.sync.lastConfig?.accessKeyId, 'AKID');
      expect(harness.sync.lastConfig?.secretAccessKey, 'top-secret-key');
      final secretField = tester.widget<TextFormField>(
        find.byKey(const Key('sync-secret-key-field')),
      );
      expect(secretField.controller?.text, isEmpty);
      expect(find.text('Sync configuration saved'), findsOneWidget);
      expect(
        tester
            .widget<FilledButton>(find.byKey(const Key('save-sync-config')))
            .onPressed,
        isNull,
      );
      await tester.drag(find.byType(ListView), const Offset(0, 1000));
      await tester.pump();
      expect(find.text('Syncing…'), findsOneWidget);

      syncCompleter.complete();
      await tester.pump();
      await tester.pump();
      expect(find.text('Sync idle'), findsOneWidget);
    });

    testWidgets('surfaces duplicate-target errors without echoing the secret', (
      tester,
    ) async {
      final harness = _unlockedHarness()
        ..sync.configureError = const SyncDuplicate('work-vault');
      addTearDown(harness.dispose);
      await tester.pumpFeature(
        const Scaffold(body: SyncPage()),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      await _fillSyncConfiguration(tester, secret: 'never-echo-this');
      await tester.ensureVisible(find.byKey(const Key('save-sync-config')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('save-sync-config')));
      await tester.pumpAndSettle();

      expect(find.textContaining('work-vault'), findsOneWidget);
      expect(find.textContaining('never-echo-this'), findsNothing);
      expect(harness.sync.syncNowCalled, isFalse);
      final secretField = tester.widget<TextFormField>(
        find.byKey(const Key('sync-secret-key-field')),
      );
      expect(secretField.controller?.text, isEmpty);
    });

    testWidgets('clears secret before a delayed save finishes', (tester) async {
      final configureCompleter = Completer<void>();
      final harness = _unlockedHarness()
        ..sync.configureCompleter = configureCompleter;
      addTearDown(harness.dispose);
      await tester.pumpFeature(
        const Scaffold(body: SyncPage()),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      await _fillSyncConfiguration(tester, secret: 'short-lived-secret');
      await tester.ensureVisible(find.byKey(const Key('save-sync-config')));
      await tester.tap(find.byKey(const Key('save-sync-config')));
      await tester.pump();

      final secretField = tester.widget<TextFormField>(
        find.byKey(const Key('sync-secret-key-field')),
      );
      expect(secretField.controller?.text, isEmpty);

      unawaited(
        Navigator.of(
          tester.element(find.byType(SyncPage)),
        ).pushReplacement(MaterialPageRoute<void>(builder: (_) => Container())),
      );
      await tester.pumpAndSettle();
      configureCompleter.complete();
      await tester.pump();
      await tester.pump();

      expect(harness.sync.syncNowCalled, isTrue);
      expect(tester.takeException(), isNull);
    });

    testWidgets('maps default endpoint controls to null and false', (
      tester,
    ) async {
      final harness = _unlockedHarness();
      addTearDown(harness.dispose);
      await tester.pumpFeature(
        const Scaffold(body: SyncPage()),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      await _fillSyncConfiguration(
        tester,
        secret: 'aws-secret',
        customEndpoint: false,
      );
      await tester.drag(find.byType(ListView), const Offset(0, -500));
      await tester.pump();
      await tester.tap(find.byKey(const Key('save-sync-config')));
      await tester.pumpAndSettle();

      expect(harness.sync.lastConfig?.endpoint, isNull);
      expect(harness.sync.lastConfig?.pathStyle, isFalse);
    });
  });

  group('Sync-in-flight workspace', () {
    testWidgets('keeps entry edit disabled until sync status is known', (
      tester,
    ) async {
      final statusCompleter = Completer<SyncStatusDto>();
      final harness = _unlockedHarness()
        ..sync.statusCompleter = statusCompleter;
      addTearDown(harness.dispose);
      await tester.pumpFeature(
        const Scaffold(body: EntryDetailPane(uuid: 'entry-1')),
        testHarness: harness,
      );
      await tester.pump();
      await tester.pump();

      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('entry-edit-action')))
            .onPressed,
        isNull,
      );

      statusCompleter.complete(
        const SyncStatusDto(configured: false, inFlight: false),
      );
      await tester.pump();
      await tester.pump();
      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('entry-edit-action')))
            .onPressed,
        isNotNull,
      );
    });

    testWidgets('keeps cached entries visible and disables editing', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(const Size(1200, 1800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final harness = _unlockedHarness()
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false);
      addTearDown(harness.dispose);
      await tester.pumpFeature(
        const Scaffold(body: EntriesPage()),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      await tester.tap(find.text('GitHub'));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('entry-edit-action')), findsOneWidget);

      harness.sync.syncController.add(const SyncEvent.started());
      await tester.pump();
      await tester.pump();

      expect(find.byKey(const Key('sync-busy-banner')), findsOneWidget);
      expect(find.text('GitHub'), findsWidgets);
      final editButton = tester.widget<IconButton>(
        find.byKey(const Key('entry-edit-action')),
      );
      expect(editButton.onPressed, isNull);
      expect(
        tester
            .widget<TextButton>(find.byKey(const Key('attachment-add-action')))
            .onPressed,
        isNull,
      );
      expect(
        tester
            .widget<IconButton>(
              find.byKey(const Key('attachment-detach-backup-codes.txt')),
            )
            .onPressed,
        isNull,
      );
      expect(
        find.byTooltip('Changes are disabled until sync finishes'),
        findsWidgets,
      );

      harness.sync.syncController.add(
        const SyncEvent.done(bridge.SyncOutcomeDto.alreadyInSync()),
      );
      await tester.pump();
      await tester.pump();

      expect(find.byKey(const Key('sync-busy-banner')), findsNothing);
      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('entry-edit-action')))
            .onPressed,
        isNotNull,
      );
      expect(
        tester
            .widget<TextButton>(find.byKey(const Key('attachment-add-action')))
            .onPressed,
        isNotNull,
      );
      expect(
        tester
            .widget<IconButton>(
              find.byKey(const Key('attachment-detach-backup-codes.txt')),
            )
            .onPressed,
        isNotNull,
      );
    });

    testWidgets('refreshes cached entry detail after sync completes', (
      tester,
    ) async {
      final refreshCompleter = Completer<EntryDetail>();
      final refreshedDetail = EntryDetail(
        uuid: 'entry-1',
        title: 'GitHub from remote',
        username: 'octocat',
        hasPassword: true,
        url: 'https://github.com',
        notes: '',
        kind: EntryKindDto.credential,
        tags: const [],
        customFields: const [],
        attachments: const [],
      );
      final harness = _unlockedHarness()
        ..entries.details['entry-1'] = refreshedDetail
        ..entries.entryDetailRefreshCompleter = refreshCompleter;
      addTearDown(harness.dispose);
      await tester.pumpFeature(
        const Scaffold(body: EntryDetailPane(uuid: 'entry-1')),
        testHarness: harness,
      );
      await tester.pumpAndSettle();
      expect(harness.entries.entryDetailCalls, 1);

      harness.sync.syncController.add(
        const SyncEvent.done(bridge.SyncOutcomeDto.fastReplaced()),
      );
      await tester.pump();
      await tester.pump();

      expect(harness.entries.entryDetailCalls, 2);
      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('entry-edit-action')))
            .onPressed,
        isNull,
      );
      expect(find.byTooltip('Refreshing entry after sync'), findsOneWidget);

      refreshCompleter.complete(refreshedDetail);
      await tester.pump();
      await tester.pump();

      expect(
        tester
            .widget<IconButton>(find.byKey(const Key('entry-edit-action')))
            .onPressed,
        isNotNull,
      );
    });

    testWidgets('late sync completion never restores workspace after lock', (
      tester,
    ) async {
      final harness = TestHarness()
        ..session.currentLockState = LockEvent.unlocked
        ..sync.status = const SyncStatusDto(configured: true, inFlight: false);
      addTearDown(harness.dispose);

      await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
      await tester.pumpAndSettle();
      expect(find.text('Entries'), findsWidgets);

      harness.sync.syncController.add(const SyncEvent.started());
      await tester.pump();
      harness.session.emitLockState(LockEvent.locked);
      await tester.pumpAndSettle();
      expect(find.text('Vault locked'), findsOneWidget);

      harness.sync.syncController.add(
        const SyncEvent.done(bridge.SyncOutcomeDto.alreadyInSync()),
      );
      await tester.pumpAndSettle();

      expect(find.text('Vault locked'), findsOneWidget);
      expect(find.byKey(const Key('sync-busy-banner')), findsNothing);
    });
  });

  group('Unlock and bootstrap sync flows', () {
    testWidgets('unlock renders the FR-041 syncing stage', (tester) async {
      final harness = TestHarness();
      final completer = Completer<VaultTree>();
      harness.session.unlockCompleter = completer;
      harness.session.vaults = const [
        VaultSummary(
          name: 'main',
          path: '/vaults/main.kdbx',
          hasKeyfile: false,
          hasSync: true,
        ),
      ];
      addTearDown(harness.dispose);

      await tester.pumpFeature(const UnlockScreen(), testHarness: harness);
      await tester.pumpAndSettle();

      await tester.enterText(
        find.widgetWithText(TextField, 'Master password'),
        'correct-password',
      );
      await tester.tap(find.text('Unlock'));
      await tester.pump();

      harness.sync.syncController.add(const SyncEvent.started());
      await tester.pump();
      await tester.pump();
      expect(find.text('Syncing…'), findsOneWidget);

      completer.complete(harness.entries.tree);
      await tester.pump();
      expect(harness.session.unlockCalled, isTrue);
    });

    testWidgets('keeps the offline warning visible after unlock completes', (
      tester,
    ) async {
      final harness = TestHarness();
      final completer = Completer<VaultTree>();
      harness.session.unlockCompleter = completer;
      harness.session.vaults = const [
        VaultSummary(
          name: 'main',
          path: '/vaults/main.kdbx',
          hasKeyfile: false,
          hasSync: true,
        ),
      ];
      addTearDown(harness.dispose);

      await tester.pumpWidget(HidlinsApp(overrides: harness.overrides));
      await tester.pumpAndSettle();
      await tester.enterText(
        find.widgetWithText(TextField, 'Master password'),
        'correct-password',
      );
      await tester.tap(find.text('Unlock'));
      await tester.pump();
      harness.sync.syncController.add(
        const SyncEvent.failed(
          HidlinsApiError.syncRemoteUnreachable(endpoint: 'offline'),
        ),
      );
      await tester.pump();
      completer.complete(harness.entries.tree);
      await tester.pumpAndSettle();

      expect(find.text('Entries'), findsWidgets);
      expect(find.text('Opened offline — sync failed'), findsOneWidget);
    });

    testWidgets('clears offline warning when unlock itself fails', (
      tester,
    ) async {
      final harness = TestHarness();
      final completer = Completer<VaultTree>();
      harness.session.unlockCompleter = completer;
      harness.session.vaults = const [
        VaultSummary(
          name: 'main',
          path: '/vaults/main.kdbx',
          hasKeyfile: false,
          hasSync: true,
        ),
      ];
      addTearDown(harness.dispose);

      await tester.pumpFeature(const UnlockScreen(), testHarness: harness);
      await tester.pumpAndSettle();
      await tester.enterText(
        find.widgetWithText(TextField, 'Master password'),
        'correct-password',
      );
      await tester.tap(find.text('Unlock'));
      await tester.pump();
      harness.sync.syncController.add(
        const SyncEvent.failed(HidlinsApiError.vaultLocked()),
      );
      await tester.pump();
      completer.completeError(const VaultIsLocked());
      await tester.pump();
      await tester.pump();

      expect(find.text('Opened offline — sync failed'), findsNothing);
    });

    testWidgets('connect form passes every S3 field to bootstrap', (
      tester,
    ) async {
      final harness = TestHarness();
      addTearDown(harness.dispose);
      await tester.pumpFeature(const ConnectSyncDialog(), testHarness: harness);
      await tester.pumpAndSettle();

      final secretField = tester.widget<EditableText>(
        find.descendant(
          of: find.byKey(const Key('connect-sync-secret-key-field')),
          matching: find.byType(EditableText),
        ),
      );
      expect(secretField.obscureText, isTrue);
      expect(secretField.autocorrect, isFalse);
      expect(secretField.enableSuggestions, isFalse);
      await _fillConnectForm(tester);
      await tester.ensureVisible(find.text('Confirm'));
      await tester.pump();
      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      expect(harness.session.bootstrapCalled, isTrue);
      expect(harness.session.lastBootstrapName, 'bootstrap-vault');
      expect(harness.session.lastBootstrapConfig?.bucket, 'remote-bucket');
      expect(harness.session.lastBootstrapConfig?.key, 'vaults/main.kdbx');
      expect(harness.session.lastBootstrapConfig?.region, 'us-west-2');
      expect(
        harness.session.lastBootstrapConfig?.endpoint,
        'http://minio.test:9000',
      );
      expect(harness.session.lastBootstrapConfig?.pathStyle, isTrue);
      expect(
        harness.session.lastBootstrapConfig?.accessKeyId,
        'BOOTSTRAP-AKID',
      );
      expect(
        harness.session.lastBootstrapConfig?.secretAccessKey,
        'bootstrap-secret',
      );
      expect(harness.session.lastBootstrapPassword, 'master-password');
    });

    testWidgets('connect form clears secret and password controllers eagerly', (
      tester,
    ) async {
      final harness = TestHarness();
      harness.session.bootstrapError = const BadCredentials();
      addTearDown(harness.dispose);
      await tester.pumpFeature(const ConnectSyncDialog(), testHarness: harness);
      await tester.pumpAndSettle();

      await _fillConnectForm(tester);
      await tester.ensureVisible(find.text('Confirm'));
      await tester.pump();
      await tester.tap(find.text('Confirm'));
      await tester.pumpAndSettle();

      // Controllers should be cleared even though bootstrap failed
      final secretField = tester.widget<TextFormField>(
        find.byKey(const Key('connect-sync-secret-key-field')),
      );
      expect(secretField.controller?.text, isEmpty);
      final passwordField = tester.widget<TextFormField>(
        find.widgetWithText(TextFormField, 'Master password'),
      );
      expect(passwordField.controller?.text, isEmpty);
    });
  });

  group('Copy and reveal error handling', () {
    testWidgets('entries page shows error snackbar when copy fails', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(const Size(1200, 1800));
      addTearDown(() => tester.binding.setSurfaceSize(null));
      final harness = _unlockedHarness()
        ..secrets.copyError = Exception('clipboard unavailable');
      addTearDown(harness.dispose);
      await tester.pumpFeature(
        const Scaffold(body: EntriesPage()),
        testHarness: harness,
      );
      await tester.pumpAndSettle();

      // Hover over the first entry to reveal copy buttons
      final gesture = await tester.createGesture(kind: PointerDeviceKind.mouse);
      await gesture.addPointer(location: Offset.zero);
      addTearDown(gesture.removePointer);
      await gesture.moveTo(tester.getCenter(find.text('GitHub')));
      await tester.pumpAndSettle();

      // Tap the password copy button on the entry row
      await tester.tap(find.byTooltip('Password').first);
      await tester.pump();
      await tester.pump();

      expect(find.text('An error occurred'), findsOneWidget);
    });

    testWidgets(
      'entry detail shows error snackbar when custom field reveal fails',
      (tester) async {
        await tester.binding.setSurfaceSize(const Size(1200, 1800));
        addTearDown(() => tester.binding.setSurfaceSize(null));
        final harness = _unlockedHarness();
        addTearDown(harness.dispose);
        await tester.pumpFeature(
          const Scaffold(body: EntryDetailPane(uuid: 'entry-1')),
          testHarness: harness,
        );
        await tester.pumpAndSettle();

        // Set reveal error after initial build so the non-protected
        // 'recovery-email' FutureBuilder resolves successfully first.
        harness.secrets.revealError = Exception('reveal failed');

        // Trigger reveal on the protected 'pin' custom field.
        // SecretText at(0) is the password field; at(1) is the 'pin' field.
        final state = tester.state<SecretTextState>(
          find.byType(SecretText).at(1),
        );
        await state.reveal();
        await tester.pump();

        expect(find.text('Could not reveal field'), findsOneWidget);
      },
    );
  });
}

Future<void> _fillSyncConfiguration(
  WidgetTester tester, {
  required String secret,
  bool customEndpoint = true,
}) async {
  await tester.enterText(
    find.widgetWithText(TextFormField, 'S3 Bucket'),
    'sync-bucket',
  );
  await tester.enterText(
    find.widgetWithText(TextFormField, 'Object key'),
    'vault.kdbx',
  );
  await tester.enterText(
    find.widgetWithText(TextFormField, 'Region'),
    'us-east-1',
  );
  if (customEndpoint) {
    await tester.enterText(
      find.widgetWithText(TextFormField, 'Endpoint (optional)'),
      'http://minio.settings:9000',
    );
    await tester.ensureVisible(find.text('Path-style addressing'));
    await tester.pump();
    await tester.tap(find.text('Path-style addressing'));
  }
  await tester.enterText(
    find.widgetWithText(TextFormField, 'Access key ID'),
    'AKID',
  );
  await tester.enterText(
    find.byKey(const Key('sync-secret-key-field')),
    secret,
  );
}

Future<void> _fillConnectForm(WidgetTester tester) async {
  final values = <String, String>{
    'Vault name': 'bootstrap-vault',
    'S3 Bucket': 'remote-bucket',
    'Object key': 'vaults/main.kdbx',
    'Region': 'us-west-2',
    'Endpoint (optional)': 'http://minio.test:9000',
    'Access key ID': 'BOOTSTRAP-AKID',
    'Secret access key': 'bootstrap-secret',
    'Master password': 'master-password',
  };
  for (final MapEntry(key: label, value: value) in values.entries) {
    await tester.enterText(find.widgetWithText(TextFormField, label), value);
  }
  await tester.ensureVisible(find.text('Path-style addressing'));
  await tester.pump();
  await tester.tap(find.text('Path-style addressing'));
}

TestHarness _unlockedHarness() {
  return TestHarness()..session.currentLockState = LockEvent.unlocked;
}

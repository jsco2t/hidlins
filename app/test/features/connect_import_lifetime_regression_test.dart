import 'dart:async';

import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/features/vaults/connect_sync_dialog.dart';
import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';
import 'package:app/src/providers/providers.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import '../fakes/fake_repositories.dart';

class ImportSync extends FakeSyncRepository {
  Completer<LocalDiscoveryStatus>? discoveryGate;
  Completer<void>? publicationGate;
  Completer<void>? setupGate;
  Completer<PairingPrompt>? importGate;
  Completer<VaultSummary?>? confirmationGate;
  Completer<void>? rejectionGate;
  Completer<void>? cancellationGate;
  Completer<void>? stopGate;
  final effects = <String>[];
  final rejected = <String>[];
  final accepted = <String>[];
  String? importedName;
  String? peerName;
  String? route;
  int imports = 0;
  int discoveries = 0;
  int cancellations = 0;
  int stops = 0;

  @override
  Future<LocalDiscoveryStatus> discover(DiscoveryKind kind) async {
    discoveries++;
    effects.add('discover');
    final result = discoveryGate == null
        ? discovery
        : await discoveryGate!.future;
    await publicationGate?.future;
    route = 'discovery';
    effects.add('publish');
    return result;
  }

  @override
  Future<void> setManualEndpoint(LocalEndpoint endpoint) async {
    effects.add('manual');
    await setupGate?.future;
    route = endpoint.address;
    await super.setManualEndpoint(endpoint);
  }

  @override
  Future<PairingPrompt> beginPairImport({
    required String name,
    required String masterPassword,
    KeyfileRef? keyfile,
  }) async {
    imports++;
    importedName = name;
    lastPairImportPassword = masterPassword;
    effects.add('import');
    if (importGate != null) return importGate!.future;
    return super.beginPairImport(
      name: name,
      masterPassword: masterPassword,
      keyfile: keyfile,
    );
  }

  @override
  Future<VaultSummary?> confirmPairing({
    required String transactionHandle,
    required bool accepted,
    required String peerDisplayName,
  }) async {
    if (!accepted) {
      rejected.add(transactionHandle);
      await rejectionGate?.future;
      effects.add('reject');
      return null;
    }
    this.accepted.add(transactionHandle);
    peerName = peerDisplayName;
    effects.add('confirm');
    if (confirmationGate != null) return confirmationGate!.future;
    return super.confirmPairing(
      transactionHandle: transactionHandle,
      accepted: accepted,
      peerDisplayName: peerDisplayName,
    );
  }

  @override
  Future<void> cancelSync() async {
    cancellations++;
    await cancellationGate?.future;
    effects.add('cancel');
  }

  @override
  Future<void> stopDiscovery() async {
    stops++;
    await stopGate?.future;
    effects.add('stop');
  }

  void finishPending() {
    if (discoveryGate case final gate? when !gate.isCompleted) {
      gate.complete(discovery);
    }
    if (importGate case final gate? when !gate.isCompleted) {
      gate.complete(pairingPrompt);
    }
    if (confirmationGate case final gate? when !gate.isCompleted) {
      gate.complete(null);
    }
    for (final gate in [
      publicationGate,
      setupGate,
      rejectionGate,
      cancellationGate,
      stopGate,
    ]) {
      if (gate != null && !gate.isCompleted) gate.complete();
    }
  }
}

class ImportFixture {
  ImportFixture({bool unlocked = false}) {
    session.currentLockState = unlocked ? LockEvent.unlocked : LockEvent.locked;
    sync.status = const SyncStatusDto(configured: true, inFlight: false);
    container = ProviderContainer(
      overrides: [
        sessionRepositoryProvider.overrideWithValue(session),
        syncRepositoryProvider.overrideWithValue(sync),
      ],
    );
  }
  final sync = ImportSync();
  final session = FakeSessionRepository();
  late final ProviderContainer container;
  final navigator = GlobalKey<NavigatorState>();
  final operations = <Future<Object?>>[];
  late MaterialPageRoute<bool> route;

  Future<void> mount(WidgetTester tester, {bool mobile = false}) async {
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          navigatorKey: navigator,
          localizationsDelegates: hidlinsLocalizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: const Scaffold(body: Text('home-marker')),
        ),
      ),
    );
    await open(tester, mobile: mobile);
  }

  Future<void> open(WidgetTester tester, {bool mobile = false}) async {
    route = MaterialPageRoute<bool>(
      builder: (_) => ConnectSyncDialog(mobileOverride: mobile),
    );
    unawaited(navigator.currentState!.push(route));
    await advanceImport(tester);
    await tester.enterText(field('Vault name'), 'original-vault');
    await tester.enterText(field('Master password'), 'import-fixture-marker');
  }

  Finder field(String label) => find.widgetWithText(TextFormField, label);

  TextEditingController controller(WidgetTester tester, String label) =>
      tester.widget<TextFormField>(field(label)).controller!;

  Future<Object?> submit(WidgetTester tester) {
    final button = tester.widget<FilledButton>(
      find.widgetWithText(FilledButton, 'Import paired vault'),
    );
    final result = Function.apply(button.onPressed!, const []) as Future<void>;
    final observed = result.then<Object?>(
      (_) => null,
      onError: (Object error, StackTrace stack) => error,
    );
    operations.add(observed);
    return observed;
  }

  Future<void> remove(WidgetTester tester) async {
    navigator.currentState!.removeRoute(route);
    await tester.pump();
    await tester.pump();
  }

  Future<void> close(WidgetTester tester) async {
    await tester.pumpWidget(const SizedBox.shrink());
    sync.finishPending();
    await tester.pump();
    await tester.pump();
    container.dispose();
    sync.dispose();
    session.dispose();
    await tester.pump();
  }
}

Future<void> advanceImport(WidgetTester tester) async {
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 350));
  await tester.pump();
}

void main() {
  testWidgets('password intake clears before discovery and import handoff', (
    tester,
  ) async {
    final f = ImportFixture();
    try {
      final gate = f.sync.discoveryGate = Completer<LocalDiscoveryStatus>();
      await f.mount(tester);
      final password = f.controller(tester, 'Master password');
      final operation = f.submit(tester);
      await tester.pump();
      expect(password.text, isEmpty);
      gate.complete(f.sync.discovery);
      await advanceImport(tester);
      expect(f.sync.lastPairImportPassword, 'import-fixture-marker');
      await tester.tap(find.text('Reject'));
      await advanceImport(tester);
      expect(await operation, isNull);
    } finally {
      await f.close(tester);
    }
  });

  testWidgets('submitted metadata is stable across discovery and SAS waits', (
    tester,
  ) async {
    final f = ImportFixture();
    try {
      final gate = f.sync.discoveryGate = Completer<LocalDiscoveryStatus>();
      await f.mount(tester);
      final name = f.controller(tester, 'Vault name');
      final peer = f.controller(tester, 'Device name');
      final operation = f.submit(tester);
      await tester.pump();
      name.text = 'edited-late';
      peer.text = 'edited-peer';
      gate.complete(f.sync.discovery);
      await advanceImport(tester);
      await tester.tap(find.text('Codes match'));
      await advanceImport(tester);
      expect(await operation, isNull);
      expect(f.sync.importedName, 'original-vault');
      expect(f.sync.peerName, 'This device');
      expect(find.text('home-marker'), findsOneWidget);
    } finally {
      await f.close(tester);
    }
  });

  for (final manual in [false, true]) {
    testWidgets(
      'dispose during ${manual ? 'manual setup' : 'discovery'} revokes import',
      (tester) async {
        final f = ImportFixture();
        try {
          if (manual) {
            f.sync.setupGate = Completer<void>();
          } else {
            f.sync.discoveryGate = Completer<LocalDiscoveryStatus>();
          }
          await f.mount(tester);
          if (manual) {
            await tester.tap(find.text('Use an IP address manually'));
            await advanceImport(tester);
            final fields = find.byType(TextFormField);
            await tester.enterText(fields.last, '192.168.1.23:42873');
          }
          final operation = f.submit(tester);
          await tester.pump();
          await f.remove(tester);
          f.sync.finishPending();
          await tester.pump();
          expect(await operation, isNull);
          expect(f.sync.imports, 0);
          expect(f.sync.cancellations, 1);
          expect(f.sync.stops, 1);
          expect(tester.takeException(), isNull);
        } finally {
          await f.close(tester);
        }
      },
    );
  }

  testWidgets('late import handle is rejected exactly once after disposal', (
    tester,
  ) async {
    final f = ImportFixture();
    try {
      final gate = f.sync.importGate = Completer<PairingPrompt>();
      await f.mount(tester);
      final operation = f.submit(tester);
      await tester.pump();
      expect(f.sync.imports, 1);
      await f.remove(tester);
      gate.complete(
        const PairingPrompt(transactionHandle: 'late-import', sas: 'ABC-DEF'),
      );
      await tester.pump();
      expect(await operation, isNull);
      expect(f.sync.rejected, ['late-import']);
      expect(find.byKey(const ValueKey('pair-import-sas')), findsNothing);
    } finally {
      await f.close(tester);
    }
  });

  testWidgets('permission decline clears intake without starting native work', (
    tester,
  ) async {
    final f = ImportFixture();
    try {
      await f.mount(tester, mobile: true);
      final password = f.controller(tester, 'Master password');
      final operation = f.submit(tester);
      await advanceImport(tester);
      expect(password.text, isEmpty);
      await tester.tap(find.text('Cancel'));
      await advanceImport(tester);
      expect(await operation, isNull);
      expect(f.sync.effects, isEmpty);
    } finally {
      await f.close(tester);
    }
  });

  testWidgets('duplicate submit is rejected while permission is pending', (
    tester,
  ) async {
    final f = ImportFixture();
    try {
      await f.mount(tester, mobile: true);
      final callback = tester
          .widget<FilledButton>(
            find.widgetWithText(FilledButton, 'Import paired vault'),
          )
          .onPressed!;
      final first = f.submit(tester);
      final second = (Function.apply(callback, const []) as Future<void>)
          .then<Object?>(
            (_) => null,
            onError: (Object error, StackTrace stack) => error,
          );
      f.operations.add(second);
      await advanceImport(tester);
      expect(find.text('Allow local network access'), findsOneWidget);
      await tester.tap(find.text('Cancel').last);
      await advanceImport(tester);
      // A second modal is itself the defect; close it so the fixture always settles.
      final duplicate = find
          .text('Allow local network access')
          .evaluate()
          .isNotEmpty;
      if (duplicate) {
        await tester.tap(find.text('Cancel').last);
        await advanceImport(tester);
      }
      expect(await first, isNull);
      expect(await second, isNull);
      expect(duplicate, isFalse);
    } finally {
      await f.close(tester);
    }
  });

  for (final permission in [true, false]) {
    testWidgets(
      'removing parent dismisses its owned ${permission ? 'permission' : 'SAS'} modal',
      (tester) async {
        final f = ImportFixture();
        try {
          await f.mount(tester, mobile: permission);
          final operation = f.submit(tester);
          await advanceImport(tester);
          await f.remove(tester);
          await advanceImport(tester);
          final stale = permission
              ? find.text('Allow local network access')
              : find.byKey(const ValueKey('pair-import-sas'));
          expect(stale, findsNothing);
          expect(await operation, isNull);
          expect(f.sync.imports, permission ? 0 : 1);
          expect(f.sync.rejected, permission ? isEmpty : ['pairing-handle']);
          expect(find.text('home-marker'), findsOneWidget);
        } finally {
          await f.close(tester);
        }
      },
    );
  }

  testWidgets(
    'disposing accepted confirmation cancels without stale navigation',
    (tester) async {
      final f = ImportFixture();
      try {
        final gate = f.sync.confirmationGate = Completer<VaultSummary?>();
        await f.mount(tester);
        final operation = f.submit(tester);
        await advanceImport(tester);
        await tester.tap(find.text('Codes match'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 300));
        expect(f.sync.accepted, ['pairing-handle']);
        await f.remove(tester);
        gate.complete(
          const VaultSummary(
            name: 'original-vault',
            path: '/fixture.kdbx',
            hasKeyfile: false,
            hasSync: true,
          ),
        );
        await advanceImport(tester);
        expect(await operation, isNull);
        expect(f.sync.cancellations, 1);
        expect(f.sync.rejected, isEmpty);
        expect(find.text('home-marker'), findsOneWidget);
      } finally {
        await f.close(tester);
      }
    },
  );

  testWidgets('locked successful import and rejected SAS remain usable', (
    tester,
  ) async {
    final f = ImportFixture();
    try {
      await f.mount(tester);
      final rejected = f.submit(tester);
      await advanceImport(tester);
      await tester.tap(find.text('Reject'));
      await advanceImport(tester);
      expect(await rejected, isNull);
      expect(f.sync.rejected, ['pairing-handle']);
      await tester.enterText(
        f.field('Master password'),
        'second-fixture-marker',
      );
      final accepted = f.submit(tester);
      await advanceImport(tester);
      await tester.tap(find.text('Codes match'));
      await advanceImport(tester);
      expect(await accepted, isNull);
      expect(f.sync.lastPairImportPassword, 'second-fixture-marker');
      expect(find.text('home-marker'), findsOneWidget);
    } finally {
      await f.close(tester);
    }
  });

  testWidgets('live typed import error remains localized and retryable', (
    tester,
  ) async {
    final f = ImportFixture();
    try {
      f.sync.beginPairImportError = const BadCredentials();
      await f.mount(tester);
      final operation = f.submit(tester);
      await advanceImport(tester);
      expect(await operation, isNull);
      expect(find.byKey(const ValueKey('pair-import-error')), findsOneWidget);
      expect(f.controller(tester, 'Master password').text, isEmpty);
      expect(
        tester
            .widget<FilledButton>(
              find.widgetWithText(FilledButton, 'Import paired vault'),
            )
            .onPressed,
        isNotNull,
      );
    } finally {
      await f.close(tester);
    }
  });
}

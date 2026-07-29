import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/api/session.dart' as bridge_api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/error.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/providers/activity_provider.dart';
import 'package:app/src/providers/providers.dart';

void main() {
  group('bridge repositories', () {
    test('provider graph constructs repositories from AppSession', () {
      final session = _FakeBridgeSession();
      final container = ProviderContainer(
        overrides: [appSessionProvider.overrideWithValue(session)],
      );
      addTearDown(container.dispose);

      expect(
        container.read(sessionRepositoryProvider),
        isA<BridgeSessionRepository>(),
      );
      expect(
        container.read(entryRepositoryProvider),
        isA<BridgeEntryRepository>(),
      );
      expect(
        container.read(secretsRepositoryProvider),
        isA<BridgeSecretsRepository>(),
      );
      expect(
        container.read(searchRepositoryProvider),
        isA<BridgeSearchRepository>(),
      );
      expect(
        container.read(totpRepositoryProvider),
        isA<BridgeTotpRepository>(),
      );
      expect(
        container.read(generatorRepositoryProvider),
        isA<BridgeGeneratorRepository>(),
      );
      expect(
        container.read(syncRepositoryProvider),
        isA<BridgeSyncRepository>(),
      );
      expect(
        container.read(prefsRepositoryProvider),
        isA<BridgePrefsRepository>(),
      );
    });

    test('secrets repository delegates every reveal without caching', () async {
      final session = _FakeBridgeSession();
      final repo = BridgeSecretsRepository(session);

      expect(await repo.revealField('entry', RevealField.password), 'secret-1');
      expect(await repo.revealField('entry', RevealField.password), 'secret-2');
      expect(session.revealCalls, 2);
    });

    test('bridge errors are mapped by the concrete repository', () async {
      final session = _FakeBridgeSession()
        ..revealError = const HidlinsApiError.authenticationFailed();
      final repo = BridgeSecretsRepository(session);

      await expectLater(
        repo.revealField('entry', RevealField.password),
        throwsA(isA<BadCredentials>()),
      );
    });

    test('activity throttle delegates to the bridge-backed session', () {
      final session = _FakeBridgeSession();
      final container = ProviderContainer(
        overrides: [appSessionProvider.overrideWithValue(session)],
      );
      addTearDown(container.dispose);

      container.read(activityThrottleProvider.notifier).reportActivity();

      expect(session.activityCalls, 1);
    });

    test('preferences preserve geometry and list pane width', () async {
      final session = _FakeBridgeSession()
        ..prefs = const bridge.UiPrefs(
          themeMode: 'dark',
          windowX: 10,
          windowY: 20,
          windowWidth: 1200,
          windowHeight: 800,
          lastVault: 'personal',
          listPaneWidth: 375.5,
        );
      final repo = BridgePrefsRepository(session);

      final prefs = await repo.getPrefs();
      expect(prefs.windowX, 10);
      expect(prefs.windowHeight, 800);
      expect(prefs.listPaneWidth, 375.5);

      await repo.setPrefs(prefs.copyWith(themeMode: 'light'));
      expect(session.prefs.themeMode, 'light');
      expect(session.prefs.windowX, 10);
      expect(session.prefs.windowHeight, 800);
      expect(session.prefs.listPaneWidth, 375.5);
    });
  });
}

final class _FakeBridgeSession implements bridge_api.AppSession {
  int revealCalls = 0;
  int activityCalls = 0;
  HidlinsApiError? revealError;
  bridge.UiPrefs prefs = const bridge.UiPrefs();

  @override
  Future<String> revealField({
    required String uuid,
    required bridge.RevealField field,
  }) async {
    revealCalls++;
    if (revealError case final error?) throw error;
    return 'secret-$revealCalls';
  }

  @override
  void reportActivity() {
    activityCalls++;
  }

  @override
  Future<bridge.UiPrefs> getPrefs() async => prefs;

  @override
  Future<void> setPrefs({required bridge.UiPrefs prefs}) async {
    this.prefs = prefs;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

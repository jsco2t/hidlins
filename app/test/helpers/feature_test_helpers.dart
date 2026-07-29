import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/providers/providers.dart';
import 'package:app/src/ui/theme.dart';
import '../fakes/fake_repositories.dart';

class TestHarness {
  final FakeSessionRepository session = FakeSessionRepository();
  final FakeEntryRepository entries = FakeEntryRepository();
  final FakeSecretsRepository secrets = FakeSecretsRepository();
  final FakeSearchRepository search = FakeSearchRepository();
  final FakeTotpRepository totp = FakeTotpRepository();
  final FakeGeneratorRepository generator = FakeGeneratorRepository();
  final FakeSyncRepository sync = FakeSyncRepository();
  final FakePrefsRepository prefs = FakePrefsRepository();

  List<Override> get overrides => [
    sessionRepositoryProvider.overrideWithValue(session),
    entryRepositoryProvider.overrideWithValue(entries),
    secretsRepositoryProvider.overrideWithValue(secrets),
    searchRepositoryProvider.overrideWithValue(search),
    totpRepositoryProvider.overrideWithValue(totp),
    generatorRepositoryProvider.overrideWithValue(generator),
    syncRepositoryProvider.overrideWithValue(sync),
    prefsRepositoryProvider.overrideWithValue(prefs),
  ];

  void dispose() {
    session.dispose();
    sync.dispose();
  }
}

extension FeatureTesterExtensions on WidgetTester {
  Future<TestHarness> pumpFeature(
    Widget child, {
    Brightness brightness = Brightness.light,
  }) async {
    final harness = TestHarness();
    await pumpWidget(
      ProviderScope(
        overrides: harness.overrides,
        child: MaterialApp(
          theme: brightness == Brightness.dark
              ? hidlinsDarkTheme()
              : hidlinsLightTheme(),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: child,
        ),
      ),
    );
    return harness;
  }
}

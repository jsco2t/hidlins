import 'package:material_ui/material_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/l10n/app_localizations.dart';
import 'package:app/src/l10n/hidlins_localizations.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/providers/providers.dart';
import 'package:app/src/ui/theme.dart';

import '../fakes/fake_repositories.dart';
import '../fakes/fake_platform_capabilities.dart';

class TestHarness {
  final FakeSessionRepository session = FakeSessionRepository();
  final FakeEntryRepository entries = FakeEntryRepository();
  final FakeSecretsRepository secrets = FakeSecretsRepository();
  final FakeSearchRepository search = FakeSearchRepository();
  final FakeTotpRepository totp = FakeTotpRepository();
  final FakeGeneratorRepository generator = FakeGeneratorRepository();
  final FakeSyncRepository sync = FakeSyncRepository();
  final FakePrefsRepository prefs = FakePrefsRepository();
  final FakeKeyfileAccessCapability keyfiles = FakeKeyfileAccessCapability();
  final FakeVaultImportCapability vaultImport = FakeVaultImportCapability([]);

  List<Override> get overrides => [
    sessionRepositoryProvider.overrideWithValue(session),
    entryRepositoryProvider.overrideWithValue(entries),
    secretsRepositoryProvider.overrideWithValue(secrets),
    searchRepositoryProvider.overrideWithValue(search),
    totpRepositoryProvider.overrideWithValue(totp),
    generatorRepositoryProvider.overrideWithValue(generator),
    syncRepositoryProvider.overrideWithValue(sync),
    prefsRepositoryProvider.overrideWithValue(prefs),
    keyfileAccessCapabilityProvider.overrideWithValue(keyfiles),
    vaultImportCapabilityProvider.overrideWithValue(vaultImport),
  ];

  void dispose() {
    session.dispose();
    sync.dispose();
  }
}

void registerTestVault(TestHarness harness) {
  harness.session.vaults = const [
    VaultSummary(
      name: 'personal',
      path: '/vaults/personal.kdbx',
      hasKeyfile: false,
      hasSync: false,
    ),
  ];
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
          localizationsDelegates: hidlinsLocalizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: child,
        ),
      ),
    );
    return harness;
  }

  Future<void> pumpFeatureWithHarness(
    Widget child,
    TestHarness harness, {
    Brightness brightness = Brightness.light,
  }) async {
    await pumpWidget(
      ProviderScope(
        overrides: harness.overrides,
        child: MaterialApp(
          theme: brightness == Brightness.dark
              ? hidlinsDarkTheme()
              : hidlinsLightTheme(),
          localizationsDelegates: hidlinsLocalizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          home: child,
        ),
      ),
    );
  }
}

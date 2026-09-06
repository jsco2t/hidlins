import 'package:flutter/foundation.dart';
import 'package:material_ui/material_ui.dart';

import 'src/app.dart';
import 'src/bridge/api/session.dart' as bridge;
import 'src/bridge/dto.dart';
import 'src/l10n/app_localizations.dart';
import 'src/l10n/hidlins_localizations.dart';
import 'src/platform/app_paths.dart';
import 'src/platform/platform_result.dart';
import 'src/platform/rust_library.dart';
import 'src/providers/providers.dart';
import 'src/ui/theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  try {
    await initializeBundledRustLibrary();
    final session = await bridge.initApp(cfg: await _appInitConfig());
    runApp(
      HidlinsApp(overrides: [appSessionProvider.overrideWithValue(session)]),
    );
  } catch (e) {
    runApp(_BridgeErrorApp(error: '$e'));
  }
}

Future<AppInitConfig> _appInitConfig() async {
  if (kIsWeb ||
      (defaultTargetPlatform != TargetPlatform.iOS &&
          defaultTargetPlatform != TargetPlatform.android)) {
    return const AppInitConfig();
  }
  final result = await const MethodChannelAppPaths().applicationSupportPath();
  return switch (result) {
    PlatformSuccess<String>(:final value) => AppInitConfig(
      stateDir: '$value/state',
      configDir: '$value/config',
    ),
    _ => throw StateError('mobile-application-support-unavailable'),
  };
}

class _BridgeErrorApp extends StatelessWidget {
  const _BridgeErrorApp({required this.error});

  final String error;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Hidlins',
      theme: hidlinsLightTheme(),
      darkTheme: hidlinsDarkTheme(),
      localizationsDelegates: hidlinsLocalizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Builder(
        builder: (context) {
          final l10n = AppLocalizations.of(context)!;
          return Scaffold(
            appBar: AppBar(title: Text(l10n.appTitle)),
            body: Center(
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: Text(
                  '${l10n.bridgeInitError}:\n$error',
                  textAlign: TextAlign.center,
                ),
              ),
            ),
          );
        },
      ),
    );
  }
}

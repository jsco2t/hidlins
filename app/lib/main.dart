import 'package:flutter/material.dart';
import 'src/app.dart';
import 'src/bridge/api/session.dart' as bridge;
import 'src/bridge/dto.dart';
import 'src/bridge/frb_generated.dart';
import 'src/l10n/app_localizations.dart';
import 'src/providers/providers.dart';
import 'src/ui/theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  try {
    await RustLib.init();
    final session = await bridge.initApp(cfg: const AppInitConfig());
    runApp(
      HidlinsApp(overrides: [appSessionProvider.overrideWithValue(session)]),
    );
  } catch (e) {
    runApp(_BridgeErrorApp(error: '$e'));
  }
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
      localizationsDelegates: AppLocalizations.localizationsDelegates,
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

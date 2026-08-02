import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'data/models.dart';
import 'l10n/app_localizations.dart';
import 'providers/activity_provider.dart';
import 'providers/providers.dart';
import 'router.dart';
import 'ui/activity_capture.dart';
import 'ui/shortcuts.dart';
import 'ui/theme.dart';

export 'ui/activity_capture.dart' show ActivityCapture;

class HidlinsApp extends StatelessWidget {
  const HidlinsApp({super.key, this.overrides = const []});

  final List<Override> overrides;

  @override
  Widget build(BuildContext context) {
    return ProviderScope(overrides: overrides, child: const _AppContent());
  }
}

class _AppContent extends ConsumerWidget {
  const _AppContent();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final router = ref.watch(routerProvider);
    final prefs = ref.watch(prefsProvider).valueOrNull;
    ref.watch(vaultCacheInvalidationProvider);
    return ActivityCapture(
      onActivity: () {
        ref.read(activityThrottleProvider.notifier).reportActivity();
      },
      onLifecycleStateChange: (state) {
        ref
            .read(sessionRepositoryProvider)
            .reportLifecycleState(_mapLifecycleState(state));
      },
      child: MaterialApp.router(
        title: 'Hidlins',
        theme: hidlinsLightTheme(),
        darkTheme: hidlinsDarkTheme(),
        themeMode: switch (prefs?.themeMode) {
          'light' => ThemeMode.light,
          'dark' => ThemeMode.dark,
          _ => ThemeMode.system,
        },
        builder: (context, child) => HidlinsShortcuts(
          onSearch: () => router.go('/search'),
          onLock: () {
            unawaited(ref.read(sessionRepositoryProvider).lockNow());
          },
          onDismiss: () {
            if (router.canPop()) router.pop();
          },
          child: child ?? const SizedBox.shrink(),
        ),
        routerConfig: router,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
      ),
    );
  }
}

LifecycleStateDto _mapLifecycleState(AppLifecycleState state) {
  return switch (state) {
    AppLifecycleState.resumed => LifecycleStateDto.resumed,
    AppLifecycleState.inactive => LifecycleStateDto.inactive,
    AppLifecycleState.paused => LifecycleStateDto.paused,
    AppLifecycleState.detached => LifecycleStateDto.detached,
    AppLifecycleState.hidden => LifecycleStateDto.hidden,
  };
}

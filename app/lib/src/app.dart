import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import 'data/models.dart';
import 'l10n/app_localizations.dart';
import 'l10n/hidlins_localizations.dart';
import 'providers/activity_provider.dart';
import 'providers/lock_state_provider.dart';
import 'providers/providers.dart';
import 'platform/lifecycle.dart';
import 'router.dart';
import 'ui/activity_capture.dart';
import 'ui/startup_warning_banner.dart';
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

class _AppContent extends ConsumerStatefulWidget {
  const _AppContent();

  @override
  ConsumerState<_AppContent> createState() => _AppContentState();
}

class _AppContentState extends ConsumerState<_AppContent> {
  NativeLifecycleEvents? _nativeLifecycle;
  final Completer<void> _disposed = Completer<void>();

  bool get _usesNativeLifecycle =>
      !kIsWeb &&
      (defaultTargetPlatform == TargetPlatform.iOS ||
          defaultTargetPlatform == TargetPlatform.android);

  @override
  void initState() {
    super.initState();
    if (_usesNativeLifecycle) {
      _nativeLifecycle = NativeLifecycleEvents()
        ..listen(_reportNativeLifecycle);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        unawaited(_reportNativeLifecycle(AppLifecycleSignal.resumed));
      });
    }
  }

  @override
  void dispose() {
    if (!_disposed.isCompleted) _disposed.complete();
    _nativeLifecycle?.dispose();
    super.dispose();
  }

  Future<void> _reportNativeLifecycle(AppLifecycleSignal state) async {
    if (!mounted) return;
    final knownLockState = ref
        .read(sessionRepositoryProvider)
        .reportLifecycleState(_mapNativeLifecycleState(state));
    if (!await _waitForRenderedLockState(knownLockState) || !mounted) return;
    await const MethodChannelLifecycle().acknowledge(state, knownLockState);
  }

  Future<bool> _waitForRenderedLockState(LockEvent expected) async {
    final observed = Completer<void>();
    final subscription = ref.listenManual(lockStateProvider, (_, next) {
      if (next.asData?.value == expected && !observed.isCompleted) {
        observed.complete();
      }
    }, fireImmediately: true);
    try {
      final rendered = await Future.any([
        observed.future.then((_) => true),
        _disposed.future.then((_) => false),
      ]).timeout(const Duration(seconds: 2), onTimeout: () => false);
      if (!rendered) return false;
      await WidgetsBinding.instance.endOfFrame;
      return mounted;
    } on TimeoutException {
      // Fail closed: native keeps the opaque shield installed when the Dart
      // route has not caught up with Rust's authoritative lock state.
      return false;
    } finally {
      subscription.close();
    }
  }

  @override
  Widget build(BuildContext context) {
    final router = ref.watch(routerProvider);
    ref.watch(vaultCacheInvalidationProvider);
    return ActivityCapture(
      onActivity: () {
        ref.read(activityThrottleProvider.notifier).reportActivity();
      },
      onLifecycleStateChange: (state) {
        if (!_usesNativeLifecycle) {
          ref
              .read(sessionRepositoryProvider)
              .reportLifecycleState(_mapLifecycleState(state));
        }
      },
      child: MaterialApp.router(
        title: 'Hidlins',
        theme: hidlinsLightTheme(),
        darkTheme: hidlinsDarkTheme(),
        themeMode: ThemeMode.system,
        routerConfig: router,
        localizationsDelegates: hidlinsLocalizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        builder: (context, child) =>
            StartupWarningBanner(child: child ?? const SizedBox.shrink()),
      ),
    );
  }
}

LifecycleStateDto _mapNativeLifecycleState(AppLifecycleSignal state) {
  return switch (state) {
    AppLifecycleSignal.resumed => LifecycleStateDto.resumed,
    AppLifecycleSignal.inactive => LifecycleStateDto.inactive,
    AppLifecycleSignal.hidden => LifecycleStateDto.hidden,
    AppLifecycleSignal.paused => LifecycleStateDto.paused,
    AppLifecycleSignal.detached => LifecycleStateDto.detached,
  };
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

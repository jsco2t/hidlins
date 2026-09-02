import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../l10n/app_localizations.dart';
import '../providers/providers.dart';

class StartupWarningBanner extends ConsumerWidget {
  const StartupWarningBanner({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final warnings = ref.watch(startupWarningsProvider);
    final degraded =
        warnings.hasError || (warnings.valueOrNull?.isNotEmpty ?? false);
    final l10n = AppLocalizations.of(context)!;

    return Column(
      children: [
        if (degraded)
          Semantics(
            container: true,
            liveRegion: true,
            label:
                '${l10n.startupSecurityWarningTitle}. '
                '${l10n.startupSecurityWarningMessage}',
            child: Material(
              color: Theme.of(context).colorScheme.errorContainer,
              child: ListTile(
                dense: true,
                leading: Icon(
                  Icons.warning_amber_rounded,
                  color: Theme.of(context).colorScheme.onErrorContainer,
                ),
                title: Text(l10n.startupSecurityWarningTitle),
                subtitle: Text(l10n.startupSecurityWarningMessage),
              ),
            ),
          ),
        Expanded(child: child),
      ],
    );
  }
}

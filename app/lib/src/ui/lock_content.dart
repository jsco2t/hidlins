import 'package:flutter/material.dart';

import '../l10n/app_localizations.dart';

class LockContent extends StatelessWidget {
  const LockContent({super.key, this.iconSize = 64});

  final double iconSize;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.lock, size: iconSize, color: colorScheme.primary),
          const SizedBox(height: 16),
          Text(
            l10n.lockScreenTitle,
            style: Theme.of(context).textTheme.headlineMedium,
          ),
        ],
      ),
    );
  }
}

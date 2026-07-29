import 'package:flutter/material.dart';

import '../../l10n/app_localizations.dart';
import '../../ui/tokens.dart';

enum FirstRunChoice { create, connectSync, import_ }

class FirstRunPage extends StatelessWidget {
  const FirstRunPage({super.key, required this.onChoice});

  final ValueChanged<FirstRunChoice> onChoice;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;

    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 400),
        child: Padding(
          padding: const EdgeInsets.all(HidlinsSpacing.xl),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.shield, size: 64, color: colorScheme.primary),
              const SizedBox(height: HidlinsSpacing.lg),
              Text(
                l10n.firstRunTitle,
                style: Theme.of(context).textTheme.headlineMedium,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: HidlinsSpacing.xl),
              _ChoiceCard(
                icon: Icons.add_circle_outline,
                title: l10n.firstRunCreateVault,
                onTap: () => onChoice(FirstRunChoice.create),
              ),
              const SizedBox(height: HidlinsSpacing.sm),
              _ChoiceCard(
                icon: Icons.sync,
                title: l10n.firstRunConnectSync,
                onTap: () => onChoice(FirstRunChoice.connectSync),
              ),
              const SizedBox(height: HidlinsSpacing.sm),
              _ChoiceCard(
                icon: Icons.file_open_outlined,
                title: l10n.firstRunImportVault,
                onTap: () => onChoice(FirstRunChoice.import_),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ChoiceCard extends StatelessWidget {
  const _ChoiceCard({
    required this.icon,
    required this.title,
    required this.onTap,
  });

  final IconData icon;
  final String title;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ListTile(
        leading: Icon(icon),
        title: Text(title),
        trailing: const Icon(Icons.chevron_right),
        onTap: onTap,
      ),
    );
  }
}

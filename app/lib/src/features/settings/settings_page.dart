import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';

class SettingsPage extends ConsumerWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context)!;
    final prefsAsync = ref.watch(prefsProvider);
    return ListView(
      padding: const EdgeInsets.all(HidlinsSpacing.md),
      children: [
        Text(
          l10n.settingsTitle,
          style: Theme.of(context).textTheme.headlineSmall,
        ),
        const SizedBox(height: HidlinsSpacing.lg),

        // Theme
        Card(
          child: Padding(
            padding: const EdgeInsets.all(HidlinsSpacing.md),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  l10n.settingsTheme,
                  style: Theme.of(context).textTheme.titleSmall,
                ),
                const SizedBox(height: HidlinsSpacing.sm),
                prefsAsync.when(
                  data: (prefs) => SegmentedButton<String>(
                    segments: [
                      ButtonSegment(
                        value: 'system',
                        label: Text(l10n.settingsThemeSystem),
                        icon: const Icon(Icons.brightness_auto),
                      ),
                      ButtonSegment(
                        value: 'light',
                        label: Text(l10n.settingsThemeLight),
                        icon: const Icon(Icons.light_mode),
                      ),
                      ButtonSegment(
                        value: 'dark',
                        label: Text(l10n.settingsThemeDark),
                        icon: const Icon(Icons.dark_mode),
                      ),
                    ],
                    selected: {prefs.themeMode ?? 'system'},
                    onSelectionChanged: (s) async {
                      await ref
                          .read(prefsProvider.notifier)
                          .setPrefs(
                            prefs.copyWith(
                              themeMode: s.first == 'system' ? null : s.first,
                            ),
                          );
                    },
                  ),
                  loading: () => const LinearProgressIndicator(),
                  error: (_, _) => const SizedBox.shrink(),
                ),
              ],
            ),
          ),
        ),
        const SizedBox(height: HidlinsSpacing.sm),

        // Auto-lock (display only)
        Card(
          child: ListTile(
            leading: const Icon(Icons.timer),
            title: Text(l10n.settingsAutoLock),
            subtitle: Text(l10n.settingsAutoLockNote),
          ),
        ),
        const SizedBox(height: HidlinsSpacing.sm),

        Card(
          child: Padding(
            padding: const EdgeInsets.all(HidlinsSpacing.md),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    const Icon(Icons.keyboard_outlined),
                    const SizedBox(width: HidlinsSpacing.sm),
                    Text(
                      l10n.settingsKeyboardShortcuts,
                      style: Theme.of(context).textTheme.titleSmall,
                    ),
                  ],
                ),
                const SizedBox(height: HidlinsSpacing.sm),
                _ShortcutRow(
                  keys: 'Ctrl/⌘ F',
                  description: l10n.shortcutSearch,
                ),
                _ShortcutRow(keys: 'N', description: l10n.shortcutNewEntry),
                _ShortcutRow(keys: 'L', description: l10n.shortcutLock),
                _ShortcutRow(
                  keys: 'Ctrl/⌘ C',
                  description: l10n.shortcutCopyPassword,
                ),
                _ShortcutRow(keys: 'Esc', description: l10n.shortcutDismiss),
              ],
            ),
          ),
        ),
        const SizedBox(height: HidlinsSpacing.sm),

        // About
        Card(
          child: Column(
            children: [
              ListTile(
                leading: const Icon(Icons.info_outline),
                title: Text(l10n.settingsAbout),
              ),
              ListTile(
                leading: const Icon(Icons.description_outlined),
                title: Text(l10n.settingsLicenses),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => showLicensePage(
                  context: context,
                  applicationName: l10n.appTitle,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _ShortcutRow extends StatelessWidget {
  const _ShortcutRow({required this.keys, required this.description});

  final String keys;
  final String description;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: HidlinsSpacing.xs),
      child: Row(
        children: [
          SizedBox(
            width: 96,
            child: Text(keys, style: Theme.of(context).textTheme.labelLarge),
          ),
          Expanded(child: Text(description)),
        ],
      ),
    );
  }
}

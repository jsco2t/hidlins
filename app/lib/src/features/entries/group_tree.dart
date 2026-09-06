import 'package:material_ui/material_ui.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../ui/tokens.dart';

class GroupTree extends StatelessWidget {
  const GroupTree({
    super.key,
    required this.root,
    required this.selectedGroupUuid,
    required this.onGroupSelected,
    this.onNewEntry,
  });

  final GroupNode root;
  final String? selectedGroupUuid;
  final ValueChanged<String?> onGroupSelected;
  final ValueChanged<String>? onNewEntry;

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.symmetric(vertical: HidlinsSpacing.sm),
      children: [
        _GroupTile(
          node: root,
          depth: 0,
          selectedGroupUuid: selectedGroupUuid,
          onGroupSelected: onGroupSelected,
          onNewEntry: onNewEntry,
          isRoot: true,
        ),
      ],
    );
  }
}

class _GroupTile extends StatelessWidget {
  const _GroupTile({
    required this.node,
    required this.depth,
    required this.selectedGroupUuid,
    required this.onGroupSelected,
    this.onNewEntry,
    this.isRoot = false,
  });

  final GroupNode node;
  final int depth;
  final String? selectedGroupUuid;
  final ValueChanged<String?> onGroupSelected;
  final ValueChanged<String>? onNewEntry;
  final bool isRoot;

  @override
  Widget build(BuildContext context) {
    final isSelected = selectedGroupUuid == node.uuid;
    final colorScheme = Theme.of(context).colorScheme;
    final label = isRoot
        ? AppLocalizations.of(context)!.groupAllEntries
        : node.name;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        Semantics(
          button: true,
          selected: isSelected,
          label: label,
          child: MenuAnchor(
            menuChildren: [
              if (onNewEntry != null)
                MenuItemButton(
                  onPressed: () => onNewEntry!(node.uuid),
                  leadingIcon: const Icon(Icons.add),
                  child: Text(AppLocalizations.of(context)!.entryCreateTitle),
                ),
            ],
            builder: (context, controller, child) => GestureDetector(
              onSecondaryTapDown: (_) {
                onGroupSelected(isRoot && isSelected ? null : node.uuid);
                controller.open();
              },
              child: child,
            ),
            child: Material(
              color: isSelected
                  ? colorScheme.secondaryContainer
                  : Colors.transparent,
              child: InkWell(
                onTap: () =>
                    onGroupSelected(isRoot && isSelected ? null : node.uuid),
                child: ConstrainedBox(
                  constraints: const BoxConstraints(minHeight: 44),
                  child: Padding(
                    padding: EdgeInsets.only(
                      left: HidlinsSpacing.md + (depth * HidlinsSpacing.md),
                      right: HidlinsSpacing.md,
                      top: HidlinsSpacing.sm,
                      bottom: HidlinsSpacing.sm,
                    ),
                    child: Row(
                      children: [
                        Icon(
                          isRoot ? Icons.folder_special : Icons.folder,
                          size: 20,
                          color: isSelected
                              ? colorScheme.onSecondaryContainer
                              : colorScheme.onSurfaceVariant,
                        ),
                        const SizedBox(width: HidlinsSpacing.sm),
                        Expanded(
                          child: Text(
                            label,
                            style: Theme.of(context).textTheme.bodyMedium
                                ?.copyWith(
                                  fontWeight: isSelected
                                      ? FontWeight.w600
                                      : null,
                                  color: isSelected
                                      ? colorScheme.onSecondaryContainer
                                      : null,
                                ),
                          ),
                        ),
                        Text(
                          '${node.entryCount}',
                          style: Theme.of(context).textTheme.labelSmall
                              ?.copyWith(color: colorScheme.onSurfaceVariant),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
        for (final child in node.children)
          _GroupTile(
            node: child,
            depth: depth + 1,
            selectedGroupUuid: selectedGroupUuid,
            onGroupSelected: onGroupSelected,
            onNewEntry: onNewEntry,
          ),
      ],
    );
  }
}

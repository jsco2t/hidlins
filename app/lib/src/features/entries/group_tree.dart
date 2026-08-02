import 'package:flutter/material.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../ui/tokens.dart';

class GroupTree extends StatelessWidget {
  const GroupTree({
    super.key,
    required this.root,
    required this.selectedGroupUuid,
    required this.onGroupSelected,
    this.onCreateEntry,
  });

  final GroupNode root;
  final String? selectedGroupUuid;
  final ValueChanged<String?> onGroupSelected;
  final ValueChanged<String>? onCreateEntry;

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
          onCreateEntry: onCreateEntry,
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
    this.onCreateEntry,
    this.isRoot = false,
  });

  final GroupNode node;
  final int depth;
  final String? selectedGroupUuid;
  final ValueChanged<String?> onGroupSelected;
  final ValueChanged<String>? onCreateEntry;
  final bool isRoot;

  @override
  Widget build(BuildContext context) {
    final isSelected = isRoot
        ? selectedGroupUuid == null
        : selectedGroupUuid == node.uuid;
    final colorScheme = Theme.of(context).colorScheme;
    final l10n = AppLocalizations.of(context)!;

    final tile = Material(
      color: isSelected ? colorScheme.secondaryContainer : Colors.transparent,
      child: MenuAnchor(
        menuChildren: [
          MenuItemButton(
            leadingIcon: const Icon(Icons.filter_list),
            onPressed: () => onGroupSelected(isRoot ? null : node.uuid),
            child: Text(l10n.groupContextShowEntries),
          ),
          if (onCreateEntry != null)
            MenuItemButton(
              leadingIcon: const Icon(Icons.add),
              onPressed: () => onCreateEntry!(node.uuid),
              child: Text(l10n.actionNewEntry),
            ),
        ],
        builder: (context, controller, child) => InkWell(
          onTap: () => onGroupSelected(isRoot ? null : node.uuid),
          onSecondaryTap: controller.open,
          child: child,
        ),
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
                  isRoot ? l10n.groupAllEntries : node.name,
                  style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    fontWeight: isSelected ? FontWeight.w600 : null,
                    color: isSelected ? colorScheme.onSecondaryContainer : null,
                  ),
                ),
              ),
              Text(
                '${node.entryCount}',
                style: Theme.of(context).textTheme.labelSmall?.copyWith(
                  color: colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ),
      ),
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        tile,
        for (final child in node.children)
          _GroupTile(
            node: child,
            depth: depth + 1,
            selectedGroupUuid: selectedGroupUuid,
            onGroupSelected: onGroupSelected,
            onCreateEntry: onCreateEntry,
          ),
      ],
    );
  }
}

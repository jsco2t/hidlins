import 'package:material_ui/material_ui.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../ui/tokens.dart';

enum GroupAction { create, rename, move, delete }

typedef GroupActionCallback = void Function(
  GroupNode group,
  GroupAction action,
);

class GroupTree extends StatelessWidget {
  const GroupTree({
    super.key,
    required this.root,
    required this.selectedGroupUuid,
    required this.onGroupSelected,
    this.onNewEntry,
    this.onAction,
    this.onGroupContextSelected,
  });

  final GroupNode root;
  final String? selectedGroupUuid;
  final ValueChanged<String?> onGroupSelected;
  final ValueChanged<String>? onNewEntry;
  final GroupActionCallback? onAction;
  final ValueChanged<String?>? onGroupContextSelected;

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
          onAction: onAction,
          onGroupContextSelected: onGroupContextSelected,
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
    this.onAction,
    this.onGroupContextSelected,
  });

  final GroupNode node;
  final int depth;
  final String? selectedGroupUuid;
  final ValueChanged<String?> onGroupSelected;
  final ValueChanged<String>? onNewEntry;
  final bool isRoot;
  final GroupActionCallback? onAction;
  final ValueChanged<String?>? onGroupContextSelected;

  BigInt _totalEntries(GroupNode group) => group.children.fold(
    group.entryCount,
    (total, child) => total + _totalEntries(child),
  );

  @override
  Widget build(BuildContext context) {
    final isSelected = isRoot
        ? selectedGroupUuid == null
        : selectedGroupUuid == node.uuid;
    final colorScheme = Theme.of(context).colorScheme;
    final label = isRoot
        ? AppLocalizations.of(context)!.groupAllEntries
        : node.name;
    final l10n = AppLocalizations.of(context)!;
    void selectContext() =>
        (onGroupContextSelected ?? onGroupSelected)(isRoot ? null : node.uuid);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: [
        MenuAnchor(
          menuChildren: [
            if (onNewEntry != null)
              MenuItemButton(
                onPressed: () => onNewEntry!(node.uuid),
                leadingIcon: const Icon(Icons.add),
                child: Text(AppLocalizations.of(context)!.entryCreateTitle),
              ),
            if (onAction != null) ...[
              MenuItemButton(
                onPressed: () => onAction!(node, GroupAction.create),
                child: Text(l10n.groupCreateTitle),
              ),
              MenuItemButton(
                onPressed: () => onAction!(node, GroupAction.rename),
                child: Text(l10n.groupRenameTitle),
              ),
              if (!isRoot) ...[
                MenuItemButton(
                  onPressed: () => onAction!(node, GroupAction.move),
                  child: Text(l10n.groupMoveTitle),
                ),
                MenuItemButton(
                  onPressed: () => onAction!(node, GroupAction.delete),
                  child: Text(l10n.groupDeleteTitle),
                ),
              ],
            ],
          ],
          builder: (context, controller, _) => GestureDetector(
            excludeFromSemantics: true,
            onSecondaryTapDown: (_) {
              selectContext();
              controller.open();
            },
            onLongPress: onAction == null
                ? null
                : () {
                    selectContext();
                    controller.open();
                  },
            child: Material(
              color: isSelected
                  ? colorScheme.secondaryContainer
                  : Colors.transparent,
              child: Row(
                children: [
                  Expanded(
                    child: MergeSemantics(
                      child: Semantics(
                        button: true,
                        selected: isSelected,
                        child: InkWell(
                          onTap: () =>
                              onGroupSelected(isRoot ? null : node.uuid),
                          child: ConstrainedBox(
                            constraints: const BoxConstraints(minHeight: 44),
                            child: Padding(
                              padding: EdgeInsets.only(
                                left:
                                    HidlinsSpacing.md +
                                    (depth * HidlinsSpacing.md),
                                right: HidlinsSpacing.md,
                                top: HidlinsSpacing.sm,
                                bottom: HidlinsSpacing.sm,
                              ),
                              child: Row(
                                children: [
                                  Icon(
                                    isRoot
                                        ? Icons.folder_special
                                        : Icons.folder,
                                    size: 20,
                                    color: isSelected
                                        ? colorScheme.onSecondaryContainer
                                        : colorScheme.onSurfaceVariant,
                                  ),
                                  const SizedBox(width: HidlinsSpacing.sm),
                                  Expanded(
                                    child: Text(
                                      label,
                                      style: Theme.of(context)
                                          .textTheme
                                          .bodyMedium
                                          ?.copyWith(
                                            fontWeight: isSelected
                                                ? FontWeight.w600
                                                : null,
                                            color: isSelected
                                                ? colorScheme
                                                      .onSecondaryContainer
                                                : null,
                                          ),
                                    ),
                                  ),
                                  Text(
                                    '${isRoot ? _totalEntries(node) : node.entryCount}',
                                    style: Theme.of(context)
                                        .textTheme
                                        .labelSmall
                                        ?.copyWith(
                                          color: colorScheme.onSurfaceVariant,
                                        ),
                                  ),
                                ],
                              ),
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                  if (onAction != null)
                    IconButton(
                      tooltip: l10n.groupActionsFor(label),
                      icon: const Icon(Icons.more_vert),
                      onPressed: controller.open,
                    ),
                ],
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
            onAction: onAction,
            onGroupContextSelected: onGroupContextSelected,
          ),
      ],
    );
  }
}

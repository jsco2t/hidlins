import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../ui/tokens.dart';
import '../../ui/widgets/entry_avatar.dart';

class EntryList extends StatefulWidget {
  const EntryList({
    super.key,
    required this.entries,
    required this.selectedUuid,
    required this.onEntrySelected,
    required this.onCopyUsername,
    required this.onCopyPassword,
  });

  final List<EntrySummary> entries;
  final String? selectedUuid;
  final ValueChanged<String> onEntrySelected;
  final ValueChanged<String> onCopyUsername;
  final ValueChanged<String> onCopyPassword;

  @override
  State<EntryList> createState() => _EntryListState();
}

class _EntryListState extends State<EntryList> {
  final _focusNode = FocusNode();

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    if (widget.entries.isEmpty) {
      return Center(
        child: Text(
          l10n.emptyStateNoEntries,
          style: Theme.of(context).textTheme.bodyLarge?.copyWith(
            color: Theme.of(context).colorScheme.onSurfaceVariant,
          ),
        ),
      );
    }

    return Focus(
      focusNode: _focusNode,
      autofocus: true,
      onKeyEvent: _handleKeyEvent,
      child: ListView.builder(
        itemCount: widget.entries.length,
        itemBuilder: (context, index) {
          final entry = widget.entries[index];
          return _EntryRow(
            entry: entry,
            isSelected: entry.uuid == widget.selectedUuid,
            onTap: () => widget.onEntrySelected(entry.uuid),
            onCopyUsername: () => widget.onCopyUsername(entry.uuid),
            onCopyPassword: () => widget.onCopyPassword(entry.uuid),
          );
        },
      ),
    );
  }

  KeyEventResult _handleKeyEvent(FocusNode node, KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }

    final entries = widget.entries;
    if (entries.isEmpty) return KeyEventResult.ignored;

    if (event.logicalKey == LogicalKeyboardKey.arrowDown) {
      final currentIndex = _selectedIndex;
      final nextIndex = currentIndex == null
          ? 0
          : (currentIndex + 1).clamp(0, entries.length - 1);
      widget.onEntrySelected(entries[nextIndex].uuid);
      return KeyEventResult.handled;
    }

    if (event.logicalKey == LogicalKeyboardKey.arrowUp) {
      final currentIndex = _selectedIndex;
      final nextIndex = currentIndex == null
          ? entries.length - 1
          : (currentIndex - 1).clamp(0, entries.length - 1);
      widget.onEntrySelected(entries[nextIndex].uuid);
      return KeyEventResult.handled;
    }

    if (event.logicalKey == LogicalKeyboardKey.enter) {
      if (widget.selectedUuid != null) {
        widget.onEntrySelected(widget.selectedUuid!);
        return KeyEventResult.handled;
      }
    }

    return KeyEventResult.ignored;
  }

  int? get _selectedIndex {
    if (widget.selectedUuid == null) return null;
    final idx = widget.entries.indexWhere((e) => e.uuid == widget.selectedUuid);
    return idx == -1 ? null : idx;
  }
}

class _EntryRow extends StatefulWidget {
  const _EntryRow({
    required this.entry,
    required this.isSelected,
    required this.onTap,
    required this.onCopyUsername,
    required this.onCopyPassword,
  });

  final EntrySummary entry;
  final bool isSelected;
  final VoidCallback onTap;
  final VoidCallback onCopyUsername;
  final VoidCallback onCopyPassword;

  @override
  State<_EntryRow> createState() => _EntryRowState();
}

class _EntryRowState extends State<_EntryRow> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final l10n = AppLocalizations.of(context)!;
    final actionsVisible = _hovered || widget.isSelected;

    return MenuAnchor(
      menuChildren: [
        MenuItemButton(
          leadingIcon: const Icon(Icons.open_in_new),
          onPressed: widget.onTap,
          child: Text(l10n.entryContextOpen),
        ),
        if (widget.entry.username.isNotEmpty)
          MenuItemButton(
            leadingIcon: const Icon(Icons.person_outline),
            onPressed: widget.onCopyUsername,
            child: Text(l10n.entryContextCopyUsername),
          ),
        MenuItemButton(
          leadingIcon: const Icon(Icons.copy),
          onPressed: widget.onCopyPassword,
          child: Text(l10n.entryContextCopyPassword),
        ),
      ],
      builder: (context, controller, child) => MouseRegion(
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: Material(
          color: widget.isSelected
              ? colorScheme.secondaryContainer
              : Colors.transparent,
          child: InkWell(
            onTap: widget.onTap,
            onSecondaryTap: controller.open,
            child: child,
          ),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: HidlinsSpacing.md,
          vertical: HidlinsSpacing.sm,
        ),
        child: Row(
          children: [
            EntryAvatar(title: widget.entry.title),
            const SizedBox(width: HidlinsSpacing.md),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          widget.entry.title,
                          style: Theme.of(context).textTheme.bodyMedium
                              ?.copyWith(fontWeight: FontWeight.w500),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      if (widget.entry.isExpired)
                        Padding(
                          padding: const EdgeInsets.only(
                            left: HidlinsSpacing.xs,
                          ),
                          child: Semantics(
                            label: l10n.entryDetailExpired,
                            child: Badge(
                              backgroundColor: colorScheme.error,
                              label: Text(
                                l10n.entryDetailExpired,
                                style: TextStyle(
                                  color: colorScheme.onError,
                                  fontSize: 10,
                                ),
                              ),
                            ),
                          ),
                        ),
                    ],
                  ),
                  if (widget.entry.username.isNotEmpty)
                    Text(
                      widget.entry.username,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: colorScheme.onSurfaceVariant,
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                ],
              ),
            ),
            IgnorePointer(
              ignoring: !actionsVisible,
              child: ExcludeFocus(
                excluding: !actionsVisible,
                child: ExcludeSemantics(
                  excluding: !actionsVisible,
                  child: AnimatedOpacity(
                    opacity: actionsVisible ? 1.0 : 0.0,
                    duration: hidlinsMotionDuration(
                      context,
                      HidlinsMotion.fast,
                    ),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        if (widget.entry.username.isNotEmpty)
                          Semantics(
                            label: l10n.entryContextCopyUsername,
                            button: true,
                            onTap: widget.onCopyUsername,
                            excludeSemantics: true,
                            child: IconButton(
                              icon: const Icon(Icons.person_outline, size: 18),
                              tooltip: l10n.entryContextCopyUsername,
                              onPressed: widget.onCopyUsername,
                            ),
                          ),
                        Semantics(
                          label: l10n.entryContextCopyPassword,
                          button: true,
                          onTap: widget.onCopyPassword,
                          excludeSemantics: true,
                          child: IconButton(
                            icon: const Icon(Icons.copy, size: 18),
                            tooltip: l10n.entryContextCopyPassword,
                            onPressed: widget.onCopyPassword,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

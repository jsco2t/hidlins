import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';
import '../../ui/shortcuts.dart';
import '../../ui/widgets/empty_state.dart';
import 'entry_detail.dart';
import 'entry_edit.dart';
import 'entry_list.dart';
import 'group_tree.dart';

class EntriesPage extends ConsumerStatefulWidget {
  const EntriesPage({super.key, this.initialUuid});

  final String? initialUuid;

  @override
  ConsumerState<EntriesPage> createState() => _EntriesPageState();
}

class _EntriesPageState extends ConsumerState<EntriesPage> {
  String? _selectedGroupUuid;
  String? _selectedEntryUuid;

  @override
  void initState() {
    super.initState();
    _selectedEntryUuid = widget.initialUuid;
  }

  @override
  void didUpdateWidget(EntriesPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.initialUuid != oldWidget.initialUuid &&
        widget.initialUuid != null) {
      setState(() => _selectedEntryUuid = widget.initialUuid);
    }
  }

  @override
  Widget build(BuildContext context) {
    final treeAsync = ref.watch(vaultTreeProvider);
    final syncState = ref.watch(syncUiStateProvider).valueOrNull;
    final l10n = AppLocalizations.of(context)!;
    final width = MediaQuery.sizeOf(context).width;
    final isExpanded = width >= HidlinsBreakpoints.medium;
    final isSyncing = syncState?.inFlight ?? false;

    return treeAsync.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Center(child: Text(l10n.errorGeneric)),
      data: (tree) {
        final filteredEntries = _filterEntries(tree);
        late final Widget workspace;

        if (isExpanded) {
          workspace = _ExpandedLayout(
            tree: tree,
            entries: filteredEntries,
            selectedGroupUuid: _selectedGroupUuid,
            selectedEntryUuid: _selectedEntryUuid,
            onGroupSelected: (uuid) =>
                setState(() => _selectedGroupUuid = uuid),
            onEntrySelected: (uuid) =>
                setState(() => _selectedEntryUuid = uuid),
            onCopyUsername: (uuid) => _copyField(uuid, CopyField.username),
            onCopyPassword: (uuid) => _copyField(uuid, CopyField.password),
            onCreateEntry: _openCreateEntry,
          );
        } else if (_selectedEntryUuid != null) {
          workspace = AnimatedSwitcher(
            duration: hidlinsMotionDuration(context, HidlinsMotion.standard),
            child: _CompactDetail(
              key: ValueKey('detail-$_selectedEntryUuid'),
              uuid: _selectedEntryUuid!,
              onBack: () => setState(() => _selectedEntryUuid = null),
            ),
          );
        } else {
          workspace = AnimatedSwitcher(
            duration: hidlinsMotionDuration(context, HidlinsMotion.standard),
            child: _CompactList(
              key: const ValueKey('entry-list'),
              entries: filteredEntries,
              onEntrySelected: (uuid) =>
                  setState(() => _selectedEntryUuid = uuid),
              onCopyUsername: (uuid) => _copyField(uuid, CopyField.username),
              onCopyPassword: (uuid) => _copyField(uuid, CopyField.password),
            ),
          );
        }

        final content = !isSyncing
            ? workspace
            : Column(
                children: [
                  Container(
                    key: const Key('sync-busy-banner'),
                    width: double.infinity,
                    color: Theme.of(context).colorScheme.tertiaryContainer,
                    padding: const EdgeInsets.symmetric(
                      horizontal: HidlinsSpacing.md,
                      vertical: HidlinsSpacing.sm,
                    ),
                    child: Row(
                      children: [
                        const SizedBox.square(
                          dimension: 18,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        ),
                        const SizedBox(width: HidlinsSpacing.sm),
                        Expanded(child: Text(l10n.syncBusyBanner)),
                      ],
                    ),
                  ),
                  Expanded(child: workspace),
                ],
              );
        return Actions(
          actions: {
            NewEntryIntent: NonEditingCallbackAction<NewEntryIntent>(
              onInvoke: (_) {
                unawaited(
                  _openCreateEntry(_selectedGroupUuid ?? tree.root.uuid),
                );
                return null;
              },
            ),
            CopySelectedPasswordIntent:
                NonEditingCallbackAction<CopySelectedPasswordIntent>(
                  onInvoke: (_) {
                    final uuid = _selectedEntryUuid;
                    if (uuid != null) {
                      unawaited(_copyField(uuid, CopyField.password));
                    }
                    return null;
                  },
                ),
          },
          child: content,
        );
      },
    );
  }

  List<EntrySummary> _filterEntries(VaultTree tree) {
    if (_selectedGroupUuid == null) return tree.entries;
    return tree.entries
        .where((e) => e.groupUuid == _selectedGroupUuid)
        .toList();
  }

  Future<void> _copyField(String uuid, CopyField field) async {
    final l10n = AppLocalizations.of(context)!;
    try {
      final repo = ref.read(secretsRepositoryProvider);
      await repo.copyEntryField(uuid, field);
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.copiedSnackbar(30))));
    } on Exception {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.errorGeneric)));
    }
  }

  Future<void> _openCreateEntry(String groupUuid) async {
    final syncState = ref.read(syncUiStateProvider).valueOrNull;
    if (syncState?.inFlight ?? false) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(AppLocalizations.of(context)!.syncActionsDisabled),
        ),
      );
      return;
    }
    final created = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => EntryEditDialog(groupUuid: groupUuid),
        fullscreenDialog: true,
      ),
    );
    if (created == true) {
      ref.invalidate(vaultTreeProvider);
    }
  }
}

class _ExpandedLayout extends StatelessWidget {
  const _ExpandedLayout({
    required this.tree,
    required this.entries,
    required this.selectedGroupUuid,
    required this.selectedEntryUuid,
    required this.onGroupSelected,
    required this.onEntrySelected,
    required this.onCopyUsername,
    required this.onCopyPassword,
    required this.onCreateEntry,
  });

  final VaultTree tree;
  final List<EntrySummary> entries;
  final String? selectedGroupUuid;
  final String? selectedEntryUuid;
  final ValueChanged<String?> onGroupSelected;
  final ValueChanged<String> onEntrySelected;
  final ValueChanged<String> onCopyUsername;
  final ValueChanged<String> onCopyPassword;
  final ValueChanged<String> onCreateEntry;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    return Row(
      children: [
        SizedBox(
          width: 220,
          child: GroupTree(
            root: tree.root,
            selectedGroupUuid: selectedGroupUuid,
            onGroupSelected: onGroupSelected,
            onCreateEntry: onCreateEntry,
          ),
        ),
        const VerticalDivider(thickness: 1, width: 1),
        Expanded(
          child: EntryList(
            entries: entries,
            selectedUuid: selectedEntryUuid,
            onEntrySelected: onEntrySelected,
            onCopyUsername: onCopyUsername,
            onCopyPassword: onCopyPassword,
          ),
        ),
        const VerticalDivider(thickness: 1, width: 1),
        Expanded(
          flex: 2,
          child: selectedEntryUuid != null
              ? EntryDetailPane(uuid: selectedEntryUuid!)
              : EmptyState(
                  icon: Icons.article_outlined,
                  title: l10n.emptyStateSelectEntry,
                  subtitle: l10n.emptyStateSelectEntrySubtitle,
                ),
        ),
      ],
    );
  }
}

class _CompactList extends StatelessWidget {
  const _CompactList({
    super.key,
    required this.entries,
    required this.onEntrySelected,
    required this.onCopyUsername,
    required this.onCopyPassword,
  });

  final List<EntrySummary> entries;
  final ValueChanged<String> onEntrySelected;
  final ValueChanged<String> onCopyUsername;
  final ValueChanged<String> onCopyPassword;

  @override
  Widget build(BuildContext context) {
    return EntryList(
      entries: entries,
      selectedUuid: null,
      onEntrySelected: onEntrySelected,
      onCopyUsername: onCopyUsername,
      onCopyPassword: onCopyPassword,
    );
  }
}

class _CompactDetail extends StatelessWidget {
  const _CompactDetail({super.key, required this.uuid, required this.onBack});

  final String uuid;
  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Align(
          alignment: Alignment.centerLeft,
          child: BackButton(onPressed: onBack),
        ),
        Expanded(child: EntryDetailPane(uuid: uuid)),
      ],
    );
  }
}

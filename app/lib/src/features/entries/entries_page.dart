import 'package:material_ui/material_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/models.dart';
import '../sync/sync_controller.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';
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

  @override
  void initState() {
    super.initState();
    if (widget.initialUuid != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          ref.read(selectedEntryProvider.notifier).select(widget.initialUuid);
        }
      });
    }
  }

  @override
  void didUpdateWidget(EntriesPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.initialUuid != null &&
        widget.initialUuid != oldWidget.initialUuid) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          ref.read(selectedEntryProvider.notifier).select(widget.initialUuid);
        }
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final treeAsync = ref.watch(vaultTreeProvider);
    final syncState = ref.watch(syncControllerProvider);
    final syncInFlight = syncState.valueOrNull?.inFlight ?? false;
    final mutationsEnabled = syncState.asData != null && !syncInFlight;
    final l10n = AppLocalizations.of(context)!;
    final width = MediaQuery.sizeOf(context).width;
    final isExpanded = width >= HidlinsBreakpoints.medium;
    final selectedEntryUuid = ref.watch(selectedEntryProvider);

    return treeAsync.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Center(child: Text(l10n.errorGeneric)),
      data: (tree) {
        final filteredEntries = _filterEntries(tree);
        late final Widget content;

        if (isExpanded) {
          content = _ExpandedLayout(
            tree: tree,
            entries: filteredEntries,
            selectedGroupUuid: _selectedGroupUuid,
            selectedEntryUuid: selectedEntryUuid,
            onGroupSelected: (uuid) =>
                setState(() => _selectedGroupUuid = uuid),
            onEntrySelected: _selectEntry,
            onCopyUsername: (uuid) => _copyField(uuid, CopyField.username),
            onCopyPassword: (uuid) => _copyField(uuid, CopyField.password),
            onEditEntry: mutationsEnabled ? _editEntry : null,
            onDeleteEntry: mutationsEnabled ? _deleteEntry : null,
            onNewEntry: mutationsEnabled ? _newEntry : null,
          );
        } else if (selectedEntryUuid != null) {
          content = _CompactDetail(
            uuid: selectedEntryUuid,
            onBack: () => _selectEntry(null),
          );
        } else {
          content = _CompactList(
            entries: filteredEntries,
            onEntrySelected: _selectEntry,
            onCopyUsername: (uuid) => _copyField(uuid, CopyField.username),
            onCopyPassword: (uuid) => _copyField(uuid, CopyField.password),
            onEditEntry: mutationsEnabled ? _editEntry : null,
            onDeleteEntry: mutationsEnabled ? _deleteEntry : null,
          );
        }

        return PopScope(
          canPop: isExpanded || selectedEntryUuid == null,
          onPopInvokedWithResult: (didPop, _) {
            if (!didPop && !isExpanded && selectedEntryUuid != null) {
              _selectEntry(null);
            }
          },
          child: Column(
            children: [
              if (syncInFlight)
                Material(
                  color: Theme.of(context).colorScheme.secondaryContainer,
                  child: ListTile(
                    dense: true,
                    leading: const SizedBox.square(
                      dimension: 20,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                    title: Text(l10n.syncStatusInProgress),
                    subtitle: Text(l10n.syncWorkspaceAvailable),
                  ),
                ),
              Expanded(child: content),
            ],
          ),
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
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.copiedSnackbar(30))));
    } on Exception {
      if (!mounted) return;
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.errorCopyFailed)));
    }
  }

  void _selectEntry(String? uuid) {
    ref.read(selectedEntryProvider.notifier).select(uuid);
  }

  Future<void> _newEntry([String? groupUuid]) async {
    if (!_canMutate()) return;
    final revision = ref
        .read(syncControllerProvider)
        .valueOrNull!
        .terminalRevision;
    final tree = await ref.read(vaultTreeProvider.future);
    if (!mounted || !_canMutate(revision)) return;
    await Navigator.of(context).push<void>(
      MaterialPageRoute(
        builder: (_) => EntryEditDialog(
          groupUuid: groupUuid ?? _selectedGroupUuid ?? tree.root.uuid,
        ),
      ),
    );
  }

  Future<void> _editEntry(String uuid) async {
    if (!_canMutate()) return;
    final revision = ref
        .read(syncControllerProvider)
        .valueOrNull!
        .terminalRevision;
    final detail = await ref.read(entryRepositoryProvider).entryDetail(uuid);
    if (!mounted || !_canMutate(revision)) return;
    final changed = await Navigator.of(context).push<bool>(
      MaterialPageRoute(builder: (_) => EntryEditDialog(detail: detail)),
    );
    if (mounted && changed == true) ref.invalidate(entryDetailProvider(uuid));
  }

  bool _canMutate([int? revision]) {
    final sync = ref.read(syncControllerProvider).asData?.value;
    return sync != null &&
        !sync.inFlight &&
        (revision == null || revision == sync.terminalRevision);
  }

  Future<void> _deleteEntry(String uuid) async {
    if (!_canMutate()) return;
    final revision = ref
        .read(syncControllerProvider)
        .valueOrNull!
        .terminalRevision;
    final l10n = AppLocalizations.of(context)!;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l10n.entryDeleteConfirmTitle),
        content: Text(l10n.entryDeleteConfirmMessage),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l10n.actionCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l10n.actionDelete),
          ),
        ],
      ),
    );
    if (confirmed == true && mounted && _canMutate(revision)) {
      await ref.read(entryRepositoryProvider).deleteEntry(uuid);
      if (mounted && ref.read(selectedEntryProvider) == uuid) {
        _selectEntry(null);
      }
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
    required this.onEditEntry,
    required this.onDeleteEntry,
    required this.onNewEntry,
  });

  final VaultTree tree;
  final List<EntrySummary> entries;
  final String? selectedGroupUuid;
  final String? selectedEntryUuid;
  final ValueChanged<String?> onGroupSelected;
  final ValueChanged<String> onEntrySelected;
  final ValueChanged<String> onCopyUsername;
  final ValueChanged<String> onCopyPassword;
  final ValueChanged<String>? onEditEntry;
  final ValueChanged<String>? onDeleteEntry;
  final ValueChanged<String>? onNewEntry;

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
            onNewEntry: onNewEntry,
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
            onEditEntry: onEditEntry,
            onDeleteEntry: onDeleteEntry,
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
    required this.entries,
    required this.onEntrySelected,
    required this.onCopyUsername,
    required this.onCopyPassword,
    required this.onEditEntry,
    required this.onDeleteEntry,
  });

  final List<EntrySummary> entries;
  final ValueChanged<String> onEntrySelected;
  final ValueChanged<String> onCopyUsername;
  final ValueChanged<String> onCopyPassword;
  final ValueChanged<String>? onEditEntry;
  final ValueChanged<String>? onDeleteEntry;

  @override
  Widget build(BuildContext context) {
    return EntryList(
      entries: entries,
      selectedUuid: null,
      onEntrySelected: onEntrySelected,
      onCopyUsername: onCopyUsername,
      onCopyPassword: onCopyPassword,
      onEditEntry: onEditEntry,
      onDeleteEntry: onDeleteEntry,
    );
  }
}

class _CompactDetail extends StatelessWidget {
  const _CompactDetail({required this.uuid, required this.onBack});

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

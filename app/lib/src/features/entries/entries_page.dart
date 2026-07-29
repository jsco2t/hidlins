import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';
import '../../ui/widgets/empty_state.dart';
import 'entry_detail.dart';
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
    final l10n = AppLocalizations.of(context)!;
    final width = MediaQuery.sizeOf(context).width;
    final isExpanded = width >= HidlinsBreakpoints.medium;

    return treeAsync.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Center(child: Text(l10n.errorGeneric)),
      data: (tree) {
        final filteredEntries = _filterEntries(tree);

        if (isExpanded) {
          return _ExpandedLayout(
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
          );
        }

        if (_selectedEntryUuid != null) {
          return _CompactDetail(
            uuid: _selectedEntryUuid!,
            onBack: () => setState(() => _selectedEntryUuid = null),
          );
        }

        return _CompactList(
          entries: filteredEntries,
          onEntrySelected: (uuid) => setState(() => _selectedEntryUuid = uuid),
          onCopyUsername: (uuid) => _copyField(uuid, CopyField.username),
          onCopyPassword: (uuid) => _copyField(uuid, CopyField.password),
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
      // ignore
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
  });

  final VaultTree tree;
  final List<EntrySummary> entries;
  final String? selectedGroupUuid;
  final String? selectedEntryUuid;
  final ValueChanged<String?> onGroupSelected;
  final ValueChanged<String> onEntrySelected;
  final ValueChanged<String> onCopyUsername;
  final ValueChanged<String> onCopyPassword;

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

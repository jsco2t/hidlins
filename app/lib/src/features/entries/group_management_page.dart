import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/lock_state_provider.dart';
import '../../providers/providers.dart';
import '../../ui/lock_guard.dart';
import '../../ui/tokens.dart';
import '../sync/sync_controller.dart';
import 'group_tree.dart';

typedef GroupsDeletedCallback = void Function(
  Set<String> groupUuids,
  Set<String> entryUuids,
);

class GroupManagementPage extends ConsumerStatefulWidget {
  const GroupManagementPage({
    super.key,
    this.selectedGroupUuid,
    required this.onDeleted,
  });

  final String? selectedGroupUuid;
  final GroupsDeletedCallback onDeleted;

  @override
  ConsumerState<GroupManagementPage> createState() =>
      _GroupManagementPageState();
}

class _GroupManagementPageState extends ConsumerState<GroupManagementPage> {
  bool _pending = false;
  String? _contextGroup;

  @override
  void initState() {
    super.initState();
    _contextGroup = widget.selectedGroupUuid;
  }

  bool _canMutate([int? revision]) {
    if (!mounted ||
        ref.read(lockStateProvider).asData?.value != LockEvent.unlocked) {
      return false;
    }
    final sync = ref.read(syncControllerProvider).asData?.value;
    return sync != null &&
        !sync.inFlight &&
        (revision == null || sync.terminalRevision == revision);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final tree = ref.watch(vaultTreeProvider);
    final sync = ref.watch(syncControllerProvider).asData?.value;
    final lock = ref.watch(lockStateProvider).asData?.value;
    final enabled =
        !_pending &&
        lock == LockEvent.unlocked &&
        sync != null &&
        !sync.inFlight;
    return LockGuard(
      child: Scaffold(
        appBar: AppBar(
          title: Text(l10n.groupsTitle),
          actions: [
            IconButton(
              tooltip: l10n.actionLock,
              icon: const Icon(Icons.lock_outline),
              onPressed: () =>
                  unawaited(ref.read(sessionRepositoryProvider).lockNow()),
            ),
          ],
        ),
        body: tree.when(
          loading: () => const Center(child: CircularProgressIndicator()),
          error: (_, _) => Center(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(l10n.errorGeneric),
                TextButton(
                  onPressed: () => ref.invalidate(vaultTreeProvider),
                  child: Text(l10n.actionRetry),
                ),
              ],
            ),
          ),
          data: (tree) => GroupTree(
            root: tree.root,
            selectedGroupUuid: _contextGroup,
            onGroupSelected: (uuid) =>
                Navigator.of(context).pop(uuid ?? tree.root.uuid),
            onGroupContextSelected: (uuid) =>
                setState(() => _contextGroup = uuid),
            onAction: enabled ? _runAction : null,
          ),
        ),
      ),
    );
  }

  GroupNode? _find(GroupNode root, String uuid) {
    if (root.uuid == uuid) return root;
    for (final child in root.children) {
      final found = _find(child, uuid);
      if (found != null) return found;
    }
    return null;
  }

  String? _parent(GroupNode root, String uuid) {
    for (final child in root.children) {
      if (child.uuid == uuid) return root.uuid;
      final found = _parent(child, uuid);
      if (found != null) return found;
    }
    return null;
  }

  Set<String> _subtree(GroupNode node) => {
    node.uuid,
    for (final child in node.children) ..._subtree(child),
  };

  void _error(String message) {
    if (mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(message)));
    }
  }

  Future<void> _runAction(GroupNode selected, GroupAction action) async {
    if (_pending || !_canMutate()) return;
    final revision = ref
        .read(syncControllerProvider)
        .asData!
        .value
        .terminalRevision;
    final onDeleted = widget.onDeleted;
    final l10n = AppLocalizations.of(context)!;
    setState(() => _pending = true);
    try {
      final tree = await ref.read(vaultTreeProvider.future);
      if (!mounted || !_canMutate(revision)) return;
      final node = _find(tree.root, selected.uuid);
      if (node == null) {
        _error(l10n.entryActionChanged);
        return;
      }
      if (node.uuid == tree.root.uuid &&
          (action == GroupAction.move || action == GroupAction.delete)) {
        return;
      }
      final repo = ref.read(entryRepositoryProvider);
      switch (action) {
        case GroupAction.create:
        case GroupAction.rename:
          final name = await showDialog<String>(
            context: context,
            builder: (_) => LockGuard(
              child: _GroupNameDialog(
                create: action == GroupAction.create,
                initial: action == GroupAction.rename ? node.name : '',
              ),
            ),
          );
          if (name == null || !_canMutate(revision)) return;
          if (action == GroupAction.create) {
            await repo.createGroup(node.uuid, name);
          } else {
            await repo.renameGroup(node.uuid, name);
          }
        case GroupAction.move:
          final excluded = _subtree(node);
          final parent = _parent(tree.root, node.uuid);
          if (parent != null) excluded.add(parent);
          final destination = await showDialog<String>(
            context: context,
            builder: (_) => LockGuard(
              child: _GroupMoveDialog(root: tree.root, excluded: excluded),
            ),
          );
          if (destination == null || !_canMutate(revision)) return;
          await repo.moveGroup(node.uuid, destination);
        case GroupAction.delete:
          final behavior = await showDialog<GroupDeleteBehavior>(
            context: context,
            builder: (_) => LockGuard(child: _GroupDeleteDialog(group: node)),
          );
          if (behavior == null || !_canMutate(revision)) return;
          final groups = _subtree(node);
          final entries = {
            for (final entry in tree.entries)
              if (groups.contains(entry.groupUuid)) entry.uuid,
          };
          await repo.deleteGroup(node.uuid, behavior);
          // Navigation belongs to the page behind this route. Notify it even
          // when our successful mutation refreshes or disposes the dialog UI.
          onDeleted(groups, entries);
          if (mounted && groups.contains(_contextGroup)) _contextGroup = null;
      }
    } on Exception {
      _error(l10n.errorGeneric);
    } finally {
      if (mounted) setState(() => _pending = false);
    }
  }
}

class _GroupNameDialog extends StatefulWidget {
  const _GroupNameDialog({required this.create, required this.initial});
  final bool create;
  final String initial;

  @override
  State<_GroupNameDialog> createState() => _GroupNameDialogState();
}

class _GroupNameDialogState extends State<_GroupNameDialog> {
  final _form = GlobalKey<FormState>();
  late final _name = TextEditingController(text: widget.initial);

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return AlertDialog(
      title: Text(
        widget.create ? l10n.groupCreateTitle : l10n.groupRenameTitle,
      ),
      content: Form(
        key: _form,
        child: TextFormField(
          controller: _name,
          autofocus: true,
          decoration: InputDecoration(labelText: l10n.groupNameLabel),
          validator: (value) => value == null || value.trim().isEmpty
              ? l10n.validatorRequired
              : null,
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.actionCancel),
        ),
        FilledButton(
          onPressed: () {
            if (_form.currentState!.validate()) {
              Navigator.of(context).pop(_name.text);
            }
          },
          child: Text(widget.create ? l10n.actionCreate : l10n.actionSave),
        ),
      ],
    );
  }
}

class _GroupMoveDialog extends StatefulWidget {
  const _GroupMoveDialog({required this.root, required this.excluded});
  final GroupNode root;
  final Set<String> excluded;

  @override
  State<_GroupMoveDialog> createState() => _GroupMoveDialogState();
}

class _GroupMoveDialogState extends State<_GroupMoveDialog> {
  String? _destination;

  Iterable<DropdownMenuItem<String>> _items(GroupNode node, String path) sync* {
    if (!widget.excluded.contains(node.uuid)) {
      yield DropdownMenuItem(value: node.uuid, child: Text(path));
    }
    for (final child in node.children) {
      yield* _items(child, '$path / ${child.name}');
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final destinations = _items(widget.root, widget.root.name).toList();
    return AlertDialog(
      title: Text(l10n.groupMoveTitle),
      content: SizedBox(
        width: 400,
        child: destinations.isEmpty
            ? Text(l10n.groupMoveNoDestinations)
            : DropdownButtonFormField<String>(
                isExpanded: true,
                itemHeight: null,
                decoration: InputDecoration(
                  labelText: l10n.entryMoveDestinationLabel,
                ),
                items: destinations,
                onChanged: (value) => setState(() => _destination = value),
              ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.actionCancel),
        ),
        FilledButton(
          onPressed: _destination == null
              ? null
              : () => Navigator.of(context).pop(_destination),
          child: Text(l10n.actionMove),
        ),
      ],
    );
  }
}

class _GroupDeleteDialog extends StatefulWidget {
  const _GroupDeleteDialog({required this.group});
  final GroupNode group;

  @override
  State<_GroupDeleteDialog> createState() => _GroupDeleteDialogState();
}

class _GroupDeleteDialogState extends State<_GroupDeleteDialog> {
  GroupDeleteBehavior _behavior = GroupDeleteBehavior.refuse;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final nonempty =
        widget.group.entryCount != BigInt.zero ||
        widget.group.children.isNotEmpty;
    final blocked = nonempty && _behavior == GroupDeleteBehavior.refuse;
    return AlertDialog(
      title: Text(l10n.groupDeleteTitle),
      content: SizedBox(
        width: 400,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(widget.group.name),
            const SizedBox(height: HidlinsSpacing.md),
            DropdownButtonFormField<GroupDeleteBehavior>(
              initialValue: GroupDeleteBehavior.refuse,
              isExpanded: true,
              itemHeight: null,
              decoration: InputDecoration(labelText: l10n.groupDeleteBehavior),
              items: [
                DropdownMenuItem(
                  value: GroupDeleteBehavior.refuse,
                  child: Text(l10n.groupDeleteOnlyEmpty),
                ),
                DropdownMenuItem(
                  value: GroupDeleteBehavior.recurse,
                  child: Text(l10n.groupDeleteSubtree),
                ),
              ],
              onChanged: (value) {
                if (value != null) setState(() => _behavior = value);
              },
            ),
            if (blocked) ...[
              const SizedBox(height: HidlinsSpacing.sm),
              Text(l10n.groupDeleteNonempty),
            ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.actionCancel),
        ),
        FilledButton(
          onPressed: blocked
              ? null
              : () => Navigator.of(context).pop(_behavior),
          child: Text(l10n.actionDelete),
        ),
      ],
    );
  }
}

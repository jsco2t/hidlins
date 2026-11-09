import 'package:material_ui/material_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/failures.dart';
import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../platform/attachment_export.dart';
import '../../platform/platform_result.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';
import '../../ui/lock_guard.dart';
import '../../ui/widgets/copy_row.dart';
import '../../ui/widgets/section_card.dart';
import '../../ui/widgets/secret_text.dart';
import '../entries/totp_block.dart';
import 'entry_edit.dart';
import 'entry_history.dart';
import 'entry_move_dialog.dart';
import '../sync/sync_controller.dart';

typedef FilePickerCallback = Future<String?> Function();
typedef EntryMovedCallback = void Function(String uuid, String groupUuid);

class EntryDetailPane extends ConsumerWidget {
  const EntryDetailPane({
    super.key,
    required this.uuid,
    this.onPickFile,
    this.onMoved,
    this.onPurged,
  });

  final String uuid;
  final FilePickerCallback? onPickFile;
  // The page owns navigation after a mutation, even if cache refresh replaces
  // this detail before its native call completes.
  final EntryMovedCallback? onMoved;
  final ValueChanged<String>? onPurged;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final detailAsync = ref.watch(entryDetailProvider(uuid));
    final sync = ref.watch(syncControllerProvider).asData?.value;
    final mutationsEnabled =
        sync != null && !sync.inFlight && !detailAsync.isLoading;
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;

    return detailAsync.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Center(
        child: Text(
          l10n.errorGeneric,
          style: TextStyle(color: colorScheme.error),
        ),
      ),
      data: (detail) => _DetailContent(
        key: ValueKey((uuid, detail)),
        detail: detail,
        uuid: uuid,
        onPickFile: onPickFile,
        onMoved: onMoved,
        onPurged: onPurged,
        mutationsEnabled: mutationsEnabled,
      ),
    );
  }
}

class _DetailContent extends ConsumerStatefulWidget {
  const _DetailContent({
    super.key,
    required this.detail,
    required this.uuid,
    this.onPickFile,
    this.onMoved,
    this.onPurged,
    required this.mutationsEnabled,
  });

  final EntryDetail detail;
  final String uuid;
  final FilePickerCallback? onPickFile;
  final EntryMovedCallback? onMoved;
  final ValueChanged<String>? onPurged;
  final bool mutationsEnabled;

  @override
  ConsumerState<_DetailContent> createState() => _DetailContentState();
}

class _DetailContentState extends ConsumerState<_DetailContent> {
  final _passwordKey = GlobalKey<SecretTextState>();
  bool _actionPending = false;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final detail = widget.detail;

    return ListView(
      padding: const EdgeInsets.all(HidlinsSpacing.md),
      children: [
        // Header
        Row(
          children: [
            Expanded(
              child: Semantics(
                headingLevel: 1,
                child: Text(
                  detail.title,
                  style: Theme.of(context).textTheme.headlineSmall,
                ),
              ),
            ),
            if (detail.expiryTime != null)
              Padding(
                padding: const EdgeInsets.only(left: HidlinsSpacing.sm),
                child: _ExpirationBadge(
                  expiryTime: detail.expiryTime!,
                  isExpired: DateTime.fromMillisecondsSinceEpoch(
                    detail.expiryTime! * 1000,
                  ).isBefore(DateTime.now()),
                ),
              ),
            IconButton(
              icon: const Icon(Icons.edit_outlined),
              tooltip: l10n.actionEdit,
              onPressed: widget.mutationsEnabled && !_actionPending
                  ? _openEdit
                  : null,
            ),
          ],
        ),
        Wrap(
          spacing: HidlinsSpacing.xs,
          runSpacing: HidlinsSpacing.xs,
          children: [
            TextButton.icon(
              onPressed: _actionPending
                  ? null
                  : () => EntryHistorySheet.show(context, widget.uuid),
              icon: const Icon(Icons.history),
              label: Text(l10n.entryHistoryTitle),
            ),
            TextButton.icon(
              onPressed: widget.mutationsEnabled && !_actionPending
                  ? _moveEntry
                  : null,
              icon: const Icon(Icons.drive_file_move_outline),
              label: Text(l10n.entryMoveTitle),
            ),
            TextButton.icon(
              onPressed: widget.mutationsEnabled && !_actionPending
                  ? _purgeEntry
                  : null,
              icon: const Icon(Icons.delete_forever_outlined),
              label: Text(l10n.entryPurgeConfirmTitle),
            ),
          ],
        ),
        const SizedBox(height: HidlinsSpacing.md),

        // Username
        if (detail.username.isNotEmpty)
          CopyRow(
            label: l10n.entryDetailUsername,
            value: detail.username,
            onCopy: () => _copy(CopyField.username),
            copyTooltip: l10n.actionCopy,
          ),

        // Password
        if (detail.hasPassword)
          SectionCard(
            title: l10n.entryDetailPassword,
            child: Row(
              children: [
                Expanded(
                  child: SecretText(
                    key: _passwordKey,
                    onReveal: () => _reveal(RevealField.password),
                  ),
                ),
                IconButton(
                  icon: Icon(
                    _passwordKey.currentState?.isRevealed == true
                        ? Icons.visibility_off
                        : Icons.visibility,
                  ),
                  tooltip: _passwordKey.currentState?.isRevealed == true
                      ? l10n.actionHide
                      : l10n.actionReveal,
                  onPressed: () async {
                    final state = _passwordKey.currentState;
                    if (state == null) return;
                    if (state.isRevealed) {
                      state.hide();
                    } else {
                      await state.reveal();
                    }
                    if (mounted) setState(() {});
                  },
                ),
                IconButton(
                  icon: const Icon(Icons.copy, size: 18),
                  tooltip: l10n.actionCopy,
                  onPressed: () async => _copy(CopyField.password),
                ),
              ],
            ),
          ),

        // URL
        if (detail.url.isNotEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: HidlinsSpacing.md,
              vertical: HidlinsSpacing.sm,
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  l10n.entryDetailUrl,
                  style: Theme.of(context).textTheme.labelSmall?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
                ),
                const SizedBox(height: 2),
                SelectableText(
                  detail.url,
                  style: Theme.of(context).textTheme.bodyMedium,
                ),
              ],
            ),
          ),

        // TOTP
        if (detail.kind == EntryKindDto.totp) TotpBlock(uuid: widget.uuid),

        // Notes
        if (detail.notes.isNotEmpty)
          SectionCard(
            title: l10n.entryDetailNotes,
            child: SelectableText(detail.notes),
          ),

        // Tags
        if (detail.tags.isNotEmpty)
          SectionCard(
            title: l10n.entryDetailTags,
            child: Wrap(
              spacing: HidlinsSpacing.sm,
              runSpacing: HidlinsSpacing.xs,
              children: detail.tags
                  .map(
                    (tag) => Chip(
                      label: Text(tag),
                      visualDensity: VisualDensity.compact,
                    ),
                  )
                  .toList(),
            ),
          ),

        // Custom fields
        if (detail.customFields.isNotEmpty)
          SectionCard(
            title: l10n.entryDetailCustomFields,
            child: Column(
              children: detail.customFields.map((field) {
                return _CustomFieldRow(
                  name: field.name,
                  isProtected: field.isProtected,
                  uuid: widget.uuid,
                );
              }).toList(),
            ),
          ),

        // Attachments
        _AttachmentSection(
          uuid: widget.uuid,
          attachments: detail.attachments,
          onPickFile: widget.onPickFile,
          mutationsEnabled: widget.mutationsEnabled,
        ),

        // Timestamps
        SectionCard(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (detail.creationTime != null)
                _TimestampRow(
                  label: l10n.entryDetailCreated,
                  epochSecs: detail.creationTime!,
                ),
              if (detail.lastModificationTime != null)
                _TimestampRow(
                  label: l10n.entryDetailModified,
                  epochSecs: detail.lastModificationTime!,
                ),
            ],
          ),
        ),
      ],
    );
  }

  Future<String?> _reveal(RevealField field) async {
    try {
      final repo = ref.read(secretsRepositoryProvider);
      return await repo.revealField(widget.uuid, field);
    } on Exception {
      if (!mounted) return null;
      final l10n = AppLocalizations.of(context)!;
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.errorRevealFailed)));
      return null;
    }
  }

  Future<void> _openEdit() async {
    final sync = ref.read(syncControllerProvider).asData?.value;
    if (_actionPending ||
        !widget.mutationsEnabled ||
        sync == null ||
        sync.inFlight) {
      return;
    }
    final detail = await ref
        .read(entryRepositoryProvider)
        .entryDetail(widget.uuid);
    if (!mounted || _actionPending || !widget.mutationsEnabled) return;
    final current = ref.read(syncControllerProvider).asData?.value;
    if (current == null ||
        current.inFlight ||
        current.terminalRevision != sync.terminalRevision) {
      return;
    }
    final changed = await Navigator.of(context).push<bool>(
      MaterialPageRoute(builder: (_) => EntryEditDialog(detail: detail)),
    );
    if (mounted && changed == true) {
      ref.invalidate(entryDetailProvider(widget.uuid));
    }
  }

  bool _canMutate([int? revision]) {
    if (!mounted || !widget.mutationsEnabled) return false;
    final sync = ref.read(syncControllerProvider).asData?.value;
    return sync != null &&
        !sync.inFlight &&
        (revision == null || sync.terminalRevision == revision);
  }

  void _showActionError(String message) {
    if (!mounted) return;
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(message)));
  }

  Future<void> _moveEntry() async {
    if (_actionPending || !_canMutate()) return;
    final revision = ref
        .read(syncControllerProvider)
        .asData!
        .value
        .terminalRevision;
    final uuid = widget.uuid;
    final onMoved = widget.onMoved;
    final l10n = AppLocalizations.of(context)!;
    setState(() => _actionPending = true);
    try {
      final tree = await ref.read(vaultTreeProvider.future);
      if (!mounted || !_canMutate(revision)) return;
      String? currentGroup;
      for (final entry in tree.entries) {
        if (entry.uuid == uuid) {
          currentGroup = entry.groupUuid;
          break;
        }
      }
      final destination = await showDialog<String>(
        context: context,
        builder: (_) => LockGuard(
          child: EntryMoveDialog(
            root: tree.root,
            currentGroupUuid: currentGroup,
          ),
        ),
      );
      if (destination == null || !mounted) return;
      if (!_canMutate(revision)) {
        _showActionError(l10n.entryActionChanged);
        return;
      }
      await ref.read(entryRepositoryProvider).moveEntry(uuid, destination);
      onMoved?.call(uuid, destination);
    } on Exception {
      _showActionError(l10n.errorGeneric);
    } finally {
      if (mounted) setState(() => _actionPending = false);
    }
  }

  Future<void> _purgeEntry() async {
    if (_actionPending || !_canMutate()) return;
    final revision = ref
        .read(syncControllerProvider)
        .asData!
        .value
        .terminalRevision;
    final uuid = widget.uuid;
    final onPurged = widget.onPurged;
    final l10n = AppLocalizations.of(context)!;
    setState(() => _actionPending = true);
    try {
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (context) => LockGuard(
          child: AlertDialog(
            title: Text(l10n.entryPurgeConfirmTitle),
            content: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(widget.detail.title),
                const SizedBox(height: HidlinsSpacing.sm),
                Text(l10n.entryPurgeConfirmMessage),
              ],
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(context).pop(false),
                child: Text(l10n.actionCancel),
              ),
              FilledButton(
                style: FilledButton.styleFrom(
                  backgroundColor: Theme.of(context).colorScheme.error,
                  foregroundColor: Theme.of(context).colorScheme.onError,
                ),
                onPressed: () => Navigator.of(context).pop(true),
                child: Text(l10n.actionDelete),
              ),
            ],
          ),
        ),
      );
      if (confirmed != true || !mounted) return;
      if (!_canMutate(revision)) {
        _showActionError(l10n.entryActionChanged);
        return;
      }
      await ref.read(entryRepositoryProvider).purgeEntry(uuid);
      if (onPurged != null) {
        onPurged(uuid);
      } else if (mounted && ref.read(selectedEntryProvider) == uuid) {
        ref.read(selectedEntryProvider.notifier).select(null);
      }
    } on Exception {
      _showActionError(l10n.errorGeneric);
    } finally {
      if (mounted) setState(() => _actionPending = false);
    }
  }

  Future<void> _copy(CopyField field) async {
    final l10n = AppLocalizations.of(context)!;
    try {
      final repo = ref.read(secretsRepositoryProvider);
      await repo.copyEntryField(widget.uuid, field);
      if (!mounted) return;
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.copiedSnackbar(30))));
    } on Exception {
      if (!mounted) return;
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text(l10n.errorGeneric)));
    }
  }
}

class _CustomFieldRow extends ConsumerStatefulWidget {
  const _CustomFieldRow({
    required this.name,
    required this.isProtected,
    required this.uuid,
  });

  final String name;
  final bool isProtected;
  final String uuid;

  @override
  ConsumerState<_CustomFieldRow> createState() => _CustomFieldRowState();
}

class _CustomFieldRowState extends ConsumerState<_CustomFieldRow> {
  final _secretKey = GlobalKey<SecretTextState>();

  @override
  Widget build(BuildContext context) {
    final name = widget.name;
    final uuid = widget.uuid;
    final isProtected = widget.isProtected;
    final l10n = AppLocalizations.of(context)!;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: HidlinsSpacing.xs),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  name,
                  style: Theme.of(context).textTheme.labelSmall?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
                ),
                const SizedBox(height: 2),
                if (isProtected)
                  SecretText(
                    key: _secretKey,
                    onReveal: () async {
                      try {
                        final repo = ref.read(secretsRepositoryProvider);
                        return await repo.revealField(
                          uuid,
                          RevealFieldCustom(name),
                        );
                      } on Exception {
                        if (!mounted) return null;
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(content: Text(l10n.errorRevealFailed)),
                        );
                        return null;
                      }
                    },
                  )
                else
                  FutureBuilder<String>(
                    future: ref
                        .read(secretsRepositoryProvider)
                        .revealField(uuid, RevealFieldCustom(name)),
                    builder: (context, snap) => Text(
                      snap.data ?? '…',
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                  ),
              ],
            ),
          ),
          if (isProtected)
            IconButton(
              icon: Icon(
                _secretKey.currentState?.isRevealed == true
                    ? Icons.visibility_off
                    : Icons.visibility,
              ),
              tooltip: _secretKey.currentState?.isRevealed == true
                  ? l10n.actionHide
                  : l10n.actionReveal,
              onPressed: () async {
                final secret = _secretKey.currentState;
                if (secret == null) return;
                if (secret.isRevealed) {
                  secret.hide();
                } else {
                  await secret.reveal();
                }
                if (mounted) setState(() {});
              },
            ),
          IconButton(
            icon: const Icon(Icons.copy, size: 18),
            tooltip: l10n.actionCopy,
            onPressed: () async {
              try {
                final repo = ref.read(secretsRepositoryProvider);
                await repo.copyEntryField(uuid, CopyFieldCustom(name));
                if (!context.mounted) return;
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(content: Text(l10n.copiedSnackbar(30))),
                );
              } on Exception {
                if (!context.mounted) return;
                ScaffoldMessenger.of(
                  context,
                ).showSnackBar(SnackBar(content: Text(l10n.errorCopyFailed)));
              }
            },
          ),
        ],
      ),
    );
  }
}

class _AttachmentSection extends ConsumerStatefulWidget {
  const _AttachmentSection({
    required this.uuid,
    required this.attachments,
    this.onPickFile,
    required this.mutationsEnabled,
  });

  final String uuid;
  final List<AttachmentMeta> attachments;
  final FilePickerCallback? onPickFile;
  final bool mutationsEnabled;

  @override
  ConsumerState<_AttachmentSection> createState() => _AttachmentSectionState();
}

class _AttachmentSectionState extends ConsumerState<_AttachmentSection> {
  String? _error;
  bool _attaching = false;

  bool _supportsExport(BuildContext context) {
    final platform = Theme.of(context).platform;
    return platform == TargetPlatform.macOS || platform == TargetPlatform.linux;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;

    return SectionCard(
      title: l10n.entryDetailAttachments,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ...widget.attachments.map(
            (a) => ListTile(
              leading: const Icon(Icons.attach_file, size: 20),
              title: Text(a.name),
              subtitle: Text(l10n.attachmentBytes('${a.sizeBytes}')),
              dense: true,
              contentPadding: EdgeInsets.zero,
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (_supportsExport(context))
                    IconButton(
                      icon: const Icon(Icons.save_alt, size: 18),
                      tooltip: l10n.attachmentExportAction(a.name),
                      onPressed: () async => _export(a.name),
                    ),
                  IconButton(
                    icon: const Icon(Icons.close, size: 18),
                    tooltip: l10n.actionDetach,
                    onPressed: widget.mutationsEnabled
                        ? () async => _confirmDetach(a.name)
                        : null,
                  ),
                ],
              ),
            ),
          ),
          if (_supportsExport(context) && widget.attachments.isNotEmpty)
            Text(
              l10n.attachmentExportWarning,
              style: Theme.of(context).textTheme.bodySmall
                  ?.copyWith(color: colorScheme.onSurfaceVariant),
            ),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: HidlinsSpacing.sm),
              child: Text(_error!, style: TextStyle(color: colorScheme.error)),
            ),
          Padding(
            padding: const EdgeInsets.only(top: HidlinsSpacing.sm),
            child: TextButton.icon(
              onPressed: widget.mutationsEnabled && !_attaching
                  ? _attach
                  : null,
              icon: const Icon(Icons.add, size: 18),
              label: Text(l10n.actionAttach),
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _attach() async {
    if (_attaching || !_canMutate()) return;
    final revision = ref
        .read(syncControllerProvider)
        .asData!
        .value
        .terminalRevision;
    final uuid = widget.uuid;
    setState(() {
      _attaching = true;
      _error = null;
    });
    try {
      String? path;
      final picker = widget.onPickFile;
      if (picker != null) {
        path = await picker();
      } else {
        final selection = await ref
            .read(attachmentExportCapabilityProvider)
            .chooseSource();
        if (!mounted || widget.uuid != uuid || !_canMutate(revision)) return;
        switch (selection) {
          case PlatformSuccess<String>(:final value):
            path = value;
          case PlatformCanceled<String>():
            return;
          default:
            setState(() {
              _error = AppLocalizations.of(context)!.attachmentSourceFailed;
            });
            return;
        }
      }
      if (path == null ||
          !mounted ||
          widget.uuid != uuid ||
          !_canMutate(revision)) {
        return;
      }
      final repo = ref.read(entryRepositoryProvider);
      await repo.addAttachment(uuid, path);
      if (mounted) setState(() => _error = null);
    } on AppFailure catch (e) {
      if (!mounted) return;
      final l10n = AppLocalizations.of(context)!;
      setState(() {
        _error = switch (e) {
          InvalidInputFailure(field: 'attachment') =>
            l10n.attachmentSizeExceeded,
          _ => l10n.errorGeneric,
        };
      });
    } on Exception {
      if (!mounted) return;
      setState(() {
        _error = AppLocalizations.of(context)!.errorGeneric;
      });
    } finally {
      if (mounted) setState(() => _attaching = false);
    }
  }

  Future<void> _export(String key) async {
    final l10n = AppLocalizations.of(context)!;
    final capability = ref.read(attachmentExportCapabilityProvider);
    final result = await capability.chooseDestination(
      sanitizeAttachmentSuggestedName(key),
    );
    if (!mounted) return;

    final destination = switch (result) {
      PlatformSuccess<String>(:final value) => value,
      PlatformCanceled<String>() => null,
      _ => '',
    };
    if (destination == null) return;
    if (destination.isEmpty) {
      setState(() => _error = l10n.attachmentExportFailed);
      return;
    }

    try {
      await ref
          .read(entryRepositoryProvider)
          .saveAttachmentTo(widget.uuid, key, destination);
      if (!mounted) return;
      setState(() => _error = null);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.attachmentExportSuccess(key))),
      );
    } on Exception {
      if (!mounted) return;
      setState(() => _error = l10n.attachmentExportFailed);
    }
  }

  Future<void> _confirmDetach(String key) async {
    if (!_canMutate()) return;
    final revision = ref
        .read(syncControllerProvider)
        .asData!
        .value
        .terminalRevision;
    final l10n = AppLocalizations.of(context)!;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.actionDetach),
        content: Text(l10n.attachmentDetachConfirm),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.actionCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(l10n.actionConfirm),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted || !_canMutate(revision)) return;

    try {
      final repo = ref.read(entryRepositoryProvider);
      await repo.removeAttachment(widget.uuid, key);
    } on Exception {
      if (!mounted) return;
      setState(() {
        _error = AppLocalizations.of(context)!.errorGeneric;
      });
    }
  }

  bool _canMutate([int? revision]) {
    final sync = ref.read(syncControllerProvider).asData?.value;
    return widget.mutationsEnabled &&
        sync != null &&
        !sync.inFlight &&
        (revision == null || revision == sync.terminalRevision);
  }
}

class _TimestampRow extends StatelessWidget {
  const _TimestampRow({required this.label, required this.epochSecs});

  final String label;
  final int epochSecs;

  @override
  Widget build(BuildContext context) {
    final dt = DateTime.fromMillisecondsSinceEpoch(epochSecs * 1000);
    final formatted =
        '${dt.year}-${dt.month.toString().padLeft(2, '0')}-${dt.day.toString().padLeft(2, '0')} ${dt.hour.toString().padLeft(2, '0')}:${dt.minute.toString().padLeft(2, '0')}';
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Wrap(
        spacing: HidlinsSpacing.sm,
        runSpacing: HidlinsSpacing.xs,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Text(
            label,
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
          Text(formatted, style: Theme.of(context).textTheme.bodySmall),
        ],
      ),
    );
  }
}

class _ExpirationBadge extends StatelessWidget {
  const _ExpirationBadge({required this.expiryTime, required this.isExpired});

  final int expiryTime;
  final bool isExpired;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;

    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: HidlinsSpacing.sm,
        vertical: 2,
      ),
      decoration: BoxDecoration(
        color: isExpired
            ? colorScheme.errorContainer
            : colorScheme.tertiaryContainer,
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        isExpired ? l10n.entryDetailExpired : l10n.entryDetailExpires,
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
          color: isExpired
              ? colorScheme.onErrorContainer
              : colorScheme.onTertiaryContainer,
        ),
      ),
    );
  }
}

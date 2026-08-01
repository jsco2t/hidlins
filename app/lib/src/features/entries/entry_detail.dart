import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/failures.dart';
import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';
import '../../ui/widgets/copy_row.dart';
import '../../ui/widgets/section_card.dart';
import '../../ui/widgets/secret_text.dart';
import '../entries/totp_block.dart';
import 'entry_edit.dart';

typedef FilePickerCallback = Future<String?> Function();

class EntryDetailPane extends ConsumerWidget {
  const EntryDetailPane({super.key, required this.uuid, this.onPickFile});

  final String uuid;
  final FilePickerCallback? onPickFile;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final detailAsync = ref.watch(entryDetailProvider(uuid));
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
        detail: detail,
        uuid: uuid,
        onPickFile: onPickFile,
        mutationsEnabled: !detailAsync.isRefreshing && !detailAsync.isReloading,
      ),
    );
  }
}

class _DetailContent extends ConsumerStatefulWidget {
  const _DetailContent({
    required this.detail,
    required this.uuid,
    required this.mutationsEnabled,
    this.onPickFile,
  });

  final EntryDetail detail;
  final String uuid;
  final bool mutationsEnabled;
  final FilePickerCallback? onPickFile;

  @override
  ConsumerState<_DetailContent> createState() => _DetailContentState();
}

class _DetailContentState extends ConsumerState<_DetailContent> {
  final _passwordKey = GlobalKey<SecretTextState>();

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final detail = widget.detail;
    final syncState = ref.watch(syncUiStateProvider);
    final syncUnavailable = !syncState.hasValue;
    final isSyncing = syncState.valueOrNull?.inFlight ?? false;
    final mutationsEnabled =
        widget.mutationsEnabled && !syncUnavailable && !isSyncing;
    final mutationTooltip = syncUnavailable || isSyncing
        ? l10n.syncActionsDisabled
        : !widget.mutationsEnabled
        ? l10n.entryRefreshInProgress
        : l10n.actionEdit;

    return ListView(
      padding: const EdgeInsets.all(HidlinsSpacing.md),
      children: [
        // Header
        Row(
          children: [
            Expanded(
              child: Text(
                detail.title,
                style: Theme.of(context).textTheme.headlineSmall,
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
            Tooltip(
              message: mutationTooltip,
              child: IconButton(
                key: const Key('entry-edit-action'),
                icon: const Icon(Icons.edit_outlined),
                onPressed: mutationsEnabled ? () => _openEdit(detail) : null,
              ),
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
                    setState(() {});
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
          mutationsEnabled: mutationsEnabled,
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
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.errorRevealFailed)));
      return null;
    }
  }

  Future<void> _copy(CopyField field) async {
    final l10n = AppLocalizations.of(context)!;
    try {
      final repo = ref.read(secretsRepositoryProvider);
      await repo.copyEntryField(widget.uuid, field);
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

  Future<void> _openEdit(EntryDetail detail) async {
    final currentDetail = ref.read(entryDetailProvider(widget.uuid));
    final syncState = ref.read(syncUiStateProvider);
    if (currentDetail.isRefreshing ||
        currentDetail.isReloading ||
        !syncState.hasValue ||
        (syncState.valueOrNull?.inFlight ?? false)) {
      return;
    }
    final changed = await Navigator.of(context).push<bool>(
      MaterialPageRoute(builder: (_) => EntryEditDialog(detail: detail)),
    );
    if (changed == true) {
      ref.invalidate(vaultTreeProvider);
      ref.invalidate(entryDetailProvider(widget.uuid));
    }
  }
}

class _CustomFieldRow extends ConsumerWidget {
  const _CustomFieldRow({
    required this.name,
    required this.isProtected,
    required this.uuid,
  });

  final String name;
  final bool isProtected;
  final String uuid;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
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
                    onReveal: () async {
                      try {
                        final repo = ref.read(secretsRepositoryProvider);
                        return await repo.revealField(
                          uuid,
                          RevealFieldCustom(name),
                        );
                      } on Exception {
                        if (!context.mounted) return null;
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(
                            content: Text(
                              AppLocalizations.of(context)!.errorRevealFailed,
                            ),
                          ),
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
    required this.mutationsEnabled,
    this.onPickFile,
  });

  final String uuid;
  final List<AttachmentMeta> attachments;
  final bool mutationsEnabled;
  final FilePickerCallback? onPickFile;

  @override
  ConsumerState<_AttachmentSection> createState() => _AttachmentSectionState();
}

class _AttachmentSectionState extends ConsumerState<_AttachmentSection> {
  String? _error;

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
              trailing: Tooltip(
                message: widget.mutationsEnabled
                    ? l10n.actionDetach
                    : l10n.syncActionsDisabled,
                child: IconButton(
                  key: Key('attachment-detach-${a.name}'),
                  icon: const Icon(Icons.close, size: 18),
                  onPressed: widget.mutationsEnabled
                      ? () async => _confirmDetach(a.name)
                      : null,
                ),
              ),
            ),
          ),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: HidlinsSpacing.sm),
              child: Text(_error!, style: TextStyle(color: colorScheme.error)),
            ),
          Padding(
            padding: const EdgeInsets.only(top: HidlinsSpacing.sm),
            child: TextButton.icon(
              key: const Key('attachment-add-action'),
              onPressed: widget.mutationsEnabled ? _attach : null,
              icon: const Icon(Icons.add, size: 18),
              label: Text(l10n.actionAttach),
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _attach() async {
    final picker = widget.onPickFile;
    if (picker == null) return;

    final path = await picker();
    if (path == null || !mounted) return;

    try {
      final repo = ref.read(entryRepositoryProvider);
      await repo.addAttachment(widget.uuid, path);
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
    }
  }

  Future<void> _confirmDetach(String key) async {
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
    if (confirmed != true || !mounted) return;

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
      child: Row(
        children: [
          Text(
            label,
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: Theme.of(context).colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(width: HidlinsSpacing.sm),
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

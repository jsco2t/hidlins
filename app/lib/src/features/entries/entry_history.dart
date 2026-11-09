import 'package:material_ui/material_ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';
import '../../ui/lock_guard.dart';

class EntryHistorySheet extends ConsumerStatefulWidget {
  const EntryHistorySheet({
    super.key,
    required this.uuid,
    this.scrollController,
  });

  final String uuid;
  final ScrollController? scrollController;

  static Future<void> show(BuildContext context, String uuid) {
    if (MediaQuery.sizeOf(context).width >= HidlinsBreakpoints.medium) {
      return showDialog<void>(
        context: context,
        builder: (_) => Dialog(
          alignment: Alignment.centerRight,
          insetPadding: EdgeInsets.zero,
          child: SizedBox(
            width: 400,
            height: double.infinity,
            child: LockGuard(child: EntryHistorySheet(uuid: uuid)),
          ),
        ),
      );
    }
    return showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      useSafeArea: true,
      builder: (_) => DraggableScrollableSheet(
        expand: false,
        initialChildSize: 0.5,
        maxChildSize: 0.9,
        builder: (context, scrollController) => LockGuard(
          child: EntryHistorySheet(
            uuid: uuid,
            scrollController: scrollController,
          ),
        ),
      ),
    );
  }

  @override
  ConsumerState<EntryHistorySheet> createState() => _EntryHistorySheetState();
}

class _EntryHistorySheetState extends ConsumerState<EntryHistorySheet> {
  late Future<List<HistorySummary>> _historyFuture;

  @override
  void initState() {
    super.initState();
    final repo = ref.read(entryRepositoryProvider);
    _historyFuture = repo.entryHistory(widget.uuid);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.all(HidlinsSpacing.md),
          child: Row(
            children: [
              Expanded(
                child: Text(
                  l10n.entryHistoryTitle,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ),
              const CloseButton(),
            ],
          ),
        ),
        const Divider(height: 1),
        Expanded(child: _historyBody(l10n)),
      ],
    );
  }

  Widget _historyBody(AppLocalizations l10n) {
    return FutureBuilder<List<HistorySummary>>(
      future: _historyFuture,
      builder: (context, snapshot) {
        if (snapshot.connectionState == ConnectionState.waiting) {
          return const Center(child: CircularProgressIndicator());
        }

        if (snapshot.hasError) {
          return Center(
            child: Padding(
              padding: const EdgeInsets.all(HidlinsSpacing.xl),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(l10n.errorGeneric),
                  const SizedBox(height: HidlinsSpacing.sm),
                  TextButton(
                    onPressed: () => setState(() {
                      _historyFuture = ref
                          .read(entryRepositoryProvider)
                          .entryHistory(widget.uuid);
                    }),
                    child: Text(l10n.actionRetry),
                  ),
                ],
              ),
            ),
          );
        }

        final history = snapshot.data ?? [];
        if (history.isEmpty) {
          return Center(
            child: Padding(
              padding: const EdgeInsets.all(HidlinsSpacing.xl),
              child: Text(
                l10n.entryHistoryEmpty,
                style: Theme.of(context).textTheme.bodyLarge,
              ),
            ),
          );
        }

        return ListView.separated(
          controller: widget.scrollController,
          itemCount: history.length,
          separatorBuilder: (_, _) => const Divider(height: 1),
          itemBuilder: (context, index) {
            final item = history[index];
            final versionNumber = history.length - index;
            return ListTile(
              leading: CircleAvatar(
                radius: 16,
                child: Text(
                  l10n.entryHistoryVersion(versionNumber),
                  style: const TextStyle(fontSize: 11),
                ),
              ),
              title: Text(item.title),
              subtitle: Text(item.username),
              trailing: item.lastModificationTime != null
                  ? Text(
                      _formatDate(item.lastModificationTime!),
                      style: Theme.of(context).textTheme.labelSmall,
                    )
                  : null,
            );
          },
        );
      },
    );
  }

  String _formatDate(int epochSecs) {
    final dt = DateTime.fromMillisecondsSinceEpoch(epochSecs * 1000);
    return '${dt.year}-${dt.month.toString().padLeft(2, '0')}-${dt.day.toString().padLeft(2, '0')}';
  }
}

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';

class EntryHistorySheet extends ConsumerStatefulWidget {
  const EntryHistorySheet({super.key, required this.uuid});

  final String uuid;

  static Future<void> show(BuildContext context, String uuid) {
    return showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      builder: (_) => DraggableScrollableSheet(
        expand: false,
        initialChildSize: 0.5,
        maxChildSize: 0.9,
        builder: (context, scrollController) => EntryHistorySheet(uuid: uuid),
      ),
    );
  }

  @override
  ConsumerState<EntryHistorySheet> createState() => _EntryHistorySheetState();
}

class _EntryHistorySheetState extends ConsumerState<EntryHistorySheet> {
  late final Future<List<HistorySummary>> _historyFuture;

  @override
  void initState() {
    super.initState();
    final repo = ref.read(entryRepositoryProvider);
    _historyFuture = repo.entryHistory(widget.uuid);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    return FutureBuilder<List<HistorySummary>>(
      future: _historyFuture,
      builder: (context, snapshot) {
        if (snapshot.connectionState == ConnectionState.waiting) {
          return const Center(child: CircularProgressIndicator());
        }

        final history = snapshot.data ?? [];
        if (history.isEmpty) {
          return Center(
            child: Padding(
              padding: const EdgeInsets.all(HidlinsSpacing.xl),
              child: Text(
                l10n.emptyStateNoEntries,
                style: Theme.of(context).textTheme.bodyLarge,
              ),
            ),
          );
        }

        return Column(
          children: [
            Padding(
              padding: const EdgeInsets.all(HidlinsSpacing.md),
              child: Text(
                l10n.entryHistoryTitle,
                style: Theme.of(context).textTheme.titleMedium,
              ),
            ),
            const Divider(height: 1),
            Expanded(
              child: ListView.separated(
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
              ),
            ),
          ],
        );
      },
    );
  }

  String _formatDate(int epochSecs) {
    final dt = DateTime.fromMillisecondsSinceEpoch(epochSecs * 1000);
    return '${dt.year}-${dt.month.toString().padLeft(2, '0')}-${dt.day.toString().padLeft(2, '0')}';
  }
}

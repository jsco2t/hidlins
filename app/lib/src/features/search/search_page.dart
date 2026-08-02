import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/activity_capture.dart';
import '../../ui/tokens.dart';
import '../../ui/widgets/empty_state.dart';
import '../../ui/widgets/entry_avatar.dart';

class SearchPage extends ConsumerStatefulWidget {
  const SearchPage({super.key});

  @override
  ConsumerState<SearchPage> createState() => _SearchPageState();
}

class _SearchPageState extends ConsumerState<SearchPage> {
  final _searchController = TextEditingController();
  final _focusNode = FocusNode();
  Timer? _debounce;
  SearchModeDto _mode = SearchModeDto.substring;
  bool _includeRecycled = false;
  List<SearchHit> _results = [];
  bool _searching = false;
  bool _hasSearched = false;

  @override
  void dispose() {
    _searchController.dispose();
    _focusNode.dispose();
    _debounce?.cancel();
    super.dispose();
  }

  void _onQueryChanged(String query) {
    ActivityCapture.reportTextInput(context);
    _debounce?.cancel();
    if (query.isEmpty) {
      setState(() {
        _results = [];
        _hasSearched = false;
      });
      return;
    }
    _debounce = Timer(const Duration(milliseconds: 300), () => _search(query));
  }

  Future<void> _search(String query) async {
    if (query.isEmpty) return;
    setState(() => _searching = true);

    try {
      final repo = ref.read(searchRepositoryProvider);
      final results = await repo.search(
        SearchOptionsDto(
          query: query,
          mode: _mode,
          includeRecycled: _includeRecycled,
        ),
      );
      if (mounted) {
        setState(() {
          _results = results;
          _searching = false;
          _hasSearched = true;
        });
      }
    } on Exception {
      if (mounted) {
        setState(() {
          _searching = false;
          _hasSearched = true;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return Scaffold(
      body: Column(
        children: [
          // Search bar
          Padding(
            padding: const EdgeInsets.all(HidlinsSpacing.md),
            child: Column(
              children: [
                Semantics(
                  label: l10n.searchHintText,
                  child: SearchBar(
                    controller: _searchController,
                    focusNode: _focusNode,
                    autoFocus: true,
                    hintText: l10n.searchHintText,
                    leading: const Icon(Icons.search),
                    trailing: [
                      if (_searchController.text.isNotEmpty)
                        IconButton(
                          icon: const Icon(Icons.close),
                          tooltip: l10n.actionClose,
                          onPressed: () {
                            _searchController.clear();
                            _onQueryChanged('');
                          },
                        ),
                    ],
                    onChanged: _onQueryChanged,
                  ),
                ),
                const SizedBox(height: HidlinsSpacing.sm),
                Row(
                  children: [
                    Expanded(
                      child: SegmentedButton<SearchModeDto>(
                        segments: [
                          ButtonSegment(
                            value: SearchModeDto.substring,
                            label: Text(l10n.searchModeSubstring),
                          ),
                          ButtonSegment(
                            value: SearchModeDto.fuzzy,
                            label: Text(l10n.searchModeFuzzy),
                          ),
                          ButtonSegment(
                            value: SearchModeDto.wildcard,
                            label: Text(l10n.searchModeWildcard),
                          ),
                        ],
                        selected: {_mode},
                        onSelectionChanged: (s) async {
                          setState(() => _mode = s.first);
                          if (_searchController.text.isNotEmpty) {
                            await _search(_searchController.text);
                          }
                        },
                      ),
                    ),
                    const SizedBox(width: HidlinsSpacing.sm),
                    FilterChip(
                      label: Text(l10n.searchIncludeRecycled),
                      selected: _includeRecycled,
                      onSelected: (v) async {
                        setState(() => _includeRecycled = v);
                        if (_searchController.text.isNotEmpty) {
                          await _search(_searchController.text);
                        }
                      },
                    ),
                  ],
                ),
              ],
            ),
          ),

          // Results
          if (_searching)
            const Padding(
              padding: EdgeInsets.all(HidlinsSpacing.lg),
              child: CircularProgressIndicator(),
            )
          else if (_hasSearched && _results.isEmpty)
            Expanded(
              child: EmptyState(
                icon: Icons.search_off,
                title: l10n.emptyStateNoResults,
              ),
            )
          else
            Expanded(
              child: ListView.builder(
                itemCount: _results.length,
                itemBuilder: (context, index) {
                  final hit = _results[index];
                  return _SearchResultRow(
                    hit: hit,
                    query: _searchController.text,
                    mode: _mode,
                  );
                },
              ),
            ),
        ],
      ),
    );
  }
}

class _SearchResultRow extends StatelessWidget {
  const _SearchResultRow({
    required this.hit,
    required this.query,
    required this.mode,
  });

  final SearchHit hit;
  final String query;
  final SearchModeDto mode;

  @override
  Widget build(BuildContext context) {
    final entry = hit.entry;
    final titleMatch = hit.matches
        .where((m) => m.field == MatchedFieldDto.title)
        .firstOrNull;

    return ListTile(
      onTap: () => context.go('/entries/${entry.uuid}'),
      leading: EntryAvatar(title: entry.title),
      title: titleMatch != null && titleMatch.ranges.isNotEmpty
          ? _HighlightedText(text: entry.title, ranges: titleMatch.ranges)
          : Text(entry.title),
      subtitle: entry.username.isNotEmpty ? Text(entry.username) : null,
      trailing: entry.isExpired
          ? Icon(
              Icons.warning_amber,
              color: Theme.of(context).colorScheme.error,
              size: 18,
            )
          : null,
    );
  }
}

class _HighlightedText extends StatelessWidget {
  const _HighlightedText({required this.text, required this.ranges});

  final String text;
  final List<(int, int)> ranges;

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;
    final spans = <TextSpan>[];
    int cursor = 0;

    for (final (start, end) in ranges) {
      if (start > cursor) {
        spans.add(TextSpan(text: text.substring(cursor, start)));
      }
      final clampedEnd = end.clamp(0, text.length);
      final clampedStart = start.clamp(0, text.length);
      if (clampedStart < clampedEnd) {
        spans.add(
          TextSpan(
            text: text.substring(clampedStart, clampedEnd),
            style: TextStyle(
              backgroundColor: colorScheme.primaryContainer,
              color: colorScheme.onPrimaryContainer,
              fontWeight: FontWeight.w600,
            ),
          ),
        );
      }
      cursor = clampedEnd;
    }

    if (cursor < text.length) {
      spans.add(TextSpan(text: text.substring(cursor)));
    }

    return RichText(
      text: TextSpan(
        style: Theme.of(context).textTheme.bodyMedium,
        children: spans,
      ),
      overflow: TextOverflow.ellipsis,
    );
  }
}

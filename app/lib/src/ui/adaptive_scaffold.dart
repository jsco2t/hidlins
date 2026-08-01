import 'package:flutter/material.dart';

import 'tokens.dart';
import '../l10n/app_localizations.dart';

class AdaptiveScaffold extends StatefulWidget {
  const AdaptiveScaffold({
    super.key,
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.body,
    this.appBar,
    this.secondaryBody,
    this.initialListPaneWidth,
    this.onListPaneWidthChanged,
  });

  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final Widget body;
  final PreferredSizeWidget? appBar;
  final Widget? secondaryBody;
  final double? initialListPaneWidth;
  final ValueChanged<double>? onListPaneWidthChanged;

  @override
  State<AdaptiveScaffold> createState() => _AdaptiveScaffoldState();
}

class _AdaptiveScaffoldState extends State<AdaptiveScaffold> {
  late double _listPaneWidth;
  bool _userDragged = false;
  static const _defaultListWidth = 320.0;
  static const _minListWidth = 240.0;
  static const _maxListWidth = 480.0;

  @override
  void initState() {
    super.initState();
    _listPaneWidth = (widget.initialListPaneWidth ?? _defaultListWidth).clamp(
      _minListWidth,
      _maxListWidth,
    );
  }

  @override
  void didUpdateWidget(AdaptiveScaffold oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!_userDragged &&
        widget.initialListPaneWidth != null &&
        oldWidget.initialListPaneWidth != widget.initialListPaneWidth) {
      _listPaneWidth = widget.initialListPaneWidth!.clamp(
        _minListWidth,
        _maxListWidth,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.sizeOf(context).width;
    final l10n = AppLocalizations.of(context)!;

    final destinations = [
      _Destination(Icons.key, l10n.navEntries),
      _Destination(Icons.search, l10n.navSearch),
      _Destination(Icons.password, l10n.navGenerator),
      _Destination(Icons.sync, l10n.navSync),
      _Destination(Icons.settings, l10n.navSettings),
    ];

    if (width < HidlinsBreakpoints.compact) {
      return _CompactLayout(
        selectedIndex: widget.selectedIndex,
        onDestinationSelected: widget.onDestinationSelected,
        destinations: destinations,
        body: widget.body,
        appBar: widget.appBar,
      );
    }

    if (width < HidlinsBreakpoints.medium) {
      return _MediumLayout(
        selectedIndex: widget.selectedIndex,
        onDestinationSelected: widget.onDestinationSelected,
        destinations: destinations,
        body: widget.body,
        appBar: widget.appBar,
        secondaryBody: widget.secondaryBody,
      );
    }

    return _ExpandedLayout(
      selectedIndex: widget.selectedIndex,
      onDestinationSelected: widget.onDestinationSelected,
      destinations: destinations,
      body: widget.body,
      appBar: widget.appBar,
      secondaryBody: widget.secondaryBody,
      listPaneWidth: _listPaneWidth,
      onDividerDrag: (dx) {
        setState(() {
          _userDragged = true;
          _listPaneWidth = (_listPaneWidth + dx).clamp(
            _minListWidth,
            _maxListWidth,
          );
        });
      },
      onDividerDragEnd: () {
        widget.onListPaneWidthChanged?.call(_listPaneWidth);
      },
    );
  }
}

class _Destination {
  const _Destination(this.icon, this.label);
  final IconData icon;
  final String label;
}

class _CompactLayout extends StatelessWidget {
  const _CompactLayout({
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.destinations,
    required this.body,
    this.appBar,
  });

  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final List<_Destination> destinations;
  final Widget body;
  final PreferredSizeWidget? appBar;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: appBar,
      body: body,
      bottomNavigationBar: NavigationBar(
        selectedIndex: selectedIndex,
        onDestinationSelected: onDestinationSelected,
        destinations: [
          for (final d in destinations)
            NavigationDestination(icon: Icon(d.icon), label: d.label),
        ],
      ),
    );
  }
}

class _MediumLayout extends StatelessWidget {
  const _MediumLayout({
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.destinations,
    required this.body,
    this.appBar,
    this.secondaryBody,
  });

  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final List<_Destination> destinations;
  final Widget body;
  final PreferredSizeWidget? appBar;
  final Widget? secondaryBody;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: appBar,
      body: Row(
        children: [
          NavigationRail(
            selectedIndex: selectedIndex,
            onDestinationSelected: onDestinationSelected,
            labelType: NavigationRailLabelType.all,
            destinations: [
              for (final d in destinations)
                NavigationRailDestination(
                  icon: Icon(d.icon),
                  label: Text(d.label),
                ),
            ],
          ),
          const VerticalDivider(thickness: 1, width: 1),
          Expanded(child: body),
          if (secondaryBody != null) ...[
            const VerticalDivider(thickness: 1, width: 1),
            Expanded(child: secondaryBody!),
          ],
        ],
      ),
    );
  }
}

class _ExpandedLayout extends StatelessWidget {
  const _ExpandedLayout({
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.destinations,
    required this.body,
    this.appBar,
    this.secondaryBody,
    required this.listPaneWidth,
    required this.onDividerDrag,
    this.onDividerDragEnd,
  });

  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final List<_Destination> destinations;
  final Widget body;
  final PreferredSizeWidget? appBar;
  final Widget? secondaryBody;
  final double listPaneWidth;
  final void Function(double dx) onDividerDrag;
  final VoidCallback? onDividerDragEnd;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: appBar,
      body: Row(
        children: [
          NavigationRail(
            selectedIndex: selectedIndex,
            onDestinationSelected: onDestinationSelected,
            extended: true,
            destinations: [
              for (final d in destinations)
                NavigationRailDestination(
                  icon: Icon(d.icon),
                  label: Text(d.label),
                ),
            ],
          ),
          const VerticalDivider(thickness: 1, width: 1),
          SizedBox(width: listPaneWidth, child: body),
          _DraggableDivider(onDrag: onDividerDrag, onDragEnd: onDividerDragEnd),
          Expanded(child: secondaryBody ?? const SizedBox.shrink()),
        ],
      ),
    );
  }
}

class _DraggableDivider extends StatelessWidget {
  const _DraggableDivider({required this.onDrag, this.onDragEnd});

  final void Function(double dx) onDrag;
  final VoidCallback? onDragEnd;

  @override
  Widget build(BuildContext context) {
    return MouseRegion(
      cursor: SystemMouseCursors.resizeColumn,
      child: GestureDetector(
        onHorizontalDragUpdate: (details) => onDrag(details.delta.dx),
        onHorizontalDragEnd: (_) => onDragEnd?.call(),
        child: const SizedBox(
          width: 8,
          child: VerticalDivider(thickness: 1, width: 1),
        ),
      ),
    );
  }
}

import 'package:flutter/material.dart';

class ActivityCapture extends StatefulWidget {
  const ActivityCapture({
    super.key,
    required this.onActivity,
    this.onLifecycleStateChange,
    required this.child,
  });

  final VoidCallback onActivity;
  final void Function(AppLifecycleState state)? onLifecycleStateChange;
  final Widget child;

  static void reportTextInput(BuildContext context) {
    context
        .findAncestorStateOfType<_ActivityCaptureState>()
        ?._reportTextInputActivity();
  }

  @override
  State<ActivityCapture> createState() => _ActivityCaptureState();
}

class _ActivityCaptureState extends State<ActivityCapture> {
  final _focusNode = FocusNode();
  late final AppLifecycleListener _lifecycleListener;

  void _reportTextInputActivity() {
    widget.onActivity();
  }

  @override
  void initState() {
    super.initState();
    _lifecycleListener = AppLifecycleListener(
      onStateChange: (state) {
        widget.onLifecycleStateChange?.call(state);
      },
    );
  }

  @override
  void dispose() {
    _lifecycleListener.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      onPointerDown: (_) => widget.onActivity(),
      onPointerMove: (_) => widget.onActivity(),
      onPointerSignal: (_) => widget.onActivity(),
      behavior: HitTestBehavior.translucent,
      child: KeyboardListener(
        focusNode: _focusNode,
        autofocus: true,
        onKeyEvent: (_) => widget.onActivity(),
        child: widget.child,
      ),
    );
  }
}

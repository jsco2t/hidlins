import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';

class SearchShortcutIntent extends Intent {
  const SearchShortcutIntent();
}

class NewEntryShortcutIntent extends Intent {
  const NewEntryShortcutIntent();
}

class LockShortcutIntent extends Intent {
  const LockShortcutIntent();
}

class CopyPasswordShortcutIntent extends Intent {
  const CopyPasswordShortcutIntent();
}

class GeneratorShortcutIntent extends Intent {
  const GeneratorShortcutIntent();
}

class DismissShortcutIntent extends Intent {
  const DismissShortcutIntent();
}

/// The single, reviewable desktop shortcut map.
///
/// Unmodified character shortcuts deliberately deactivate while an editable
/// control owns focus so typing a password, title, or search query is never
/// interpreted as an application command.
class DesktopShortcuts extends StatelessWidget {
  const DesktopShortcuts({
    super.key,
    required this.onSearch,
    required this.onNewEntry,
    required this.onLock,
    required this.onCopyPassword,
    required this.onGenerator,
    required this.onDismiss,
    required this.child,
  });

  final VoidCallback onSearch;
  final VoidCallback onNewEntry;
  final VoidCallback onLock;
  final VoidCallback onCopyPassword;
  final VoidCallback onGenerator;
  final VoidCallback onDismiss;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Shortcuts(
      shortcuts: <ShortcutActivator, Intent>{
        const SingleActivator(LogicalKeyboardKey.keyF, control: true):
            const SearchShortcutIntent(),
        const SingleActivator(LogicalKeyboardKey.keyF, meta: true):
            const SearchShortcutIntent(),
        const _NonTextActivator(SingleActivator(LogicalKeyboardKey.slash)):
            const SearchShortcutIntent(),
        const _NonTextActivator(SingleActivator(LogicalKeyboardKey.keyN)):
            const NewEntryShortcutIntent(),
        const _NonTextActivator(SingleActivator(LogicalKeyboardKey.keyL)):
            const LockShortcutIntent(),
        const SingleActivator(LogicalKeyboardKey.keyC, control: true):
            const CopyPasswordShortcutIntent(),
        const SingleActivator(LogicalKeyboardKey.keyC, meta: true):
            const CopyPasswordShortcutIntent(),
        const _NonTextActivator(SingleActivator(LogicalKeyboardKey.keyG)):
            const GeneratorShortcutIntent(),
        const SingleActivator(LogicalKeyboardKey.escape):
            const DismissShortcutIntent(),
      },
      child: Actions(
        actions: <Type, Action<Intent>>{
          SearchShortcutIntent: CallbackAction<SearchShortcutIntent>(
            onInvoke: (_) => onSearch(),
          ),
          NewEntryShortcutIntent: CallbackAction<NewEntryShortcutIntent>(
            onInvoke: (_) => onNewEntry(),
          ),
          LockShortcutIntent: CallbackAction<LockShortcutIntent>(
            onInvoke: (_) => onLock(),
          ),
          CopyPasswordShortcutIntent:
              CallbackAction<CopyPasswordShortcutIntent>(
                onInvoke: (_) => onCopyPassword(),
              ),
          GeneratorShortcutIntent: CallbackAction<GeneratorShortcutIntent>(
            onInvoke: (_) => onGenerator(),
          ),
          DismissShortcutIntent: CallbackAction<DismissShortcutIntent>(
            onInvoke: (_) => onDismiss(),
          ),
        },
        child: Focus(autofocus: true, child: child),
      ),
    );
  }
}

class _NonTextActivator extends ShortcutActivator {
  const _NonTextActivator(this.delegate);

  final SingleActivator delegate;

  @override
  Iterable<LogicalKeyboardKey> get triggers => delegate.triggers;

  @override
  bool accepts(KeyEvent event, HardwareKeyboard state) {
    if (_editableHasFocus()) return false;
    return delegate.accepts(event, state);
  }

  @override
  String debugDescribeKeys() => delegate.debugDescribeKeys();

  static bool _editableHasFocus() {
    final context = FocusManager.instance.primaryFocus?.context;
    return context?.widget is EditableText ||
        context?.findAncestorWidgetOfExactType<EditableText>() != null;
  }
}

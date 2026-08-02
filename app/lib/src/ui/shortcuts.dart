import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Opens the global search surface.
class SearchVaultIntent extends Intent {
  const SearchVaultIntent();
}

/// Opens the new-entry flow for the currently selected group.
class NewEntryIntent extends Intent {
  const NewEntryIntent();
}

/// Locks the active vault immediately.
class LockVaultIntent extends Intent {
  const LockVaultIntent();
}

/// Copies the selected entry's password through the Rust clipboard door.
class CopySelectedPasswordIntent extends Intent {
  const CopySelectedPasswordIntent();
}

/// Dismisses the top-most route or overlay.
class DismissSurfaceIntent extends Intent {
  const DismissSurfaceIntent();
}

const Map<ShortcutActivator, Intent> hidlinsShortcutBindings = {
  SingleActivator(LogicalKeyboardKey.keyF, control: true): SearchVaultIntent(),
  SingleActivator(LogicalKeyboardKey.keyF, meta: true): SearchVaultIntent(),
  SingleActivator(LogicalKeyboardKey.keyN): NewEntryIntent(),
  SingleActivator(LogicalKeyboardKey.keyL): LockVaultIntent(),
  SingleActivator(LogicalKeyboardKey.keyC, control: true):
      CopySelectedPasswordIntent(),
  SingleActivator(LogicalKeyboardKey.keyC, meta: true):
      CopySelectedPasswordIntent(),
  SingleActivator(LogicalKeyboardKey.escape): DismissSurfaceIntent(),
};

/// Global desktop shortcut dispatcher.
///
/// Entry-specific actions are supplied closer to [EntriesPage], so the same
/// bindings do nothing on surfaces where they do not make sense. Plain-letter
/// actions are disabled while an editable text control owns focus; typing an
/// `l` in an edit form must never lock the vault.
class HidlinsShortcuts extends StatelessWidget {
  const HidlinsShortcuts({
    super.key,
    required this.onSearch,
    required this.onLock,
    required this.onDismiss,
    required this.child,
  });

  final VoidCallback onSearch;
  final VoidCallback onLock;
  final VoidCallback onDismiss;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Shortcuts(
      shortcuts: hidlinsShortcutBindings,
      child: Actions(
        actions: {
          SearchVaultIntent: CallbackAction<SearchVaultIntent>(
            onInvoke: (_) => onSearch(),
          ),
          LockVaultIntent: NonEditingCallbackAction<LockVaultIntent>(
            onInvoke: (_) => onLock(),
          ),
          DismissSurfaceIntent: CallbackAction<DismissSurfaceIntent>(
            onInvoke: (_) => onDismiss(),
          ),
        },
        child: Focus(child: child),
      ),
    );
  }
}

/// An action that yields to [EditableText] controls.
class NonEditingCallbackAction<T extends Intent> extends CallbackAction<T> {
  NonEditingCallbackAction({required super.onInvoke});

  @override
  bool isEnabled(T intent) {
    final focusContext = FocusManager.instance.primaryFocus?.context;
    final editing =
        focusContext?.widget is EditableText ||
        focusContext?.findAncestorWidgetOfExactType<EditableText>() != null;
    return !editing && super.isEnabled(intent);
  }
}

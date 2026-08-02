import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/activity_capture.dart';
import '../../ui/tokens.dart';
import '../../ui/widgets/password_text.dart';

enum _GeneratorMode { random, diceware }

class GeneratorPage extends ConsumerStatefulWidget {
  const GeneratorPage({super.key, this.onUse});

  final ValueChanged<String>? onUse;

  @override
  ConsumerState<GeneratorPage> createState() => _GeneratorPageState();
}

class _GeneratorPageState extends ConsumerState<GeneratorPage>
    with SingleTickerProviderStateMixin {
  _GeneratorMode _mode = _GeneratorMode.random;

  int _length = 20;
  bool _lowercase = true;
  bool _uppercase = true;
  bool _digits = true;
  bool _symbols = true;
  bool _excludeAmbiguous = false;

  int _wordCount = 6;
  final _separatorCtrl = TextEditingController(text: '-');

  GeneratedSecret? _result;
  bool _generating = false;
  late AnimationController _rotateCtrl;

  @override
  void initState() {
    super.initState();
    _rotateCtrl = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 400),
    );
    unawaited(_generate());
  }

  @override
  void dispose() {
    _separatorCtrl.dispose();
    _rotateCtrl.dispose();
    super.dispose();
  }

  Future<void> _generate() async {
    setState(() => _generating = true);
    unawaited(_rotateCtrl.forward(from: 0));

    try {
      final repo = ref.read(generatorRepositoryProvider);
      final result = _mode == _GeneratorMode.random
          ? await repo.generatePassword(
              PasswordOptionsDto(
                length: _length,
                lowercase: _lowercase,
                uppercase: _uppercase,
                digits: _digits,
                symbols: _symbols,
                excludeAmbiguous: _excludeAmbiguous,
              ),
            )
          : await repo.generatePassphrase(
              PassphraseOptionsDto(
                words: _wordCount,
                separator: _separatorCtrl.text.isEmpty
                    ? '-'
                    : _separatorCtrl.text,
              ),
            );
      if (mounted) setState(() => _result = result);
    } on Exception {
      // ignore generation failures
    } finally {
      if (mounted) setState(() => _generating = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;

    return Scaffold(
      body: Padding(
        padding: const EdgeInsets.all(HidlinsSpacing.md),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              l10n.generatorTitle,
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: HidlinsSpacing.lg),

            // Generated output
            Card(
              color: colorScheme.surfaceContainerLow,
              child: Padding(
                padding: const EdgeInsets.all(HidlinsSpacing.lg),
                child: Column(
                  children: [
                    if (_result != null)
                      PasswordText(password: _result!.value, fontSize: 20)
                    else if (_generating)
                      const CircularProgressIndicator()
                    else
                      const SizedBox(height: 28),
                    const SizedBox(height: HidlinsSpacing.md),
                    if (_result != null)
                      _EntropyMeter(bits: _result!.entropyBits),
                  ],
                ),
              ),
            ),
            const SizedBox(height: HidlinsSpacing.md),

            // Action buttons
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                MediaQuery.disableAnimationsOf(context)
                    ? IconButton.filled(
                        icon: const Icon(Icons.refresh),
                        tooltip: l10n.generatorRegenerate,
                        onPressed: _generating ? null : _generate,
                      )
                    : RotationTransition(
                        key: const Key('generator-rotation'),
                        turns: _rotateCtrl,
                        child: IconButton.filled(
                          icon: const Icon(Icons.refresh),
                          tooltip: l10n.generatorRegenerate,
                          onPressed: _generating ? null : _generate,
                        ),
                      ),
                const SizedBox(width: HidlinsSpacing.md),
                IconButton.filled(
                  icon: const Icon(Icons.copy),
                  tooltip: l10n.generatorCopy,
                  onPressed: _result == null ? null : _copyResult,
                ),
                if (widget.onUse != null) ...[
                  const SizedBox(width: HidlinsSpacing.md),
                  FilledButton(
                    onPressed: _result == null
                        ? null
                        : () => widget.onUse!(_result!.value),
                    child: Text(l10n.actionUse),
                  ),
                ],
              ],
            ),
            const SizedBox(height: HidlinsSpacing.lg),

            // Mode toggle
            SegmentedButton<_GeneratorMode>(
              segments: [
                ButtonSegment(
                  value: _GeneratorMode.random,
                  label: Text(l10n.generatorRandom),
                ),
                ButtonSegment(
                  value: _GeneratorMode.diceware,
                  label: Text(l10n.generatorDiceware),
                ),
              ],
              selected: {_mode},
              onSelectionChanged: (s) async {
                setState(() => _mode = s.first);
                await _generate();
              },
            ),
            const SizedBox(height: HidlinsSpacing.md),

            // Options
            Expanded(
              child: SingleChildScrollView(
                child: _mode == _GeneratorMode.random
                    ? _RandomOptions(
                        length: _length,
                        onLengthChanged: (v) async {
                          setState(() => _length = v);
                          await _generate();
                        },
                        lowercase: _lowercase,
                        onLowercaseChanged: (v) async {
                          setState(() => _lowercase = v);
                          await _generate();
                        },
                        uppercase: _uppercase,
                        onUppercaseChanged: (v) async {
                          setState(() => _uppercase = v);
                          await _generate();
                        },
                        digits: _digits,
                        onDigitsChanged: (v) async {
                          setState(() => _digits = v);
                          await _generate();
                        },
                        symbols: _symbols,
                        onSymbolsChanged: (v) async {
                          setState(() => _symbols = v);
                          await _generate();
                        },
                        excludeAmbiguous: _excludeAmbiguous,
                        onExcludeAmbiguousChanged: (v) async {
                          setState(() => _excludeAmbiguous = v);
                          await _generate();
                        },
                      )
                    : _DicewareOptions(
                        wordCount: _wordCount,
                        onWordCountChanged: (v) async {
                          setState(() => _wordCount = v);
                          await _generate();
                        },
                        separatorCtrl: _separatorCtrl,
                        onSeparatorChanged: () async => _generate(),
                      ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _copyResult() async {
    if (_result == null) return;
    final l10n = AppLocalizations.of(context)!;
    try {
      final repo = ref.read(secretsRepositoryProvider);
      await repo.copyEntryField('generator', CopyField.password);
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(l10n.copiedSnackbar(30))));
      }
    } on Exception {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.errorCopyFailed)));
    }
  }
}

class _RandomOptions extends StatelessWidget {
  const _RandomOptions({
    required this.length,
    required this.onLengthChanged,
    required this.lowercase,
    required this.onLowercaseChanged,
    required this.uppercase,
    required this.onUppercaseChanged,
    required this.digits,
    required this.onDigitsChanged,
    required this.symbols,
    required this.onSymbolsChanged,
    required this.excludeAmbiguous,
    required this.onExcludeAmbiguousChanged,
  });

  final int length;
  final ValueChanged<int> onLengthChanged;
  final bool lowercase;
  final ValueChanged<bool> onLowercaseChanged;
  final bool uppercase;
  final ValueChanged<bool> onUppercaseChanged;
  final bool digits;
  final ValueChanged<bool> onDigitsChanged;
  final bool symbols;
  final ValueChanged<bool> onSymbolsChanged;
  final bool excludeAmbiguous;
  final ValueChanged<bool> onExcludeAmbiguousChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Text(l10n.generatorLength),
            const SizedBox(width: HidlinsSpacing.sm),
            Text('$length', style: Theme.of(context).textTheme.titleMedium),
          ],
        ),
        Semantics(
          label: '${l10n.generatorLength}: $length',
          child: Slider(
            value: length.toDouble(),
            min: 4,
            max: 128,
            divisions: 124,
            onChanged: (v) => onLengthChanged(v.round()),
          ),
        ),
        const SizedBox(height: HidlinsSpacing.sm),
        Wrap(
          spacing: HidlinsSpacing.sm,
          runSpacing: HidlinsSpacing.xs,
          children: [
            FilterChip(
              label: Text(l10n.generatorLowercase),
              selected: lowercase,
              onSelected: onLowercaseChanged,
            ),
            FilterChip(
              label: Text(l10n.generatorUppercase),
              selected: uppercase,
              onSelected: onUppercaseChanged,
            ),
            FilterChip(
              label: Text(l10n.generatorDigits),
              selected: digits,
              onSelected: onDigitsChanged,
            ),
            FilterChip(
              label: Text(l10n.generatorSymbols),
              selected: symbols,
              onSelected: onSymbolsChanged,
            ),
            FilterChip(
              label: Text(l10n.generatorExcludeAmbiguous),
              selected: excludeAmbiguous,
              onSelected: onExcludeAmbiguousChanged,
            ),
          ],
        ),
      ],
    );
  }
}

class _DicewareOptions extends StatelessWidget {
  const _DicewareOptions({
    required this.wordCount,
    required this.onWordCountChanged,
    required this.separatorCtrl,
    required this.onSeparatorChanged,
  });

  final int wordCount;
  final ValueChanged<int> onWordCountChanged;
  final TextEditingController separatorCtrl;
  final VoidCallback onSeparatorChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Text(l10n.generatorWords),
            const SizedBox(width: HidlinsSpacing.sm),
            Text('$wordCount', style: Theme.of(context).textTheme.titleMedium),
          ],
        ),
        Semantics(
          label: '${l10n.generatorWords}: $wordCount',
          child: Slider(
            value: wordCount.toDouble(),
            min: 3,
            max: 12,
            divisions: 9,
            onChanged: (v) => onWordCountChanged(v.round()),
          ),
        ),
        const SizedBox(height: HidlinsSpacing.md),
        SizedBox(
          width: 120,
          child: TextField(
            controller: separatorCtrl,
            decoration: InputDecoration(
              labelText: l10n.generatorSeparator,
              border: const OutlineInputBorder(),
              isDense: true,
            ),
            onChanged: (_) {
              ActivityCapture.reportTextInput(context);
              onSeparatorChanged();
            },
          ),
        ),
      ],
    );
  }
}

class _EntropyMeter extends StatelessWidget {
  const _EntropyMeter({required this.bits});

  final double bits;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final colorScheme = Theme.of(context).colorScheme;
    final fraction = (bits / 128).clamp(0.0, 1.0);
    final color = bits < 40
        ? colorScheme.error
        : bits < 60
        ? Colors.orange
        : bits < 80
        ? Colors.amber
        : colorScheme.primary;

    return Column(
      children: [
        LinearProgressIndicator(
          value: fraction,
          color: color,
          backgroundColor: colorScheme.surfaceContainerHighest,
        ),
        const SizedBox(height: HidlinsSpacing.xs),
        Text(
          l10n.generatorEntropy(bits.toStringAsFixed(1)),
          style: Theme.of(context).textTheme.labelSmall?.copyWith(color: color),
        ),
      ],
    );
  }
}

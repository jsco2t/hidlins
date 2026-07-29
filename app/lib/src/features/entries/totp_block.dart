import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/models.dart';
import '../../l10n/app_localizations.dart';
import '../../providers/providers.dart';
import '../../ui/tokens.dart';
import '../../ui/typography.dart';
import '../../ui/widgets/countdown_ring.dart';
import '../../ui/widgets/section_card.dart';

class TotpBlock extends ConsumerStatefulWidget {
  const TotpBlock({super.key, required this.uuid});

  final String uuid;

  @override
  ConsumerState<TotpBlock> createState() => TotpBlockState();
}

class TotpBlockState extends ConsumerState<TotpBlock> {
  Timer? _timer;
  TotpCode? _code;

  @override
  void initState() {
    super.initState();
    _refresh();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) => _refresh());
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  void _refresh() {
    final repo = ref.read(totpRepositoryProvider);
    final code = repo.totpNow(widget.uuid);
    if (code != null && mounted) {
      setState(() => _code = code);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    final code = _code;

    if (code == null) return const SizedBox.shrink();

    final formattedCode = _formatCode(code.code);

    return SectionCard(
      title: l10n.entryDetailTotp,
      child: Row(
        children: [
          CountdownRing(
            remainingSecs: code.remainingSecs,
            periodSecs: code.period,
          ),
          const SizedBox(width: HidlinsSpacing.md),
          Expanded(
            child: Semantics(
              label: '${l10n.entryDetailTotp}: $formattedCode',
              child: Text(
                formattedCode,
                style: monospaceTextStyle(context, fontSize: 24),
              ),
            ),
          ),
          IconButton(
            icon: const Icon(Icons.copy),
            tooltip: l10n.actionCopy,
            onPressed: () => _copyTotpCode(),
          ),
        ],
      ),
    );
  }

  Future<void> _copyTotpCode() async {
    final l10n = AppLocalizations.of(context)!;
    try {
      final repo = ref.read(secretsRepositoryProvider);
      await repo.copyEntryField(widget.uuid, CopyField.totpCode);
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(l10n.copiedSnackbar(30))));
    } on Exception {
      // ignore
    }
  }

  String _formatCode(String code) {
    if (code.length == 6) {
      return '${code.substring(0, 3)} ${code.substring(3)}';
    }
    if (code.length == 8) {
      return '${code.substring(0, 4)} ${code.substring(4)}';
    }
    return code;
  }
}

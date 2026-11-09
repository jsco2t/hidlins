import 'package:app/src/bridge/api/session.dart' as api;
import 'package:app/src/bridge/dto.dart' as bridge;
import 'package:app/src/bridge/error.dart';
import 'package:app/src/data/bridge_repositories.dart';
import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/platform/platform_result.dart';
import 'package:app/src/platform/secure_clipboard.dart';
import 'package:flutter_test/flutter_test.dart';

class _Session implements api.AppSession {
  bridge.GeneratedSecret? submitted;
  int desktopCopies = 0;
  int prepared = 0;
  int entryCopies = 0;
  int serial = 40;
  Object? error;

  @override
  Future<bridge.GeneratedSecret> generatePassword({
    required bridge.PasswordOptionsDto opts,
  }) async => bridge.GeneratedSecret(
    value: 'random-result-marker',
    entropyBits: 80,
    copyOrigin: BigInt.from(++serial),
  );

  @override
  Future<bridge.GeneratedSecret> generatePassphrase({
    required bridge.PassphraseOptionsDto opts,
  }) async => bridge.GeneratedSecret(
    value: 'diceware-result-marker',
    entropyBits: 81,
    copyOrigin: BigInt.from(++serial),
  );

  @override
  Future<void> copyGeneratedSecret({
    required bridge.GeneratedSecret result,
  }) async {
    desktopCopies++;
    submitted = result;
    if (error case final failure?) throw failure;
  }

  @override
  Future<bridge.ClipboardTransferTicket> prepareGeneratedClipboardTransfer({
    required bridge.GeneratedSecret result,
  }) async {
    prepared++;
    submitted = result;
    if (error case final failure?) throw failure;
    return bridge.ClipboardTransferTicket(
      id: 'hct-generated-result',
      expiresAfterSecs: 30,
    );
  }

  @override
  Future<void> copyEntryField({
    required String uuid,
    required bridge.CopyField field,
  }) async {
    entryCopies++;
    throw StateError('generated copying must not use an entry');
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class _Clipboard implements SecureClipboardCapability {
  final transfers = <PreparedClipboardTransfer>[];
  PlatformResult<PlatformUnit> result = const PlatformSuccess(platformUnit);

  @override
  Future<PlatformResult<PlatformUnit>> copyPrepared(
    PreparedClipboardTransfer transfer,
  ) async {
    transfers.add(transfer);
    return result;
  }
}

void main() {
  test(
    'desktop generated copying preserves value, entropy and original origin',
    () async {
      final session = _Session();
      final result = await BridgeGeneratorRepository(session)
          .generatePassword(const PasswordOptionsDto());
      expect(result.copyOrigin, BigInt.from(41));
      await BridgeSecretsRepository(session).copyGeneratedSecret(result);
      expect(session.submitted?.value, result.value);
      expect(session.submitted?.entropyBits, result.entropyBits);
      expect(session.submitted?.copyOrigin, result.copyOrigin);
      expect(session.desktopCopies, 1);
      expect(session.prepared, 0);
      expect(session.entryCopies, 0);
    },
  );

  test(
    'independent Diceware generation cannot replace the displayed origin',
    () async {
      final session = _Session();
      final generator = BridgeGeneratorRepository(session);
      final displayed = await generator.generatePassword(
        const PasswordOptionsDto(),
      );
      final newer = await generator.generatePassphrase(
        const PassphraseOptionsDto(),
      );
      expect(newer.copyOrigin, BigInt.from(42));
      await BridgeSecretsRepository(session).copyGeneratedSecret(displayed);
      expect(session.submitted?.value, displayed.value);
      expect(session.submitted?.copyOrigin, BigInt.from(41));
      await BridgeSecretsRepository(session).copyGeneratedSecret(newer);
      expect(session.submitted?.value, newer.value);
      expect(session.submitted?.copyOrigin, BigInt.from(42));
    },
  );

  test(
    'mobile generated copying hands only the prepared ticket to native',
    () async {
      final session = _Session();
      final clipboard = _Clipboard();
      final generated = await BridgeGeneratorRepository(session)
          .generatePassphrase(const PassphraseOptionsDto());
      await BridgeSecretsRepository.mobile(
        session,
        clipboard,
      ).copyGeneratedSecret(generated);
      expect(session.submitted?.value, generated.value);
      expect(session.submitted?.copyOrigin, generated.copyOrigin);
      expect(session.prepared, 1);
      expect(session.desktopCopies, 0);
      expect(session.entryCopies, 0);
      expect(clipboard.transfers.single.id, 'hct-generated-result');
      expect(clipboard.transfers.single.expiresAfterSeconds, 30);
      expect(
        clipboard.transfers.single.toString(),
        isNot(contains(generated.value)),
      );
    },
  );

  test(
    'missing generated origin fails without substituting a fresh origin',
    () async {
      final session = _Session();
      await expectLater(
        BridgeSecretsRepository(session).copyGeneratedSecret(
          const GeneratedSecret(value: 'unbound-marker', entropyBits: 80),
        ),
        throwsA(isA<InvalidInputFailure>()),
      );
      expect(session.submitted, isNull);
      expect(session.desktopCopies + session.prepared + session.entryCopies, 0);
    },
  );

  test(
    'revoked native preparation is mapped before any platform handoff',
    () async {
      final session = _Session()..error = const HidlinsApiError.vaultLocked();
      final clipboard = _Clipboard();
      final result = await BridgeGeneratorRepository(session)
          .generatePassword(const PasswordOptionsDto());
      await expectLater(
        BridgeSecretsRepository.mobile(
          session,
          clipboard,
        ).copyGeneratedSecret(result),
        throwsA(isA<VaultIsLocked>()),
      );
      expect(clipboard.transfers, isEmpty);
    },
  );

  test(
    'all existing mobile failure states remain explicit for generated copying',
    () async {
      final session = _Session();
      final generated = await BridgeGeneratorRepository(session)
          .generatePassword(const PasswordOptionsDto());
      for (final result in <PlatformResult<PlatformUnit>>[
        const PlatformUnsupported(),
        const PlatformCanceled(),
        const PlatformDenied(),
        const PlatformStale(),
        const PlatformFailure('fixture-code'),
      ]) {
        final clipboard = _Clipboard()..result = result;
        await expectLater(
          BridgeSecretsRepository.mobile(
            session,
            clipboard,
          ).copyGeneratedSecret(generated),
          throwsA(isA<AppFailure>()),
        );
        expect(clipboard.transfers, hasLength(1));
      }
    },
  );
}

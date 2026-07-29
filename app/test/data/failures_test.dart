import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/bridge/error.dart';
import 'package:app/src/data/failures.dart';

void main() {
  group('mapApiError', () {
    final table = <HidlinsApiError, Type>{
      const HidlinsApiError.authenticationFailed(): BadCredentials,
      const HidlinsApiError.vaultLocked(): VaultIsLocked,
      const HidlinsApiError.vaultBusySyncing(): VaultIsBusy,
      const HidlinsApiError.vaultContended(holderPid: 42): VaultContended,
      const HidlinsApiError.keyfileRequired(): KeyfileNeeded,
      const HidlinsApiError.pathExists(path: '/v.kdbx'): PathAlreadyExists,
      const HidlinsApiError.fileNotFound(path: '/v.kdbx'): NotFound,
      const HidlinsApiError.invalidInput(field: 'name', reason: 'empty'):
          InvalidInputFailure,
      const HidlinsApiError.registryChanged(): InternalFailure,
      const HidlinsApiError.invalidFormat(): InvalidInputFailure,
      const HidlinsApiError.registryMalformed(): InternalFailure,
      const HidlinsApiError.syncNotConfigured(): SyncNotReady,
      const HidlinsApiError.syncRemoteUnreachable(endpoint: 'https://s3'):
          SyncUnreachable,
      const HidlinsApiError.syncAuthFailed(): SyncAuthFailure,
      const HidlinsApiError.syncConflictUnresolvable(backupPath: '/bak'):
          SyncConflict,
      const HidlinsApiError.syncDuplicateTarget(existingVault: 'v1'):
          SyncDuplicate,
      const HidlinsApiError.io(context: 'disk full'): IoFailure,
      const HidlinsApiError.internal(context: 'panic'): InternalFailure,
    };

    for (final entry in table.entries) {
      test('${entry.key.runtimeType} maps to ${entry.value}', () {
        final result = mapApiError(entry.key);
        expect(result, isA<AppFailure>());
        expect(result.runtimeType, entry.value);
      });
    }

    test('VaultContended preserves holderPid', () {
      final result = mapApiError(
        const HidlinsApiError.vaultContended(holderPid: 99),
      );
      expect(result, isA<VaultContended>());
      expect((result as VaultContended).holderPid, 99);
    });

    test('SyncConflict preserves backupPath', () {
      final result = mapApiError(
        const HidlinsApiError.syncConflictUnresolvable(backupPath: '/b.kdbx'),
      );
      expect(result, isA<SyncConflict>());
      expect((result as SyncConflict).backupPath, '/b.kdbx');
    });

    test('SyncUnreachable preserves endpoint', () {
      final result = mapApiError(
        const HidlinsApiError.syncRemoteUnreachable(endpoint: 'https://minio'),
      );
      expect(result, isA<SyncUnreachable>());
      expect((result as SyncUnreachable).endpoint, 'https://minio');
    });

    test('SyncDuplicate preserves existingVault', () {
      final result = mapApiError(
        const HidlinsApiError.syncDuplicateTarget(existingVault: 'work'),
      );
      expect(result, isA<SyncDuplicate>());
      expect((result as SyncDuplicate).existingVault, 'work');
    });
  });
}

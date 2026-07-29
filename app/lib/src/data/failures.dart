import '../bridge/error.dart';

sealed class AppFailure implements Exception {
  const AppFailure();
}

class BadCredentials extends AppFailure {
  const BadCredentials();
}

class VaultIsLocked extends AppFailure {
  const VaultIsLocked();
}

class VaultIsBusy extends AppFailure {
  const VaultIsBusy();
}

class VaultContended extends AppFailure {
  final int? holderPid;
  const VaultContended({this.holderPid});
}

class KeyfileNeeded extends AppFailure {
  const KeyfileNeeded();
}

class PathAlreadyExists extends AppFailure {
  final String path;
  const PathAlreadyExists(this.path);
}

class NotFound extends AppFailure {
  final String path;
  const NotFound(this.path);
}

class InvalidInputFailure extends AppFailure {
  final String field;
  final String reason;
  const InvalidInputFailure({required this.field, required this.reason});
}

class SyncNotReady extends AppFailure {
  const SyncNotReady();
}

class SyncUnreachable extends AppFailure {
  final String? endpoint;
  const SyncUnreachable({this.endpoint});
}

class SyncAuthFailure extends AppFailure {
  const SyncAuthFailure();
}

class SyncConflict extends AppFailure {
  final String backupPath;
  const SyncConflict(this.backupPath);
}

class SyncDuplicate extends AppFailure {
  final String existingVault;
  const SyncDuplicate(this.existingVault);
}

class IoFailure extends AppFailure {
  final String context;
  const IoFailure(this.context);
}

class InternalFailure extends AppFailure {
  final String context;
  const InternalFailure(this.context);
}

AppFailure mapApiError(HidlinsApiError error) {
  return switch (error) {
    HidlinsApiError_AuthenticationFailed() => const BadCredentials(),
    HidlinsApiError_VaultLocked() => const VaultIsLocked(),
    HidlinsApiError_VaultBusySyncing() => const VaultIsBusy(),
    HidlinsApiError_VaultContended(:final holderPid) => VaultContended(
      holderPid: holderPid,
    ),
    HidlinsApiError_KeyfileRequired() => const KeyfileNeeded(),
    HidlinsApiError_PathExists(:final path) => PathAlreadyExists(path),
    HidlinsApiError_FileNotFound(:final path) => NotFound(path),
    HidlinsApiError_InvalidInput(:final field, :final reason) =>
      InvalidInputFailure(field: field, reason: reason),
    HidlinsApiError_RegistryChanged() => const InternalFailure(
      'registry changed',
    ),
    HidlinsApiError_InvalidFormat() => const InvalidInputFailure(
      field: 'format',
      reason: 'invalid vault format',
    ),
    HidlinsApiError_RegistryMalformed() => const InternalFailure(
      'registry malformed',
    ),
    HidlinsApiError_SyncNotConfigured() => const SyncNotReady(),
    HidlinsApiError_SyncRemoteUnreachable(:final endpoint) => SyncUnreachable(
      endpoint: endpoint,
    ),
    HidlinsApiError_SyncAuthFailed() => const SyncAuthFailure(),
    HidlinsApiError_SyncConflictUnresolvable(:final backupPath) => SyncConflict(
      backupPath,
    ),
    HidlinsApiError_SyncDuplicateTarget(:final existingVault) => SyncDuplicate(
      existingVault,
    ),
    HidlinsApiError_Io(:final context) => IoFailure(context),
    HidlinsApiError_Internal(:final context) => InternalFailure(context),
  };
}

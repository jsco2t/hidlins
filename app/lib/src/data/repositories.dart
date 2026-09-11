import 'dart:async';
import 'dart:ui' show VoidCallback;

import 'models.dart';

abstract class SessionRepository {
  Future<VaultTree> unlock(
    String name,
    String masterPassword, {
    KeyfileRef? keyfile,
  });
  Future<void> lockNow();
  Future<void> shutdown();
  Stream<LockEvent> lockEvents();
  BigInt droppedLockEvents();
  Future<List<String>> startupWarnings();
  void reportActivity();
  LockEvent reportLifecycleState(LifecycleStateDto state);

  Future<List<VaultSummary>> listVaults();
  Future<VaultSummary> createVault({
    required String name,
    required String masterPassword,
    String? fileName,
    KeyfileRef? keyfile,
    required bool confirmedNoRecovery,
  });
  Future<VaultSummary> registerExistingVault({
    required String name,
    required String kdbxPath,
    KeyfileRef? keyfile,
  });
  Future<void> deregisterVault(String name, {required bool deleteFile});
  Future<void> changeMasterPassword(String current, String newPassword);
}

abstract class EntryRepository {
  Future<VaultTree> vaultTree();
  Future<EntryDetail> entryDetail(String uuid);
  Future<String> createEntry(String groupUuid, EntryDraftDto draft);
  Future<void> updateEntry(String uuid, EntryEditDto edit);
  Future<void> deleteEntry(String uuid);
  Future<void> purgeEntry(String uuid);
  Future<void> moveEntry(String uuid, String groupUuid);
  Future<List<HistorySummary>> entryHistory(String uuid);

  Future<void> createGroup(String parentUuid, String name);
  Future<void> renameGroup(String uuid, String name);
  Future<void> moveGroup(String uuid, String parentUuid);
  Future<void> deleteGroup(String uuid, GroupDeleteBehavior behavior);
  Future<List<String>> listTags();

  Future<void> setExpiration(String uuid, int epochSecs);
  Future<void> clearExpiration(String uuid);

  Future<List<AttachmentMeta>> listAttachments(String uuid);
  Future<void> addAttachment(String uuid, String sourcePath);
  Future<void> removeAttachment(String uuid, String key);
  Future<void> saveAttachmentTo(String uuid, String key, String destPath);

  VoidCallback? onMutation;
}

abstract class SecretsRepository {
  Future<String> revealField(String uuid, RevealField field);
  Future<void> copyEntryField(String uuid, CopyField field);
}

abstract class SearchRepository {
  Future<List<SearchHit>> search(SearchOptionsDto options);
}

abstract class TotpRepository {
  TotpCode? totpNow(String uuid);
}

abstract class GeneratorRepository {
  Future<GeneratedSecret> generatePassword(PasswordOptionsDto options);
  Future<GeneratedSecret> generatePassphrase(PassphraseOptionsDto options);
}

abstract class SyncRepository {
  Future<SyncStatusDto> syncStatus();
  Future<LocalDiscoveryStatus> discover(DiscoveryKind kind);
  Future<void> openDiscoverySettings();
  Future<void> stopDiscovery();
  Future<void> setManualEndpoint(LocalEndpoint endpoint);
  Future<void> configureLocalSync(LocalSyncRole role);
  Future<PairingPrompt> beginPairing();
  Future<PairingPrompt> beginPairImport({
    required String name,
    required String masterPassword,
    KeyfileRef? keyfile,
  });
  Future<VaultSummary?> confirmPairing({
    required String transactionHandle,
    required bool accepted,
    required String peerDisplayName,
  });
  Future<List<SyncPeer>> listPeers();
  Future<void> renamePeer(String peerId, String displayName);
  Future<void> revokePeer(String peerId);
  Future<List<LocalEndpoint>> serverEndpoints();
  Future<LocalEndpoint> startServer(LocalEndpoint endpoint);
  Future<void> stopServer();
  Future<void> openPairingWindow();
  Future<void> closePairingWindow();
  Future<void> cancelSync();

  /// Discover trusted routes, then consume the once-per-process automatic
  /// startup attempt. Implementations must preserve this ordering.
  Future<void> startStartupSync();
  Future<void> syncNow();
  Future<void> clearSyncConfig(String name);
  Stream<SyncEvent> syncEvents();
}

abstract class PrefsRepository {
  Future<UiPrefs> getPrefs();
  Future<void> setPrefs(UiPrefs prefs);
}

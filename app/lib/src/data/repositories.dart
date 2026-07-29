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
  void reportActivity();
  void reportLifecycleState(LifecycleStateDto state);

  Future<List<VaultSummary>> listVaults();
  Future<VaultSummary> createVault({
    required String name,
    required String masterPassword,
    String? fileName,
    KeyfileRef? keyfile,
    required bool confirmedNoRecovery,
  });
  Future<void> deregisterVault(String name, {required bool deleteFile});
  Future<void> changeMasterPassword(String current, String newPassword);
  Future<VaultSummary> bootstrapFromRemote({
    required String name,
    required S3ConfigDto config,
    required String masterPassword,
    KeyfileRef? keyfile,
  });
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
  Future<void> configureSync(S3ConfigDto config);
  Future<void> syncNow();
  Future<void> clearSyncConfig(String name);
  Stream<SyncEvent> syncEvents();
  Stream<ClipboardEvent> clipboardEvents();
}

abstract class PrefsRepository {
  Future<UiPrefs> getPrefs();
  Future<void> setPrefs(UiPrefs prefs);
}

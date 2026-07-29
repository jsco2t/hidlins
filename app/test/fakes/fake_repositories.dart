import 'dart:async';
import 'dart:ui' show VoidCallback;

import 'package:app/src/data/failures.dart';
import 'package:app/src/data/models.dart';
import 'package:app/src/data/repositories.dart';

class FakeSessionRepository implements SessionRepository {
  final StreamController<LockEvent> lockController =
      StreamController<LockEvent>.broadcast();
  LockEvent currentLockState = LockEvent.locked;
  BigInt droppedLockEventCount = BigInt.zero;
  Object? droppedLockEventsError;

  bool unlockCalled = false;
  String? lastUnlockName;
  String? lastUnlockPassword;
  AppFailure? unlockError;
  VaultTree? unlockResult;
  bool lockNowCalled = false;
  bool shutdownCalled = false;

  List<VaultSummary> vaults = [];
  bool createVaultCalled = false;
  String? lastCreateName;
  bool? lastConfirmedNoRecovery;
  AppFailure? createVaultError;

  bool deregisterCalled = false;
  String? lastDeregisterName;
  bool? lastDeregisterDeleteFile;

  bool changePasswordCalled = false;
  AppFailure? changePasswordError;

  bool bootstrapCalled = false;
  AppFailure? bootstrapError;

  @override
  Future<VaultTree> unlock(
    String name,
    String masterPassword, {
    KeyfileRef? keyfile,
  }) async {
    unlockCalled = true;
    lastUnlockName = name;
    lastUnlockPassword = masterPassword;
    if (unlockError != null) throw unlockError!;
    emitLockState(LockEvent.unlocked);
    return unlockResult ?? _defaultTree;
  }

  @override
  Future<void> lockNow() async {
    lockNowCalled = true;
    currentLockState = LockEvent.locked;
    lockController.add(LockEvent.locked);
  }

  @override
  Future<void> shutdown() async {
    shutdownCalled = true;
  }

  @override
  Stream<LockEvent> lockEvents() async* {
    yield currentLockState;
    yield* lockController.stream;
  }

  @override
  BigInt droppedLockEvents() {
    if (droppedLockEventsError case final error?) throw error;
    return droppedLockEventCount;
  }

  void emitLockState(LockEvent state) {
    currentLockState = state;
    lockController.add(state);
  }

  int activityCount = 0;
  final List<LifecycleStateDto> lifecycleStates = [];

  @override
  void reportActivity() {
    activityCount++;
  }

  @override
  void reportLifecycleState(LifecycleStateDto state) {
    lifecycleStates.add(state);
  }

  @override
  Future<List<VaultSummary>> listVaults() async => vaults;

  @override
  Future<VaultSummary> createVault({
    required String name,
    required String masterPassword,
    String? fileName,
    KeyfileRef? keyfile,
    required bool confirmedNoRecovery,
  }) async {
    createVaultCalled = true;
    lastCreateName = name;
    lastConfirmedNoRecovery = confirmedNoRecovery;
    if (createVaultError != null) throw createVaultError!;
    final vault = VaultSummary(
      name: name,
      path: '/vaults/$name.kdbx',
      hasKeyfile: keyfile != null,
      hasSync: false,
    );
    vaults.add(vault);
    return vault;
  }

  @override
  Future<void> deregisterVault(String name, {required bool deleteFile}) async {
    deregisterCalled = true;
    lastDeregisterName = name;
    lastDeregisterDeleteFile = deleteFile;
    vaults.removeWhere((v) => v.name == name);
  }

  @override
  Future<void> changeMasterPassword(String current, String newPassword) async {
    changePasswordCalled = true;
    if (changePasswordError != null) throw changePasswordError!;
  }

  @override
  Future<VaultSummary> bootstrapFromRemote({
    required String name,
    required S3ConfigDto config,
    required String masterPassword,
    KeyfileRef? keyfile,
  }) async {
    bootstrapCalled = true;
    if (bootstrapError != null) throw bootstrapError!;
    final vault = VaultSummary(
      name: name,
      path: '/vaults/$name.kdbx',
      hasKeyfile: keyfile != null,
      hasSync: true,
    );
    vaults.add(vault);
    return vault;
  }

  void dispose() {
    unawaited(lockController.close());
  }
}

class FakeEntryRepository implements EntryRepository {
  VaultTree tree = _defaultTree;
  final Map<String, EntryDetail> details = {};
  final Map<String, List<HistorySummary>> histories = {};
  final Map<String, List<AttachmentMeta>> attachmentMap = {};

  bool createEntryCalled = false;
  String? lastCreateGroup;
  EntryDraftDto? lastDraft;

  bool updateEntryCalled = false;
  String? lastUpdateUuid;
  EntryEditDto? lastEdit;

  bool deleteEntryCalled = false;
  String? lastDeleteUuid;

  bool purgeEntryCalled = false;

  bool moveEntryCalled = false;
  String? lastMoveUuid;
  String? lastMoveGroup;

  bool createGroupCalled = false;
  bool renameGroupCalled = false;
  bool deleteGroupCalled = false;
  GroupDeleteBehavior? lastDeleteGroupBehavior;

  bool setExpirationCalled = false;
  bool clearExpirationCalled = false;

  bool addAttachmentCalled = false;
  String? lastAttachmentPath;
  bool removeAttachmentCalled = false;
  String? lastRemovedAttachmentKey;
  bool saveAttachmentCalled = false;
  String? lastSaveAttachmentDest;

  AppFailure? attachmentError;

  @override
  VoidCallback? onMutation;

  @override
  Future<VaultTree> vaultTree() async => tree;

  @override
  Future<EntryDetail> entryDetail(String uuid) async {
    return details[uuid] ?? _defaultDetail(uuid);
  }

  @override
  Future<String> createEntry(String groupUuid, EntryDraftDto draft) async {
    createEntryCalled = true;
    lastCreateGroup = groupUuid;
    lastDraft = draft;
    onMutation?.call();
    return 'new-uuid';
  }

  @override
  Future<void> updateEntry(String uuid, EntryEditDto edit) async {
    updateEntryCalled = true;
    lastUpdateUuid = uuid;
    lastEdit = edit;
    onMutation?.call();
  }

  @override
  Future<void> deleteEntry(String uuid) async {
    deleteEntryCalled = true;
    lastDeleteUuid = uuid;
    onMutation?.call();
  }

  @override
  Future<void> purgeEntry(String uuid) async {
    purgeEntryCalled = true;
    onMutation?.call();
  }

  @override
  Future<void> moveEntry(String uuid, String groupUuid) async {
    moveEntryCalled = true;
    lastMoveUuid = uuid;
    lastMoveGroup = groupUuid;
    onMutation?.call();
  }

  @override
  Future<List<HistorySummary>> entryHistory(String uuid) async {
    return histories[uuid] ?? [];
  }

  @override
  Future<void> createGroup(String parentUuid, String name) async {
    createGroupCalled = true;
    onMutation?.call();
  }

  @override
  Future<void> renameGroup(String uuid, String name) async {
    renameGroupCalled = true;
    onMutation?.call();
  }

  @override
  Future<void> moveGroup(String uuid, String parentUuid) async {
    onMutation?.call();
  }

  @override
  Future<void> deleteGroup(String uuid, GroupDeleteBehavior behavior) async {
    deleteGroupCalled = true;
    lastDeleteGroupBehavior = behavior;
    onMutation?.call();
  }

  @override
  Future<List<String>> listTags() async => ['work', 'personal', 'finance'];

  @override
  Future<void> setExpiration(String uuid, int epochSecs) async {
    setExpirationCalled = true;
    onMutation?.call();
  }

  @override
  Future<void> clearExpiration(String uuid) async {
    clearExpirationCalled = true;
    onMutation?.call();
  }

  @override
  Future<List<AttachmentMeta>> listAttachments(String uuid) async {
    return attachmentMap[uuid] ?? [];
  }

  @override
  Future<void> addAttachment(String uuid, String sourcePath) async {
    addAttachmentCalled = true;
    lastAttachmentPath = sourcePath;
    if (attachmentError != null) throw attachmentError!;
    onMutation?.call();
  }

  @override
  Future<void> removeAttachment(String uuid, String key) async {
    removeAttachmentCalled = true;
    lastRemovedAttachmentKey = key;
    onMutation?.call();
  }

  @override
  Future<void> saveAttachmentTo(
    String uuid,
    String key,
    String destPath,
  ) async {
    saveAttachmentCalled = true;
    lastSaveAttachmentDest = destPath;
  }
}

class FakeSecretsRepository implements SecretsRepository {
  bool revealCalled = false;
  String? lastRevealUuid;
  RevealField? lastRevealField;
  String revealResult = 'revealed-secret';

  bool copyCalled = false;
  String? lastCopyUuid;
  CopyField? lastCopyField;

  @override
  Future<String> revealField(String uuid, RevealField field) async {
    revealCalled = true;
    lastRevealUuid = uuid;
    lastRevealField = field;
    return revealResult;
  }

  @override
  Future<void> copyEntryField(String uuid, CopyField field) async {
    copyCalled = true;
    lastCopyUuid = uuid;
    lastCopyField = field;
  }
}

class FakeSearchRepository implements SearchRepository {
  List<SearchHit> results = [];
  bool searchCalled = false;
  SearchOptionsDto? lastOptions;

  @override
  Future<List<SearchHit>> search(SearchOptionsDto options) async {
    searchCalled = true;
    lastOptions = options;
    return results;
  }
}

class FakeTotpRepository implements TotpRepository {
  final Map<String, TotpCode> codes = {};

  @override
  TotpCode? totpNow(String uuid) => codes[uuid];
}

class FakeGeneratorRepository implements GeneratorRepository {
  bool generatePasswordCalled = false;
  PasswordOptionsDto? lastPasswordOptions;
  GeneratedSecret passwordResult = const GeneratedSecret(
    value: 'xK9#mP2!qL',
    entropyBits: 65.0,
  );

  bool generatePassphraseCalled = false;
  PassphraseOptionsDto? lastPassphraseOptions;
  GeneratedSecret passphraseResult = const GeneratedSecret(
    value: 'correct-horse-battery-staple',
    entropyBits: 77.5,
  );

  @override
  Future<GeneratedSecret> generatePassword(PasswordOptionsDto options) async {
    generatePasswordCalled = true;
    lastPasswordOptions = options;
    return passwordResult;
  }

  @override
  Future<GeneratedSecret> generatePassphrase(
    PassphraseOptionsDto options,
  ) async {
    generatePassphraseCalled = true;
    lastPassphraseOptions = options;
    return passphraseResult;
  }
}

class FakeSyncRepository implements SyncRepository {
  SyncStatusDto status = const SyncStatusDto(
    configured: false,
    inFlight: false,
  );
  bool syncNowCalled = false;
  bool configureCalled = false;
  S3ConfigDto? lastConfig;

  final StreamController<SyncEvent> syncController =
      StreamController<SyncEvent>.broadcast();

  final StreamController<ClipboardEvent> clipController =
      StreamController<ClipboardEvent>.broadcast();

  @override
  Future<SyncStatusDto> syncStatus() async => status;

  @override
  Future<void> configureSync(S3ConfigDto config) async {
    configureCalled = true;
    lastConfig = config;
  }

  @override
  Future<void> syncNow() async {
    syncNowCalled = true;
  }

  @override
  Future<void> clearSyncConfig(String name) async {}

  @override
  Stream<SyncEvent> syncEvents() => syncController.stream;

  @override
  Stream<ClipboardEvent> clipboardEvents() => clipController.stream;

  void dispose() {
    unawaited(syncController.close());
    unawaited(clipController.close());
  }
}

class FakePrefsRepository implements PrefsRepository {
  UiPrefs prefs = const UiPrefs();
  bool setPrefsCalled = false;
  UiPrefs? lastPrefs;

  @override
  Future<UiPrefs> getPrefs() async => prefs;

  @override
  Future<void> setPrefs(UiPrefs p) async {
    setPrefsCalled = true;
    lastPrefs = p;
    prefs = p;
  }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

final _defaultTree = VaultTree(
  root: GroupNode(
    uuid: 'root-uuid',
    name: 'Root',
    children: [
      GroupNode(
        uuid: 'banking-uuid',
        name: 'Banking',
        children: const [],
        entryCount: BigInt.from(1),
      ),
      GroupNode(
        uuid: 'social-uuid',
        name: 'Social',
        children: const [],
        entryCount: BigInt.from(1),
      ),
    ],
    entryCount: BigInt.from(1),
  ),
  entries: [
    EntrySummary(
      uuid: 'entry-1',
      title: 'GitHub',
      username: 'octocat',
      url: 'https://github.com',
      kind: EntryKindDto.credential,
      hasTotp: true,
      hasAttachments: false,
      isExpired: false,
      groupUuid: 'root-uuid',
      tags: const ['work'],
    ),
    EntrySummary(
      uuid: 'entry-2',
      title: 'Bank of Test',
      username: 'banker',
      url: 'https://bank.test',
      kind: EntryKindDto.credential,
      hasTotp: false,
      hasAttachments: true,
      isExpired: false,
      groupUuid: 'banking-uuid',
      tags: const ['finance'],
    ),
    EntrySummary(
      uuid: 'entry-3',
      title: 'Expired Account',
      username: 'old-user',
      url: '',
      kind: EntryKindDto.credential,
      hasTotp: false,
      hasAttachments: false,
      isExpired: true,
      groupUuid: 'social-uuid',
      tags: const [],
    ),
  ],
);

EntryDetail _defaultDetail(String uuid) => EntryDetail(
  uuid: uuid,
  title: 'GitHub',
  username: 'octocat',
  hasPassword: true,
  url: 'https://github.com',
  notes: 'Development account',
  kind: EntryKindDto.credential,
  tags: const ['work', 'dev'],
  customFields: const [
    CustomFieldDto(name: 'pin', isProtected: true),
    CustomFieldDto(name: 'recovery-email', isProtected: false),
  ],
  attachments: const [AttachmentMeta(name: 'backup-codes.txt', sizeBytes: 256)],
  creationTime: 1700000000,
  lastModificationTime: 1700100000,
);

// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appTitle => 'Hidlins';

  @override
  String get lockScreenTitle => 'Vault locked';

  @override
  String get lockScreenUnlockButton => 'Unlock';

  @override
  String get lockScreenPasswordLabel => 'Master password';

  @override
  String get lockScreenPasswordHint => 'Enter your master password';

  @override
  String get lockScreenKeyfileLabel => 'Keyfile';

  @override
  String get lockScreenKeyfileSelect => 'Select keyfile';

  @override
  String get lockScreenWrongPassword => 'Wrong password or keyfile';

  @override
  String lockScreenVaultContended(String pid) {
    return 'Vault in use by another process (PID $pid)';
  }

  @override
  String get lockScreenUnlocking => 'Unlocking…';

  @override
  String get lockScreenSyncing => 'Syncing…';

  @override
  String get lockScreenOfflineWarning => 'Opened offline — sync failed';

  @override
  String get lockScreenSelectVault => 'Select a vault';

  @override
  String get navEntries => 'Entries';

  @override
  String get navSearch => 'Search';

  @override
  String get navGenerator => 'Generator';

  @override
  String get navSync => 'Sync';

  @override
  String get navSettings => 'Settings';

  @override
  String get navVaults => 'Vaults';

  @override
  String get actionCopy => 'Copy';

  @override
  String get actionCancel => 'Cancel';

  @override
  String get actionSave => 'Save';

  @override
  String get actionDelete => 'Delete';

  @override
  String get actionSearch => 'Search';

  @override
  String get actionCreate => 'Create';

  @override
  String get actionEdit => 'Edit';

  @override
  String get actionLock => 'Lock';

  @override
  String get actionReveal => 'Reveal';

  @override
  String get actionHide => 'Hide';

  @override
  String get actionPurge => 'Delete permanently';

  @override
  String get actionMove => 'Move';

  @override
  String get actionRename => 'Rename';

  @override
  String get actionConfirm => 'Confirm';

  @override
  String get actionGenerate => 'Generate';

  @override
  String get actionUse => 'Use';

  @override
  String get actionAttach => 'Attach file';

  @override
  String get actionDetach => 'Remove';

  @override
  String get actionSaveAs => 'Save as…';

  @override
  String get actionClose => 'Close';

  @override
  String copiedSnackbar(int seconds) {
    return 'Copied — clears in ${seconds}s';
  }

  @override
  String get clipboardCleared => 'Clipboard cleared';

  @override
  String get emptyStateNoEntries => 'No entries yet';

  @override
  String get emptyStateNoEntriesSubtitle =>
      'Add your first entry to get started';

  @override
  String get emptyStateNoResults => 'No results found';

  @override
  String get emptyStateNoVaults => 'No vaults registered';

  @override
  String get emptyStateNoVaultsSubtitle => 'Create or import a vault to begin';

  @override
  String get emptyStateSelectEntry => 'Select an entry';

  @override
  String get emptyStateSelectEntrySubtitle =>
      'Choose an entry from the list to view details';

  @override
  String get syncStatusIdle => 'Sync idle';

  @override
  String get syncStatusInProgress => 'Syncing…';

  @override
  String get syncStatusFailed => 'Sync failed';

  @override
  String get syncStatusSuccess => 'Synced';

  @override
  String get settingsTitle => 'Settings';

  @override
  String get settingsTheme => 'Theme';

  @override
  String get settingsThemeSystem => 'System';

  @override
  String get settingsThemeLight => 'Light';

  @override
  String get settingsThemeDark => 'Dark';

  @override
  String get settingsAutoLock => 'Auto-lock timeout';

  @override
  String get settingsAutoLockNote => 'Configured via CLI or TUI';

  @override
  String get settingsAbout => 'About';

  @override
  String get settingsVersion => 'Version';

  @override
  String get settingsLicenses => 'Open source licenses';

  @override
  String get generatorTitle => 'Password generator';

  @override
  String get generatorRandom => 'Random';

  @override
  String get generatorDiceware => 'Diceware';

  @override
  String get generatorLength => 'Length';

  @override
  String get generatorWords => 'Words';

  @override
  String get generatorLowercase => 'Lowercase';

  @override
  String get generatorUppercase => 'Uppercase';

  @override
  String get generatorDigits => 'Digits';

  @override
  String get generatorSymbols => 'Symbols';

  @override
  String get generatorExcludeAmbiguous => 'Exclude ambiguous';

  @override
  String get generatorSeparator => 'Separator';

  @override
  String generatorEntropy(String bits) {
    return '$bits bits';
  }

  @override
  String get generatorCopy => 'Copy password';

  @override
  String get generatorRegenerate => 'Regenerate';

  @override
  String get entryDetailTitle => 'Entry';

  @override
  String get entryDetailUsername => 'Username';

  @override
  String get entryDetailPassword => 'Password';

  @override
  String get entryDetailUrl => 'URL';

  @override
  String get entryDetailNotes => 'Notes';

  @override
  String get entryDetailTotp => 'TOTP';

  @override
  String get entryDetailAttachments => 'Attachments';

  @override
  String get entryDetailHistory => 'History';

  @override
  String get entryDetailTags => 'Tags';

  @override
  String get entryDetailCustomFields => 'Custom fields';

  @override
  String get entryDetailCreated => 'Created';

  @override
  String get entryDetailModified => 'Modified';

  @override
  String get entryDetailExpires => 'Expires';

  @override
  String get entryDetailExpired => 'Expired';

  @override
  String get entryEditTitle => 'Edit entry';

  @override
  String get entryCreateTitle => 'New entry';

  @override
  String get entryEditUnsavedTitle => 'Unsaved changes';

  @override
  String get entryEditUnsavedMessage => 'Discard changes?';

  @override
  String get entryEditDiscard => 'Discard';

  @override
  String get entryEditKindCredential => 'Credential';

  @override
  String get entryEditKindSecureNote => 'Secure note';

  @override
  String get entryEditKindTotp => 'TOTP';

  @override
  String get entryEditTitleLabel => 'Title';

  @override
  String get entryEditUsernameLabel => 'Username';

  @override
  String get entryEditPasswordLabel => 'Password';

  @override
  String get entryEditUrlLabel => 'URL';

  @override
  String get entryEditNotesLabel => 'Notes';

  @override
  String get entryEditTotpUriLabel => 'TOTP URI';

  @override
  String get entryEditTagsLabel => 'Tags (comma-separated)';

  @override
  String get entryEditAddCustomField => 'Add custom field';

  @override
  String get entryEditCustomFieldName => 'Field name';

  @override
  String get entryEditCustomFieldValue => 'Value';

  @override
  String get entryEditCustomFieldProtected => 'Protected';

  @override
  String get entryDeleteConfirmTitle => 'Delete entry';

  @override
  String get entryDeleteConfirmMessage => 'Move this entry to the recycle bin?';

  @override
  String get entryPurgeConfirmTitle => 'Permanently delete';

  @override
  String get entryPurgeConfirmMessage => 'This cannot be undone.';

  @override
  String get entryMoveTitle => 'Move entry';

  @override
  String get entryHistoryTitle => 'Entry history';

  @override
  String entryHistoryVersion(int number) {
    return 'v$number';
  }

  @override
  String get groupCreateTitle => 'New group';

  @override
  String get groupRenameTitle => 'Rename group';

  @override
  String get groupDeleteTitle => 'Delete group';

  @override
  String get groupDeleteMoveToParent => 'Move contents to parent';

  @override
  String get groupDeleteMoveToRecycleBin => 'Move contents to recycle bin';

  @override
  String get groupNameLabel => 'Group name';

  @override
  String get recycleBin => 'Recycle bin';

  @override
  String get attachmentSizeExceeded => 'File too large';

  @override
  String get attachmentDetachConfirm => 'Remove this attachment?';

  @override
  String attachmentBytes(String size) {
    return '$size bytes';
  }

  @override
  String get expirationSet => 'Set expiration';

  @override
  String get expirationClear => 'Clear expiration';

  @override
  String get searchHintText => 'Search entries…';

  @override
  String get searchModeSubstring => 'Contains';

  @override
  String get searchModeFuzzy => 'Fuzzy';

  @override
  String get searchModeWildcard => 'Wildcard';

  @override
  String get searchIncludeRecycled => 'Include recycled';

  @override
  String get vaultCreateTitle => 'Create vault';

  @override
  String get vaultImportTitle => 'Import vault';

  @override
  String get vaultConnectSyncTitle => 'Connect to sync';

  @override
  String get vaultCreateNameLabel => 'Vault name';

  @override
  String get vaultCreatePasswordLabel => 'Master password';

  @override
  String get vaultCreateConfirmPasswordLabel => 'Confirm password';

  @override
  String get vaultCreateNoRecoveryWarning =>
      'There is no password recovery. If you forget your master password, your data is permanently lost.';

  @override
  String get vaultCreateNoRecoveryConfirm => 'Type CONFIRM to acknowledge';

  @override
  String get vaultCreateNoRecoveryPhrase => 'CONFIRM';

  @override
  String get vaultCreateKeyfileLabel => 'Protect with keyfile (optional)';

  @override
  String get vaultDeregisterTitle => 'Remove vault';

  @override
  String get vaultDeregisterMessage => 'Remove this vault from the registry?';

  @override
  String get vaultDeregisterDeleteFile => 'Also delete the vault file';

  @override
  String get vaultDeregisterDeleteConfirm =>
      'Are you sure? This permanently deletes the vault file.';

  @override
  String get vaultChangePasswordTitle => 'Change master password';

  @override
  String get vaultChangePasswordCurrent => 'Current password';

  @override
  String get vaultChangePasswordNew => 'New password';

  @override
  String get vaultChangePasswordConfirm => 'Confirm new password';

  @override
  String get vaultChangePasswordSuccess => 'Master password changed';

  @override
  String get vaultChangePasswordReenterSync =>
      'Password changed. Re-enter your S3 sync credentials.';

  @override
  String get vaultPasswordMismatch => 'Passwords do not match';

  @override
  String get firstRunTitle => 'Welcome to Hidlins';

  @override
  String get firstRunCreateVault => 'Create a new vault';

  @override
  String get firstRunConnectSync => 'Connect to an existing vault via sync';

  @override
  String get firstRunImportVault => 'Import a .kdbx file';

  @override
  String get bridgeInitError => 'Bridge initialization failed';

  @override
  String get errorGeneric => 'An error occurred';

  @override
  String get errorCopyFailed => 'Copy failed';

  @override
  String get errorRevealFailed => 'Could not reveal field';

  @override
  String get errorVaultAlreadyExists => 'A vault with this name already exists';

  @override
  String get validatorRequired => 'Required';

  @override
  String get syncS3BucketLabel => 'S3 Bucket';

  @override
  String get syncObjectKeyLabel => 'Object key';

  @override
  String get syncRegionLabel => 'Region';

  @override
  String get syncEndpointLabel => 'Endpoint (optional)';

  @override
  String get syncPathStyleLabel => 'Path-style addressing';

  @override
  String get syncAccessKeyIdLabel => 'Access key ID';

  @override
  String get syncSecretAccessKeyLabel => 'Secret access key';

  @override
  String syncErrorDuplicate(String existingVault) {
    return 'Duplicate sync target: already used by \"$existingVault\"';
  }

  @override
  String syncErrorUnreachable(String endpoint) {
    return 'Cannot reach $endpoint';
  }

  @override
  String get syncErrorAuthFailed => 'S3 authentication failed';
}

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations? of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations);
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[Locale('en')];

  /// No description provided for @appTitle.
  ///
  /// In en, this message translates to:
  /// **'Hidlins'**
  String get appTitle;

  /// No description provided for @lockScreenTitle.
  ///
  /// In en, this message translates to:
  /// **'Vault locked'**
  String get lockScreenTitle;

  /// No description provided for @lockScreenUnlockButton.
  ///
  /// In en, this message translates to:
  /// **'Unlock'**
  String get lockScreenUnlockButton;

  /// No description provided for @lockScreenPasswordLabel.
  ///
  /// In en, this message translates to:
  /// **'Master password'**
  String get lockScreenPasswordLabel;

  /// No description provided for @lockScreenPasswordHint.
  ///
  /// In en, this message translates to:
  /// **'Enter your master password'**
  String get lockScreenPasswordHint;

  /// No description provided for @lockScreenKeyfileLabel.
  ///
  /// In en, this message translates to:
  /// **'Keyfile'**
  String get lockScreenKeyfileLabel;

  /// No description provided for @lockScreenKeyfileSelect.
  ///
  /// In en, this message translates to:
  /// **'Select keyfile'**
  String get lockScreenKeyfileSelect;

  /// No description provided for @lockScreenWrongPassword.
  ///
  /// In en, this message translates to:
  /// **'Wrong password or keyfile'**
  String get lockScreenWrongPassword;

  /// No description provided for @lockScreenVaultContended.
  ///
  /// In en, this message translates to:
  /// **'Vault in use by another process (PID {pid})'**
  String lockScreenVaultContended(String pid);

  /// No description provided for @lockScreenUnlocking.
  ///
  /// In en, this message translates to:
  /// **'Unlocking…'**
  String get lockScreenUnlocking;

  /// No description provided for @lockScreenSyncing.
  ///
  /// In en, this message translates to:
  /// **'Syncing…'**
  String get lockScreenSyncing;

  /// No description provided for @lockScreenOfflineWarning.
  ///
  /// In en, this message translates to:
  /// **'Opened offline — sync failed'**
  String get lockScreenOfflineWarning;

  /// No description provided for @lockScreenSelectVault.
  ///
  /// In en, this message translates to:
  /// **'Select a vault'**
  String get lockScreenSelectVault;

  /// No description provided for @navEntries.
  ///
  /// In en, this message translates to:
  /// **'Entries'**
  String get navEntries;

  /// No description provided for @navSearch.
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get navSearch;

  /// No description provided for @navGenerator.
  ///
  /// In en, this message translates to:
  /// **'Generator'**
  String get navGenerator;

  /// No description provided for @navSync.
  ///
  /// In en, this message translates to:
  /// **'Sync'**
  String get navSync;

  /// No description provided for @navSettings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get navSettings;

  /// No description provided for @navVaults.
  ///
  /// In en, this message translates to:
  /// **'Vaults'**
  String get navVaults;

  /// No description provided for @actionCopy.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get actionCopy;

  /// No description provided for @actionCancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get actionCancel;

  /// No description provided for @actionSave.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get actionSave;

  /// No description provided for @actionDelete.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get actionDelete;

  /// No description provided for @actionSearch.
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get actionSearch;

  /// No description provided for @actionCreate.
  ///
  /// In en, this message translates to:
  /// **'Create'**
  String get actionCreate;

  /// No description provided for @actionEdit.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get actionEdit;

  /// No description provided for @actionLock.
  ///
  /// In en, this message translates to:
  /// **'Lock'**
  String get actionLock;

  /// No description provided for @actionReveal.
  ///
  /// In en, this message translates to:
  /// **'Reveal'**
  String get actionReveal;

  /// No description provided for @actionHide.
  ///
  /// In en, this message translates to:
  /// **'Hide'**
  String get actionHide;

  /// No description provided for @actionPurge.
  ///
  /// In en, this message translates to:
  /// **'Delete permanently'**
  String get actionPurge;

  /// No description provided for @actionMove.
  ///
  /// In en, this message translates to:
  /// **'Move'**
  String get actionMove;

  /// No description provided for @actionRename.
  ///
  /// In en, this message translates to:
  /// **'Rename'**
  String get actionRename;

  /// No description provided for @actionConfirm.
  ///
  /// In en, this message translates to:
  /// **'Confirm'**
  String get actionConfirm;

  /// No description provided for @actionGenerate.
  ///
  /// In en, this message translates to:
  /// **'Generate'**
  String get actionGenerate;

  /// No description provided for @actionUse.
  ///
  /// In en, this message translates to:
  /// **'Use'**
  String get actionUse;

  /// No description provided for @actionAttach.
  ///
  /// In en, this message translates to:
  /// **'Attach file'**
  String get actionAttach;

  /// No description provided for @actionDetach.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get actionDetach;

  /// No description provided for @actionSaveAs.
  ///
  /// In en, this message translates to:
  /// **'Save as…'**
  String get actionSaveAs;

  /// No description provided for @actionClose.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get actionClose;

  /// No description provided for @actionRetry.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get actionRetry;

  /// No description provided for @copiedSnackbar.
  ///
  /// In en, this message translates to:
  /// **'Copied — clears in {seconds}s'**
  String copiedSnackbar(int seconds);

  /// No description provided for @clipboardCleared.
  ///
  /// In en, this message translates to:
  /// **'Clipboard cleared'**
  String get clipboardCleared;

  /// No description provided for @emptyStateNoEntries.
  ///
  /// In en, this message translates to:
  /// **'No entries yet'**
  String get emptyStateNoEntries;

  /// No description provided for @emptyStateNoEntriesSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Add your first entry to get started'**
  String get emptyStateNoEntriesSubtitle;

  /// No description provided for @emptyStateNoResults.
  ///
  /// In en, this message translates to:
  /// **'No results found'**
  String get emptyStateNoResults;

  /// No description provided for @emptyStateNoVaults.
  ///
  /// In en, this message translates to:
  /// **'No vaults registered'**
  String get emptyStateNoVaults;

  /// No description provided for @emptyStateNoVaultsSubtitle.
  ///
  /// In en, this message translates to:
  /// **'Create or import a vault to begin'**
  String get emptyStateNoVaultsSubtitle;

  /// No description provided for @emptyStateSelectEntry.
  ///
  /// In en, this message translates to:
  /// **'Select an entry'**
  String get emptyStateSelectEntry;

  /// No description provided for @emptyStateSelectEntrySubtitle.
  ///
  /// In en, this message translates to:
  /// **'Choose an entry from the list to view details'**
  String get emptyStateSelectEntrySubtitle;

  /// No description provided for @syncStatusIdle.
  ///
  /// In en, this message translates to:
  /// **'Sync idle'**
  String get syncStatusIdle;

  /// No description provided for @syncStatusInProgress.
  ///
  /// In en, this message translates to:
  /// **'Syncing…'**
  String get syncStatusInProgress;

  /// No description provided for @syncStatusFailed.
  ///
  /// In en, this message translates to:
  /// **'Sync failed'**
  String get syncStatusFailed;

  /// No description provided for @syncStatusSuccess.
  ///
  /// In en, this message translates to:
  /// **'Synced'**
  String get syncStatusSuccess;

  /// No description provided for @settingsTitle.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settingsTitle;

  /// No description provided for @settingsTheme.
  ///
  /// In en, this message translates to:
  /// **'Theme'**
  String get settingsTheme;

  /// No description provided for @settingsThemeSystem.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get settingsThemeSystem;

  /// No description provided for @settingsThemeLight.
  ///
  /// In en, this message translates to:
  /// **'Light'**
  String get settingsThemeLight;

  /// No description provided for @settingsThemeDark.
  ///
  /// In en, this message translates to:
  /// **'Dark'**
  String get settingsThemeDark;

  /// No description provided for @settingsAutoLock.
  ///
  /// In en, this message translates to:
  /// **'Auto-lock timeout'**
  String get settingsAutoLock;

  /// No description provided for @settingsAutoLockNote.
  ///
  /// In en, this message translates to:
  /// **'Configured via CLI or TUI'**
  String get settingsAutoLockNote;

  /// No description provided for @settingsAbout.
  ///
  /// In en, this message translates to:
  /// **'About'**
  String get settingsAbout;

  /// No description provided for @settingsVersion.
  ///
  /// In en, this message translates to:
  /// **'Version'**
  String get settingsVersion;

  /// No description provided for @settingsLicenses.
  ///
  /// In en, this message translates to:
  /// **'Open source licenses'**
  String get settingsLicenses;

  /// No description provided for @generatorTitle.
  ///
  /// In en, this message translates to:
  /// **'Password generator'**
  String get generatorTitle;

  /// No description provided for @generatorRandom.
  ///
  /// In en, this message translates to:
  /// **'Random'**
  String get generatorRandom;

  /// No description provided for @generatorDiceware.
  ///
  /// In en, this message translates to:
  /// **'Diceware'**
  String get generatorDiceware;

  /// No description provided for @generatorLength.
  ///
  /// In en, this message translates to:
  /// **'Length'**
  String get generatorLength;

  /// No description provided for @generatorWords.
  ///
  /// In en, this message translates to:
  /// **'Words'**
  String get generatorWords;

  /// No description provided for @generatorLowercase.
  ///
  /// In en, this message translates to:
  /// **'Lowercase'**
  String get generatorLowercase;

  /// No description provided for @generatorUppercase.
  ///
  /// In en, this message translates to:
  /// **'Uppercase'**
  String get generatorUppercase;

  /// No description provided for @generatorDigits.
  ///
  /// In en, this message translates to:
  /// **'Digits'**
  String get generatorDigits;

  /// No description provided for @generatorSymbols.
  ///
  /// In en, this message translates to:
  /// **'Symbols'**
  String get generatorSymbols;

  /// No description provided for @generatorExcludeAmbiguous.
  ///
  /// In en, this message translates to:
  /// **'Exclude ambiguous'**
  String get generatorExcludeAmbiguous;

  /// No description provided for @generatorSeparator.
  ///
  /// In en, this message translates to:
  /// **'Separator'**
  String get generatorSeparator;

  /// No description provided for @generatorEntropy.
  ///
  /// In en, this message translates to:
  /// **'{bits} bits'**
  String generatorEntropy(String bits);

  /// No description provided for @generatorCopy.
  ///
  /// In en, this message translates to:
  /// **'Copy password'**
  String get generatorCopy;

  /// No description provided for @generatorRegenerate.
  ///
  /// In en, this message translates to:
  /// **'Regenerate'**
  String get generatorRegenerate;

  /// No description provided for @entryDetailTitle.
  ///
  /// In en, this message translates to:
  /// **'Entry'**
  String get entryDetailTitle;

  /// No description provided for @entryDetailUsername.
  ///
  /// In en, this message translates to:
  /// **'Username'**
  String get entryDetailUsername;

  /// No description provided for @entryDetailPassword.
  ///
  /// In en, this message translates to:
  /// **'Password'**
  String get entryDetailPassword;

  /// No description provided for @entryDetailUrl.
  ///
  /// In en, this message translates to:
  /// **'URL'**
  String get entryDetailUrl;

  /// No description provided for @entryDetailNotes.
  ///
  /// In en, this message translates to:
  /// **'Notes'**
  String get entryDetailNotes;

  /// No description provided for @entryDetailTotp.
  ///
  /// In en, this message translates to:
  /// **'TOTP'**
  String get entryDetailTotp;

  /// No description provided for @entryDetailAttachments.
  ///
  /// In en, this message translates to:
  /// **'Attachments'**
  String get entryDetailAttachments;

  /// No description provided for @entryDetailHistory.
  ///
  /// In en, this message translates to:
  /// **'History'**
  String get entryDetailHistory;

  /// No description provided for @entryDetailTags.
  ///
  /// In en, this message translates to:
  /// **'Tags'**
  String get entryDetailTags;

  /// No description provided for @entryDetailCustomFields.
  ///
  /// In en, this message translates to:
  /// **'Custom fields'**
  String get entryDetailCustomFields;

  /// No description provided for @entryDetailCreated.
  ///
  /// In en, this message translates to:
  /// **'Created'**
  String get entryDetailCreated;

  /// No description provided for @entryDetailModified.
  ///
  /// In en, this message translates to:
  /// **'Modified'**
  String get entryDetailModified;

  /// No description provided for @entryDetailExpires.
  ///
  /// In en, this message translates to:
  /// **'Expires'**
  String get entryDetailExpires;

  /// No description provided for @entryDetailExpired.
  ///
  /// In en, this message translates to:
  /// **'Expired'**
  String get entryDetailExpired;

  /// No description provided for @entryEditTitle.
  ///
  /// In en, this message translates to:
  /// **'Edit entry'**
  String get entryEditTitle;

  /// No description provided for @entryCreateTitle.
  ///
  /// In en, this message translates to:
  /// **'New entry'**
  String get entryCreateTitle;

  /// No description provided for @entryEditUnsavedTitle.
  ///
  /// In en, this message translates to:
  /// **'Unsaved changes'**
  String get entryEditUnsavedTitle;

  /// No description provided for @entryEditUnsavedMessage.
  ///
  /// In en, this message translates to:
  /// **'Discard changes?'**
  String get entryEditUnsavedMessage;

  /// No description provided for @entryEditDiscard.
  ///
  /// In en, this message translates to:
  /// **'Discard'**
  String get entryEditDiscard;

  /// No description provided for @entryEditReloadAfterSync.
  ///
  /// In en, this message translates to:
  /// **'The vault changed during sync. Close and reopen this editor before saving.'**
  String get entryEditReloadAfterSync;

  /// No description provided for @entryRefreshInProgress.
  ///
  /// In en, this message translates to:
  /// **'Refreshing entry after sync'**
  String get entryRefreshInProgress;

  /// No description provided for @entryEditKindCredential.
  ///
  /// In en, this message translates to:
  /// **'Credential'**
  String get entryEditKindCredential;

  /// No description provided for @entryEditKindSecureNote.
  ///
  /// In en, this message translates to:
  /// **'Secure note'**
  String get entryEditKindSecureNote;

  /// No description provided for @entryEditKindTotp.
  ///
  /// In en, this message translates to:
  /// **'TOTP'**
  String get entryEditKindTotp;

  /// No description provided for @entryEditTitleLabel.
  ///
  /// In en, this message translates to:
  /// **'Title'**
  String get entryEditTitleLabel;

  /// No description provided for @entryEditUsernameLabel.
  ///
  /// In en, this message translates to:
  /// **'Username'**
  String get entryEditUsernameLabel;

  /// No description provided for @entryEditPasswordLabel.
  ///
  /// In en, this message translates to:
  /// **'Password'**
  String get entryEditPasswordLabel;

  /// No description provided for @entryEditUrlLabel.
  ///
  /// In en, this message translates to:
  /// **'URL'**
  String get entryEditUrlLabel;

  /// No description provided for @entryEditNotesLabel.
  ///
  /// In en, this message translates to:
  /// **'Notes'**
  String get entryEditNotesLabel;

  /// No description provided for @entryEditTotpUriLabel.
  ///
  /// In en, this message translates to:
  /// **'TOTP URI'**
  String get entryEditTotpUriLabel;

  /// No description provided for @entryEditTagsLabel.
  ///
  /// In en, this message translates to:
  /// **'Tags (comma-separated)'**
  String get entryEditTagsLabel;

  /// No description provided for @entryEditAddCustomField.
  ///
  /// In en, this message translates to:
  /// **'Add custom field'**
  String get entryEditAddCustomField;

  /// No description provided for @entryEditCustomFieldName.
  ///
  /// In en, this message translates to:
  /// **'Field name'**
  String get entryEditCustomFieldName;

  /// No description provided for @entryEditCustomFieldValue.
  ///
  /// In en, this message translates to:
  /// **'Value'**
  String get entryEditCustomFieldValue;

  /// No description provided for @entryEditCustomFieldProtected.
  ///
  /// In en, this message translates to:
  /// **'Protected'**
  String get entryEditCustomFieldProtected;

  /// No description provided for @entryDeleteConfirmTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete entry'**
  String get entryDeleteConfirmTitle;

  /// No description provided for @entryDeleteConfirmMessage.
  ///
  /// In en, this message translates to:
  /// **'Move this entry to the recycle bin?'**
  String get entryDeleteConfirmMessage;

  /// No description provided for @entryPurgeConfirmTitle.
  ///
  /// In en, this message translates to:
  /// **'Permanently delete'**
  String get entryPurgeConfirmTitle;

  /// No description provided for @entryPurgeConfirmMessage.
  ///
  /// In en, this message translates to:
  /// **'This cannot be undone.'**
  String get entryPurgeConfirmMessage;

  /// No description provided for @entryMoveTitle.
  ///
  /// In en, this message translates to:
  /// **'Move entry'**
  String get entryMoveTitle;

  /// No description provided for @entryHistoryTitle.
  ///
  /// In en, this message translates to:
  /// **'Entry history'**
  String get entryHistoryTitle;

  /// No description provided for @entryHistoryVersion.
  ///
  /// In en, this message translates to:
  /// **'v{number}'**
  String entryHistoryVersion(int number);

  /// No description provided for @groupCreateTitle.
  ///
  /// In en, this message translates to:
  /// **'New group'**
  String get groupCreateTitle;

  /// No description provided for @groupRenameTitle.
  ///
  /// In en, this message translates to:
  /// **'Rename group'**
  String get groupRenameTitle;

  /// No description provided for @groupDeleteTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete group'**
  String get groupDeleteTitle;

  /// No description provided for @groupDeleteMoveToParent.
  ///
  /// In en, this message translates to:
  /// **'Move contents to parent'**
  String get groupDeleteMoveToParent;

  /// No description provided for @groupDeleteMoveToRecycleBin.
  ///
  /// In en, this message translates to:
  /// **'Move contents to recycle bin'**
  String get groupDeleteMoveToRecycleBin;

  /// No description provided for @groupNameLabel.
  ///
  /// In en, this message translates to:
  /// **'Group name'**
  String get groupNameLabel;

  /// No description provided for @recycleBin.
  ///
  /// In en, this message translates to:
  /// **'Recycle bin'**
  String get recycleBin;

  /// No description provided for @attachmentSizeExceeded.
  ///
  /// In en, this message translates to:
  /// **'File too large'**
  String get attachmentSizeExceeded;

  /// No description provided for @attachmentDetachConfirm.
  ///
  /// In en, this message translates to:
  /// **'Remove this attachment?'**
  String get attachmentDetachConfirm;

  /// No description provided for @attachmentBytes.
  ///
  /// In en, this message translates to:
  /// **'{size} bytes'**
  String attachmentBytes(String size);

  /// No description provided for @expirationSet.
  ///
  /// In en, this message translates to:
  /// **'Set expiration'**
  String get expirationSet;

  /// No description provided for @expirationClear.
  ///
  /// In en, this message translates to:
  /// **'Clear expiration'**
  String get expirationClear;

  /// No description provided for @searchHintText.
  ///
  /// In en, this message translates to:
  /// **'Search entries…'**
  String get searchHintText;

  /// No description provided for @searchModeSubstring.
  ///
  /// In en, this message translates to:
  /// **'Contains'**
  String get searchModeSubstring;

  /// No description provided for @searchModeFuzzy.
  ///
  /// In en, this message translates to:
  /// **'Fuzzy'**
  String get searchModeFuzzy;

  /// No description provided for @searchModeWildcard.
  ///
  /// In en, this message translates to:
  /// **'Wildcard'**
  String get searchModeWildcard;

  /// No description provided for @searchIncludeRecycled.
  ///
  /// In en, this message translates to:
  /// **'Include recycled'**
  String get searchIncludeRecycled;

  /// No description provided for @vaultCreateTitle.
  ///
  /// In en, this message translates to:
  /// **'Create vault'**
  String get vaultCreateTitle;

  /// No description provided for @vaultImportTitle.
  ///
  /// In en, this message translates to:
  /// **'Import vault'**
  String get vaultImportTitle;

  /// No description provided for @vaultConnectSyncTitle.
  ///
  /// In en, this message translates to:
  /// **'Connect to sync'**
  String get vaultConnectSyncTitle;

  /// No description provided for @vaultCreateNameLabel.
  ///
  /// In en, this message translates to:
  /// **'Vault name'**
  String get vaultCreateNameLabel;

  /// No description provided for @vaultCreatePasswordLabel.
  ///
  /// In en, this message translates to:
  /// **'Master password'**
  String get vaultCreatePasswordLabel;

  /// No description provided for @vaultCreateConfirmPasswordLabel.
  ///
  /// In en, this message translates to:
  /// **'Confirm password'**
  String get vaultCreateConfirmPasswordLabel;

  /// No description provided for @vaultCreateNoRecoveryWarning.
  ///
  /// In en, this message translates to:
  /// **'There is no password recovery. If you forget your master password, your data is permanently lost.'**
  String get vaultCreateNoRecoveryWarning;

  /// No description provided for @vaultCreateNoRecoveryConfirm.
  ///
  /// In en, this message translates to:
  /// **'Type CONFIRM to acknowledge'**
  String get vaultCreateNoRecoveryConfirm;

  /// No description provided for @vaultCreateNoRecoveryPhrase.
  ///
  /// In en, this message translates to:
  /// **'CONFIRM'**
  String get vaultCreateNoRecoveryPhrase;

  /// No description provided for @vaultCreateKeyfileLabel.
  ///
  /// In en, this message translates to:
  /// **'Protect with keyfile (optional)'**
  String get vaultCreateKeyfileLabel;

  /// No description provided for @vaultDeregisterTitle.
  ///
  /// In en, this message translates to:
  /// **'Remove vault'**
  String get vaultDeregisterTitle;

  /// No description provided for @vaultDeregisterMessage.
  ///
  /// In en, this message translates to:
  /// **'Remove this vault from the registry?'**
  String get vaultDeregisterMessage;

  /// No description provided for @vaultDeregisterDeleteFile.
  ///
  /// In en, this message translates to:
  /// **'Also delete the vault file'**
  String get vaultDeregisterDeleteFile;

  /// No description provided for @vaultDeregisterDeleteConfirm.
  ///
  /// In en, this message translates to:
  /// **'Are you sure? This permanently deletes the vault file.'**
  String get vaultDeregisterDeleteConfirm;

  /// No description provided for @vaultChangePasswordTitle.
  ///
  /// In en, this message translates to:
  /// **'Change master password'**
  String get vaultChangePasswordTitle;

  /// No description provided for @vaultChangePasswordCurrent.
  ///
  /// In en, this message translates to:
  /// **'Current password'**
  String get vaultChangePasswordCurrent;

  /// No description provided for @vaultChangePasswordNew.
  ///
  /// In en, this message translates to:
  /// **'New password'**
  String get vaultChangePasswordNew;

  /// No description provided for @vaultChangePasswordConfirm.
  ///
  /// In en, this message translates to:
  /// **'Confirm new password'**
  String get vaultChangePasswordConfirm;

  /// No description provided for @vaultChangePasswordSuccess.
  ///
  /// In en, this message translates to:
  /// **'Master password changed'**
  String get vaultChangePasswordSuccess;

  /// No description provided for @vaultChangePasswordReenterSync.
  ///
  /// In en, this message translates to:
  /// **'Password changed. Re-enter your S3 sync credentials.'**
  String get vaultChangePasswordReenterSync;

  /// No description provided for @vaultPasswordMismatch.
  ///
  /// In en, this message translates to:
  /// **'Passwords do not match'**
  String get vaultPasswordMismatch;

  /// No description provided for @firstRunTitle.
  ///
  /// In en, this message translates to:
  /// **'Welcome to Hidlins'**
  String get firstRunTitle;

  /// No description provided for @firstRunCreateVault.
  ///
  /// In en, this message translates to:
  /// **'Create a new vault'**
  String get firstRunCreateVault;

  /// No description provided for @firstRunConnectSync.
  ///
  /// In en, this message translates to:
  /// **'Connect to an existing vault via sync'**
  String get firstRunConnectSync;

  /// No description provided for @firstRunImportVault.
  ///
  /// In en, this message translates to:
  /// **'Import a .kdbx file'**
  String get firstRunImportVault;

  /// No description provided for @bridgeInitError.
  ///
  /// In en, this message translates to:
  /// **'Bridge initialization failed'**
  String get bridgeInitError;

  /// No description provided for @errorGeneric.
  ///
  /// In en, this message translates to:
  /// **'An error occurred'**
  String get errorGeneric;

  /// No description provided for @errorCopyFailed.
  ///
  /// In en, this message translates to:
  /// **'Copy failed'**
  String get errorCopyFailed;

  /// No description provided for @errorRevealFailed.
  ///
  /// In en, this message translates to:
  /// **'Could not reveal field'**
  String get errorRevealFailed;

  /// No description provided for @errorVaultAlreadyExists.
  ///
  /// In en, this message translates to:
  /// **'A vault with this name already exists'**
  String get errorVaultAlreadyExists;

  /// No description provided for @validatorRequired.
  ///
  /// In en, this message translates to:
  /// **'Required'**
  String get validatorRequired;

  /// No description provided for @syncS3BucketLabel.
  ///
  /// In en, this message translates to:
  /// **'S3 Bucket'**
  String get syncS3BucketLabel;

  /// No description provided for @syncObjectKeyLabel.
  ///
  /// In en, this message translates to:
  /// **'Object key'**
  String get syncObjectKeyLabel;

  /// No description provided for @syncRegionLabel.
  ///
  /// In en, this message translates to:
  /// **'Region'**
  String get syncRegionLabel;

  /// No description provided for @syncEndpointLabel.
  ///
  /// In en, this message translates to:
  /// **'Endpoint (optional)'**
  String get syncEndpointLabel;

  /// No description provided for @syncPathStyleLabel.
  ///
  /// In en, this message translates to:
  /// **'Path-style addressing'**
  String get syncPathStyleLabel;

  /// No description provided for @syncAccessKeyIdLabel.
  ///
  /// In en, this message translates to:
  /// **'Access key ID'**
  String get syncAccessKeyIdLabel;

  /// No description provided for @syncSecretAccessKeyLabel.
  ///
  /// In en, this message translates to:
  /// **'Secret access key'**
  String get syncSecretAccessKeyLabel;

  /// No description provided for @syncErrorDuplicate.
  ///
  /// In en, this message translates to:
  /// **'Duplicate sync target: already used by \"{existingVault}\"'**
  String syncErrorDuplicate(String existingVault);

  /// No description provided for @syncErrorUnreachable.
  ///
  /// In en, this message translates to:
  /// **'Cannot reach {endpoint}'**
  String syncErrorUnreachable(String endpoint);

  /// No description provided for @syncErrorAuthFailed.
  ///
  /// In en, this message translates to:
  /// **'S3 authentication failed'**
  String get syncErrorAuthFailed;

  /// No description provided for @syncTitle.
  ///
  /// In en, this message translates to:
  /// **'Vault sync'**
  String get syncTitle;

  /// No description provided for @syncNow.
  ///
  /// In en, this message translates to:
  /// **'Sync now'**
  String get syncNow;

  /// No description provided for @syncNotConfigured.
  ///
  /// In en, this message translates to:
  /// **'Sync is not configured'**
  String get syncNotConfigured;

  /// No description provided for @syncConfigureTitle.
  ///
  /// In en, this message translates to:
  /// **'S3 configuration'**
  String get syncConfigureTitle;

  /// No description provided for @syncConfigureSuccess.
  ///
  /// In en, this message translates to:
  /// **'Sync configuration saved'**
  String get syncConfigureSuccess;

  /// No description provided for @syncCredentialSourceLabel.
  ///
  /// In en, this message translates to:
  /// **'Credential storage'**
  String get syncCredentialSourceLabel;

  /// No description provided for @syncCredentialSourceEncrypted.
  ///
  /// In en, this message translates to:
  /// **'Encrypted in the vault registry'**
  String get syncCredentialSourceEncrypted;

  /// No description provided for @syncBusyBanner.
  ///
  /// In en, this message translates to:
  /// **'Syncing in the background. Cached entries remain available.'**
  String get syncBusyBanner;

  /// No description provided for @syncActionsDisabled.
  ///
  /// In en, this message translates to:
  /// **'Changes are disabled until sync finishes'**
  String get syncActionsDisabled;

  /// No description provided for @syncOutcomeAlreadyInSync.
  ///
  /// In en, this message translates to:
  /// **'Vault is already in sync'**
  String get syncOutcomeAlreadyInSync;

  /// No description provided for @syncOutcomePushed.
  ///
  /// In en, this message translates to:
  /// **'Local changes uploaded'**
  String get syncOutcomePushed;

  /// No description provided for @syncOutcomeFastReplaced.
  ///
  /// In en, this message translates to:
  /// **'Vault updated from remote'**
  String get syncOutcomeFastReplaced;

  /// No description provided for @syncOutcomeMerged.
  ///
  /// In en, this message translates to:
  /// **'Local and remote changes merged'**
  String get syncOutcomeMerged;

  /// No description provided for @syncOutcomeUnknown.
  ///
  /// In en, this message translates to:
  /// **'Sync completed'**
  String get syncOutcomeUnknown;

  /// No description provided for @syncConflictTitle.
  ///
  /// In en, this message translates to:
  /// **'Sync needs attention'**
  String get syncConflictTitle;

  /// No description provided for @syncConflictMessage.
  ///
  /// In en, this message translates to:
  /// **'The conflict could not be merged automatically. Both sides were preserved and the pre-merge vault was saved as a backup.'**
  String get syncConflictMessage;

  /// No description provided for @syncConflictBackupLabel.
  ///
  /// In en, this message translates to:
  /// **'Backup:'**
  String get syncConflictBackupLabel;

  /// No description provided for @syncFailureGeneric.
  ///
  /// In en, this message translates to:
  /// **'Sync failed. Your local vault is still available.'**
  String get syncFailureGeneric;

  /// No description provided for @syncEndpointDefault.
  ///
  /// In en, this message translates to:
  /// **'remote'**
  String get syncEndpointDefault;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}

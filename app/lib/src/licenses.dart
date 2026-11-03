import 'package:flutter/foundation.dart';

const _effWordlistNotice = r'''EFF Large Wordlist (2016)
Creator: Joseph Bonneau, Electronic Frontier Foundation (EFF).
Source: https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt
Licensed under Creative Commons Attribution 4.0 International (CC-BY-4.0).
License: https://creativecommons.org/licenses/by/4.0/
Upstream terms: https://www.eff.org/copyright
This copy of the wordlist is unmodified.
''';

bool _assetLicensesRegistered = false;

/// Register bundled asset notices lazily for the existing license page.
void registerHidlinsAssetLicenses() {
  if (_assetLicensesRegistered) return;
  _assetLicensesRegistered = true;
  LicenseRegistry.addLicense(() async* {
    yield LicenseEntryWithLineBreaks(const [
      'EFF Large Wordlist',
    ], _effWordlistNotice);
  });
}

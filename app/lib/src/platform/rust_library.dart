import 'package:flutter/foundation.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart'
    show ExternalLibrary;

import '../bridge/frb_generated.dart';

/// Loads the Cargokit framework name that is actually embedded in Apple apps.
Future<void> initializeBundledRustLibrary() {
  if (defaultTargetPlatform == TargetPlatform.iOS ||
      defaultTargetPlatform == TargetPlatform.macOS) {
    return RustLib.init(
      externalLibrary: ExternalLibrary.open(
        'rust_lib_app.framework/rust_lib_app',
      ),
    );
  }
  return RustLib.init();
}

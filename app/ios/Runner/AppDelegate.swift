import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    // The Rust bridge is FFI-only and linked by CocoaPods/Cargokit, so it has
    // no GeneratedPluginRegistrant entry. Register Hidlins' fixed native
    // mechanism channels against the implicit engine explicitly.
    guard let registrar = engineBridge.pluginRegistry.registrar(
      forPlugin: "HidlinsPlatformServices"
    ) else {
      preconditionFailure("Hidlins platform registrar unavailable")
    }
    HidlinsPlatformServices.shared.register(with: registrar.messenger())
  }
}

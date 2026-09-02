import Flutter
import UIKit

class SceneDelegate: FlutterSceneDelegate {
  override func scene(
    _ scene: UIScene,
    willConnectTo session: UISceneSession,
    options connectionOptions: UIScene.ConnectionOptions
  ) {
    super.scene(scene, willConnectTo: session, options: connectionOptions)
    HidlinsPlatformServices.shared.snapshotShield.show(in: window)
  }

  override func sceneWillResignActive(_ scene: UIScene) {
    HidlinsPlatformServices.shared.reportLifecycle("inactive", window: window)
    super.sceneWillResignActive(scene)
  }

  override func sceneDidEnterBackground(_ scene: UIScene) {
    HidlinsPlatformServices.shared.reportLifecycle("paused", window: window)
    super.sceneDidEnterBackground(scene)
  }

  override func sceneDidBecomeActive(_ scene: UIScene) {
    super.sceneDidBecomeActive(scene)
    // The shield stays installed until Dart has synchronously reported the
    // raw signal to Rust and sends an acknowledgement on the same channel.
    HidlinsPlatformServices.shared.reportLifecycle("resumed", window: window)
  }
}

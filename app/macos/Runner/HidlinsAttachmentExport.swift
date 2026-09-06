import Cocoa
import FlutterMacOS

final class HidlinsAttachmentExport {
  static let channelName = "app.hidlins/attachment_export"
  static let methodName = "chooseDestination"

  private var channel: FlutterMethodChannel?

  func register(with messenger: FlutterBinaryMessenger) {
    let channel = FlutterMethodChannel(
      name: Self.channelName,
      binaryMessenger: messenger
    )
    channel.setMethodCallHandler { [weak self] call, result in
      self?.handle(call, result: result)
    }
    self.channel = channel
  }

  private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard call.method == Self.methodName else {
      result(FlutterMethodNotImplemented)
      return
    }
    guard
      let arguments = call.arguments as? [String: Any],
      let suggestedName = arguments["suggestedName"] as? String,
      Self.isSafeSuggestedName(suggestedName)
    else {
      result(Self.failure("invalid-suggested-name"))
      return
    }

    let panel = NSSavePanel()
    panel.nameFieldStringValue = suggestedName
    panel.canCreateDirectories = true
    panel.isExtensionHidden = false
    panel.begin { response in
      if response == .OK, let path = panel.url?.path, !path.isEmpty {
        result(["status": "success", "value": path])
      } else if response == .cancel {
        result(["status": "canceled"])
      } else {
        result(Self.failure("save-panel-failed"))
      }
    }
  }

  static func isSafeSuggestedName(_ value: String) -> Bool {
    !value.isEmpty
      && value != "."
      && value != ".."
      && !value.contains("/")
      && !value.contains("\\")
      && value.unicodeScalars.allSatisfy {
        !CharacterSet.controlCharacters.contains($0)
      }
  }

  private static func failure(_ code: String) -> [String: String] {
    ["status": "failure", "code": code]
  }
}

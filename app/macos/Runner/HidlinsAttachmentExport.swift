import Cocoa
import FlutterMacOS

final class HidlinsAttachmentExport {
  static let channelName = "app.hidlins/attachment_export"
  static let methodName = "chooseDestination"

  private var channel: FlutterMethodChannel?
  private let makeOpenPanel: () -> NSOpenPanel
  private let makeSavePanel: () -> NSSavePanel

  init(
    makeOpenPanel: @escaping () -> NSOpenPanel = { NSOpenPanel() },
    makeSavePanel: @escaping () -> NSSavePanel = { NSSavePanel() }
  ) {
    self.makeOpenPanel = makeOpenPanel
    self.makeSavePanel = makeSavePanel
  }

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
    if call.method == "chooseSource" {
      guard call.arguments == nil || call.arguments is NSNull else {
        result(Self.failure("invalid-source-arguments"))
        return
      }
      let panel = makeOpenPanel()
      panel.title = "Attach file"
      panel.canChooseFiles = true
      panel.canChooseDirectories = false
      panel.allowsMultipleSelection = false
      panel.canCreateDirectories = false
      panel.begin { response in
        if response == .OK {
          guard let url = panel.url, url.isFileURL,
                !url.path.isEmpty, url.path.hasPrefix("/")
          else {
            result(Self.failure("source-dialog-failed"))
            return
          }
          result(["status": "success", "value": url.path])
        } else if response == .cancel {
          result(["status": "canceled"])
        } else {
          result(Self.failure("source-dialog-failed"))
        }
      }
      return
    }
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

    let panel = makeSavePanel()
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

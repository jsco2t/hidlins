import Cocoa
import Darwin
import FlutterMacOS

private enum CheckError: Error { case failed(String) }
private func require(_ value: Bool, _ message: String) throws {
  if !value { throw CheckError.failed(message) }
}

private final class ControlledOpenPanel: NSOpenPanel {
  var selectedURL: URL?
  var response: NSApplication.ModalResponse = .cancel
  var began = false
  override var url: URL? { selectedURL }
  override func begin(completionHandler handler: @escaping (NSApplication.ModalResponse) -> Void) {
    began = true
    handler(response)
  }
}

private final class ControlledSavePanel: NSSavePanel {
  var selectedURL: URL?
  var response: NSApplication.ModalResponse = .cancel
  var began = false
  override var url: URL? { selectedURL }
  override func begin(completionHandler handler: @escaping (NSApplication.ModalResponse) -> Void) {
    began = true
    handler(response)
  }
}

private final class SelectionMessenger: NSObject, FlutterBinaryMessenger {
  private var handler: FlutterBinaryMessageHandler?
  private let codec = FlutterStandardMethodCodec.sharedInstance()
  func send(onChannel channel: String, message: Data?) {}
  func send(onChannel channel: String, message: Data?, binaryReply callback: FlutterBinaryReply?) { callback?(nil) }
  func setMessageHandlerOnChannel(
    _ channel: String, binaryMessageHandler handler: FlutterBinaryMessageHandler?
  ) -> FlutterBinaryMessengerConnection {
    if channel == HidlinsAttachmentExport.channelName { self.handler = handler }
    return 1
  }
  func cleanUpConnection(_ connection: FlutterBinaryMessengerConnection) { handler = nil }
  func call(_ method: String, arguments: Any? = nil) throws -> [String: Any] {
    guard let handler else { throw CheckError.failed("production-handler-not-installed") }
    var replies = 0
    var envelope: [String: Any]?
    handler(codec.encode(FlutterMethodCall(methodName: method, arguments: arguments))) { data in
      replies += 1
      if let data { envelope = self.codec.decodeEnvelope(data) as? [String: Any] }
    }
    try require(replies == 1, "single-synchronous-controlled-panel-reply")
    guard let envelope else { throw CheckError.failed("missing-result-envelope") }
    return envelope
  }
}

@main
private enum AttachmentSelectionTests {
  static func main() {
    do {
      NSApplication.shared.setActivationPolicy(.accessory)
      let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
      try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
      defer { try? FileManager.default.removeItem(at: root) }
      let file = root.appendingPathComponent("attachment unicode-é.txt")
      let bytes = Data("first-party-native-picker-fixture".utf8)
      try bytes.write(to: file)
      let cases: [(NSApplication.ModalResponse, URL?, String)] = [
        (.OK, file, "success"),
        (.cancel, nil, "canceled"),
        (.abort, nil, "failure"),
        (.OK, nil, "failure"),
        (.OK, URL(string: "https://example.invalid/file"), "failure"),
      ]
      var passed = 0
      for null in [false, true] {
        for (response, selected, status) in cases {
          let panel = ControlledOpenPanel()
          panel.response = response
          panel.selectedURL = selected
          let adapter = HidlinsAttachmentExport(makeOpenPanel: { panel })
          let messenger = SelectionMessenger()
          adapter.register(with: messenger)
          let result = try messenger.call("chooseSource", arguments: null ? NSNull() : nil)
          try require(panel.began, "open-panel-must-begin")
          try require(panel.canChooseFiles && !panel.canChooseDirectories && !panel.allowsMultipleSelection,
                      "single-file-open-policy")
          try require(!panel.canCreateDirectories, "source-picker-must-not-create-folders")
          try require(result["status"] as? String == status, "open-response-status")
          if status == "success" {
            try require(result["value"] as? String == file.path && result.count == 2, "exact-selected-file-path")
          } else if status == "failure" {
            try require(result["code"] as? String == "source-dialog-failed" && result.count == 2,
                        "bounded-source-failure")
          } else { try require(result.count == 1, "canceled-has-no-path") }
          try require(!panel.isVisible, "controlled-tests-must-not-display-a-panel")
          passed += 1
        }
      }
      for response in [NSApplication.ModalResponse.OK, .cancel, .abort] {
        let panel = ControlledSavePanel()
        panel.response = response
        let destination = root.appendingPathComponent("export.bin")
        panel.selectedURL = destination
        let adapter = HidlinsAttachmentExport(makeSavePanel: { panel })
        let messenger = SelectionMessenger()
        adapter.register(with: messenger)
        let result = try messenger.call("chooseDestination", arguments: ["suggestedName": "export.bin"])
        try require(panel.began && panel.canCreateDirectories && !panel.isExtensionHidden, "save-policy-preserved")
        let expected = response == .OK ? "success" : response == .cancel ? "canceled" : "failure"
        try require(result["status"] as? String == expected, "save-response-status")
        if response == .OK { try require(result["value"] as? String == destination.path, "exact-save-path") }
        try require(!FileManager.default.fileExists(atPath: destination.path), "chooser-must-not-write-file-bytes")
        passed += 1
      }
      try require(try Data(contentsOf: file) == bytes, "chooser-must-not-change-source-bytes")
      print("PASS native Cocoa selection callbacks: \(passed) cases; no desktop input or displayed test panels")
    } catch {
      print("FAIL native Cocoa selection callbacks: \(error)")
      exit(1)
    }
  }
}

import Cocoa
import FlutterMacOS
import XCTest

@testable import Hidlins

private final class AttachmentTestMessenger: NSObject, FlutterBinaryMessenger {
  private let codec = FlutterStandardMethodCodec.sharedInstance()
  private var handlers: [String: (FlutterBinaryMessengerConnection, FlutterBinaryMessageHandler)] = [:]
  private var nextConnection: FlutterBinaryMessengerConnection = 0

  func send(onChannel channel: String, message: Data?) {}
  func send(onChannel channel: String, message: Data?, binaryReply callback: FlutterBinaryReply?) {
    callback?(nil)
  }

  func setMessageHandlerOnChannel(
    _ channel: String, binaryMessageHandler handler: FlutterBinaryMessageHandler?
  ) -> FlutterBinaryMessengerConnection {
    nextConnection += 1
    if let handler { handlers[channel] = (nextConnection, handler) }
    else { handlers.removeValue(forKey: channel) }
    return nextConnection
  }

  func cleanUpConnection(_ connection: FlutterBinaryMessengerConnection) {
    if let channel = handlers.first(where: { $0.value.0 == connection })?.key {
      handlers.removeValue(forKey: channel)
    }
  }

  func call(_ method: String, arguments: Any? = nil, reply: @escaping (Data?) -> Void) {
    guard let handler = handlers[HidlinsAttachmentExport.channelName]?.1 else {
      XCTFail("Production attachment channel was not installed")
      return
    }
    handler(codec.encode(FlutterMethodCall(methodName: method, arguments: arguments)), reply)
  }

  func decode(_ data: Data?) -> [String: Any]? {
    guard let data else { return nil }
    return codec.decodeEnvelope(data) as? [String: Any]
  }
}

class RunnerTests: XCTestCase {
  @MainActor
  private func visiblePanel(open: Bool) -> NSSavePanel? {
    let deadline = Date().addingTimeInterval(5)
    while Date() < deadline {
      if let panel = NSApp.windows.compactMap({ $0 as? NSSavePanel }).first(where: {
        $0.isVisible && (($0 is NSOpenPanel) == open)
      }) { return panel }
      RunLoop.current.run(until: Date().addingTimeInterval(0.02))
    }
    return nil
  }

  @MainActor
  private func pump(until condition: () -> Bool) {
    let deadline = Date().addingTimeInterval(5)
    while !condition() && Date() < deadline {
      RunLoop.current.run(until: Date().addingTimeInterval(0.02))
    }
    XCTAssertTrue(condition(), "Native panel callback did not complete")
  }

  @MainActor
  func testSourceChooserInstallsSingleFileOpenPanelAndReturnsCancellation() throws {
    let adapter = HidlinsAttachmentExport()
    let messenger = AttachmentTestMessenger()
    adapter.register(with: messenger)
    var result: [String: Any]?
    var callbacks = 0
    var notImplemented = false
    messenger.call("chooseSource") { data in
      callbacks += 1
      notImplemented = data == nil
      result = messenger.decode(data)
    }
    XCTAssertFalse(notImplemented, "Native chooseSource must be implemented")
    let panel = try XCTUnwrap(visiblePanel(open: true) as? NSOpenPanel,
                             "Native chooseSource must present a real NSOpenPanel")
    defer { if panel.isVisible { panel.cancel(nil) } }
    XCTAssertTrue(panel.canChooseFiles)
    XCTAssertFalse(panel.canChooseDirectories)
    XCTAssertFalse(panel.allowsMultipleSelection)
    panel.cancel(nil)
    pump { callbacks == 1 }
    XCTAssertEqual(result?["status"] as? String, "canceled")
    XCTAssertEqual(result?.count, 1)
    XCTAssertEqual(callbacks, 1)
  }

  @MainActor
  func testDestinationChooserPreservesRealSavePanelAndCancellation() throws {
    let adapter = HidlinsAttachmentExport()
    let messenger = AttachmentTestMessenger()
    adapter.register(with: messenger)
    var result: [String: Any]?
    var callbacks = 0
    messenger.call("chooseDestination", arguments: ["suggestedName": "hidlins-test-export.bin"]) { data in
      callbacks += 1
      result = messenger.decode(data)
    }
    let panel = try XCTUnwrap(visiblePanel(open: false), "SAVE must still present a native panel")
    defer { if panel.isVisible { panel.cancel(nil) } }
    XCTAssertTrue(panel.canCreateDirectories)
    XCTAssertFalse(panel.isExtensionHidden)
    panel.cancel(nil)
    pump { callbacks == 1 }
    XCTAssertEqual(result?["status"] as? String, "canceled")
    XCTAssertEqual(result?.count, 1)
  }

  func testSourceChooserRejectsNonNullArgumentsWithoutOpeningPanel() {
    let adapter = HidlinsAttachmentExport()
    let messenger = AttachmentTestMessenger()
    adapter.register(with: messenger)
    let invalidArguments: [Any] = [[String: String](), ["path": "/not-a-selection"], "file", 1, true, ["file"]]
    for arguments in invalidArguments {
      var result: [String: Any]?
      var callbacks = 0
      messenger.call("chooseSource", arguments: arguments) { data in
        callbacks += 1
        result = messenger.decode(data)
      }
      XCTAssertEqual(result?["status"] as? String, "failure")
      XCTAssertEqual(result?["code"] as? String, "invalid-source-arguments")
      XCTAssertEqual(result?.count, 2)
      XCTAssertEqual(callbacks, 1)
    }
  }

  func testDestinationValidationAndUnknownMethodsRemainBounded() {
    let adapter = HidlinsAttachmentExport()
    let messenger = AttachmentTestMessenger()
    adapter.register(with: messenger)
    for invalid in ["", ".", "..", "../x", "a/b", "a\\b", "a\nx"] {
      var result: [String: Any]?
      messenger.call("chooseDestination", arguments: ["suggestedName": invalid]) { data in
        result = messenger.decode(data)
      }
      XCTAssertEqual(result?["status"] as? String, "failure")
      XCTAssertEqual(result?["code"] as? String, "invalid-suggested-name")
      XCTAssertEqual(result?.count, 2)
    }
    var callbacks = 0
    messenger.call("unknownMethod") { data in
      callbacks += 1
      XCTAssertNil(data)
    }
    XCTAssertEqual(callbacks, 1)
    XCTAssertTrue(HidlinsAttachmentExport.isSafeSuggestedName("résumé 1.txt"))
  }
}

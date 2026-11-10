import Flutter
import Foundation
import UniformTypeIdentifiers
import UIKit
import XCTest

@testable import Runner

private final class ManualDiscoveryTask: LocalDiscoveryScheduledTask {
  private let action: () -> Void
  private(set) var isCanceled = false

  init(action: @escaping () -> Void) {
    self.action = action
  }

  func cancel() {
    isCanceled = true
  }

  func fireEvenIfCanceled() {
    action()
  }
}

private final class ManualDiscoveryScheduler: LocalDiscoveryScheduling {
  private(set) var tasks: [ManualDiscoveryTask] = []

  func schedule(
    afterMilliseconds _: Int,
    action: @escaping () -> Void
  ) -> LocalDiscoveryScheduledTask {
    let task = ManualDiscoveryTask(action: action)
    tasks.append(task)
    return task
  }
}

final class RunnerTests: XCTestCase {
  @MainActor
  func testBuiltAppLoadsRealBridgeAndCapturesInstalledResources() throws {
    XCTAssertEqual(hidlins_ios_consume_clipboard(nil, 0, nil, nil), 1)
    XCTAssertNotNil(Bundle.main.url(forResource: "Assets", withExtension: "car"))
    XCTAssertEqual(
      Bundle.main.object(forInfoDictionaryKey: "UILaunchStoryboardName") as? String,
      "LaunchScreen"
    )

    let scene = try XCTUnwrap(
      UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
    )
    let window = try XCTUnwrap(scene.windows.first)
    let image = UIGraphicsImageRenderer(bounds: window.bounds).image { context in
      window.layer.render(in: context.cgContext)
    }
    let capture = XCTAttachment(image: image)
    capture.name = "hidlins-installed-app-\(UIDevice.current.systemName)-\(UIDevice.current.systemVersion)"
    capture.lifetime = .keepAlways
    add(capture)
  }

  func testSnapshotShieldIsOpaqueAndIdempotent() {
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 320, height: 640))
    let shield = SnapshotShieldController()

    shield.show(in: window)
    shield.show(in: window)

    XCTAssertTrue(shield.isVisible)
    XCTAssertEqual(window.subviews.filter { $0.accessibilityIdentifier == "hidlins.snapshot-shield" }.count, 1)
    XCTAssertEqual(window.subviews.last?.backgroundColor, UIColor.systemBackground)

    shield.hide()
    XCTAssertFalse(shield.isVisible)
  }

  func testLifecycleAcknowledgementRequiresAuthoritativeLockState() throws {
    XCTAssertNil(
      LifecycleAcknowledgement(arguments: ["state": "resumed", "acknowledged": true])
    )
    XCTAssertNil(
      LifecycleAcknowledgement(arguments: [
        "state": "resumed", "acknowledged": true, "lockState": "unknown",
      ])
    )

    let acknowledgement = try XCTUnwrap(
      LifecycleAcknowledgement(arguments: [
        "state": "resumed", "acknowledged": true, "lockState": "locked",
      ])
    )
    XCTAssertEqual(acknowledgement.state, "resumed")
    XCTAssertEqual(acknowledgement.lockState, "locked")
  }

  func testPasteboardPolicyIsLocalAndExpiresAtBoundedTTL() throws {
    let now = Date(timeIntervalSince1970: 1_000)
    let options = try PasteboardPolicy.options(ttlSeconds: 30, now: now)

    XCTAssertEqual(options[.localOnly] as? Bool, true)
    XCTAssertEqual(options[.expirationDate] as? Date, now.addingTimeInterval(30))
    XCTAssertThrowsError(try PasteboardPolicy.options(ttlSeconds: 0, now: now))
    XCTAssertThrowsError(try PasteboardPolicy.options(ttlSeconds: 301, now: now))
  }

  func testAtomicVaultImportCopiesIntoOwnedStorageWithoutChangingSource() throws {
    let root = FileManager.default.temporaryDirectory
      .appendingPathComponent(UUID().uuidString, isDirectory: true)
    let sourceDirectory = root.appendingPathComponent("provider", isDirectory: true)
    let ownedDirectory = root.appendingPathComponent("owned", isDirectory: true)
    try FileManager.default.createDirectory(at: sourceDirectory, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }

    let source = sourceDirectory.appendingPathComponent("personal.kdbx")
    let bytes = Data("kdbx-fixture".utf8)
    try bytes.write(to: source)

    let importer = AtomicVaultImporter(ownedDirectory: ownedDirectory)
    let imported = try importer.copyFromProvider(source)

    XCTAssertEqual(try Data(contentsOf: source), bytes)
    XCTAssertEqual(try Data(contentsOf: imported), bytes)
    XCTAssertEqual(imported.deletingLastPathComponent(), ownedDirectory)
    XCTAssertFalse(imported.path.contains(".import-"))
  }

  func testApplicationSupportStoreUsesHidlinsOwnedDirectories() throws {
    let root = FileManager.default.temporaryDirectory
      .appendingPathComponent(UUID().uuidString, isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }

    let store = ApplicationSupportStore(baseDirectory: root)
    let base = try store.prepare()

    XCTAssertEqual(base, root.appendingPathComponent("Hidlins", isDirectory: true))
    var isDirectory: ObjCBool = false
    XCTAssertTrue(FileManager.default.fileExists(atPath: base.path, isDirectory: &isDirectory))
    XCTAssertTrue(isDirectory.boolValue)
  }

  func testPlatformEnvelopeNeverIncludesNativeErrorDetails() {
    XCTAssertEqual(PlatformEnvelope.failure(code: "bad-code"), ["status": "failure", "code": "bad-code"])
    XCTAssertEqual(PlatformEnvelope.failure(code: "contains secret data"), ["status": "failure", "code": "platform-error"])
  }

  func testLocalDiscoveryPayloadIsBoundedAndCarriesNoServiceMetadata() {
    let candidates = (0..<20).map { index in
      [
        "address": "192.168.1.\(index)",
        "port": 42_873,
        "scopeId": 0,
        "vault": "must-not-cross",
      ] as [String: Any]
    }
    let payload = LocalDiscoveryPayload.make(permission: .granted, candidates: candidates)
    let bounded = payload["candidates"] as? [[String: Any]]

    XCTAssertEqual(payload["permission"] as? String, "granted")
    XCTAssertEqual(bounded?.count, LocalDiscoveryPayload.maximumCandidates)
    XCTAssertFalse(String(describing: payload).contains("vault"))
    XCTAssertFalse(String(describing: payload).contains("key"))
    XCTAssertFalse(String(describing: payload).contains("sas"))
  }

  func testLocalDiscoveryStripsIPv6ZoneTextAndKeepsNumericScopeSeparate() {
    XCTAssertEqual(LocalDiscoveryService.routeHost("fe80::1%en0"), "fe80::1")
    XCTAssertEqual(LocalDiscoveryService.routeHost("fe80::1%7"), "fe80::1")
    XCTAssertNil(LocalDiscoveryService.routeHost(""))
    XCTAssertNil(LocalDiscoveryService.routeHost(String(repeating: "a", count: 65)))
  }

  // LNS-IOS-012: canceled timeout work cannot cross a stop/restart generation boundary.
  func testStoppedAttemptTimeoutCannotCompleteRestartedDiscovery() {
    let scheduler = ManualDiscoveryScheduler()
    var browsers: [NetServiceBrowser] = []
    let service = LocalDiscoveryService(
      scheduler: scheduler,
      browserFactory: {
        let browser = NetServiceBrowser()
        browsers.append(browser)
        return browser
      },
      browserSearch: { _, _ in },
      browserStop: { _ in }
    )
    var firstResults: [Any?] = []
    var secondResults: [Any?] = []

    service.discover(kind: "trusted", timeoutMilliseconds: 1_000) {
      firstResults.append($0)
    }
    service.stop()
    service.discover(kind: "trusted", timeoutMilliseconds: 1_000) {
      secondResults.append($0)
    }
    XCTAssertEqual(browsers.count, 2)
    XCTAssertEqual(firstResults.count, 1)
    XCTAssertTrue(scheduler.tasks[0].isCanceled)

    scheduler.tasks[0].fireEvenIfCanceled()

    XCTAssertTrue(secondResults.isEmpty, "attempt A's timeout must not complete attempt B")
    scheduler.tasks[1].fireEvenIfCanceled()
    XCTAssertEqual(secondResults.count, 1)
    XCTAssertTrue(scheduler.tasks[1].isCanceled)
    scheduler.tasks[1].fireEvenIfCanceled()
    service.stop()
    XCTAssertEqual(secondResults.count, 1, "active timeout completion must be exactly once")
  }

  // LNS-IOS-013: stale browser errors and state changes cannot mutate a restarted attempt.
  func testStoppedAttemptDelegateCallbacksCannotMutateRestartedDiscovery() {
    let scheduler = ManualDiscoveryScheduler()
    var browsers: [NetServiceBrowser] = []
    let service = LocalDiscoveryService(
      scheduler: scheduler,
      browserFactory: {
        let browser = NetServiceBrowser()
        browsers.append(browser)
        return browser
      },
      browserSearch: { _, _ in },
      browserStop: { _ in }
    )
    var firstResults: [Any?] = []
    var secondResults: [Any?] = []

    service.discover(kind: "trusted", timeoutMilliseconds: 1_000) {
      firstResults.append($0)
    }
    let firstBrowser = browsers[0]
    service.stop()
    service.stop()
    XCTAssertEqual(firstResults.count, 1, "repeated stop must not complete twice")
    XCTAssertNil(firstBrowser.delegate)

    service.discover(kind: "trusted", timeoutMilliseconds: 1_000) {
      secondResults.append($0)
    }
    let secondBrowser = browsers[1]
    let staleService = NetService(
      domain: "local.",
      type: "_hidlins-sync._tcp.",
      name: "stale",
      port: 42_873
    )
    service.netServiceBrowserWillSearch(secondBrowser)
    XCTAssertEqual(service.permission, .granted)
    service.netServiceBrowserWillSearch(firstBrowser)
    service.netServiceBrowser(firstBrowser, didFind: staleService, moreComing: false)
    service.netServiceBrowser(
      firstBrowser,
      didNotSearch: [NetService.errorCode: NSNumber(value: -1)]
    )
    scheduler.tasks[0].fireEvenIfCanceled()

    XCTAssertEqual(service.permission, .granted)
    XCTAssertTrue(secondResults.isEmpty)
    XCTAssertFalse(scheduler.tasks[1].isCanceled)
    XCTAssertNil(staleService.delegate)

    service.netServiceBrowser(
      secondBrowser,
      didNotSearch: [NetService.errorCode: NSNumber(value: -1)]
    )
    XCTAssertEqual(secondResults.count, 1)
    XCTAssertEqual(service.permission, .restricted)
    XCTAssertTrue(scheduler.tasks[1].isCanceled)
    XCTAssertNil(secondBrowser.delegate)

    service.netServiceBrowser(
      secondBrowser,
      didNotSearch: [NetService.errorCode: NSNumber(value: -72_008)]
    )
    scheduler.tasks[1].fireEvenIfCanceled()
    service.stop()
    XCTAssertEqual(secondResults.count, 1, "active error completion must be exactly once")
  }

  // LNS-IOS-014: an overlapping start is rejected without taking browser or timer ownership.
  func testRepeatedStartDoesNotReplaceActiveDiscoveryOwnership() {
    let scheduler = ManualDiscoveryScheduler()
    var browsers: [NetServiceBrowser] = []
    let service = LocalDiscoveryService(
      scheduler: scheduler,
      browserFactory: {
        let browser = NetServiceBrowser()
        browsers.append(browser)
        return browser
      },
      browserSearch: { _, _ in },
      browserStop: { _ in }
    )
    var activeResults: [Any?] = []
    var rejectedResults: [Any?] = []

    service.discover(kind: "trusted", timeoutMilliseconds: 1_000) {
      activeResults.append($0)
    }
    service.discover(kind: "pairing", timeoutMilliseconds: 1_000) {
      rejectedResults.append($0)
    }

    XCTAssertEqual(browsers.count, 1)
    XCTAssertEqual(scheduler.tasks.count, 1)
    XCTAssertTrue(activeResults.isEmpty)
    XCTAssertEqual(rejectedResults.count, 1)
    XCTAssertTrue(String(describing: rejectedResults[0]).contains("invalid-discovery-request"))

    service.stop()
    service.stop()
    XCTAssertEqual(activeResults.count, 1)
    XCTAssertTrue(scheduler.tasks[0].isCanceled)
  }

  func testPrivacyManifestDeclaresNoCollectionTrackingOrTrackingDomains() throws {
    let url = try XCTUnwrap(
      Bundle.main.url(forResource: "PrivacyInfo", withExtension: "xcprivacy")
    )
    let data = try Data(contentsOf: url)
    let plist = try XCTUnwrap(
      PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any]
    )

    XCTAssertEqual(plist["NSPrivacyTracking"] as? Bool, false)
    XCTAssertEqual(plist["NSPrivacyTrackingDomains"] as? [String], [])
    XCTAssertTrue(try XCTUnwrap(plist["NSPrivacyCollectedDataTypes"] as? [Any]).isEmpty)

    let accessed = try XCTUnwrap(plist["NSPrivacyAccessedAPITypes"] as? [[String: Any]])
    let categories = Set(accessed.compactMap { $0["NSPrivacyAccessedAPIType"] as? String })
    XCTAssertEqual(
      categories,
      [
        "NSPrivacyAccessedAPICategoryFileTimestamp",
        "NSPrivacyAccessedAPICategorySystemBootTime",
        "NSPrivacyAccessedAPICategoryUserDefaults",
      ]
    )
  }

  @MainActor
  func testRealPasteboardWriteExpiresWithoutReturningPayload() async throws {
    let canary = Array("hidlins-device-clipboard-canary".utf8)
    let accepted = canary.withUnsafeBufferPointer { buffer in
      HidlinsPlatformServices.shared.writeClipboard(
        bytes: buffer.baseAddress!,
        length: buffer.count,
        ttlSeconds: 1
      )
    }
    XCTAssertTrue(accepted)
    XCTAssertEqual(
      UIPasteboard.general.data(forPasteboardType: UTType.utf8PlainText.identifier),
      Data(canary)
    )
    let response = PlatformEnvelope.success()
    XCTAssertFalse(String(describing: response).contains("hidlins-device-clipboard-canary"))

    try await Task.sleep(nanoseconds: 2_000_000_000)
    XCTAssertNotEqual(
      UIPasteboard.general.data(forPasteboardType: UTType.utf8PlainText.identifier),
      Data(canary)
    )
  }
}

// Exercise the installed production method channel, including its codec boundary.
// Native lifecycle events are captured without automatically acknowledging them.
private final class LifecycleTestMessenger: NSObject, FlutterBinaryMessenger {
  private let codec = FlutterStandardMethodCodec.sharedInstance()
  private var handlers: [String: (FlutterBinaryMessengerConnection, FlutterBinaryMessageHandler)] = [:]
  private var nextConnection: FlutterBinaryMessengerConnection = 0
  private(set) var observations: [[String: Any]] = []

  func send(onChannel channel: String, message: Data?) {
    send(onChannel: channel, message: message, binaryReply: nil)
  }

  func send(onChannel channel: String, message: Data?, binaryReply callback: FlutterBinaryReply?) {
    if channel == "app.hidlins/lifecycle", let message {
      let call = codec.decodeMethodCall(message)
      if call.method == "reportState", let arguments = call.arguments as? [String: Any] {
        observations.append(arguments)
      }
    }
    callback?(codec.encodeSuccessEnvelope(nil))
  }

  func setMessageHandlerOnChannel(
    _ channel: String,
    binaryMessageHandler handler: FlutterBinaryMessageHandler?
  ) -> FlutterBinaryMessengerConnection {
    nextConnection += 1
    if let handler {
      handlers[channel] = (nextConnection, handler)
    } else {
      handlers.removeValue(forKey: channel)
    }
    return nextConnection
  }

  func cleanUpConnection(_ connection: FlutterBinaryMessengerConnection) {
    if let channel = handlers.first(where: { $0.value.0 == connection })?.key {
      handlers.removeValue(forKey: channel)
    }
  }

  func call(_ arguments: [String: Any]) -> [String: Any]? {
    guard let handler = handlers["app.hidlins/lifecycle"]?.1 else {
      XCTFail("Production lifecycle handler was not installed")
      return nil
    }
    var result: [String: Any]?
    handler(codec.encode(FlutterMethodCall(methodName: "reportState", arguments: arguments))) { data in
      if let data { result = self.codec.decodeEnvelope(data) as? [String: Any] }
    }
    return result
  }
}

extension RunnerTests {
  @MainActor
  func testLifecycleDelayedResumeAcknowledgementCannotUncoverInactiveOrPaused() {
    for laterState in ["inactive", "paused"] {
      let services = HidlinsPlatformServices()
      let messenger = LifecycleTestMessenger()
      services.register(with: messenger)
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 320, height: 640))
      services.reportLifecycle("resumed", window: window)
      let resumedToken = messenger.observations.last?["observationToken"] as? String
      XCTAssertNotNil(resumedToken)
      services.reportLifecycle(laterState, window: window)
      let current = messenger.observations.last
      let token = current?["observationToken"] as? String
      XCTAssertNotNil(token)
      XCTAssertNotEqual(resumedToken, token)
      XCTAssertTrue(services.snapshotShield.isVisible)

      let reply = messenger.call([
        "state": "resumed", "acknowledged": true, "lockState": "unlocked",
        "observationToken": resumedToken ?? "absent-before-repair",
      ])
      XCTAssertEqual(reply?["status"] as? String, "stale")
      XCTAssertTrue(services.snapshotShield.isVisible, "Delayed resumed acknowledgement must retain cover")
      XCTAssertEqual(window.subviews.last?.accessibilityIdentifier, "hidlins.snapshot-shield")
      XCTAssertEqual(messenger.observations.last?["state"] as? String, laterState)
      XCTAssertEqual(messenger.observations.last?["observationToken"] as? String, token)
    }
  }

  @MainActor
  func testLifecycleResumeInactiveResumeRejectsFirstResumeToken() {
    let services = HidlinsPlatformServices()
    let messenger = LifecycleTestMessenger()
    services.register(with: messenger)
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 320, height: 640))
    services.reportLifecycle("resumed", window: window)
    let old = messenger.observations.last?["observationToken"] as? String
    services.reportLifecycle("inactive", window: window)
    services.reportLifecycle("resumed", window: window)
    let current = messenger.observations.last?["observationToken"] as? String
    XCTAssertNotNil(old)
    XCTAssertNotNil(current)
    XCTAssertNotEqual(old, current, "A resumed state must not reuse an earlier observation token")
    let reply = messenger.call([
      "state": "resumed", "acknowledged": true, "lockState": "unlocked",
      "observationToken": old ?? "absent-before-repair",
    ])
    XCTAssertEqual(reply?["status"] as? String, "stale")
    XCTAssertTrue(services.snapshotShield.isVisible, "ABA acknowledgement must retain cover")
    XCTAssertEqual(messenger.observations.last?["state"] as? String, "resumed")
    XCTAssertEqual(messenger.observations.last?["observationToken"] as? String, current)
  }

  @MainActor
  func testLifecycleTokenlessStartupCannotUncoverAndBootstrapReplaysSameObservation() {
    let services = HidlinsPlatformServices()
    let messenger = LifecycleTestMessenger()
    services.register(with: messenger)
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 320, height: 640))
    services.snapshotShield.show(in: window)
    let reply = messenger.call(["state": "resumed", "acknowledged": true, "lockState": "locked"])
    XCTAssertEqual(reply?["status"] as? String, "stale")
    XCTAssertTrue(services.snapshotShield.isVisible, "Tokenless startup must retain native cover")
    let startupToken = messenger.observations.last?["observationToken"] as? String
    XCTAssertNotNil(startupToken)
    XCTAssertEqual(messenger.observations.last?["state"] as? String, "inactive")
    for _ in 0..<2 {
      let replay = messenger.call(["requestObservation": true])
      XCTAssertEqual(replay?["status"] as? String, "success")
      XCTAssertEqual(messenger.observations.last?["observationToken"] as? String, startupToken)
      XCTAssertEqual(messenger.observations.last?["state"] as? String, "inactive")
      XCTAssertTrue(services.snapshotShield.isVisible)
    }
  }

  @MainActor
  func testLifecycleOnlyExactCurrentResumedAcknowledgementCanHideShield() {
    for lockState in ["locked", "unlocked"] {
      let services = HidlinsPlatformServices()
      let messenger = LifecycleTestMessenger()
      services.register(with: messenger)
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 320, height: 640))
      services.reportLifecycle("inactive", window: window)
      let inactive = messenger.observations.last?["observationToken"] as? String
      let inactiveReply = messenger.call([
        "state": "inactive", "acknowledged": true, "lockState": lockState,
        "observationToken": inactive ?? "absent-before-repair",
      ])
      XCTAssertEqual(inactiveReply?["status"] as? String, "success")
      XCTAssertTrue(services.snapshotShield.isVisible)
      services.reportLifecycle("resumed", window: window)
      let current = messenger.observations.last?["observationToken"] as? String
      XCTAssertNotNil(current)
      let replay = messenger.call(["requestObservation": true])
      XCTAssertEqual(replay?["status"] as? String, "success")
      XCTAssertEqual(messenger.observations.last?["observationToken"] as? String, current)
      XCTAssertTrue(services.snapshotShield.isVisible)
      let reply = messenger.call([
        "state": "resumed", "acknowledged": true, "lockState": lockState,
        "observationToken": current ?? "absent-before-repair",
      ])
      XCTAssertEqual(reply?["status"] as? String, "success")
      XCTAssertFalse(services.snapshotShield.isVisible)
      XCTAssertFalse(window.subviews.contains { $0.accessibilityIdentifier == "hidlins.snapshot-shield" })
    }
  }
}

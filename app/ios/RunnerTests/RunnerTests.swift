import Foundation
import UniformTypeIdentifiers
import UIKit
import XCTest

@testable import Runner

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

#if HIDLINS_IOS_PUBLICATION_FIXTURE
import Foundation
import UIKit
import XCTest

@testable import Runner

private final class PublicationProbe: @unchecked Sendable {
  let entered = DispatchSemaphore(value: 0)
  let release = DispatchSemaphore(value: 0)
  let coordinated = DispatchSemaphore(value: 0)
  let mutex = NSLock()
  let services = HidlinsPlatformServices()
  var reachedWait = false
  var completedEarly = false
  var coordinatorOK = false
  var published = false
  var receiverCalls = 0
}

private func nativePublicationReceiver(
  _ bytes: UnsafePointer<UInt8>?, _ length: Int, _ context: UnsafeMutableRawPointer?
) -> Bool {
  guard let bytes, let context else { return false }
  let probe = Unmanaged<PublicationProbe>.fromOpaque(context).takeUnretainedValue()
  guard Thread.isMainThread else { return false }
  probe.receiverCalls += 1
  probe.entered.signal()
  guard probe.release.wait(timeout: .now() + 5) == .success else { return false }
  let accepted = probe.services.writeClipboard(bytes: bytes, length: length, ttlSeconds: 1)
  probe.published = accepted && UIPasteboard.general.string == "ios-publication-native-canary"
  return probe.published
}

final class NativePublicationTests: XCTestCase {
  @MainActor
  func testActualCConsumerBlocksRevocationThroughNativePasteboardPublication() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    var ticket = [UInt8](repeating: 0, count: 128)
    defer {
      XCTAssertEqual(hidlins_ios_publication_fixture_finish(), 0, "Bounded fixture cleanup")
      for index in ticket.indices { ticket[index] = 0 }
      try? FileManager.default.removeItem(at: root)
      if UIPasteboard.general.string == "ios-publication-native-canary" { UIPasteboard.general.items = [] }
    }
    let path = Array(root.path.utf8)
    let count = path.withUnsafeBufferPointer { pathBytes in
      ticket.withUnsafeMutableBufferPointer { output in
        hidlins_ios_publication_fixture_create(pathBytes.baseAddress, pathBytes.count,
                                               output.baseAddress, output.count)
      }
    }
    XCTAssertGreaterThan(count, 0, "Real session/ticket fixture must initialize")
    guard count > 0 && Int(count) <= ticket.count else { return }
    let probe = PublicationProbe()
    DispatchQueue.global().async {
      defer { probe.release.signal(); probe.coordinated.signal() }
      guard probe.entered.wait(timeout: .now() + 5) == .success,
            hidlins_ios_publication_fixture_lock() == 0 else { return }
      let deadline = Date().addingTimeInterval(5)
      var flags = hidlins_ios_publication_fixture_status()
      while flags & 3 == 0 && Date() < deadline {
        Thread.sleep(forTimeInterval: 0.005)
        flags = hidlins_ios_publication_fixture_status()
      }
      probe.mutex.lock()
      probe.reachedWait = flags & 1 != 0
      probe.completedEarly = flags & 2 != 0
      probe.coordinatorOK = true
      probe.mutex.unlock()
    }
    let context = Unmanaged.passUnretained(probe).toOpaque()
    let status = ticket.withUnsafeBufferPointer { buffer in
      hidlins_ios_consume_clipboard(buffer.baseAddress, Int(count), nativePublicationReceiver, context)
    }
    XCTAssertEqual(status, 0, "Actual C transfer must publish successfully")
    XCTAssertEqual(probe.coordinated.wait(timeout: .now() + 5), .success)
    probe.mutex.lock()
    let coordinated = probe.coordinatorOK
    let reachedWait = probe.reachedWait
    let early = probe.completedEarly
    probe.mutex.unlock()
    XCTAssertTrue(coordinated)
    XCTAssertTrue(reachedWait, "Lock must reach the active publication fence")
    XCTAssertFalse(early, "Lock must not return while native publication is blocked")
    XCTAssertTrue(probe.published, "Existing native publisher must reach real UIPasteboard")
    let deadline = Date().addingTimeInterval(5)
    while hidlins_ios_publication_fixture_status() & 2 == 0 && Date() < deadline {
      RunLoop.current.run(until: Date().addingTimeInterval(0.01))
    }
    XCTAssertEqual(hidlins_ios_publication_fixture_status(), 15,
                   "Lock must complete successfully and discard vault/credentials after publication")
    let calls = probe.receiverCalls
    let consumedAgain = ticket.withUnsafeBufferPointer { buffer in
      hidlins_ios_consume_clipboard(buffer.baseAddress, Int(count), nativePublicationReceiver, context)
    }
    XCTAssertEqual(consumedAgain, 2, "Ticket remains one-shot after native publication/revocation")
    XCTAssertEqual(probe.receiverCalls, calls)
  }
}
#endif

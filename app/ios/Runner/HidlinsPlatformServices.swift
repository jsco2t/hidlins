import Flutter
import Foundation
import UniformTypeIdentifiers
import UIKit

enum PlatformServiceError: Error {
  case invalidArguments
  case invalidTTL
  case unavailable
}

enum PlatformEnvelope {
  static func success(_ value: Any? = nil) -> [String: Any] {
    ["status": "success", "value": value as Any]
  }

  static func canceled() -> [String: String] { ["status": "canceled"] }
  static func stale() -> [String: String] { ["status": "stale"] }
  static func unsupported() -> [String: String] { ["status": "unsupported"] }

  static func failure(code: String) -> [String: String] {
    let safe = code.range(of: #"^[a-zA-Z0-9._-]{1,64}$"#, options: .regularExpression) != nil
      ? code
      : "platform-error"
    return ["status": "failure", "code": safe]
  }
}

enum PasteboardPolicy {
  static func options(ttlSeconds: Int, now: Date = Date()) throws -> [UIPasteboard.OptionsKey: Any] {
    guard (1...300).contains(ttlSeconds) else { throw PlatformServiceError.invalidTTL }
    return [
      .localOnly: true,
      .expirationDate: now.addingTimeInterval(TimeInterval(ttlSeconds)),
    ]
  }
}

struct LifecycleAcknowledgement {
  let state: String
  let lockState: String

  init?(arguments: [String: Any]) {
    let validStates = ["resumed", "inactive", "hidden", "paused", "detached"]
    let validLockStates = ["locked", "unlocked"]
    guard arguments["acknowledged"] as? Bool == true,
          let state = arguments["state"] as? String,
          validStates.contains(state),
          let lockState = arguments["lockState"] as? String,
          validLockStates.contains(lockState)
    else { return nil }
    self.state = state
    self.lockState = lockState
  }
}

final class SnapshotShieldController {
  private var shieldView: UIView?

  var isVisible: Bool { shieldView?.superview != nil }

  func show(in window: UIWindow?) {
    guard let window else { return }
    if let shieldView, shieldView.superview === window {
      window.bringSubviewToFront(shieldView)
      return
    }
    shieldView?.removeFromSuperview()
    let shield = UIView(frame: window.bounds)
    shield.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    shield.backgroundColor = .systemBackground
    shield.accessibilityIdentifier = "hidlins.snapshot-shield"
    shield.isAccessibilityElement = true
    shield.accessibilityLabel = "Hidlins is locked"
    window.addSubview(shield)
    shieldView = shield
  }

  func hide() {
    shieldView?.removeFromSuperview()
    shieldView = nil
  }
}

struct ApplicationSupportStore {
  let baseDirectory: URL

  init(baseDirectory: URL? = nil) {
    self.baseDirectory = baseDirectory ?? FileManager.default.urls(
      for: .applicationSupportDirectory,
      in: .userDomainMask
    ).first!
  }

  func prepare() throws -> URL {
    let directory = baseDirectory.appendingPathComponent("Hidlins", isDirectory: true)
    try FileManager.default.createDirectory(
      at: directory,
      withIntermediateDirectories: true,
      attributes: [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication]
    )
    return directory
  }
}

struct AtomicVaultImporter {
  let ownedDirectory: URL

  func copyFromProvider(_ source: URL) throws -> URL {
    try FileManager.default.createDirectory(
      at: ownedDirectory,
      withIntermediateDirectories: true,
      attributes: [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication]
    )
    let baseName = source.deletingPathExtension().lastPathComponent
    let safeBase = baseName.replacingOccurrences(
      of: #"[^a-zA-Z0-9._ -]"#,
      with: "-",
      options: .regularExpression
    )
    guard !safeBase.isEmpty else { throw PlatformServiceError.invalidArguments }

    var destination = ownedDirectory.appendingPathComponent(safeBase).appendingPathExtension("kdbx")
    if FileManager.default.fileExists(atPath: destination.path) {
      destination = ownedDirectory
        .appendingPathComponent("\(safeBase)-\(UUID().uuidString)")
        .appendingPathExtension("kdbx")
    }
    let temporary = ownedDirectory.appendingPathComponent(".import-\(UUID().uuidString)")
    defer { try? FileManager.default.removeItem(at: temporary) }
    try FileManager.default.copyItem(at: source, to: temporary)
    try FileManager.default.setAttributes(
      [.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
      ofItemAtPath: temporary.path
    )
    try FileManager.default.moveItem(at: temporary, to: destination)
    return destination
  }
}

final class SecurityScopedKeyfileStore {
  private let defaults: UserDefaults
  private let prefix = "hidlins.keyfile."
  private var active: [String: URL] = [:]

  init(defaults: UserDefaults = .standard) {
    self.defaults = defaults
  }

  func save(_ url: URL) throws -> String {
    let bookmark = try url.bookmarkData(
      options: .minimalBookmark,
      includingResourceValuesForKeys: nil,
      relativeTo: nil
    )
    let identifier = UUID().uuidString
    defaults.set(bookmark, forKey: prefix + identifier)
    return identifier
  }

  func resolve(_ identifier: String) throws -> (url: URL, stale: Bool) {
    guard let bookmark = defaults.data(forKey: prefix + identifier) else {
      throw PlatformServiceError.unavailable
    }
    var stale = false
    let url = try URL(
      resolvingBookmarkData: bookmark,
      options: [.withoutUI],
      relativeTo: nil,
      bookmarkDataIsStale: &stale
    )
    guard !stale else { return (url, true) }
    guard url.startAccessingSecurityScopedResource() else {
      throw PlatformServiceError.unavailable
    }
    active[identifier]?.stopAccessingSecurityScopedResource()
    active[identifier] = url
    return (url, false)
  }

  func release(_ identifier: String) {
    active.removeValue(forKey: identifier)?.stopAccessingSecurityScopedResource()
  }
}

private func receiveClipboardBytes(
  _ bytes: UnsafePointer<UInt8>?,
  _ length: Int,
  _ context: UnsafeMutableRawPointer?
) -> Bool {
  guard let bytes, length > 0, let context else { return false }
  let service = Unmanaged<HidlinsPlatformServices>.fromOpaque(context).takeUnretainedValue()
  return service.writeClipboard(
    bytes: bytes,
    length: length,
    ttlSeconds: service.pendingClipboardTTL
  )
}

final class HidlinsPlatformServices: NSObject, UIDocumentPickerDelegate {
  static let shared = HidlinsPlatformServices()

  let snapshotShield = SnapshotShieldController()
  private let keyfiles = SecurityScopedKeyfileStore()
  private var lifecycleChannel: FlutterMethodChannel?
  private var pendingPickerResult: FlutterResult?
  private var pickerPurpose: PickerPurpose?
  fileprivate var pendingClipboardTTL = 30
  private var clipboardClear: DispatchWorkItem?

  private enum PickerPurpose { case vault, keyfile }

  func register(with messenger: FlutterBinaryMessenger) {
    let lifecycle = FlutterMethodChannel(name: "app.hidlins/lifecycle", binaryMessenger: messenger)
    lifecycle.setMethodCallHandler { [weak self] call, result in
      guard call.method == "reportState",
            let arguments = call.arguments as? [String: Any],
            let acknowledgement = LifecycleAcknowledgement(arguments: arguments)
      else {
        result(PlatformEnvelope.failure(code: "invalid-lifecycle-ack"))
        return
      }
      if acknowledgement.state == "resumed" { self?.snapshotShield.hide() }
      result(PlatformEnvelope.success())
    }
    lifecycleChannel = lifecycle

    registerChannel("app.hidlins/paths", messenger: messenger) { [weak self] call, result in
      guard call.method == "applicationSupportPath" else {
        result(PlatformEnvelope.unsupported())
        return
      }
      do {
        let path = try ApplicationSupportStore().prepare().path
        result(PlatformEnvelope.success(path))
      } catch {
        result(PlatformEnvelope.failure(code: "app-support-unavailable"))
      }
      _ = self
    }

    registerChannel("app.hidlins/clipboard", messenger: messenger) { [weak self] call, result in
      self?.handleClipboard(call: call, result: result)
    }
    registerChannel("app.hidlins/vault_import", messenger: messenger) { [weak self] call, result in
      guard call.method == "pickVault" else {
        result(PlatformEnvelope.unsupported())
        return
      }
      self?.presentPicker(purpose: .vault, result: result)
    }
    registerChannel("app.hidlins/keyfile", messenger: messenger) { [weak self] call, result in
      self?.handleKeyfile(call: call, result: result)
    }
  }

  private func registerChannel(
    _ name: String,
    messenger: FlutterBinaryMessenger,
    handler: @escaping FlutterMethodCallHandler
  ) {
    FlutterMethodChannel(name: name, binaryMessenger: messenger).setMethodCallHandler(handler)
  }

  func reportLifecycle(_ state: String, window: UIWindow?) {
    if state != "resumed" { snapshotShield.show(in: window) }
    lifecycleChannel?.invokeMethod("reportState", arguments: ["state": state])
  }

  private func handleClipboard(call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard call.method == "copySecret",
          let arguments = call.arguments as? [String: Any],
          let transferID = arguments["transferId"] as? String,
          let ttl = arguments["ttlSeconds"] as? Int,
          !transferID.isEmpty,
          (1...300).contains(ttl)
    else {
      result(PlatformEnvelope.failure(code: "invalid-clipboard-request"))
      return
    }

    pendingClipboardTTL = ttl
    let context = Unmanaged.passUnretained(self).toOpaque()
    let status = transferID.utf8CString.withUnsafeBytes { rawBuffer -> Int32 in
      guard let base = rawBuffer.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return 1 }
      return hidlins_ios_consume_clipboard(
        base,
        max(0, rawBuffer.count - 1),
        receiveClipboardBytes,
        context
      )
    }
    if status == 0 {
      result(PlatformEnvelope.success())
    } else {
      result(PlatformEnvelope.failure(code: "clipboard-transfer-\(status)"))
    }
  }

  func writeClipboard(
    bytes: UnsafePointer<UInt8>,
    length: Int,
    ttlSeconds: Int
  ) -> Bool {
    do {
      var data = Data(bytes: bytes, count: length)
      defer { data.resetBytes(in: 0..<data.count) }
      let pasteboard = UIPasteboard.general
      let options = try PasteboardPolicy.options(ttlSeconds: ttlSeconds)
      pasteboard.setItems([[UTType.utf8PlainText.identifier: data]], options: options)
      let changeCount = pasteboard.changeCount
      clipboardClear?.cancel()
      let clear = DispatchWorkItem {
        if pasteboard.changeCount == changeCount { pasteboard.items = [] }
      }
      clipboardClear = clear
      DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(ttlSeconds), execute: clear)
      return true
    } catch {
      return false
    }
  }

  private func handleKeyfile(call: FlutterMethodCall, result: @escaping FlutterResult) {
    switch call.method {
    case "pickReference":
      presentPicker(purpose: .keyfile, result: result)
    case "resolveReference":
      guard let arguments = call.arguments as? [String: Any],
            let reference = arguments["reference"] as? String
      else {
        result(PlatformEnvelope.failure(code: "invalid-keyfile-reference"))
        return
      }
      do {
        let resolved = try keyfiles.resolve(reference)
        result(resolved.stale ? PlatformEnvelope.stale() : PlatformEnvelope.success(resolved.url.path))
      } catch {
        result(PlatformEnvelope.failure(code: "keyfile-unavailable"))
      }
    case "releaseReference":
      guard let arguments = call.arguments as? [String: Any],
            let reference = arguments["reference"] as? String
      else {
        result(PlatformEnvelope.failure(code: "invalid-keyfile-reference"))
        return
      }
      keyfiles.release(reference)
      result(PlatformEnvelope.success())
    default:
      result(PlatformEnvelope.unsupported())
    }
  }

  private func presentPicker(purpose: PickerPurpose, result: @escaping FlutterResult) {
    guard pendingPickerResult == nil,
          let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first,
          let presenter = scene.windows.first(where: { $0.isKeyWindow })?.rootViewController
    else {
      result(PlatformEnvelope.failure(code: "picker-unavailable"))
      return
    }
    let types: [UTType] = purpose == .vault
      ? [UTType(filenameExtension: "kdbx") ?? .data]
      : [.data]
    let picker = UIDocumentPickerViewController(forOpeningContentTypes: types, asCopy: false)
    picker.delegate = self
    picker.allowsMultipleSelection = false
    pendingPickerResult = result
    pickerPurpose = purpose
    presenter.present(picker, animated: true)
  }

  func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
    finishPicker(PlatformEnvelope.canceled())
  }

  func documentPicker(
    _ controller: UIDocumentPickerViewController,
    didPickDocumentsAt urls: [URL]
  ) {
    guard let source = urls.first, let purpose = pickerPurpose else {
      finishPicker(PlatformEnvelope.failure(code: "picker-empty"))
      return
    }
    let accessing = source.startAccessingSecurityScopedResource()
    defer { if accessing { source.stopAccessingSecurityScopedResource() } }
    do {
      switch purpose {
      case .vault:
        let base = try ApplicationSupportStore().prepare()
        let destination = try AtomicVaultImporter(
          ownedDirectory: base.appendingPathComponent("Vaults", isDirectory: true)
        ).copyFromProvider(source)
        finishPicker(PlatformEnvelope.success([
          "sourceReference": destination.path,
          "displayName": destination.deletingPathExtension().lastPathComponent,
        ]))
      case .keyfile:
        let reference = try keyfiles.save(source)
        finishPicker(PlatformEnvelope.success([
          "reference": reference,
          "displayName": source.lastPathComponent,
        ]))
      }
    } catch {
      finishPicker(PlatformEnvelope.failure(code: "document-access-failed"))
    }
  }

  private func finishPicker(_ envelope: Any) {
    let result = pendingPickerResult
    pendingPickerResult = nil
    pickerPurpose = nil
    result?(envelope)
  }
}

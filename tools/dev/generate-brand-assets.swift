#!/usr/bin/env swift

import CoreGraphics
import CryptoKit
import Foundation
import ImageIO
import UniformTypeIdentifiers

enum BrandAssetError: Error, CustomStringConvertible {
    case cannotLoad(String)
    case cannotCreate(String)
    case cannotWrite(String)

    var description: String {
        switch self {
        case .cannotLoad(let path): "cannot load PNG: \(path)"
        case .cannotCreate(let path): "cannot create raster context: \(path)"
        case .cannotWrite(let path): "cannot write PNG: \(path)"
        }
    }
}

func loadPNG(_ path: String) throws -> CGImage {
    let url = URL(fileURLWithPath: path) as CFURL
    guard let source = CGImageSourceCreateWithURL(url, nil),
          let image = CGImageSourceCreateImageAtIndex(source, 0, nil)
    else { throw BrandAssetError.cannotLoad(path) }
    return image
}

func render(
    source: CGImage,
    size: Int,
    scale: CGFloat = 1,
    background: (CGFloat, CGFloat, CGFloat, CGFloat)? = nil,
    to path: String
) throws {
    let colorSpace = CGColorSpaceCreateDeviceRGB()
    guard let context = CGContext(
        data: nil,
        width: size,
        height: size,
        bitsPerComponent: 8,
        bytesPerRow: 0,
        space: colorSpace,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    ) else { throw BrandAssetError.cannotCreate(path) }

    context.interpolationQuality = .high
    context.clear(CGRect(x: 0, y: 0, width: size, height: size))
    if let background {
        context.setFillColor(
            red: background.0,
            green: background.1,
            blue: background.2,
            alpha: background.3
        )
        context.fill(CGRect(x: 0, y: 0, width: size, height: size))
    }

    let renderedSize = CGFloat(size) * scale
    let inset = (CGFloat(size) - renderedSize) / 2
    context.draw(
        source,
        in: CGRect(x: inset, y: inset, width: renderedSize, height: renderedSize)
    )

    guard let output = context.makeImage() else {
        throw BrandAssetError.cannotCreate(path)
    }
    let url = URL(fileURLWithPath: path) as CFURL
    guard let destination = CGImageDestinationCreateWithURL(
        url,
        UTType.png.identifier as CFString,
        1,
        nil
    ) else { throw BrandAssetError.cannotWrite(path) }
    CGImageDestinationAddImage(destination, output, nil)
    guard CGImageDestinationFinalize(destination) else {
        throw BrandAssetError.cannotWrite(path)
    }
}

let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
let navyPath = root.appendingPathComponent(
    "app/assets/branding/source/hidlins-navy-1024.png"
).path
let monoPath = root.appendingPathComponent(
    "app/assets/branding/source/hidlins-mono-1024.png"
).path
let navy = try loadPNG(navyPath)
let mono = try loadPNG(monoPath)
let navyBackground = (
    CGFloat(0x17) / 255,
    CGFloat(0x2B) / 255,
    CGFloat(0x3A) / 255,
    CGFloat(1)
)

let iosDirectory = root.appendingPathComponent(
    "app/ios/Runner/Assets.xcassets/AppIcon.appiconset"
).path
let iosIcons: [(String, Int)] = [
    ("Icon-App-20x20@1x.png", 20),
    ("Icon-App-20x20@2x.png", 40),
    ("Icon-App-20x20@3x.png", 60),
    ("Icon-App-29x29@1x.png", 29),
    ("Icon-App-29x29@2x.png", 58),
    ("Icon-App-29x29@3x.png", 87),
    ("Icon-App-40x40@1x.png", 40),
    ("Icon-App-40x40@2x.png", 80),
    ("Icon-App-40x40@3x.png", 120),
    ("Icon-App-60x60@2x.png", 120),
    ("Icon-App-60x60@3x.png", 180),
    ("Icon-App-76x76@1x.png", 76),
    ("Icon-App-76x76@2x.png", 152),
    ("Icon-App-83.5x83.5@2x.png", 167),
    ("Icon-App-1024x1024@1x.png", 1024),
]
for (name, size) in iosIcons {
    try render(
        source: navy,
        size: size,
        background: navyBackground,
        to: URL(fileURLWithPath: iosDirectory).appendingPathComponent(name).path
    )
}

let iosLaunchDirectory = root.appendingPathComponent(
    "app/ios/Runner/Assets.xcassets/LaunchImage.imageset"
).path
let iosLaunchImages: [(String, Int)] = [
    ("LaunchImage.png", 168),
    ("LaunchImage@2x.png", 336),
    ("LaunchImage@3x.png", 504),
]
for (name, size) in iosLaunchImages {
    try render(
        source: navy,
        size: size,
        to: URL(fileURLWithPath: iosLaunchDirectory).appendingPathComponent(name).path
    )
}

let androidRes = root.appendingPathComponent("app/android/app/src/main/res")
let legacyIcons: [(String, Int)] = [
    ("mipmap-mdpi/ic_launcher.png", 48),
    ("mipmap-hdpi/ic_launcher.png", 72),
    ("mipmap-xhdpi/ic_launcher.png", 96),
    ("mipmap-xxhdpi/ic_launcher.png", 144),
    ("mipmap-xxxhdpi/ic_launcher.png", 192),
]
for (name, size) in legacyIcons {
    try render(
        source: navy,
        size: size,
        to: androidRes.appendingPathComponent(name).path
    )
}

// Android's documented adaptive-icon safe zone is the centered 66×66 region
// of the 108×108 canvas. Scale the supplied monochrome treatment to 66% and
// retain transparency so the launcher-provided navy background remains flat.
try render(
    source: mono,
    size: 1024,
    scale: 0.66,
    to: androidRes.appendingPathComponent(
        "drawable-nodpi/ic_launcher_foreground.png"
    ).path
)

struct AssetRecord: Codable {
    let path: String
    let sourceTreatment: String
    let width: Int
    let height: Int
    let sha256: String
    let alphaPolicy: String
    let platformUse: String
    let transform: String
}

func sha256(_ path: String) throws -> String {
    let digest = SHA256.hash(data: try Data(contentsOf: URL(fileURLWithPath: path)))
    return digest.map { String(format: "%02x", $0) }.joined()
}

var records: [AssetRecord] = []
func record(
    _ relativePath: String,
    treatment: String,
    size: Int,
    alpha: String,
    use: String,
    transform: String
) throws {
    let repositoryPath = root.appendingPathComponent("app/\(relativePath)").path
    records.append(AssetRecord(
        path: relativePath,
        sourceTreatment: treatment,
        width: size,
        height: size,
        sha256: try sha256(repositoryPath),
        alphaPolicy: alpha,
        platformUse: use,
        transform: transform
    ))
}

for treatment in ["mono", "navy", "vintage-dark"] {
    try record(
        "assets/branding/masters/hidlins-\(treatment).svg",
        treatment: treatment,
        size: 1024,
        alpha: "canonical-vector-viewBox",
        use: "canonical-master",
        transform: "exact-supplied-svg"
    )
}
try record(
    "assets/branding/source/hidlins-navy-1024.png",
    treatment: "navy", size: 1024, alpha: "preserve-supplied-alpha",
    use: "launcher-generation-source", transform: "exact-supplied-raster"
)
try record(
    "assets/branding/source/hidlins-mono-1024.png",
    treatment: "mono", size: 1024, alpha: "preserve-supplied-alpha",
    use: "android-adaptive-generation-source", transform: "exact-supplied-raster"
)
try record(
    "assets/branding/runtime/hidlins-navy-256.png",
    treatment: "navy", size: 256, alpha: "preserve-supplied-alpha",
    use: "in-app-light", transform: "exact-supplied-raster"
)
try record(
    "assets/branding/runtime/hidlins-vintage-dark-256.png",
    treatment: "vintage-dark", size: 256, alpha: "preserve-supplied-alpha",
    use: "in-app-dark", transform: "exact-supplied-raster"
)

for size in [16, 32, 64, 128, 256, 512, 1024] {
    try record(
        "macos/Runner/Assets.xcassets/AppIcon.appiconset/app_icon_\(size).png",
        treatment: "navy", size: size, alpha: "preserve-supplied-alpha",
        use: "macos-app-icon", transform: "exact-supplied-raster"
    )
}
for (name, size) in iosIcons {
    try record(
        "ios/Runner/Assets.xcassets/AppIcon.appiconset/\(name)",
        treatment: "navy", size: size, alpha: "opaque-required",
        use: "ios-app-icon",
        transform: "CoreGraphics resize; flatten #172B3A beneath supplied navy 1024"
    )
}
for (name, size) in iosLaunchImages {
    try record(
        "ios/Runner/Assets.xcassets/LaunchImage.imageset/\(name)",
        treatment: "navy", size: size, alpha: "preserve-supplied-alpha",
        use: "ios-launch-screen",
        transform: "CoreGraphics resize from supplied navy 1024"
    )
}
for (name, size) in legacyIcons {
    try record(
        "android/app/src/main/res/\(name)",
        treatment: "navy", size: size, alpha: "preserve-supplied-alpha",
        use: "android-legacy-launcher",
        transform: "CoreGraphics resize from supplied navy 1024"
    )
}
try record(
    "android/app/src/main/res/drawable-nodpi/ic_launcher_foreground.png",
    treatment: "mono", size: 1024, alpha: "transparent-safe-zone",
    use: "android-adaptive-foreground",
    transform: "CoreGraphics scale supplied mono 1024 to 66%; center on transparent canvas"
)
for size in [16, 32, 256] {
    try record(
        "linux/assets/icons/app.hidlins-\(size).png",
        treatment: "navy", size: size, alpha: "preserve-supplied-alpha",
        use: "linux-window-and-hicolor-icon", transform: "exact-supplied-raster"
    )
}

let manifest: [String: Any] = [
    "schemaVersion": 1,
    "archiveSha256": "99818bc4dfd16b3ab876d58a7cd3c5edd7ae1274ab42cb9d1ce13f9ad79561d6",
    "generator": "tools/dev/generate-brand-assets.swift",
    "assets": try records.map { record in
        let data = try JSONEncoder().encode(record)
        return try JSONSerialization.jsonObject(with: data)
    },
]
let manifestData = try JSONSerialization.data(
    withJSONObject: manifest,
    options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
)
try manifestData.write(
    to: root.appendingPathComponent("app/assets/branding/manifest.json")
)

print("Generated opaque iOS and safe-zone Android launcher rasters and manifest.")

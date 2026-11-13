import Foundation
import CoreGraphics
import ImageIO

// Use the default iOS artwork as-is inside the macOS tile, with transparent
// padding for the Dock and Finder.
let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let sourceURL = root.appendingPathComponent("ios/ListenerNative/Assets.xcassets/AppIcon.appiconset/Light.png")
let source = CGImageSourceCreateWithURL(sourceURL as CFURL, nil)!
let artwork = CGImageSourceCreateImageAtIndex(source, 0, nil)!
let colorSpace = CGColorSpace(name: CGColorSpace.sRGB)!

func context(_ size: Int) -> CGContext {
    CGContext(data: nil, width: size, height: size, bitsPerComponent: 8,
              bytesPerRow: size * 4, space: colorSpace,
              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
}

func writePNG(_ image: CGImage, to url: URL) {
    let destination = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, image, nil)
    precondition(CGImageDestinationFinalize(destination), "Could not write \(url.path)")
}

let canvas = context(1024)
let tile = CGRect(x: 104, y: 104, width: 816, height: 816)
canvas.addPath(CGPath(roundedRect: tile, cornerWidth: 176, cornerHeight: 176, transform: nil))
canvas.clip()
canvas.interpolationQuality = .high
canvas.draw(artwork, in: tile)
let icon = canvas.makeImage()!
writePNG(icon, to: root.appendingPathComponent("assets/app-icon.png"))

func resized(_ size: Int) -> CGImage {
    let output = context(size)
    output.interpolationQuality = .high
    output.draw(icon, in: CGRect(x: 0, y: 0, width: size, height: size))
    return output.makeImage()!
}

let icons = root.appendingPathComponent("src-tauri/icons", isDirectory: true)
for (name, size) in [("32x32.png", 32), ("64x64.png", 64), ("128x128.png", 128),
                     ("128x128@2x.png", 256), ("icon.png", 1024)] {
    writePNG(resized(size), to: icons.appendingPathComponent(name))
}

let temporary = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
let iconset = temporary.appendingPathComponent("Listener.iconset", isDirectory: true)
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: temporary) }
for size in [16, 32, 128, 256, 512] {
    writePNG(resized(size), to: iconset.appendingPathComponent("icon_\(size)x\(size).png"))
    writePNG(resized(size * 2), to: iconset.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", iconset.path, "-o", icons.appendingPathComponent("icon.icns").path]
try iconutil.run()
iconutil.waitUntilExit()
precondition(iconutil.terminationStatus == 0, "iconutil export failed")
print("Exported Listener app icon, macOS ICNS and PNG sizes.")

import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

// Run from the repository root: swift ios/render-app-icon.swift
let directory = URL(fileURLWithPath: "ios/ListenerNative/Assets.xcassets/AppIcon.appiconset", isDirectory: true)
let size = 1024
let sRGB = CGColorSpace(name: CGColorSpace.sRGB)!
let gray = CGColorSpace(name: CGColorSpace.genericGrayGamma2_2)!
let square = CGRect(x: 0, y: 0, width: size, height: size)

func color(_ rgb: UInt32, alpha: CGFloat = 1) -> CGColor {
    CGColor(colorSpace: sRGB, components: [
        CGFloat((rgb >> 16) & 255) / 255, CGFloat((rgb >> 8) & 255) / 255, CGFloat(rgb & 255) / 255, alpha,
    ])!
}

func linear(_ context: CGContext, _ colors: [CGColor], from: CGPoint, to: CGPoint) {
    let gradient = CGGradient(colorsSpace: colors[0].colorSpace, colors: colors as CFArray, locations: nil)!
    context.drawLinearGradient(gradient, start: from, end: to, options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
}

func ambient(_ context: CGContext, _ rgb: UInt32, opacity: CGFloat, at center: CGPoint, radius: CGFloat) {
    let gradient = CGGradient(colorsSpace: sRGB, colors: [color(rgb, alpha: opacity), color(rgb, alpha: 0)] as CFArray,
                              locations: [0, 1])!
    context.drawRadialGradient(gradient, startCenter: center, startRadius: 0, endCenter: center, endRadius: radius,
                               options: [])
}

enum Appearance: String, CaseIterable {
    case light = "Light", dark = "Dark", tinted = "Tinted"
}

for appearance in Appearance.allCases {
    let tinted = appearance == .tinted
    let dark = appearance == .dark
    let alpha: CGImageAlphaInfo = tinted ? .none : dark ? .premultipliedLast : .noneSkipLast
    let context = CGContext(data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
                            space: tinted ? gray : sRGB, bitmapInfo: alpha.rawValue)!

    if tinted {
        context.setFillColor(CGColor(gray: 0.08, alpha: 1))
        context.fill(square)
    } else if dark {
        ambient(context, 0x335DDF, opacity: 0.32, at: CGPoint(x: 512, y: 500), radius: 390)
    } else {
        linear(context, [color(0xD8E6FF), color(0xFAFCFF)], from: CGPoint(x: 920, y: 80), to: CGPoint(x: 90, y: 920))
        ambient(context, 0x83A8FC, opacity: 0.30, at: CGPoint(x: 814, y: 220), radius: 480)
    }

    // Preserve the six lengths measured on the original mark and scale them as a group.
    let originalHeights: [CGFloat] = [100, 311, 502, 200, 371, 100]
    let heights = originalHeights.map { $0 * 102 / 94 }
    let barWidth: CGFloat = 57
    for (index, height) in heights.enumerated() {
        let x = CGFloat(229 + index * 102)
        let box = CGRect(x: x, y: 512 - height / 2, width: barWidth, height: height)
        let path = CGPath(roundedRect: box, cornerWidth: barWidth / 2, cornerHeight: barWidth / 2, transform: nil)
        if !tinted {
            context.saveGState()
            context.setShadow(offset: CGSize(width: 0, height: -12), blur: 24,
                              color: color(dark ? 0x3A66E7 : 0x233B8A, alpha: dark ? 0.42 : 0.28))
            context.setFillColor(color(dark ? 0x4770ED : 0x3059CB))
            context.addPath(path)
            context.fillPath()
            context.restoreGState()
        }
        context.saveGState()
        context.addPath(path)
        context.clip()
        let palette: [CGColor]
        if tinted {
            palette = [CGColor(gray: 0.69, alpha: 1), CGColor(gray: 0.98, alpha: 1)]
        } else if dark {
            palette = [color(0x597BFF), color(0x9EDCFF)]
        } else {
            palette = [color(0x1B3B9D), color(0x5F9AFF)]
        }
        linear(context, palette, from: CGPoint(x: box.maxX, y: box.minY), to: CGPoint(x: box.minX, y: box.maxY))
        context.restoreGState()
    }

    let output = directory.appendingPathComponent(appearance.rawValue + ".png")
    let destination = CGImageDestinationCreateWithURL(output as CFURL, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, context.makeImage()!, nil)
    precondition(CGImageDestinationFinalize(destination), "Unable to write \(output.path)")
}

import AppKit
import Foundation

guard CommandLine.arguments.count >= 3 else {
    fputs("usage: generate_icons.swift <source-png> <output-dir>\n", stderr)
    exit(1)
}

let sourcePath = CommandLine.arguments[1]
let outputDir = CommandLine.arguments[2]
let sourceURL = URL(fileURLWithPath: sourcePath)
let outputURL = URL(fileURLWithPath: outputDir, isDirectory: true)

let fileManager = FileManager.default
try fileManager.createDirectory(at: outputURL, withIntermediateDirectories: true)

guard let sourceImage = NSImage(contentsOf: sourceURL) else {
    fputs("failed to load source image: \(sourcePath)\n", stderr)
    exit(1)
}

func whitenBackground(_ image: NSImage, size: CGFloat) -> NSBitmapImageRep? {
    let px = Int(size)
    guard let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: px,
        pixelsHigh: px,
        bitsPerSample: 8,
        samplesPerPixel: 4,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: .deviceRGB,
        bytesPerRow: px * 4,
        bitsPerPixel: 32
    ) else {
        return nil
    }

    NSGraphicsContext.saveGraphicsState()
    guard let context = NSGraphicsContext(bitmapImageRep: rep) else {
        NSGraphicsContext.restoreGraphicsState()
        return nil
    }
    NSGraphicsContext.current = context

    NSColor.clear.setFill()
    NSBezierPath(rect: NSRect(x: 0, y: 0, width: size, height: size)).fill()

    let canvasInset = max(1.0, size * 0.06)
    let canvasRect = NSRect(
        x: canvasInset,
        y: canvasInset,
        width: size - canvasInset * 2,
        height: size - canvasInset * 2
    )
    let cornerRadius = max(3.0, size * 0.225)

    NSColor.white.setFill()
    NSBezierPath(
        roundedRect: canvasRect,
        xRadius: cornerRadius,
        yRadius: cornerRadius
    ).fill()

    NSColor(calibratedWhite: 0.92, alpha: 1.0).setStroke()
    let stroke = NSBezierPath(
        roundedRect: canvasRect.insetBy(dx: 0.5, dy: 0.5),
        xRadius: max(2.0, cornerRadius - 0.5),
        yRadius: max(2.0, cornerRadius - 0.5)
    )
    stroke.lineWidth = max(1.0, size * 0.012)
    stroke.stroke()

    let imageInset = size * 0.14

    image.draw(
        in: NSRect(
            x: imageInset,
            y: imageInset,
            width: size - imageInset * 2,
            height: size - imageInset * 2
        ),
        from: .zero,
        operation: .sourceOver,
        fraction: 1.0
    )

    NSGraphicsContext.restoreGraphicsState()

    guard let data = rep.bitmapData else {
        return rep
    }

    let threshold: UInt8 = 224
    for y in 0..<px {
        for x in 0..<px {
            let offset = y * rep.bytesPerRow + x * 4
            let r = data[offset]
            let g = data[offset + 1]
            let b = data[offset + 2]
            let a = data[offset + 3]

            if a > 0 && r >= threshold && g >= threshold && b >= threshold {
                data[offset] = 255
                data[offset + 1] = 255
                data[offset + 2] = 255
                data[offset + 3] = 255
            }
        }
    }

    return rep
}

func writePNG(rep: NSBitmapImageRep, to url: URL) throws {
    guard let data = rep.representation(using: .png, properties: [:]) else {
        throw NSError(domain: "generate_icons", code: 2, userInfo: [NSLocalizedDescriptionKey: "failed to encode png"])
    }
    try data.write(to: url)
}

let outputs: [(String, CGFloat)] = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
]

for (name, size) in outputs {
    guard let rep = whitenBackground(sourceImage, size: size) else {
        fputs("failed to render size \(size)\n", stderr)
        exit(1)
    }
    try writePNG(rep: rep, to: outputURL.appendingPathComponent(name))
}

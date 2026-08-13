// Renders the Dev Cockpit app icon (1024px) and the menu-bar template icon.
// Usage: swift scripts/render-icons.swift <output-dir>
import AppKit
import CoreGraphics

func ctx(_ size: Int) -> CGContext {
    return CGContext(
        data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    )!
}

func savePNG(_ c: CGContext, to url: URL) {
    let img = c.makeImage()!
    let rep = NSBitmapImageRep(cgImage: img)
    let data = rep.representation(using: .png, properties: [:])!
    try! data.write(to: url)
    print("wrote \(url.path)")
}

/// Gauge glyph: outer arc + needle + center dot — a cockpit dial.
func drawGauge(_ c: CGContext, size: CGFloat, color: CGColor, lineScale: CGFloat = 1.0) {
    let s = size
    let center = CGPoint(x: s / 2, y: s / 2 - s * 0.02)
    let radius = s * 0.30
    let lw = s * 0.075 * lineScale

    c.setStrokeColor(color)
    c.setFillColor(color)
    c.setLineCap(.round)
    c.setLineWidth(lw)

    // Dial arc: 210° → -30° (open at the bottom)
    let start = CGFloat(210.0 * .pi / 180.0)
    let end = CGFloat(-30.0 * .pi / 180.0)
    c.addArc(center: center, radius: radius, startAngle: start, endAngle: end, clockwise: true)
    c.strokePath()

    // Needle pointing to upper-right (active feel)
    let angle = CGFloat(52.0 * .pi / 180.0)
    let needleLen = radius * 0.78
    let tip = CGPoint(x: center.x + cos(angle) * needleLen, y: center.y + sin(angle) * needleLen)
    c.setLineWidth(lw * 0.9)
    c.move(to: center)
    c.addLine(to: tip)
    c.strokePath()

    // Hub
    c.fillEllipse(in: CGRect(x: center.x - lw * 0.75, y: center.y - lw * 0.75, width: lw * 1.5, height: lw * 1.5))

    // Tick dots on the arc ends
    for a in [210.0, -30.0] {
        let r = CGFloat(a * .pi / 180.0)
        let p = CGPoint(x: center.x + cos(r) * radius, y: center.y + sin(r) * radius)
        c.fillEllipse(in: CGRect(x: p.x - lw * 0.5, y: p.y - lw * 0.5, width: lw, height: lw))
    }
}

let outDir = URL(fileURLWithPath: CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : ".")
try? FileManager.default.createDirectory(at: outDir, withIntermediateDirectories: true)

// ---------------- App icon (1024) ----------------
let S = 1024
let c = ctx(S)
let sf = CGFloat(S)

// macOS-style rounded square with margin
let margin = sf * 0.098
let rect = CGRect(x: margin, y: margin, width: sf - margin * 2, height: sf - margin * 2)
let path = CGPath(roundedRect: rect, cornerWidth: sf * 0.185, cornerHeight: sf * 0.185, transform: nil)

c.addPath(path)
c.clip()
// deep slate gradient
let colors = [
    CGColor(srgbRed: 0.117, green: 0.129, blue: 0.180, alpha: 1),
    CGColor(srgbRed: 0.043, green: 0.050, blue: 0.086, alpha: 1),
] as CFArray
let grad = CGGradient(colorsSpace: CGColorSpace(name: CGColorSpace.sRGB)!, colors: colors, locations: [0, 1])!
c.drawLinearGradient(grad, start: CGPoint(x: 0, y: sf), end: CGPoint(x: sf * 0.3, y: 0), options: [])

// subtle top edge light
c.setStrokeColor(CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 0.10))
c.setLineWidth(sf * 0.008)
c.addPath(path)
c.strokePath()

// gauge in accent green (status color of the product)
drawGauge(c, size: sf, color: CGColor(srgbRed: 0.20, green: 0.84, blue: 0.29, alpha: 1.0))

// small port dots row under the gauge (ports metaphor)
let dotY = sf * 0.245
let dotR = sf * 0.022
let dotColors: [CGColor] = [
    CGColor(srgbRed: 0.20, green: 0.84, blue: 0.29, alpha: 1),
    CGColor(srgbRed: 0.20, green: 0.84, blue: 0.29, alpha: 1),
    CGColor(srgbRed: 1.00, green: 0.84, blue: 0.04, alpha: 1),
    CGColor(srgbRed: 0.56, green: 0.56, blue: 0.58, alpha: 1),
]
let totalW = CGFloat(dotColors.count - 1) * sf * 0.075
for (i, col) in dotColors.enumerated() {
    let x = sf / 2 - totalW / 2 + CGFloat(i) * sf * 0.075
    c.setFillColor(col)
    c.fillEllipse(in: CGRect(x: x - dotR, y: dotY - dotR, width: dotR * 2, height: dotR * 2))
}

savePNG(c, to: outDir.appendingPathComponent("app-icon.png"))

// ---------------- Tray template icons ----------------
for (name, px) in [("tray.png", 44), ("tray@1x.png", 22)] {
    let t = ctx(px)
    // template: pure black with alpha; macOS recolors it
    drawGauge(t, size: CGFloat(px), color: CGColor(srgbRed: 0, green: 0, blue: 0, alpha: 1), lineScale: 1.15)
    savePNG(t, to: outDir.appendingPathComponent(name))
}
print("done")

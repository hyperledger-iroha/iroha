#if canImport(CoreGraphics)
import CoreGraphics
import Foundation
import IrohaSwift

/// Draws Petal Stream frames with CoreGraphics.
///
/// The renderer paints a ``PetalDrawList`` in the order the draw list
/// documents (background, finder blossoms and notches, rounded tiles with
/// clipped round-capped glyph strokes, lit ring dots), so the result matches
/// the reference software renderer up to anti-aliasing.
public enum PetalCoreGraphicsRenderer {
    /// Draws `drawList` as a square centred in `rect`.
    ///
    /// The context's user space must have its origin at the top-left with
    /// `y` growing downward, as in UIKit, SwiftUI `Canvas` and flipped AppKit
    /// views. Bitmap contexts created with `CGContext(data:…)` are `y`-up:
    /// flip them first, or use ``makeImage(_:size:)``.
    public static func draw(_ drawList: PetalDrawList, in context: CGContext, rect: CGRect) {
        let side = min(rect.width, rect.height)
        guard side > 0, side.isFinite else { return }
        let canvas = CGFloat(PetalDrawList.canvasSize)
        let scale = side / canvas
        let palette = drawList.palette
        context.saveGState()
        defer { context.restoreGState() }
        context.translateBy(x: rect.midX - side / 2, y: rect.midY - side / 2)
        context.scaleBy(x: scale, y: scale)

        // 1. background
        context.setFillColor(color(palette.background))
        context.fill(CGRect(x: 0, y: 0, width: canvas, height: canvas))

        // 2. finder blossoms, then the notches cut into the petal tips
        context.setFillColor(color(palette.light))
        for finder in drawList.finders {
            context.fillEllipse(in: bounds(finder.core))
            for petal in finder.petals { context.fillEllipse(in: bounds(petal)) }
        }
        context.setFillColor(color(palette.background))
        for finder in drawList.finders {
            for notch in finder.notches { context.fillEllipse(in: bounds(notch)) }
        }

        // 3. tiles and their katakana
        context.setLineCap(.round)
        context.setLineJoin(.round)
        for tile in drawList.tiles {
            if let fill = tile.fill {
                context.setFillColor(color(fill))
                context.addPath(CGPath(
                    roundedRect: cgRect(tile.rect),
                    cornerWidth: CGFloat(tile.cornerRadius),
                    cornerHeight: CGFloat(tile.cornerRadius),
                    transform: nil
                ))
                context.fillPath()
            }
            context.saveGState()
            context.clip(to: cgRect(tile.glyphBox))
            context.setStrokeColor(color(tile.ink))
            context.setLineWidth(CGFloat(tile.strokeWidth))
            for polyline in tile.strokes where polyline.count >= 2 {
                context.beginPath()
                context.addLines(between: polyline.map { CGPoint(x: $0.x, y: $0.y) })
                context.strokePath()
            }
            context.restoreGState()
        }

        // 4. lit ring dots
        context.setFillColor(color(palette.pink))
        for dot in drawList.dots { context.fillEllipse(in: bounds(dot)) }
    }

    /// Renders `drawList` into a new opaque sRGB image of `size` × `size`
    /// pixels; `nil` when `size` is not positive or no context is available.
    public static func makeImage(_ drawList: PetalDrawList, size: Int) -> CGImage? {
        guard size > 0, size <= 16_384,
              let space = CGColorSpace(name: CGColorSpace.sRGB),
              let context = CGContext(
                data: nil,
                width: size,
                height: size,
                bitsPerComponent: 8,
                bytesPerRow: 0,
                space: space,
                bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
              ) else {
            return nil
        }
        context.translateBy(x: 0, y: CGFloat(size))
        context.scaleBy(x: 1, y: -1)
        draw(drawList, in: context, rect: CGRect(x: 0, y: 0, width: size, height: size))
        return context.makeImage()
    }

    private static func color(_ color: PetalColor) -> CGColor {
        CGColor(
            srgbRed: CGFloat(color.red) / 255,
            green: CGFloat(color.green) / 255,
            blue: CGFloat(color.blue) / 255,
            alpha: 1
        )
    }

    private static func bounds(_ circle: PetalDrawList.Circle) -> CGRect {
        CGRect(
            x: circle.center.x - circle.radius,
            y: circle.center.y - circle.radius,
            width: 2 * circle.radius,
            height: 2 * circle.radius
        )
    }

    private static func cgRect(_ rect: PetalRect) -> CGRect {
        CGRect(x: rect.x, y: rect.y, width: rect.width, height: rect.height)
    }
}
#endif

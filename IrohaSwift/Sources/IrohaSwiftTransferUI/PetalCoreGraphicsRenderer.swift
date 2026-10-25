#if canImport(CoreGraphics)
import CoreGraphics
import Foundation
import IrohaSwift

/// Draws Petal Stream frames with CoreGraphics.
///
/// The renderer paints a ``PetalDrawList`` in the order the draw list
/// documents (background, finder blossoms and notches, the faint `天` clipped
/// to its disc, the lit petals as rotated ellipses, lit ring dots), so the
/// result matches the reference software renderer up to anti-aliasing. Every
/// element class is one path filled once (one path per petal colour), so a
/// frame of about 1,700 lit petals costs a handful of fills; the cosmetic petal
/// notch is omitted (`specs/petal_stream.md` §2.3 allows it).
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
        let blossoms = CGMutablePath()
        let notches = CGMutablePath()
        for finder in drawList.finders {
            blossoms.addEllipse(in: bounds(finder.core))
            for petal in finder.petals { blossoms.addEllipse(in: bounds(petal)) }
            for notch in finder.notches { notches.addEllipse(in: bounds(notch)) }
        }
        fill(blossoms, color(palette.light), in: context)
        fill(notches, color(palette.background), in: context)

        // 3. the faint 天 under the petals, clipped to its disc (one path, so the
        //    edges between lattice cells leave no anti-aliasing seams)
        let backdrop = CGMutablePath()
        backdrop.addRects(drawList.backdrop.map(cgRect))
        context.saveGState()
        context.addEllipse(in: bounds(PetalDrawList.backdropClip))
        context.clip()
        fill(backdrop, color(palette.backdrop), in: context)
        context.restoreGState()

        // 4. lit petals: an ellipse along each petal's outward angle, one path per colour
        let halfLength = CGFloat(PetalDrawList.petalHalfLength)
        let halfWidth = CGFloat(PetalDrawList.petalHalfWidth)
        let ellipse = CGRect(x: -halfLength, y: -halfWidth, width: 2 * halfLength, height: 2 * halfWidth)
        let light = CGMutablePath()
        let pink = CGMutablePath()
        for petal in drawList.petals {
            let cosine = CGFloat(cos(petal.angle))
            let sine = CGFloat(sin(petal.angle))
            let placement = CGAffineTransform(
                a: cosine, b: sine, c: -sine, d: cosine,
                tx: CGFloat(petal.center.x), ty: CGFloat(petal.center.y)
            )
            (petal.light ? light : pink).addEllipse(in: ellipse, transform: placement)
        }
        fill(light, color(palette.light), in: context)
        fill(pink, color(palette.pink), in: context)

        // 5. lit ring dots
        let dots = CGMutablePath()
        for dot in drawList.dots { dots.addEllipse(in: bounds(dot)) }
        fill(dots, color(palette.pink), in: context)
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

    private static func fill(_ path: CGPath, _ fillColor: CGColor, in context: CGContext) {
        guard !path.isEmpty else { return }
        context.setFillColor(fillColor)
        context.addPath(path)
        context.fillPath()
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

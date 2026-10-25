#if canImport(CoreGraphics)
import CoreGraphics
import Foundation
import IrohaSwift

/// Draws Petal Stream frames with CoreGraphics.
///
/// The renderer paints a ``PetalDrawList`` in the order the draw list
/// documents (background, finder blossoms and notches, the faint `天` clipped
/// to its disc, the lit petals as notched ellipses along their turned axes,
/// lit ring dots), antialiased, so the result matches the reference software
/// renderer up to anti-aliasing. Every element class is one path filled once
/// (one path per petal colour), so a frame of about 1,700 lit petals costs a
/// handful of fills. The petal notch is drawn: the turn reader tells a petal
/// from one turned by half a turn by it (`specs/petal_stream.md` §2.3).
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
        // petals are 12 × 7.5 units: only antialiased edges keep their shape and turn
        context.setAllowsAntialiasing(true)
        context.setShouldAntialias(true)
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

        // 4. lit petals: the notched ellipse along each petal's axis (its outward
        //    direction plus its turn), one path per colour
        let light = CGMutablePath()
        let pink = CGMutablePath()
        for petal in drawList.petals {
            let cosine = CGFloat(cos(petal.angle))
            let sine = CGFloat(sin(petal.angle))
            let placement = CGAffineTransform(
                a: cosine, b: sine, c: -sine, d: cosine,
                tx: CGFloat(petal.center.x), ty: CGFloat(petal.center.y)
            )
            (petal.light ? light : pink).addPath(petalOutline, transform: placement)
        }
        fill(light, color(palette.light), in: context)
        fill(pink, color(palette.pink), in: context)

        // 5. lit ring dots
        let dots = CGMutablePath()
        for dot in drawList.dots { dots.addEllipse(in: bounds(dot)) }
        fill(dots, color(palette.pink), in: context)
    }

    /// The outline of a petal in its own frame (axis along `x`, centre at the
    /// origin): the ellipse of half-length ``PetalDrawList/petalHalfLength``
    /// and half-width ``PetalDrawList/petalHalfWidth`` minus the notch, a disc
    /// of radius ``PetalDrawList/petalNotchRadius`` centred
    /// ``PetalDrawList/petalNotchDistance`` along the axis. The ellipse arc
    /// runs round the back of the petal between the two points where the
    /// notch circle crosses it, and the notch arc closes the outline through
    /// the inside of the ellipse.
    static let petalOutline: CGPath = {
        let a = PetalDrawList.petalHalfLength
        let b = PetalDrawList.petalHalfWidth
        let r = PetalDrawList.petalNotchRadius
        let d = PetalDrawList.petalNotchDistance
        // the notch circle (d + r cos φ, r sin φ) meets the ellipse where
        // ((d + r cos φ) / a)² + (r sin φ / b)² = 1, a quadratic in cos φ
        let qa = r * r / (a * a) - r * r / (b * b)
        let qb = 2 * d * r / (a * a)
        let qc = d * d / (a * a) + r * r / (b * b) - 1
        let root = (qb * qb - 4 * qa * qc).squareRoot()
        let cosine = [(-qb + root) / (2 * qa), (-qb - root) / (2 * qa)].first { abs($0) <= 1 } ?? -1
        let phi = acos(cosine)
        // the ellipse parameter of the upper crossing
        let t = atan2(r * sin(phi) / b, (d + r * cosine) / a)
        let path = CGMutablePath()
        path.addArc(
            center: .zero,
            radius: 1,
            startAngle: CGFloat(t),
            endAngle: CGFloat(2 * Double.pi - t),
            clockwise: false,
            transform: CGAffineTransform(scaleX: CGFloat(a), y: CGFloat(b))
        )
        path.addArc(
            center: CGPoint(x: d, y: 0),
            radius: CGFloat(r),
            startAngle: CGFloat(2 * Double.pi - phi),
            endAngle: CGFloat(phi),
            clockwise: true
        )
        path.closeSubpath()
        return path
    }()

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

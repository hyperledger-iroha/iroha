import Foundation

/// An 8-bit sRGB colour.
public struct PetalColor: Equatable, Hashable, Sendable {
    /// Red channel.
    public var red: UInt8
    /// Green channel.
    public var green: UInt8
    /// Blue channel.
    public var blue: UInt8

    public init(red: UInt8, green: UInt8, blue: UInt8) {
        self.red = red
        self.green = green
        self.blue = blue
    }
}

/// Colours of a Petal frame.
public struct PetalPalette: Equatable, Sendable {
    /// Background.
    public var background: PetalColor
    /// Light tile fill and finders.
    public var light: PetalColor
    /// Sakura pink of dots and of glyphs on dark tiles.
    public var pink: PetalColor
    /// Glyph colour on a light tile.
    public var ink: PetalColor

    public init(background: PetalColor, light: PetalColor, pink: PetalColor, ink: PetalColor) {
        self.background = background
        self.light = light
        self.pink = pink
        self.ink = ink
    }

    /// The reference palette: black, pale sakura, sakura pink and near-black ink.
    public static let standard = PetalPalette(
        background: PetalColor(red: 0, green: 0, blue: 0),
        light: PetalColor(red: 250, green: 235, blue: 244),
        pink: PetalColor(red: 245, green: 175, blue: 208),
        ink: PetalColor(red: 20, green: 4, blue: 14)
    )
}

/// Software rendering options.
public struct PetalRenderOptions: Equatable, Sendable {
    /// Output side in pixels.
    public var size: Int
    /// Samples per pixel side for anti-aliasing (1–4).
    public var supersample: Int
    /// Colours.
    public var palette: PetalPalette

    public init(size: Int = 1024, supersample: Int = 3, palette: PetalPalette = .standard) {
        self.size = size
        self.supersample = supersample
        self.palette = palette
    }
}

/// Reference software renderer (port of `crates/iroha_petal/src/render.rs`).
///
/// The picture is black, with four sakura-blossom finders in the canvas
/// corners and a `天`-shaped field of 256 tiles inside three dotted rings. A
/// light tile is a pale rounded square with a near-black katakana; a dark tile
/// is empty except for its sakura-pink katakana. The output is pixel-identical
/// to the Rust reference. Platform code may instead draw a ``PetalDrawList``
/// with its own 2D API: geometry and polarity are the same.
public enum PetalRenderer {
    static let bitmapSize = 128

    /// 128×128 ink bitmaps of the sixteen glyphs.
    static let glyphBitmaps: [Bool] = {
        let n = bitmapSize
        var bitmaps = [Bool](repeating: false, count: PetalGlyphs.count * n * n)
        for glyph in 0..<PetalGlyphs.count {
            for v in 0..<n {
                for u in 0..<n {
                    let x = (Double(u) + 0.5) / Double(n) * PetalGlyphs.grid
                    let y = (Double(v) + 0.5) / Double(n) * PetalGlyphs.grid
                    bitmaps[(glyph * n + v) * n + u] = PetalGlyphs.isInked(glyph: glyph, x: x, y: y)
                }
            }
        }
        return bitmaps
    }()

    /// Renders one frame as interleaved RGB.
    ///
    /// - Throws: ``PetalImageError/invalidRenderOptions`` when `options.size`
    ///   is not positive or `options.supersample` is outside 1–4.
    public static func render(
        _ cells: PetalFrameCells,
        options: PetalRenderOptions = PetalRenderOptions()
    ) throws -> PetalRGBImage {
        guard options.size > 0, options.size <= 16_384, (1...4).contains(options.supersample) else {
            throw PetalImageError.invalidRenderOptions
        }
        let shader = Shader(cells: cells, palette: options.palette)
        let size = options.size
        let s = options.supersample
        let unit = PetalLayout.canvas / Double(size)
        let n = UInt32(s * s)
        var data = [UInt8](repeating: 0, count: size * size * 3)
        data.withUnsafeMutableBufferPointer { data in
            for py in 0..<size {
                for px in 0..<size {
                    var r: UInt32 = 0
                    var g: UInt32 = 0
                    var b: UInt32 = 0
                    for sy in 0..<s {
                        for sx in 0..<s {
                            let x = (Double(px) + (Double(sx) + 0.5) / Double(s)) * unit
                            let y = (Double(py) + (Double(sy) + 0.5) / Double(s)) * unit
                            let color = shader.shade(x, y)
                            r += UInt32(color.red)
                            g += UInt32(color.green)
                            b += UInt32(color.blue)
                        }
                    }
                    let at = (py * size + px) * 3
                    data[at] = UInt8(truncatingIfNeeded: (r + n / 2) / n)
                    data[at + 1] = UInt8(truncatingIfNeeded: (g + n / 2) / n)
                    data[at + 2] = UInt8(truncatingIfNeeded: (b + n / 2) / n)
                }
            }
        }
        return PetalRGBImage(width: size, height: size, pixels: data)
    }

    /// Inside test for a rounded square of half-side `half` and corner
    /// radius `radius`, relative to its centre.
    @inline(__always)
    static func inRoundedSquare(_ dx: Double, _ dy: Double, _ half: Double, _ radius: Double) -> Bool {
        let ax = abs(dx)
        let ay = abs(dy)
        if ax > half || ay > half { return false }
        let cx = ax - (half - radius)
        let cy = ay - (half - radius)
        return cx <= 0.0 || cy <= 0.0 || cx * cx + cy * cy <= radius * radius
    }

    /// Per-sample colour function; layout tables are copied in once so the
    /// inner loop touches no lazily initialised globals.
    private struct Shader {
        let light: [Bool]
        let glyph: [UInt8]
        let dots: [Bool]
        let palette: PetalPalette
        let finderCenters = PetalLayout.finderCenters
        let tileLookup = PetalLayout.tileLookup
        let bitmaps = PetalRenderer.glyphBitmaps
        let ringRadii = PetalLayout.ringRadii
        let ringSlots = PetalLayout.ringSlots
        let ringOffsets = (0..<PetalLayout.ringCount).map(PetalLayout.ringOffset)

        init(cells: PetalFrameCells, palette: PetalPalette) {
            light = cells.light
            glyph = cells.glyph
            dots = cells.dots
            self.palette = palette
        }

        func shade(_ x: Double, _ y: Double) -> PetalColor {
            // finders
            for center in finderCenters {
                let dx = x - center.x
                let dy = y - center.y
                if (dx * dx + dy * dy).squareRoot() <= PetalLayout.finderOuter {
                    return PetalLayout.finderLit(dx: dx, dy: dy) ? palette.light : palette.background
                }
            }
            // tiles
            let lattice = PetalLayout.tileOrigin
            let pitch = PetalLayout.tilePitch
            if x >= lattice && y >= lattice {
                let col = Int((x - lattice) / pitch)
                let row = Int((y - lattice) / pitch)
                if col < PetalLayout.tileGrid && row < PetalLayout.tileGrid {
                    let tile = tileLookup[row * PetalLayout.tileGrid + col]
                    if tile >= 0 {
                        let cx = lattice + pitch * (Double(col) + 0.5)
                        let cy = lattice + pitch * (Double(row) + 0.5)
                        let dx = x - cx
                        let dy = y - cy
                        if !inRoundedSquare(dx, dy, PetalLayout.tileSize / 2.0, PetalLayout.tileCornerRadius) {
                            return palette.background
                        }
                        let boxHalf = PetalLayout.glyphBox / 2.0
                        var inked = false
                        if abs(dx) < boxHalf && abs(dy) < boxHalf {
                            let n = PetalRenderer.bitmapSize
                            let u = Int((dx + boxHalf) / (2.0 * boxHalf) * Double(n))
                            let v = Int((dy + boxHalf) / (2.0 * boxHalf) * Double(n))
                            let symbol = Int(glyph[tile] & 0x0F)
                            inked = bitmaps[(symbol * n + min(v, n - 1)) * n + min(u, n - 1)]
                        }
                        switch (light[tile], inked) {
                        case (true, false): return palette.light
                        case (true, true): return palette.ink
                        case (false, true): return palette.pink
                        case (false, false): return palette.background
                        }
                    }
                }
            }
            // ring dots
            let dx = x - PetalLayout.center
            let dy = y - PetalLayout.center
            let radius = (dx * dx + dy * dy).squareRoot()
            let tau = 2 * Double.pi
            for ring in 0..<ringRadii.count {
                let ringRadius = ringRadii[ring]
                if abs(radius - ringRadius) > PetalLayout.dotRadius { continue }
                let slots = ringSlots[ring]
                var theta = atan2(dy, dx)
                if theta < 0.0 { theta += tau }
                let slot = Int((theta / tau * Double(slots)).rounded()) % slots
                if !dots[ringOffsets[ring] + slot] { continue }
                let angle = tau * Double(slot) / Double(slots)
                let px = ringRadius * cos(angle)
                let py = ringRadius * sin(angle)
                if ((dx - px) * (dx - px) + (dy - py) * (dy - py)).squareRoot() <= PetalLayout.dotRadius {
                    return palette.pink
                }
            }
            return palette.background
        }
    }
}

/// An axis-aligned rectangle in canvas units.
public struct PetalRect: Equatable, Sendable {
    /// Left edge.
    public var x: Double
    /// Top edge.
    public var y: Double
    /// Width.
    public var width: Double
    /// Height.
    public var height: Double

    public init(x: Double, y: Double, width: Double, height: Double) {
        self.x = x
        self.y = y
        self.width = width
        self.height = height
    }
}

/// A platform-neutral vector description of one frame for native 2D APIs
/// (CoreGraphics, SwiftUI `Canvas`, Metal, …).
///
/// All geometry is in canvas units (``canvasSize`` per side, origin top-left,
/// `y` down). Paint in this order to reproduce ``PetalRenderer``:
///
/// 1. fill the canvas with ``PetalPalette/background``;
/// 2. for each finder fill the ``Finder/core`` and every petal circle with
///    ``PetalPalette/light``, then fill every notch circle with the background;
/// 3. for each tile fill ``Tile/fill`` (light tiles only) as a rounded rect,
///    then stroke the ``Tile/strokes`` polylines with ``Tile/ink``, width
///    ``Tile/strokeWidth``, round caps and joins, clipped to ``Tile/glyphBox``;
/// 4. fill every lit ``dots`` circle with ``PetalPalette/pink``.
public struct PetalDrawList: Equatable, Sendable {
    /// A circle.
    public struct Circle: Equatable, Sendable {
        /// Centre in canvas units.
        public let center: PetalPoint
        /// Radius in canvas units.
        public let radius: Double
    }

    /// A sakura-blossom finder.
    public struct Finder: Equatable, Sendable {
        /// Solid centre disc.
        public let core: Circle
        /// The five petal discs, the first pointing straight up.
        public let petals: [Circle]
        /// The five notches cut into the petal tips (painted with the
        /// background colour).
        public let notches: [Circle]
    }

    /// One data tile with its katakana.
    public struct Tile: Equatable, Sendable {
        /// Tile index (`0..<256`).
        public let index: Int
        /// The tile square; drawn as a rounded rect.
        public let rect: PetalRect
        /// Corner radius of the rounded rect.
        public let cornerRadius: Double
        /// Polarity: light tiles are filled, dark tiles are not.
        public let light: Bool
        /// Glyph symbol (`0..<16`).
        public let glyph: Int
        /// The square the glyph is drawn into; strokes are clipped to it.
        public let glyphBox: PetalRect
        /// Glyph polylines in canvas units.
        public let strokes: [[PetalPoint]]
        /// Stroke width in canvas units (`6.5 / 32` of the glyph box).
        public let strokeWidth: Double
        /// Tile fill colour, `nil` for a dark (unfilled) tile.
        public let fill: PetalColor?
        /// Glyph colour (ink on light tiles, pink on dark tiles).
        public let ink: PetalColor
    }

    /// Canvas side in design units.
    public static let canvasSize = PetalLayout.canvas
    /// Colours of the frame.
    public let palette: PetalPalette
    /// The four corner finders, clockwise from top-left.
    public let finders: [Finder]
    /// The 256 tiles in tile order.
    public let tiles: [Tile]
    /// The lit ring dots (gates and data dots).
    public let dots: [Circle]

    /// Builds the draw list of `cells`.
    public init(cells: PetalFrameCells, palette: PetalPalette = .standard) {
        self.palette = palette
        finders = PetalLayout.finderCenters.map { center in
            var petals: [Circle] = []
            var notches: [Circle] = []
            for petal in 0..<PetalLayout.finderPetals {
                let angle = PetalLayout.petalAngle(petal)
                petals.append(Circle(
                    center: PetalPoint(
                        x: center.x + PetalLayout.finderPetalDistance * cos(angle),
                        y: center.y + PetalLayout.finderPetalDistance * sin(angle)
                    ),
                    radius: PetalLayout.finderPetalRadius
                ))
                notches.append(Circle(
                    center: PetalPoint(
                        x: center.x + PetalLayout.finderOuter * cos(angle),
                        y: center.y + PetalLayout.finderOuter * sin(angle)
                    ),
                    radius: PetalLayout.finderNotchRadius
                ))
            }
            return Finder(
                core: Circle(center: center, radius: PetalLayout.finderCore),
                petals: petals,
                notches: notches
            )
        }
        let half = PetalLayout.tileSize / 2
        let boxHalf = PetalLayout.glyphBox / 2
        let scale = PetalLayout.glyphBox / PetalGlyphs.grid
        tiles = (0..<PetalLayout.tileCount).map { index in
            let center = PetalLayout.tileCenter(index)
            let light = cells.light[index]
            let glyph = Int(cells.glyph[index] & 0x0F)
            let originX = center.x - boxHalf
            let originY = center.y - boxHalf
            return Tile(
                index: index,
                rect: PetalRect(x: center.x - half, y: center.y - half, width: PetalLayout.tileSize, height: PetalLayout.tileSize),
                cornerRadius: PetalLayout.tileCornerRadius,
                light: light,
                glyph: glyph,
                glyphBox: PetalRect(x: originX, y: originY, width: PetalLayout.glyphBox, height: PetalLayout.glyphBox),
                strokes: PetalGlyphs.strokes[glyph].map { polyline in
                    polyline.map { PetalPoint(x: originX + $0.x * scale, y: originY + $0.y * scale) }
                },
                strokeWidth: PetalGlyphs.strokeWidth * scale,
                fill: light ? palette.light : nil,
                ink: light ? palette.ink : palette.pink
            )
        }
        var dots: [Circle] = []
        let tau = 2 * Double.pi
        for ring in 0..<PetalLayout.ringCount {
            let slots = PetalLayout.ringSlots[ring]
            for slot in 0..<slots where cells.dots[PetalLayout.ringOffset(ring) + slot] {
                let angle = tau * Double(slot) / Double(slots)
                dots.append(Circle(
                    center: PetalPoint(
                        x: PetalLayout.center + PetalLayout.ringRadii[ring] * cos(angle),
                        y: PetalLayout.center + PetalLayout.ringRadii[ring] * sin(angle)
                    ),
                    radius: PetalLayout.dotRadius
                ))
            }
        }
        self.dots = dots
    }
}

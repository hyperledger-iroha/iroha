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
    /// Finders and the petals inside the `天`.
    public var light: PetalColor
    /// Sakura pink of the ring dots and of the petals outside the `天`.
    public var pink: PetalColor
    /// The faint `天` drawn under the petals (a tenth of the light colour).
    public var backdrop: PetalColor

    public init(background: PetalColor, light: PetalColor, pink: PetalColor, backdrop: PetalColor) {
        self.background = background
        self.light = light
        self.pink = pink
        self.backdrop = backdrop
    }

    /// The reference palette: black, pale sakura, sakura pink and a faint `天`.
    public static let standard = PetalPalette(
        background: PetalColor(red: 0, green: 0, blue: 0),
        light: PetalColor(red: 250, green: 235, blue: 244),
        pink: PetalColor(red: 245, green: 175, blue: 208),
        backdrop: PetalColor(red: 25, green: 24, blue: 24)
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
/// corners, three dotted rings, and inside them a field of petals on
/// concentric bands, each lit petal turned about its centre by its turn. A lit
/// petal whose centre lies inside the `天` is drawn in the light colour, the
/// others in sakura pink, over a faint backdrop of the `天` itself, so the
/// character shows through the storm of petals. The output is pixel-identical
/// to the Rust reference. Platform code may instead draw a ``PetalDrawList``
/// with its own 2D API: geometry, turns and polarity are the same.
public enum PetalRenderer {
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

    /// Per-sample colour function, shading in the reference's order: finders,
    /// ring dots, petals, the backdrop `天`, background. Layout tables are
    /// copied in once so the inner loop touches no lazily initialised globals.
    private struct Shader {
        let dots: [Bool]
        let petals: [Bool]
        let turns: [UInt8]
        let palette: PetalPalette
        let finderCenters = PetalLayout.finderCenters
        let ringRadii = PetalLayout.ringRadii
        let ringSlots = PetalLayout.ringSlots
        let ringOffsets = (0..<PetalLayout.ringCount).map(PetalLayout.ringOffset)
        let bandSlots = PetalLayout.bandSlots
        let bandOffsets = PetalLayout.bandOffsets
        let cellX = PetalLayout.cellCenterX
        let cellY = PetalLayout.cellCenterY
        let axisSin = PetalTurns.axisSin
        let axisCos = PetalTurns.axisCos
        let lightCells = PetalLayout.lightCells

        init(cells: PetalFrameCells, palette: PetalPalette) {
            dots = cells.dots
            petals = cells.petals
            turns = cells.turns
            self.palette = palette
        }

        /// The colour of the lit petal at `(x, y)`, if any: the cells of the
        /// nearest three bands and the nearest three slots of each are tried.
        func petal(_ x: Double, _ y: Double, _ radius: Double, _ theta: Double) -> PetalColor? {
            let tau = 2 * Double.pi
            let reach = PetalLayout.petalHalfLength
            let nearest = ((radius - PetalLayout.bandInner) / PetalLayout.bandSpacing).rounded()
            for step in -1...1 {
                let band = nearest + Double(step)
                if band < 0.0 || band >= Double(PetalLayout.bandCount) { continue }
                let index = Int(band)
                if abs(radius - PetalLayout.bandRadius(index)) > reach { continue }
                let n = bandSlots[index]
                let around = PetalNumeric.saturatingInt(
                    (theta / tau * Double(n) - PetalLayout.bandPhase(index)).rounded()
                )
                for slot in (around - 1)...(around + 1) {
                    let cell = bandOffsets[index] + ((slot % n) + n) % n
                    if !petals[cell] { continue }
                    // the petal's axis: its outward direction plus its turn
                    let axis = Int(turns[cell]) * PetalLayout.fieldCells + cell
                    if PetalLayout.petalLit(x - cellX[cell], y - cellY[cell], sin: axisSin[axis], cos: axisCos[axis]) {
                        return lightCells[cell] ? palette.light : palette.pink
                    }
                }
            }
            return nil
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
            let dx = x - PetalLayout.center
            let dy = y - PetalLayout.center
            let radius = (dx * dx + dy * dy).squareRoot()
            let tau = 2 * Double.pi
            var theta = atan2(dy, dx)
            if theta < 0.0 { theta += tau }
            // ring dots
            for ring in 0..<ringRadii.count {
                let ringRadius = ringRadii[ring]
                if abs(radius - ringRadius) > PetalLayout.dotRadius { continue }
                let slots = ringSlots[ring]
                let slot = PetalNumeric.saturatingInt((theta / tau * Double(slots)).rounded()) % slots
                if !dots[ringOffsets[ring] + slot] { continue }
                let (s, c) = PetalNumeric.sinCos(tau * Double(slot) / Double(slots))
                let px = ringRadius * c
                let py = ringRadius * s
                if ((dx - px) * (dx - px) + (dy - py) * (dy - py)).squareRoot() <= PetalLayout.dotRadius {
                    return palette.pink
                }
            }
            // petals, then the faint 天 under them
            if radius <= PetalLayout.backdropRadius {
                if let color = petal(x, y, radius, theta) { return color }
                if PetalLayout.inMask(x: x, y: y) { return palette.backdrop }
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
/// 3. fill the ``backdrop`` cells with ``PetalPalette/backdrop``, clipped to
///    the disc ``backdropClip``;
/// 4. fill every lit ``petals`` entry in its ``Petal/color``, antialiased: an
///    ellipse of half-length ``petalHalfLength`` along ``Petal/angle`` and
///    half-width ``petalHalfWidth``, centred on ``Petal/center``, minus the
///    notch at its tip (a disc of radius ``petalNotchRadius`` centred
///    ``petalNotchDistance`` along ``Petal/angle``). The notch must be drawn:
///    it tells a petal from one turned by half a turn, and the turn reader's
///    templates include it;
/// 5. fill every lit ``dots`` circle with ``PetalPalette/pink``.
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

    /// A lit petal of the field.
    public struct Petal: Equatable, Sendable {
        /// Field cell index (`0..<3376`).
        public let cell: Int
        /// Centre in canvas units.
        public let center: PetalPoint
        /// The petal's long axis (radians, clockwise on screen from 3
        /// o'clock): its outward direction turned by its turn
        /// (``PetalTurns/turnAngle(_:)`` of ``PetalFrameCells/turns``).
        public let angle: Double
        /// Whether the petal lies inside the `天` (drawn in the light colour).
        public let light: Bool
        /// Fill colour: ``PetalPalette/light`` inside the `天`,
        /// ``PetalPalette/pink`` outside.
        public let color: PetalColor
    }

    /// Canvas side in design units.
    public static let canvasSize = PetalLayout.canvas
    /// Half the length of a petal along ``Petal/angle``.
    public static let petalHalfLength = PetalLayout.petalHalfLength
    /// Half the width of a petal.
    public static let petalHalfWidth = PetalLayout.petalHalfWidth
    /// Radius of the notch at a petal's tip.
    public static let petalNotchRadius = PetalLayout.petalNotchRadius
    /// Distance from a petal's centre to its notch centre, along its axis.
    public static let petalNotchDistance = PetalLayout.petalNotchDistance
    /// The disc the ``backdrop`` is clipped to.
    public static let backdropClip = Circle(
        center: PetalPoint(x: PetalLayout.center, y: PetalLayout.center),
        radius: PetalLayout.backdropRadius
    )
    /// Every `#` cell of the `天` mask lattice, row by row.
    static let backdropCells: [PetalRect] = {
        var cells: [PetalRect] = []
        for row in 0..<PetalLayout.maskGrid {
            for column in 0..<PetalLayout.maskGrid where PetalLayout.maskCells[row * PetalLayout.maskGrid + column] {
                cells.append(PetalRect(
                    x: PetalLayout.maskOrigin + PetalLayout.maskPitch * Double(column),
                    y: PetalLayout.maskOrigin + PetalLayout.maskPitch * Double(row),
                    width: PetalLayout.maskPitch,
                    height: PetalLayout.maskPitch
                ))
            }
        }
        return cells
    }()

    /// Colours of the frame.
    public let palette: PetalPalette
    /// The four corner finders, clockwise from top-left.
    public let finders: [Finder]
    /// The faint `天`: every `#` cell of the mask lattice, row by row,
    /// unclipped (clip to ``backdropClip``).
    public let backdrop: [PetalRect]
    /// The lit petals in cell order.
    public let petals: [Petal]
    /// The lit ring dots (gates and data dots).
    public let dots: [Circle]

    /// Builds the draw list of `cells`.
    public init(cells: PetalFrameCells, palette: PetalPalette = .standard) {
        self.palette = palette
        finders = PetalLayout.finderCenters.map { center in
            var petals: [Circle] = []
            var notches: [Circle] = []
            for petal in 0..<PetalLayout.finderPetals {
                let (s, c) = PetalNumeric.sinCos(PetalLayout.finderPetalAngle(petal))
                petals.append(Circle(
                    center: PetalPoint(
                        x: center.x + PetalLayout.finderPetalDistance * c,
                        y: center.y + PetalLayout.finderPetalDistance * s
                    ),
                    radius: PetalLayout.finderPetalRadius
                ))
                notches.append(Circle(
                    center: PetalPoint(
                        x: center.x + PetalLayout.finderOuter * c,
                        y: center.y + PetalLayout.finderOuter * s
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
        backdrop = Self.backdropCells
        var petals: [Petal] = []
        petals.reserveCapacity(PetalLayout.fieldCells / 2 + 64)
        for cell in 0..<PetalLayout.fieldCells where cells.petals[cell] {
            let light = PetalLayout.lightCells[cell]
            petals.append(Petal(
                cell: cell,
                center: PetalLayout.cellCenter(cell),
                angle: PetalLayout.cellAngles[cell] + PetalTurns.turnAngle(cells.turns[cell]),
                light: light,
                color: light ? palette.light : palette.pink
            ))
        }
        self.petals = petals
        var dots: [Circle] = []
        let tau = 2 * Double.pi
        for ring in 0..<PetalLayout.ringCount {
            let slots = PetalLayout.ringSlots[ring]
            for slot in 0..<slots where cells.dots[PetalLayout.ringOffset(ring) + slot] {
                let (s, c) = PetalNumeric.sinCos(tau * Double(slot) / Double(slots))
                dots.append(Circle(
                    center: PetalPoint(
                        x: PetalLayout.center + PetalLayout.ringRadii[ring] * c,
                        y: PetalLayout.center + PetalLayout.ringRadii[ring] * s
                    ),
                    radius: PetalLayout.dotRadius
                ))
            }
        }
        self.dots = dots
    }
}

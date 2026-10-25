import Foundation

/// A point in Petal canvas units or image pixels.
public struct PetalPoint: Equatable, Hashable, Sendable {
    /// Horizontal coordinate.
    public var x: Double
    /// Vertical coordinate (grows downward).
    public var y: Double

    public init(x: Double, y: Double) {
        self.x = x
        self.y = y
    }
}

/// What a ring slot is used for.
public enum PetalSlotRole: Equatable, Hashable, Sendable {
    /// A gate dot, always lit.
    case gate
    /// A slot next to a gate, always dark.
    case `guard`
    /// Carries data bit `n` of lane `D`.
    case data(UInt16)
    /// Unused data slot, always dark.
    case spare
}

/// Normative Petal frame geometry (port of `crates/iroha_petal/src/layout.rs`).
///
/// All coordinates are design units on a square canvas of ``canvas`` units
/// with the origin at the top-left and `y` growing downward. A renderer
/// scales the canvas to any pixel size; a decoder maps camera pixels back to
/// design units. Cell order is defined by integer indices only, and the slot
/// count of every band is a fixed table.
public enum PetalLayout {
    /// Canvas side length in design units.
    public static let canvas: Double = 1024
    /// Canvas centre coordinate on both axes.
    public static let center: Double = 512

    // MARK: - Petal field

    /// Number of petal bands (concentric rings of field cells).
    public static let bandCount = 34
    /// Radius of band 0.
    public static let bandInner: Double = 16
    /// Radial distance between consecutive bands.
    public static let bandSpacing: Double = 11
    /// Cells on each band, inner band first: `4 · round(2π r / 50)` for the
    /// band radius `r`, about one cell every 12.5 units. Every count is a
    /// multiple of four, so the four sectors split each band evenly and the
    /// field maps onto itself under quarter turns. The table, not the formula,
    /// is normative.
    public static let bandSlots: [Int] = [
        8, 12, 20, 24, 32, 36, 40, 48, 52, 56, 64, 68, 76, 80, 84, 92, 96, 104, 108, 112, 120, 124,
        128, 136, 140, 148, 152, 156, 164, 168, 172, 180, 184, 192,
    ]
    /// Number of field cells.
    public static let fieldCells = 3376
    /// Angular sectors of the field; each is one lane codeword.
    public static let sectors = 4
    /// Cells in each sector.
    public static let sectorCellCount = fieldCells / sectors
    // The petal sizes are single-precision constants in the reference; they
    // are widened from `Float` so that sampling and rendering stay bit-exact.
    /// Radius of the disc a petal stands in for: decoders sample within it,
    /// and no other petal, however it is turned, reaches into it.
    public static let petalRadius = Double(Float(4.6))
    /// Half the length of a drawn petal, along its axis (the outward radial
    /// direction, turned by the petal's turn, see ``PetalTurns``).
    public static let petalHalfLength = Double(Float(6.0))
    /// Half the width of a drawn petal.
    public static let petalHalfWidth = Double(Float(3.73))
    /// Radius of the notch cut into each petal tip; it tells a petal from one
    /// turned by half a turn.
    public static let petalNotchRadius = Double(Float(1.12))
    /// Distance from a petal centre to its notch centre, along its axis.
    public static let petalNotchDistance = Double(Float(6.24))

    /// Index of the first cell of every band (and the total at the end).
    public static let bandOffsets: [Int] = {
        var offsets = [0]
        for slots in bandSlots {
            precondition(slots % sectors == 0, "band slot counts are multiples of four")
            offsets.append(offsets[offsets.count - 1] + slots)
        }
        precondition(offsets[bandCount] == fieldCells, "the band table must hold every cell")
        return offsets
    }()

    /// Radius of band `band`.
    public static func bandRadius(_ band: Int) -> Double {
        bandInner + bandSpacing * Double(band)
    }

    /// Fraction of a slot by which band `band` is turned: odd bands sit half a
    /// slot over, so neighbouring bands interleave.
    public static func bandPhase(_ band: Int) -> Double {
        band % 2 == 1 ? 0.5 : 0.0
    }

    /// Band and slot of field cell `cell` (`0..<fieldCells`).
    public static func cellBandSlot(_ cell: Int) -> (band: Int, slot: Int) {
        let band = Int(cellBands[cell])
        return (band, cell - bandOffsets[band])
    }

    /// Angle (radians, clockwise on screen from 3 o'clock) of the centre of
    /// field cell `cell`.
    public static func cellAngle(_ cell: Int) -> Double {
        cellAngles[cell]
    }

    /// Canvas coordinates of the centre of field cell `cell`.
    public static func cellCenter(_ cell: Int) -> PetalPoint {
        PetalPoint(x: cellCenterX[cell], y: cellCenterY[cell])
    }

    /// The cells of sector `sector` (`0..<4`) in bit order: band by band from
    /// the centre out, and within a band in ascending slot order. Sector `s`
    /// holds slots `s·n/4 … (s+1)·n/4 − 1` of every band of `n` slots. The
    /// first `8 × 105` cells carry the sector's codeword; the last four are
    /// spare and stay dark.
    public static func sectorCells(_ sector: Int) -> [Int] {
        sectorTable[sector]
    }

    /// Band of every field cell.
    static let cellBands: [UInt8] = {
        var bands = [UInt8](repeating: 0, count: fieldCells)
        for band in 0..<bandCount {
            for cell in bandOffsets[band]..<bandOffsets[band + 1] { bands[cell] = UInt8(band) }
        }
        return bands
    }()

    /// ``cellAngle(_:)`` of every cell, computed in the reference's order of
    /// operations: `τ (slot + phase) / n`.
    static let cellAngles: [Double] = (0..<fieldCells).map { cell in
        let band = Int(cellBands[cell])
        let slot = cell - bandOffsets[band]
        return (2 * Double.pi) * (Double(slot) + bandPhase(band)) / Double(bandSlots[band])
    }

    /// Cosine of every cell angle (`PetalNumeric.sinCos`, like the reference's
    /// fused sine and cosine).
    static let cellCos: [Double] = cellAngles.map { PetalNumeric.sinCos($0).cos }
    /// Sine of every cell angle.
    static let cellSin: [Double] = cellAngles.map { PetalNumeric.sinCos($0).sin }
    /// Horizontal centre of every cell.
    static let cellCenterX: [Double] = (0..<fieldCells).map { cell in
        center + bandRadius(Int(cellBands[cell])) * cellCos[cell]
    }
    /// Vertical centre of every cell.
    static let cellCenterY: [Double] = (0..<fieldCells).map { cell in
        center + bandRadius(Int(cellBands[cell])) * cellSin[cell]
    }

    /// ``sectorCells(_:)`` of the four sectors.
    static let sectorTable: [[Int]] = (0..<sectors).map { sector in
        var cells: [Int] = []
        cells.reserveCapacity(sectorCellCount)
        for band in 0..<bandCount {
            let quarter = bandSlots[band] / sectors
            for slot in sector * quarter..<(sector + 1) * quarter {
                cells.append(bandOffsets[band] + slot)
            }
        }
        return cells
    }

    // MARK: - The 天

    /// Number of lattice columns and rows of the `天` mask.
    public static let maskGrid = 20
    /// Canvas coordinate of the mask lattice's left and top edge.
    public static let maskOrigin: Double = 182
    /// Mask lattice pitch in design units.
    public static let maskPitch: Double = 33
    /// Radius beyond which the `天` backdrop is not drawn.
    public static let backdropRadius: Double = 390

    /// The `天` silhouette, one string per lattice row from top to bottom.
    /// `#` marks a lattice cell inside the character. Petals whose centre lies
    /// in a `#` cell are drawn in the light colour, the others in pink, and the
    /// `#` cells inside ``backdropRadius`` are drawn as a faint backdrop, so the
    /// character shows through the petals. The mask is mirror-symmetric left to
    /// right and not top to bottom.
    public static let mask: [String] = [
        "....############....",
        "...##############...",
        "..################..",
        ".##################.",
        "###..............###",
        "###..............###",
        "#########..#########",
        "#########..#########",
        "###..............###",
        "###..............###",
        "########....########",
        "########....########",
        "#######......#######",
        "#######..##..#######",
        "######..####..######",
        "#####...####...#####",
        ".###...######...###.",
        "..###.########.###..",
        "......########......",
        ".....##########.....",
    ]

    /// `maskCells[row * maskGrid + column]` is whether that lattice cell is a `#`.
    static let maskCells: [Bool] = mask.flatMap { row in row.map { $0 == "#" } }

    /// Whether the canvas point `(x, y)` lies inside the `天` mask.
    public static func inMask(x: Double, y: Double) -> Bool {
        if x < maskOrigin || y < maskOrigin { return false }
        // Rust `as usize`: saturating, NaN maps to zero
        let column = PetalNumeric.saturatingInt((x - maskOrigin) / maskPitch)
        let row = PetalNumeric.saturatingInt((y - maskOrigin) / maskPitch)
        return column < maskGrid && row < maskGrid && maskCells[row * maskGrid + column]
    }

    /// Whether field cell `cell` lies inside the `天` (drawn in the light colour).
    public static func cellLight(_ cell: Int) -> Bool {
        lightCells[cell]
    }

    /// ``cellLight(_:)`` of every cell.
    static let lightCells: [Bool] = (0..<fieldCells).map { inMask(x: cellCenterX[$0], y: cellCenterY[$0]) }

    // MARK: - Rings

    /// Number of concentric dot rings.
    public static let ringCount = 3
    /// Ring radii in design units.
    public static let ringRadii: [Double] = [405, 440, 475]
    /// Dot slots on each ring (multiples of four so the cardinal gates sit on
    /// a slot).
    public static let ringSlots: [Int] = [88, 100, 108]
    /// Radius of a drawn ring dot.
    public static let dotRadius: Double = 9
    /// Total dot slots over the three rings.
    public static let totalSlots = 296
    /// Dots in each cardinal gate, per ring, for the right, bottom and left
    /// gates. There is deliberately no gate at the top.
    public static let gateDots: [[Int]] = [[1, 1, 3], [2, 2, 2], [1, 2, 2]]
    /// Number of lane `D` bits carried by the rings.
    public static let dataBits = 240

    /// Offset of ring `ring` inside the flat slot index space.
    public static func ringOffset(_ ring: Int) -> Int {
        switch ring {
        case 0: return 0
        case 1: return ringSlots[0]
        default: return ringSlots[0] + ringSlots[1]
        }
    }

    static func gateSlots(ring: Int) -> (gates: [Int], guards: [Int]) {
        let n = ringSlots[ring]
        let bases = [0, n / 4, n / 2] // right, bottom, left
        var gates: [Int] = []
        var guards: [Int] = []
        for (gate, base) in bases.enumerated() {
            let count = gateDots[gate][ring]
            let first: Int
            let last: Int
            switch count {
            case 1: (first, last) = (base, base)
            case 2: (first, last) = (base, base + 1)
            default: (first, last) = (base + n - 1, base + 1)
            }
            let span = (last + n - first) % n + 1
            for step in 0..<span {
                gates.append((first + step) % n)
            }
            guards.append((first + n - 1) % n)
            guards.append((last + 1) % n)
        }
        return (gates, guards)
    }

    /// The role of every ring slot in flat index order.
    public static let slotRoles: [PetalSlotRole] = {
        var roles = [PetalSlotRole](repeating: .spare, count: totalSlots)
        for ring in 0..<ringCount {
            let (gates, guards) = gateSlots(ring: ring)
            let offset = ringOffset(ring)
            for slot in guards { roles[offset + slot] = .guard }
            for slot in gates { roles[offset + slot] = .gate }
        }
        var next: UInt16 = 0
        for index in roles.indices where roles[index] == .spare && Int(next) < dataBits {
            roles[index] = .data(next)
            next += 1
        }
        return roles
    }()

    /// Flat slot index of every lane `D` bit, in bit order.
    public static let dataSlots: [Int] = {
        var slots = [Int](repeating: 0, count: dataBits)
        for (index, role) in slotRoles.enumerated() {
            if case .data(let bit) = role { slots[Int(bit)] = index }
        }
        return slots
    }()

    /// Flat indices of the always-lit gate slots, ascending.
    public static let gateSlots: [Int] = slotRoles.indices.filter { slotRoles[$0] == .gate }
    /// Flat indices of the always-dark guard slots, ascending.
    public static let guardSlots: [Int] = slotRoles.indices.filter { slotRoles[$0] == .guard }

    /// Splits a flat slot index into `(ring, slot)`.
    public static func splitSlot(_ flat: Int) -> (ring: Int, slot: Int) {
        if flat < ringSlots[0] { return (0, flat) }
        if flat < ringSlots[0] + ringSlots[1] { return (1, flat - ringSlots[0]) }
        return (2, flat - ringSlots[0] - ringSlots[1])
    }

    /// Canvas coordinates of the centre of slot `slot` on ring `ring`.
    ///
    /// Slot `0` is at 3 o'clock and slots advance clockwise on the screen.
    /// The value is computed in single precision exactly like the reference
    /// (`f32` trigonometry) and widened to `Double`.
    public static func slotCenter(ring: Int, slot: Int) -> PetalPoint {
        // Rust `f32::consts::TAU` is 2π rounded to nearest (0x40C90FDB);
        // `2 * Float.pi` is one ulp smaller because `Float.pi` rounds toward zero.
        let tau = Float(2 * Double.pi)
        let theta = tau * Float(slot) / Float(ringSlots[ring])
        let radius = Float(ringRadii[ring])
        let (s, c) = PetalNumeric.sinCos(theta)
        let x: Float = 512 + radius * c
        let y: Float = 512 + radius * s
        return PetalPoint(x: Double(x), y: Double(y))
    }

    /// ``slotCenter(ring:slot:)`` of every flat slot index.
    static let flatSlotCenters: [PetalPoint] = (0..<totalSlots).map { flat in
        let (ring, slot) = splitSlot(flat)
        return slotCenter(ring: ring, slot: slot)
    }

    // MARK: - Finders

    /// Canvas coordinates of the four corner finders, clockwise from top-left.
    public static let finderCenters: [PetalPoint] = [
        PetalPoint(x: 72, y: 72),
        PetalPoint(x: 952, y: 72),
        PetalPoint(x: 952, y: 952),
        PetalPoint(x: 72, y: 952),
    ]
    /// Radius of the finder's solid centre disc.
    public static let finderCore: Double = 12
    /// Number of petals of a finder blossom.
    public static let finderPetals = 5
    /// Distance from the finder centre to each petal centre.
    public static let finderPetalDistance: Double = 34
    /// Radius of each petal.
    public static let finderPetalRadius: Double = 26
    /// Radius of the notch cut into each petal tip.
    public static let finderNotchRadius: Double = 6
    /// Outer radius of a finder blossom (tip of a petal).
    public static let finderOuter: Double = 60

    /// Angle of finder petal `petal`; the first petal points straight up.
    static func finderPetalAngle(_ petal: Int) -> Double {
        -Double.pi / 2 + (2 * Double.pi) * Double(petal) / Double(finderPetals)
    }

    /// Whether the point `(dx, dy)`, relative to a finder centre, is lit.
    ///
    /// A finder is a solid five-petal sakura blossom whose first petal points
    /// straight up. The petal notches are cosmetic; decoders only rely on the
    /// blossom being one large, isolated, roughly round blob.
    public static func finderLit(dx: Double, dy: Double) -> Bool {
        if (dx * dx + dy * dy).squareRoot() <= finderCore { return true }
        for petal in 0..<finderPetals {
            let (s, c) = PetalNumeric.sinCos(finderPetalAngle(petal))
            let cx = finderPetalDistance * c
            let cy = finderPetalDistance * s
            if ((dx - cx) * (dx - cx) + (dy - cy) * (dy - cy)).squareRoot() <= finderPetalRadius {
                let nx = finderOuter * c
                let ny = finderOuter * s
                return ((dx - nx) * (dx - nx) + (dy - ny) * (dy - ny)).squareRoot() > finderNotchRadius
            }
        }
        return false
    }

    // MARK: - Petals

    /// Whether the point `(dx, dy)`, relative to the centre of a petal that
    /// points along `angle`, is lit: an ellipse of half-length
    /// ``petalHalfLength`` along `angle` and half-width ``petalHalfWidth``,
    /// minus the notch at its tip.
    public static func petalLit(dx: Double, dy: Double, angle: Double) -> Bool {
        let (s, c) = PetalNumeric.sinCos(angle)
        return petalLit(dx, dy, sin: s, cos: c)
    }

    /// ``petalLit(dx:dy:angle:)`` with the sine and cosine of the angle given.
    @inline(__always)
    static func petalLit(_ dx: Double, _ dy: Double, sin: Double, cos: Double) -> Bool {
        let along = dx * cos + dy * sin
        let across = -dx * sin + dy * cos
        let u = along / petalHalfLength
        let v = across / petalHalfWidth
        if u * u + v * v > 1.0 { return false }
        let tip = along - petalNotchDistance
        return tip * tip + across * across > petalNotchRadius * petalNotchRadius
    }
}

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

/// Lattice column and row of one data tile.
public struct PetalTilePosition: Equatable, Hashable, Sendable {
    /// Lattice column (`0..<20`).
    public let column: Int
    /// Lattice row (`0..<20`).
    public let row: Int
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
/// design units. Cell order is defined by integer indices only.
public enum PetalLayout {
    /// Canvas side length in design units.
    public static let canvas: Double = 1024
    /// Canvas centre coordinate on both axes.
    public static let center: Double = 512

    /// Number of tile lattice columns and rows.
    public static let tileGrid = 20
    /// Canvas coordinate of the lattice's left and top edge.
    public static let tileOrigin: Double = 222
    /// Lattice pitch in design units.
    public static let tilePitch: Double = 29
    /// Side of a drawn tile (pitch minus a 4-unit gutter).
    public static let tileSize: Double = 25
    /// Corner radius of a drawn tile.
    public static let tileCornerRadius: Double = 3
    /// Side of the square glyph box inside a tile.
    public static let glyphBox: Double = 23
    /// Number of data tiles in the `天` mask.
    public static let tileCount = 256

    /// The `天` silhouette, one string per lattice row from top to bottom.
    /// `#` marks a data tile. The mask is mirror-symmetric left to right and
    /// not symmetric top to bottom, so it also tells a decoder which way is up.
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

    /// Lattice position of every data tile in row-major order.
    public static let tiles: [PetalTilePosition] = {
        var tiles: [PetalTilePosition] = []
        tiles.reserveCapacity(tileCount)
        for (row, line) in mask.enumerated() {
            for (column, character) in line.enumerated() where character == "#" {
                tiles.append(PetalTilePosition(column: column, row: row))
            }
        }
        precondition(tiles.count == tileCount, "the Petal mask must contain exactly 256 tiles")
        return tiles
    }()

    /// Tile index at `[row * tileGrid + column]`, or `-1` outside the mask.
    static let tileLookup: [Int] = {
        var table = [Int](repeating: -1, count: tileGrid * tileGrid)
        for (index, position) in tiles.enumerated() {
            table[position.row * tileGrid + position.column] = index
        }
        return table
    }()

    /// Canvas coordinates of the centre of tile `index` (`0..<256`).
    public static func tileCenter(_ index: Int) -> PetalPoint {
        let position = tiles[index]
        return PetalPoint(
            x: tileOrigin + tilePitch * (Double(position.column) + 0.5),
            y: tileOrigin + tilePitch * (Double(position.row) + 0.5)
        )
    }

    /// Number of concentric dot rings.
    public static let ringCount = 3
    /// Ring radii in design units.
    public static let ringRadii: [Double] = [360, 410, 460]
    /// Dot slots on each ring (multiples of four so the cardinal gates sit on
    /// a slot).
    public static let ringSlots: [Int] = [80, 92, 104]
    /// Radius of a drawn ring dot.
    public static let dotRadius: Double = 11
    /// Total dot slots over the three rings.
    public static let totalSlots = 276
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
        let x: Float = 512 + radius * cos(theta)
        let y: Float = 512 + radius * sin(theta)
        return PetalPoint(x: Double(x), y: Double(y))
    }

    /// ``slotCenter(ring:slot:)`` of every flat slot index.
    static let flatSlotCenters: [PetalPoint] = (0..<totalSlots).map { flat in
        let (ring, slot) = splitSlot(flat)
        return slotCenter(ring: ring, slot: slot)
    }

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

    /// Angle of petal `petal`; the first petal points straight up.
    static func petalAngle(_ petal: Int) -> Double {
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
            let angle = petalAngle(petal)
            let cx = finderPetalDistance * cos(angle)
            let cy = finderPetalDistance * sin(angle)
            if ((dx - cx) * (dx - cx) + (dy - cy) * (dy - cy)).squareRoot() <= finderPetalRadius {
                let nx = finderOuter * cos(angle)
                let ny = finderOuter * sin(angle)
                return ((dx - nx) * (dx - nx) + (dy - ny) * (dy - ny)).squareRoot() > finderNotchRadius
            }
        }
        return false
    }
}

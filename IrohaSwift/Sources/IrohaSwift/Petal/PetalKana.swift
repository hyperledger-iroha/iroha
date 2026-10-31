import Foundation

/// One of the four kana lanes of a card: the polarity lane (`P0`, `P1`) and the
/// glyph lane (`K0`, `K1`) of the band above the square (0) and of the band
/// below it (1). Each is one Reed–Solomon codeword, XOR-whitened with a fixed
/// xorshift32 sequence (see ``PetalKana``).
public enum PetalKanaLane: Int, CaseIterable, Hashable, Sendable {
    /// The light or dark tiles of the band above the square.
    case p0
    /// The katakana of the band above the square.
    case k0
    /// The light or dark tiles of the band below the square.
    case p1
    /// The katakana of the band below the square.
    case k1

    /// The polarity (`glyph` false) or glyph lane of `band` (`0..<2`); `nil`
    /// for a band out of range.
    public init?(band: Int, glyph: Bool) {
        guard (0..<PetalLayout.kanaBands).contains(band) else { return nil }
        self.init(rawValue: 2 * band + (glyph ? 1 : 0))
    }

    /// Band: 0 above the square, 1 below it.
    public var band: Int { rawValue / 2 }
    /// Whether the lane carries the tiles' katakana (`K`) rather than their
    /// polarity (`P`).
    public var isGlyph: Bool { rawValue % 2 == 1 }
    /// The lane's name in reports: `P0`, `K0`, `P1` or `K1`.
    public var name: String { (isGlyph ? "K" : "P") + String(band) }
    /// Codeword length in bytes.
    public var wordLength: Int { isGlyph ? PetalKana.glyphWordLength : PetalKana.polarityWordLength }
    /// Parity bytes.
    public var parityLength: Int { isGlyph ? PetalKana.glyphParityLength : PetalKana.polarityParityLength }
    /// Data bytes: the atoms, then their check.
    public var dataLength: Int { wordLength - parityLength }
    /// Atoms carried.
    public var atomCount: Int { isGlyph ? PetalKana.glyphAtomCount : PetalKana.polarityAtomCount }
    /// The byte that names the lane in its check: `0x10 | band << 1 | kind`
    /// (polarity 0, glyph 1).
    public var code: UInt8 { UInt8(0x10 | band << 1 | (isGlyph ? 1 : 0)) }
    /// The fixed whitening sequence of the lane: one byte per codeword byte
    /// from a ``PetalXorshift32`` seeded "POL0", "POL1" (polarity) or "KAN0",
    /// "KAN1" (glyphs).
    public var whitening: [UInt8] { Self.whitenings[rawValue] }

    var codec: PetalReedSolomon { isGlyph ? Self.glyphCode : Self.polarityCode }

    private static let whitenings: [[UInt8]] = allCases.map { lane in
        var rng = PetalXorshift32(seed: (lane.isGlyph ? 0x4B41_4E30 : 0x504F_4C30) &+ UInt32(lane.band))
        return (0..<lane.wordLength).map { _ in rng.nextByte() }
    }
    private static let polarityCode = PetalReedSolomon(validatedParityLength: PetalKana.polarityParityLength)
    private static let glyphCode = PetalReedSolomon(validatedParityLength: PetalKana.glyphParityLength)
}

/// A kana lane read from a capture, with the header (stream tag and frame) its
/// check matched.
public struct PetalKanaResult: Equatable, Sendable {
    /// The lane data (atoms and check).
    public let result: PetalLaneResult
    /// The header the check matched: the square's frame or a neighbouring one.
    public let header: PetalLaneHeader
}

/// What a reader decided for one data tile (see
/// ``PetalKana/observeBand(_:frame:band:)``).
public struct PetalKanaTileReading: Equatable, Sendable {
    /// Index into ``PetalLayout/kanaTiles``.
    public let tile: Int
    /// Decided polarity.
    public let light: Bool
    /// How far the tile's mean level lay from the threshold, in half-contrasts.
    public let lightMargin: Double
    /// Decided glyph (`0..<16`).
    public let glyph: Int
    /// How far the glyph's correlation led the runner-up's.
    public let glyphLead: Double
}

/// What a reader saw of one band (see ``PetalKana/observeBand(_:frame:band:)``).
public struct PetalKanaBandReading: Equatable, Sendable {
    /// The blur (σ in canvas units) the pilots chose.
    public let blur: Double
    /// Pilots found.
    public let pilots: Int
    /// Every data tile of the band in data order, or `nil` for tiles it could
    /// not read.
    public let tiles: [PetalKanaTileReading?]
}

/// The kana layer of Petal Stream: data in the katakana tiles of the bands
/// above and below the square (port of `crates/iroha_petal/src/kana.rs`).
///
/// The card (``PetalLayout/cardHeight``) is the square code with a band of
/// katakana tiles above and below it, in the part of an upright phone's camera
/// frame that the square leaves unused. Each band holds
/// ``PetalLayout/kanaBandTiles`` tiles (``PetalLayout/kanaTiles``); eight are
/// pilots with fixed symbols (``PetalLayout/kanaPilotSymbols``) and the other
/// ``dataTiles`` carry two lanes:
///
/// | lane | cells | codeword | data | parity |
/// |------|-------|----------|------|--------|
/// | `P0`, `P1` polarity | 296 data tiles × 1 bit (light or dark) | 37 B | 20 B | 17 B |
/// | `K0`, `K1` glyph | 298 data tiles × 4 bits (one of sixteen katakana) | 149 B | 116 B | 33 B |
///
/// A light tile is filled light with its glyph in ink; a dark tile is black
/// with its glyph in pink. Data tile `i` of a band (its tiles that are not
/// pilots, in reading order) shows bit `i` of the band's polarity codeword
/// (most significant bit first; the last two data tiles, beyond the codeword,
/// are light) and nibble `i` of its glyph codeword (the high nibble of byte
/// `i / 2` for even `i`).
///
/// Every kana lane carries fountain atoms from its own repair range
/// (``idBase``, see ``kanaFirstID(frame:lane:)``) and a CRC-32C check that
/// binds them to the stream tag, the frame and the lane
/// (``kanaCheck(_:lane:atoms:)``). The lanes have no header: a reader takes the
/// frame from the square's lanes and also tries the neighbouring frames,
/// because a rolling shutter or a stale frame can show a band from the frame
/// before or after the square.
///
/// ``readKana(_:frame:header:skip:)`` reads the bands of a decoded capture. The
/// square's pose, extended beyond the square, places every tile; each band's
/// pilots are found by a small search around their places, and an affine
/// correction fitted to their offsets moves every tile of the band. The pilots
/// also pick the blur of the glyph templates and the levels that tell light
/// tiles from dark ones. Each data tile then takes the polarity its mean level
/// shows and the glyph whose template correlates best with its samples. The
/// reader's templates are built once, on first use; every function is
/// thread-safe.
public enum PetalKana {
    /// Data tiles of each band.
    public static let dataTiles = PetalLayout.kanaBandTiles - PetalLayout.kanaPilots
    /// Codeword length of a glyph lane in bytes (two data tiles per byte).
    public static let glyphWordLength = dataTiles / 2
    /// Parity bytes of a glyph lane.
    public static let glyphParityLength = 33
    /// Atoms carried by each glyph lane.
    public static let glyphAtomCount = 7
    /// Codeword length of a polarity lane in bytes (eight data tiles per byte).
    public static let polarityWordLength = dataTiles / 8
    /// Parity bytes of a polarity lane.
    public static let polarityParityLength = 17
    /// Atoms carried by each polarity lane.
    public static let polarityAtomCount = 1
    /// Bytes of the check that closes a kana lane's data.
    public static let checkLength = 4
    /// Fountain id of the first kana atom of frame 0, `2³¹ + 2²⁰`: just above
    /// the turn lanes' range.
    public static let idBase: UInt32 = PetalTurns.idBase + (1 << 20)
    /// Kana atom ids reserved per frame (eight per band).
    static let idsPerFrame: UInt32 = 16
    /// Gaussian blurs (σ in canvas units) of the glyph templates; the pilots
    /// pick one per band.
    public static let blurs: [Double] = [1.0, 1.6, 2.4, 3.4, 4.8]
    /// Samples per tile side: points `(i + ½) · 25 / 12` units from the tile's
    /// top-left corner.
    public static let grid = 12
    /// Samples per tile side in the coarse pilot search: points `(i + ½) · 25 / 6`.
    public static let coarseGrid = 6

    /// Fountain id of the first atom `lane` carries in `frame`:
    /// `idBase + 16 · frame + 8 · band`, plus 7 for the polarity lane (the
    /// glyph lane's seven atoms come first).
    public static func kanaFirstID(frame: UInt16, lane: PetalKanaLane) -> UInt32 {
        let slot: UInt32 = lane.isGlyph ? 0 : UInt32(glyphAtomCount)
        return idBase &+ UInt32(frame) &* idsPerFrame &+ UInt32(lane.band) &* 8 &+ slot
    }

    /// The check of a kana lane: CRC-32C over the stream tag, the frame
    /// (big-endian), the lane's ``PetalKanaLane/code`` and the atoms.
    public static func kanaCheck(_ header: PetalLaneHeader, lane: PetalKanaLane, atoms: [UInt8]) -> UInt32 {
        checkOf(header, lane: lane, atoms: atoms[...])
    }

    private static func checkOf(_ header: PetalLaneHeader, lane: PetalKanaLane, atoms: ArraySlice<UInt8>) -> UInt32 {
        var bytes: [UInt8] = [
            header.tag,
            UInt8(truncatingIfNeeded: header.frame >> 8),
            UInt8(truncatingIfNeeded: header.frame),
            lane.code,
        ]
        bytes.append(contentsOf: atoms)
        return IrohaPeerCRC32CV1.checksum(bytes)
    }

    /// The data of a kana lane: its ``PetalKanaLane/atomCount`` atoms followed
    /// by their ``kanaCheck(_:lane:atoms:)`` (big-endian).
    ///
    /// - Throws: ``PetalCodecError/invalidShape`` unless `atoms` holds exactly
    ///   the lane's atoms.
    public static func kanaData(_ header: PetalLaneHeader, lane: PetalKanaLane, atoms: [UInt8]) throws -> [UInt8] {
        guard atoms.count == lane.atomCount * PetalLane.atomLength else { throw PetalCodecError.invalidShape }
        return kanaDataValidated(header, lane: lane, atoms: atoms)
    }

    static func kanaDataValidated(_ header: PetalLaneHeader, lane: PetalKanaLane, atoms: [UInt8]) -> [UInt8] {
        let check = kanaCheck(header, lane: lane, atoms: atoms)
        var data = atoms
        data.reserveCapacity(lane.dataLength)
        data.append(UInt8(truncatingIfNeeded: check >> 24))
        data.append(UInt8(truncatingIfNeeded: check >> 16))
        data.append(UInt8(truncatingIfNeeded: check >> 8))
        data.append(UInt8(truncatingIfNeeded: check))
        return data
    }

    /// The atoms of kana lane `data` whose check matches `header` and `lane`;
    /// `nil` for a length mismatch or a wrong check.
    public static func kanaPacket(_ header: PetalLaneHeader, lane: PetalKanaLane, data: [UInt8]) -> PetalAtomPacket? {
        guard data.count == lane.dataLength, checkMatches(header, lane: lane, data: data) else { return nil }
        let atomLength = PetalLane.atomLength
        return PetalAtomPacket(
            header: header,
            firstID: kanaFirstID(frame: header.frame, lane: lane),
            atoms: (0..<lane.atomCount).map { index in
                Array(data[(index * atomLength)..<((index + 1) * atomLength)])
            }
        )
    }

    /// Whether the check closing kana lane `data` matches `header` and `lane`.
    private static func checkMatches(_ header: PetalLaneHeader, lane: PetalKanaLane, data: [UInt8]) -> Bool {
        let split = lane.atomCount * PetalLane.atomLength
        let check = checkOf(header, lane: lane, atoms: data[..<split])
        return data[split] == UInt8(truncatingIfNeeded: check >> 24)
            && data[split + 1] == UInt8(truncatingIfNeeded: check >> 16)
            && data[split + 2] == UInt8(truncatingIfNeeded: check >> 8)
            && data[split + 3] == UInt8(truncatingIfNeeded: check)
    }

    /// Encodes kana lane `data` into the transmitted (whitened) codeword.
    ///
    /// - Throws: ``PetalCodecError/invalidShape`` unless `data` is exactly
    ///   ``PetalKanaLane/dataLength`` bytes.
    public static func encodeKanaLane(_ lane: PetalKanaLane, data: [UInt8]) throws -> [UInt8] {
        guard data.count == lane.dataLength else { throw PetalCodecError.invalidShape }
        return encodeValidated(lane, data: data)
    }

    static func encodeValidated(_ lane: PetalKanaLane, data: [UInt8]) -> [UInt8] {
        var word = lane.codec.encodeValidated(data)
        let mask = lane.whitening
        for index in word.indices { word[index] ^= mask[index] }
        return word
    }

    /// Decodes a transmitted kana codeword, returning its data and how many
    /// byte positions the Reed–Solomon decoder rewrote. `erasures` lists byte
    /// positions the caller distrusts. The check is not verified here (see
    /// ``kanaPacket(_:lane:data:)``).
    ///
    /// - Throws: ``PetalCodecError`` when the word has the wrong length or is
    ///   uncorrectable.
    public static func decodeKanaLaneCounted(
        _ lane: PetalKanaLane,
        _ transmitted: [UInt8],
        erasures: [Int] = []
    ) throws -> (data: [UInt8], corrected: Int) {
        guard transmitted.count == lane.wordLength else { throw PetalCodecError.invalidShape }
        let mask = lane.whitening
        var word = transmitted
        for index in word.indices { word[index] ^= mask[index] }
        let corrected = try lane.codec.decode(&word, erasures: erasures)
        return (Array(word.prefix(lane.dataLength)), corrected)
    }

    // MARK: - Tile symbols

    /// The glyph and polarity of every tile with only the pilots set: the
    /// pilots show their fixed symbols, data tiles are light with glyph 0.
    static let pilotSymbols: (glyphs: [UInt8], light: [Bool]) = {
        var glyphs = [UInt8](repeating: 0, count: PetalLayout.kanaTileCount)
        var light = [Bool](repeating: true, count: PetalLayout.kanaTileCount)
        for (index, tile) in PetalLayout.kanaTiles.enumerated() {
            guard let pilot = tile.pilot else { continue }
            glyphs[index] = UInt8(PetalLayout.kanaPilotSymbols[pilot].glyph)
            light[index] = PetalLayout.kanaPilotSymbols[pilot].light
        }
        return (glyphs, light)
    }()

    /// Shows the transmitted codewords of `band` on its data tiles: `polarity`
    /// (``polarityWordLength`` bytes) and `glyphs` (``glyphWordLength`` bytes).
    static func setSymbols(
        glyphs: inout [UInt8],
        light: inout [Bool],
        band: Int,
        polarity: [UInt8],
        glyphWord: [UInt8]
    ) {
        for (i, tile) in PetalLayout.kanaDataTable[band].enumerated() {
            light[tile] = i < polarityWordLength * 8 ? (polarity[i / 8] >> (7 - i % 8)) & 1 == 1 : true
            let byte = glyphWord[i / 2]
            glyphs[tile] = i % 2 == 0 ? byte >> 4 : byte & 0x0F
        }
    }

    // MARK: - Reading

    /// The template value of `glyph` (a light or dark tile) under blur `blur`
    /// (an index into ``blurs``) at grid point `(i, j)`, mean-centred and
    /// scaled to unit length: what conformance tests compare an SDK's tables
    /// with. Zero for an argument out of range.
    public static func templateProbe(blur: Int, glyph: Int, light: Bool, i: Int, j: Int) -> Double {
        guard blurs.indices.contains(blur), (0..<PetalGlyphs.glyphCount).contains(glyph),
              (0..<grid).contains(i), (0..<grid).contains(j) else { return 0 }
        let tables = Tables.shared
        return (light ? tables.light : tables.dark)[Tables.offset(blur, glyph) + j * grid + i]
    }

    /// The coarse template value of `glyph` (a light or dark tile) at coarse
    /// grid point `(i, j)`: what conformance tests compare an SDK's tables
    /// with. Zero for an argument out of range.
    public static func coarseTemplateProbe(glyph: Int, light: Bool, i: Int, j: Int) -> Double {
        guard (0..<PetalGlyphs.glyphCount).contains(glyph),
              (0..<coarseGrid).contains(i), (0..<coarseGrid).contains(j) else { return 0 }
        let tables = Tables.shared
        return (light ? tables.coarseLight : tables.coarseDark)[glyph * Tables.coarsePoints + j * coarseGrid + i]
    }

    /// Reads the kana lanes of the bands not flagged in `skip` (two flags,
    /// band 0 first) of a capture whose square `frame` decoded under `header`
    /// (the stream tag and frame its lanes name). A lane passes when its
    /// Reed–Solomon codeword decodes, with 0, ⅛, ¼, ⅓ or ½ of its parity bytes'
    /// worth of its least confident bytes erased, into data whose
    /// ``kanaCheck(_:lane:atoms:)`` matches `header`'s frame, the frame after
    /// it or the frame before it. Returns four entries in
    /// ``PetalKanaLane/allCases`` order, `nil` where no lane was read.
    public static func readKana(
        _ image: PetalLuma,
        frame: PetalDecodedFrame,
        header: PetalLaneHeader,
        skip: [Bool] = [false, false]
    ) -> [PetalKanaResult?] {
        var results = [PetalKanaResult?](repeating: nil, count: PetalKanaLane.allCases.count)
        Reader.lease { reader in
            image.withView { view in
                for band in 0..<PetalLayout.kanaBands where !(band < skip.count && skip[band]) {
                    guard reader.observe(view, pose: frame.pose, band: band) else { continue }
                    var polarity = [UInt8](repeating: 0, count: polarityWordLength)
                    var polarityConfidence = [Double](repeating: .greatestFiniteMagnitude, count: polarityWordLength)
                    var glyphs = [UInt8](repeating: 0, count: glyphWordLength)
                    var glyphConfidence = [Double](repeating: .greatestFiniteMagnitude, count: glyphWordLength)
                    for i in 0..<dataTiles {
                        let read = reader.tileRead[i]
                        let light = read && reader.tileLight[i]
                        let margin = read ? reader.tileMargin[i] : 0.0
                        let glyph = read ? reader.tileGlyph[i] : 0
                        let lead = read ? reader.tileLead[i] : 0.0
                        if i < polarityWordLength * 8 {
                            if light { polarity[i / 8] |= 1 << (7 - i % 8) }
                            polarityConfidence[i / 8] = PetalNumeric.minNum(polarityConfidence[i / 8], margin)
                        }
                        glyphs[i / 2] |= i % 2 == 0 ? glyph << 4 : glyph
                        glyphConfidence[i / 2] = PetalNumeric.minNum(glyphConfidence[i / 2], lead)
                    }
                    let polarityLane = PetalKanaLane(band: band, glyph: false)!
                    let glyphLane = PetalKanaLane(band: band, glyph: true)!
                    results[polarityLane.rawValue] = decodeKanaWord(
                        polarityLane, header: header, word: polarity, confidence: polarityConfidence
                    )
                    results[glyphLane.rawValue] = decodeKanaWord(
                        glyphLane, header: header, word: glyphs, confidence: glyphConfidence
                    )
                }
            }
        }
        return results
    }

    /// What ``readKana(_:frame:header:skip:)`` sees of band `band` (`0..<2`)
    /// before any Reed–Solomon correction, for diagnostics; `nil` when fewer
    /// than two pilots are found, the pilots show no contrast between light and
    /// dark tiles, or the band is out of range.
    public static func observeBand(_ image: PetalLuma, frame: PetalDecodedFrame, band: Int) -> PetalKanaBandReading? {
        guard (0..<PetalLayout.kanaBands).contains(band) else { return nil }
        var reading: PetalKanaBandReading?
        Reader.lease { reader in
            image.withView { view in
                guard reader.observe(view, pose: frame.pose, band: band) else { return }
                let data = PetalLayout.kanaDataTable[band]
                reading = PetalKanaBandReading(
                    blur: blurs[reader.blur],
                    pilots: reader.foundCount,
                    tiles: (0..<dataTiles).map { i in
                        guard reader.tileRead[i] else { return nil }
                        return PetalKanaTileReading(
                            tile: data[i],
                            light: reader.tileLight[i],
                            lightMargin: reader.tileMargin[i],
                            glyph: Int(reader.tileGlyph[i]),
                            glyphLead: reader.tileLead[i]
                        )
                    }
                )
            }
        }
        return reading
    }

    /// Tries Reed–Solomon with 0, n/8, n/4, n/3 and n/2 of the least confident
    /// bytes of `word` erased (n the parity bytes; a stable ranking: ties keep
    /// byte order) and returns the first result whose check matches `header`'s
    /// frame, the frame after it or the frame before it.
    static func decodeKanaWord(
        _ lane: PetalKanaLane,
        header: PetalLaneHeader,
        word: [UInt8],
        confidence: [Double]
    ) -> PetalKanaResult? {
        let keys = confidence.map(PetalNumeric.totalOrderKey)
        // Stable ascending sort under total order (Rust `sort_by`).
        let order = word.indices.sorted { a, b in
            keys[a] != keys[b] ? keys[a] < keys[b] : a < b
        }
        let nsym = lane.parityLength
        // Rust `Vec::dedup`: drop consecutive repeats.
        var schedule: [Int] = []
        for erasures in [0, nsym / 8, nsym / 4, nsym / 3, nsym / 2] where schedule.last != erasures {
            schedule.append(erasures)
        }
        for erasures in schedule {
            let positions = Array(order.prefix(erasures))
            var trial = word
            // zero the erased bytes so stale values cannot leak through
            for position in positions { trial[position] = 0 }
            guard let (data, corrected) = try? decodeKanaLaneCounted(lane, trial, erasures: positions) else {
                continue
            }
            // the square's frame, the frame after it, the frame before it
            for frame in [header.frame, header.frame &+ 1, header.frame &- 1] {
                let candidate = PetalLaneHeader(tag: header.tag, frame: frame)
                if checkMatches(candidate, lane: lane, data: data) {
                    return PetalKanaResult(
                        result: PetalLaneResult(data: data, corrected: corrected, erasures: positions.count),
                        header: candidate
                    )
                }
            }
        }
        return nil
    }

    /// The kana reader's templates, built once on first use and kept for the
    /// life of the process (about 0.2 MB): for every blur of ``blurs``, the
    /// coverage rasters of the tile's inside and of every glyph's ink, blurred
    /// and sampled at the ``grid`` points and mean-centred; a light tile's
    /// template is the inside less the ink, a dark tile's the ink, each also
    /// scaled to unit length. The coarse templates are the same under blur 3.4
    /// at the ``coarseGrid`` points.
    final class Tables: @unchecked Sendable {
        static let shared = Tables()
        /// Samples of a tile.
        static let points = PetalKana.grid * PetalKana.grid
        /// Samples of a tile in the coarse pilot search.
        static let coarsePoints = PetalKana.coarseGrid * PetalKana.coarseGrid
        /// The blur of the coarse templates (3.4 units).
        static let coarseBlur = 3
        /// Template raster step in canvas units.
        static let rasterStep = 0.5
        /// The raster starts this far outside the tile square, so blur reaches
        /// in from the gutter.
        static let rasterMargin = 4.0
        /// Raster side in steps.
        static let rasterSide = 67

        /// Tile-local coordinate of every grid index on either axis:
        /// `(i + ½) · 25 / 12` …
        let grid: UnsafeMutablePointer<Double>
        /// … and of the coarse grid: `(i + ½) · 25 / 6`.
        let coarseGrid: UnsafeMutablePointer<Double>
        /// Per blur (at `blur · points`): the tile's inside at the grid points,
        /// mean-centred.
        let inside: UnsafeMutablePointer<Double>
        /// Per blur and glyph (at ``offset(_:_:)``): the ink at the grid
        /// points, mean-centred.
        let ink: UnsafeMutablePointer<Double>
        /// Per blur and glyph (`blur · 16 + glyph`): the lengths of the light
        /// (inside less ink) and dark (ink) templates.
        let lightNorm: UnsafeMutablePointer<Double>
        let darkNorm: UnsafeMutablePointer<Double>
        /// Per blur and glyph (at ``offset(_:_:)``): the light and dark
        /// templates scaled to unit length.
        let light: UnsafeMutablePointer<Double>
        let dark: UnsafeMutablePointer<Double>
        /// Per glyph (at `glyph · 36`): the coarse light and dark templates,
        /// unit length.
        let coarseLight: UnsafeMutablePointer<Double>
        let coarseDark: UnsafeMutablePointer<Double>

        /// Offset of the templates of `glyph` under blur `blur` in
        /// ``ink``, ``light`` and ``dark``.
        static func offset(_ blur: Int, _ glyph: Int) -> Int {
            (blur * PetalGlyphs.glyphCount + glyph) * points
        }

        private static func table(_ count: Int) -> UnsafeMutablePointer<Double> {
            let table = UnsafeMutablePointer<Double>.allocate(capacity: count)
            table.initialize(repeating: 0, count: count)
            return table
        }

        private init() {
            let glyphs = PetalGlyphs.glyphCount
            let points = Self.points
            let coarsePoints = Self.coarsePoints
            grid = Self.table(PetalKana.grid)
            coarseGrid = Self.table(PetalKana.coarseGrid)
            inside = Self.table(blurs.count * points)
            ink = Self.table(blurs.count * glyphs * points)
            lightNorm = Self.table(blurs.count * glyphs)
            darkNorm = Self.table(blurs.count * glyphs)
            light = Self.table(blurs.count * glyphs * points)
            dark = Self.table(blurs.count * glyphs * points)
            coarseLight = Self.table(glyphs * coarsePoints)
            coarseDark = Self.table(glyphs * coarsePoints)
            for i in 0..<PetalKana.grid { grid[i] = Self.gridPoint(i, PetalKana.grid) }
            for i in 0..<PetalKana.coarseGrid { coarseGrid[i] = Self.gridPoint(i, PetalKana.coarseGrid) }
            let insideRaster = Self.raster(nil)
            let inkRasters = (0..<glyphs).map { Self.raster($0) }
            let cells = Self.rasterSide * Self.rasterSide
            var pass = [Double](repeating: 0, count: cells)
            var insideBlurred = [Double](repeating: 0, count: cells)
            var inkBlurred = [Double](repeating: 0, count: cells)
            var lightValues = [Double](repeating: 0, count: points)
            var coarseInside = [Double](repeating: 0, count: coarsePoints)
            var coarseInk = [Double](repeating: 0, count: coarsePoints)
            var coarseLightValues = [Double](repeating: 0, count: coarsePoints)
            var fine = [Double](repeating: 0, count: points)
            for blur in blurs.indices {
                let sigma = blurs[blur]
                Self.blurRaster(insideRaster, sigma: sigma, pass: &pass, out: &insideBlurred)
                Self.sampleRaster(insideBlurred, side: PetalKana.grid, out: &fine)
                for p in 0..<points { inside[blur * points + p] = fine[p] }
                for glyph in 0..<glyphs {
                    Self.blurRaster(inkRasters[glyph], sigma: sigma, pass: &pass, out: &inkBlurred)
                    let at = Self.offset(blur, glyph)
                    Self.sampleRaster(inkBlurred, side: PetalKana.grid, out: &fine)
                    for p in 0..<points { ink[at + p] = fine[p] }
                    for p in 0..<points { lightValues[p] = inside[blur * points + p] - ink[at + p] }
                    let lightLength = Self.norm(lightValues)
                    let darkLength = Self.norm(fine)
                    lightNorm[blur * glyphs + glyph] = lightLength
                    darkNorm[blur * glyphs + glyph] = darkLength
                    for p in 0..<points {
                        light[at + p] = lightValues[p] / lightLength
                        dark[at + p] = ink[at + p] / darkLength
                    }
                    if blur == Self.coarseBlur {
                        Self.sampleRaster(insideBlurred, side: PetalKana.coarseGrid, out: &coarseInside)
                        Self.sampleRaster(inkBlurred, side: PetalKana.coarseGrid, out: &coarseInk)
                        for p in 0..<coarsePoints { coarseLightValues[p] = coarseInside[p] - coarseInk[p] }
                        let coarseLightLength = Self.norm(coarseLightValues)
                        let coarseDarkLength = Self.norm(coarseInk)
                        for p in 0..<coarsePoints {
                            coarseLight[glyph * coarsePoints + p] = coarseLightValues[p] / coarseLightLength
                            coarseDark[glyph * coarsePoints + p] = coarseInk[p] / coarseDarkLength
                        }
                    }
                }
            }
        }

        /// Tile-local coordinate of grid index `i` on either axis of a grid of
        /// `side` points.
        static func gridPoint(_ i: Int, _ side: Int) -> Double {
            (Double(i) + 0.5) * PetalLayout.kanaTile / Double(side)
        }

        /// Coverage on the template raster of the tile's inside (`glyph` nil)
        /// or of `glyph`'s ink: raster point `a` is at `−4 + a / 2` units; each
        /// value is the share of its 4 × 4 sub-points `(m − 1.5) / 8` units
        /// around it that are covered.
        static func raster(_ glyph: Int?) -> [Double] {
            let side = rasterSide
            var table = [Double](repeating: 0, count: side * side)
            for b in 0..<side {
                for a in 0..<side {
                    let u0 = -rasterMargin + Double(a) * rasterStep
                    let v0 = -rasterMargin + Double(b) * rasterStep
                    var hits = 0
                    for n in 0..<4 {
                        for m in 0..<4 {
                            let u = u0 + (Double(m) - 1.5) / 8.0
                            let v = v0 + (Double(n) - 1.5) / 8.0
                            let covered: Bool
                            if let glyph {
                                covered = PetalLayout.inked(glyph, u, v)
                            } else {
                                covered = PetalLayout.kanaInside(u: u, v: v)
                            }
                            if covered { hits += 1 }
                        }
                    }
                    table[b * side + a] = Double(hits) / 16.0
                }
            }
            return table
        }

        /// Separable Gaussian blur of a raster into `out` (`pass` is scratch):
        /// taps every ``rasterStep`` out to `⌈3σ / step⌉`, weights
        /// `exp(−d² / (2σ²))` normalised by their sum, points outside counting
        /// as zero; rows first.
        static func blurRaster(_ table: [Double], sigma: Double, pass: inout [Double], out: inout [Double]) {
            let reach = Int((3.0 * sigma / rasterStep).rounded(.up))
            var weights = (-reach...reach).map { k -> Double in
                let d = Double(k) * rasterStep
                return exp(-(d * d) / (2.0 * sigma * sigma))
            }
            var sum = 0.0
            for weight in weights { sum += weight }
            for index in weights.indices { weights[index] = weights[index] / sum }
            blurPass(table, &pass, weights, reach, alongRows: true)
            blurPass(pass, &out, weights, reach, alongRows: false)
        }

        private static func blurPass(
            _ source: [Double], _ out: inout [Double], _ weights: [Double], _ reach: Int, alongRows: Bool
        ) {
            let side = rasterSide
            for b in 0..<side {
                for a in 0..<side {
                    var acc = 0.0
                    for k in -reach...reach {
                        let aa = alongRows ? a + k : a
                        let bb = alongRows ? b : b + k
                        if aa < 0 || bb < 0 || aa >= side || bb >= side { continue }
                        acc += weights[k + reach] * source[bb * side + aa]
                    }
                    out[b * side + a] = acc
                }
            }
        }

        /// A blurred raster at the `side` × `side` grid points (bilinear) into
        /// `out`, mean-centred.
        static func sampleRaster(_ table: [Double], side: Int, out: inout [Double]) {
            let rasterSide = Self.rasterSide
            for j in 0..<side {
                for i in 0..<side {
                    let fa = (gridPoint(i, side) + rasterMargin) / rasterStep
                    let fb = (gridPoint(j, side) + rasterMargin) / rasterStep
                    let a0 = Int(fa.rounded(.down))
                    let b0 = Int(fb.rounded(.down))
                    let ta = fa - Double(a0)
                    let tb = fb - Double(b0)
                    let top = table[b0 * rasterSide + a0] * (1.0 - ta) + table[b0 * rasterSide + a0 + 1] * ta
                    let bottom = table[(b0 + 1) * rasterSide + a0] * (1.0 - ta)
                        + table[(b0 + 1) * rasterSide + a0 + 1] * ta
                    out[j * side + i] = top * (1.0 - tb) + bottom * tb
                }
            }
            let n = side * side
            var sum = 0.0
            for p in 0..<n { sum += out[p] }
            let mean = sum / Double(n)
            for p in 0..<n { out[p] = out[p] - mean }
        }

        /// The length of `values`, at least `1e-12`.
        static func norm(_ values: [Double]) -> Double {
            var sum = 0.0
            for value in values { sum += value * value }
            return PetalNumeric.maxNum(sum.squareRoot(), 1e-12)
        }
    }

    /// The band reader with its scratch buffers, reused between captures (one
    /// at a time: see ``lease(_:)``).
    final class Reader {
        private static let pilots = PetalLayout.kanaPilots
        /// Step of the coarse pilot search in canvas units.
        private static let coarseStep = 2.0
        /// A pilot whose coarse correlation is below this is not followed further.
        private static let pilotCoarseMin = 0.35
        /// A pilot whose final correlation is below this is not used.
        private static let pilotMin = 0.5
        private static let refineCoarse: [Double] = [1.0]
        private static let refineFine: [Double] = [1.0, 0.5, 0.25]

        private let tables = Tables.shared
        /// The samples of the last ``sampleTile(_:_:_:_:_:_:)``, minus their
        /// mean, with their count, mean and energy (sum of squares).
        private let samples = UnsafeMutablePointer<Double>.allocate(capacity: Tables.points)
        private var count = 0
        private var mean = 0.0
        private var energy = 0.0
        private var view: PetalLumaView?
        private var pose = PetalPose.plain(.identity)

        // the best shift of the last search and its correlation
        private var bestX = 0.0
        private var bestY = 0.0
        private var bestScore = 0.0

        // pilots kept after the coarse search, then found
        private var coarseTile = [Int](repeating: 0, count: Reader.pilots)
        private var coarseX = [Double](repeating: 0, count: Reader.pilots)
        private var coarseY = [Double](repeating: 0, count: Reader.pilots)
        private var coarseCount = 0
        private var foundCentreX = [Double](repeating: 0, count: Reader.pilots)
        private var foundCentreY = [Double](repeating: 0, count: Reader.pilots)
        private var foundShiftX = [Double](repeating: 0, count: Reader.pilots)
        private var foundShiftY = [Double](repeating: 0, count: Reader.pilots)
        private var foundLevel = [Double](repeating: 0, count: Reader.pilots)
        private var foundLight = [Bool](repeating: false, count: Reader.pilots)
        private(set) var foundCount = 0

        // the shift field (per axis: constant, x and y terms) and level lines
        private var fieldX = [0.0, 0.0, 0.0]
        private var fieldY = [0.0, 0.0, 0.0]
        private var fieldCentre = 0.0
        private var lightIntercept = 0.0
        private var lightSlope = 0.0
        private var darkIntercept = 0.0
        private var darkSlope = 0.0

        // the band's readings, in data order
        private(set) var blur = 0
        private(set) var tileRead = [Bool](repeating: false, count: PetalKana.dataTiles)
        private(set) var tileLight = [Bool](repeating: false, count: PetalKana.dataTiles)
        private(set) var tileMargin = [Double](repeating: 0, count: PetalKana.dataTiles)
        private(set) var tileGlyph = [UInt8](repeating: 0, count: PetalKana.dataTiles)
        private(set) var tileLead = [Double](repeating: 0, count: PetalKana.dataTiles)
        private var scores = [Double](repeating: 0, count: PetalGlyphs.glyphCount)

        init() {
            samples.initialize(repeating: 0, count: Tables.points)
        }

        deinit {
            samples.deallocate()
        }

        private static let lock = NSLock()
        private static var idle: [Reader] = []

        /// Runs `body` with a reader no other thread uses: an idle one, or a
        /// new one when every reader is busy. At most two are kept.
        static func lease(_ body: (Reader) -> Void) {
            lock.lock()
            let reader = idle.popLast() ?? Reader()
            lock.unlock()
            body(reader)
            reader.view = nil
            lock.lock()
            if idle.count < 2 { idle.append(reader) }
            lock.unlock()
        }

        /// Reads band `band`'s data tiles into the tile arrays; `false` when
        /// fewer than two pilots are found or the pilots show no contrast
        /// between light and dark tiles.
        func observe(_ view: PetalLumaView, pose: PetalPose, band: Int) -> Bool {
            if view.width <= 0 || view.height <= 0 { return false }
            self.view = view
            self.pose = pose
            let half = PetalLayout.kanaTile / 2.0
            // coarse search of every pilot in view
            coarseCount = 0
            for tile in PetalLayout.kanaPilotTable[band] where tileInImage(tile) {
                coarsePilot(tile)
                if bestScore >= Self.pilotCoarseMin {
                    coarseTile[coarseCount] = tile
                    coarseX[coarseCount] = bestX
                    coarseY[coarseCount] = bestY
                    coarseCount += 1
                }
            }
            if coarseCount < 2 { return false }
            // the blur under which the pilots, each at its best shift within a
            // unit, correlate best
            blur = 0
            var best = -Double.greatestFiniteMagnitude
            for index in blurs.indices {
                var sum = 0.0
                for c in 0..<coarseCount {
                    refinePilot(coarseTile[c], index, coarseX[c], coarseY[c], Self.refineCoarse)
                    sum += bestScore
                }
                if sum > best {
                    best = sum
                    blur = index
                }
            }
            foundCount = 0
            for c in 0..<coarseCount {
                let tile = coarseTile[c]
                refinePilot(tile, blur, coarseX[c], coarseY[c], Self.refineFine)
                if bestScore < Self.pilotMin { continue }
                let shiftX = bestX
                let shiftY = bestY
                sampleTile(PetalLayout.kanaTileX[tile], PetalLayout.kanaTileY[tile], shiftX, shiftY, tables.grid, PetalKana.grid)
                foundCentreX[foundCount] = PetalLayout.kanaTileX[tile] + half
                foundCentreY[foundCount] = PetalLayout.kanaTileY[tile] + half
                foundShiftX[foundCount] = shiftX
                foundShiftY[foundCount] = shiftY
                foundLevel[foundCount] = mean
                foundLight[foundCount] = PetalLayout.kanaPilotSymbols[PetalLayout.kanaTilePilot[tile]].light
                foundCount += 1
            }
            if foundCount < 2 { return false }
            fitShiftField()
            guard let lightLine = levelLine(light: true) else { return false }
            (lightIntercept, lightSlope) = lightLine
            guard let darkLine = levelLine(light: false) else { return false }
            (darkIntercept, darkSlope) = darkLine
            let points = Tables.points
            let data = PetalLayout.kanaDataTable[band]
            for i in 0..<PetalKana.dataTiles {
                let tile = data[i]
                tileRead[i] = false
                if !tileInImage(tile) { continue }
                let x0 = PetalLayout.kanaTileX[tile]
                let y0 = PetalLayout.kanaTileY[tile]
                let centreX = x0 + half
                let centreY = y0 + half
                let (shiftX, shiftY) = shiftAt(centreX, centreY)
                sampleTile(x0, y0, shiftX, shiftY, tables.grid, PetalKana.grid)
                let x = (centreX - PetalLayout.canvas / 2.0) / 512.0
                let high = lightIntercept + lightSlope * x
                let low = darkIntercept + darkSlope * x
                let halfContrast = (high - low) / 2.0
                if halfContrast <= 1.0 { continue }
                let threshold = (high + low) / 2.0
                // a light template is the inside less the ink, so one dot
                // product with the inside and one with each glyph's ink give
                // both polarities' correlations
                for g in 0..<PetalGlyphs.glyphCount { scores[g] = 0.0 }
                if energy > 1e-9 {
                    let root = energy.squareRoot()
                    let insideDot = dot(tables.inside + blur * points)
                    for g in 0..<PetalGlyphs.glyphCount {
                        let inkDot = dot(tables.ink + Tables.offset(blur, g))
                        let norms = blur * PetalGlyphs.glyphCount + g
                        let light = (insideDot - inkDot) / tables.lightNorm[norms] / root
                        let dark = inkDot / tables.darkNorm[norms] / root
                        scores[g] = PetalNumeric.maxNum(light, dark)
                    }
                }
                var first = 0
                for g in 1..<PetalGlyphs.glyphCount where scores[g] > scores[first] { first = g }
                var second = -Double.greatestFiniteMagnitude
                for g in 0..<PetalGlyphs.glyphCount where g != first {
                    second = PetalNumeric.maxNum(second, scores[g])
                }
                tileRead[i] = true
                tileLight[i] = mean > threshold
                tileMargin[i] = abs(mean - threshold) / halfContrast
                tileGlyph[i] = UInt8(first)
                tileLead[i] = scores[first] - second
            }
            return true
        }

        /// Coarse search of a pilot: shifts on a ``coarseStep`` grid within its
        /// reach, sampled on the coarse grid under the coarse template; leaves
        /// the best shift in ``bestX``, ``bestY`` and its correlation in
        /// ``bestScore``.
        private func coarsePilot(_ tile: Int) {
            let symbol = PetalLayout.kanaPilotSymbols[PetalLayout.kanaTilePilot[tile]]
            let template = (symbol.light ? tables.coarseLight : tables.coarseDark) + symbol.glyph * Tables.coarsePoints
            let x = PetalLayout.kanaTileX[tile]
            let y = PetalLayout.kanaTileY[tile]
            let steps = Int((pilotReach(y + PetalLayout.kanaTile / 2.0) / Self.coarseStep).rounded(.down))
            var bx = 0.0
            var by = 0.0
            var best = -Double.greatestFiniteMagnitude
            for dy in -steps...steps {
                for dx in -steps...steps {
                    let sx = Self.coarseStep * Double(dx)
                    let sy = Self.coarseStep * Double(dy)
                    sampleTile(x, y, sx, sy, tables.coarseGrid, PetalKana.coarseGrid)
                    let score = correlate(template)
                    if score > best {
                        bx = sx
                        by = sy
                        best = score
                    }
                }
            }
            bestX = bx
            bestY = by
            bestScore = best
        }

        /// Refines a pilot's shift under blur `blurIndex`: the best of the
        /// shifts one step around it, for each of `steps` in turn (ties keep
        /// the earlier); the result is left in ``bestX``, ``bestY`` and
        /// ``bestScore``.
        private func refinePilot(_ tile: Int, _ blurIndex: Int, _ startX: Double, _ startY: Double, _ steps: [Double]) {
            var bx = startX
            var by = startY
            var best = pilotScore(tile, blurIndex, startX, startY)
            for step in steps {
                let centreX = bx
                let centreY = by
                for dy in -1...1 {
                    for dx in -1...1 where dx != 0 || dy != 0 {
                        let sx = centreX + step * Double(dx)
                        let sy = centreY + step * Double(dy)
                        let score = pilotScore(tile, blurIndex, sx, sy)
                        if score > best {
                            bx = sx
                            by = sy
                            best = score
                        }
                    }
                }
            }
            bestX = bx
            bestY = by
            bestScore = best
        }

        /// The correlation of a pilot tile's samples, moved by `(sx, sy)`, with
        /// its template under blur `blurIndex`.
        private func pilotScore(_ tile: Int, _ blurIndex: Int, _ sx: Double, _ sy: Double) -> Double {
            let symbol = PetalLayout.kanaPilotSymbols[PetalLayout.kanaTilePilot[tile]]
            sampleTile(PetalLayout.kanaTileX[tile], PetalLayout.kanaTileY[tile], sx, sy, tables.grid, PetalKana.grid)
            return correlate((symbol.light ? tables.light : tables.dark) + Tables.offset(blurIndex, symbol.glyph))
        }

        /// Samples the tile whose top-left corner is `(x, y)` (square
        /// coordinates), moved by `(sx, sy)`, at the `side` × `side` points of
        /// `grid` into ``samples``: the pose maps the tile's centre `o` and the
        /// points one unit to its right and below it (`o + u`, `o + v`), and
        /// grid point `(a, b)` from the centre is sampled at `(o + b·v) + a·u`.
        /// The samples are left minus their mean; the mean goes to ``mean`` and
        /// their energy (the sum of their squares) to ``energy``.
        private func sampleTile(
            _ x: Double, _ y: Double, _ sx: Double, _ sy: Double, _ grid: UnsafeMutablePointer<Double>, _ side: Int
        ) {
            let view = self.view!
            let half = PetalLayout.kanaTile / 2.0
            let cx = x + half + sx
            let cy = y + half + sy
            let (ox, oy) = pose.apply(cx, cy)
            let (rx, ry) = pose.apply(cx + 1.0, cy)
            let (dx, dy) = pose.apply(cx, cy + 1.0)
            let ux = rx - ox
            let uy = ry - oy
            let vx = dx - ox
            let vy = dy - oy
            var sum = 0.0
            var index = 0
            for j in 0..<side {
                let b = grid[j] - half
                let rowX = ox + b * vx
                let rowY = oy + b * vy
                for i in 0..<side {
                    let a = grid[i] - half
                    let value = view.sample(rowX + a * ux, rowY + a * uy)
                    samples[index] = value
                    sum += value
                    index += 1
                }
            }
            let m = sum / Double(index)
            var e = 0.0
            for k in 0..<index {
                let centred = samples[k] - m
                samples[k] = centred
                e += centred * centred
            }
            count = index
            mean = m
            energy = e
        }

        /// The dot product of the last ``samples`` with `template`.
        private func dot(_ template: UnsafeMutablePointer<Double>) -> Double {
            var sum = 0.0
            for k in 0..<count { sum += samples[k] * template[k] }
            return sum
        }

        /// Correlation of the last ``samples`` (mean-centred, energy
        /// ``energy``) with the unit, mean-centred `template`.
        private func correlate(_ template: UnsafeMutablePointer<Double>) -> Double {
            energy <= 1e-9 ? 0.0 : dot(template) / energy.squareRoot()
        }

        /// Whether every corner of `tile` maps inside the image.
        private func tileInImage(_ tile: Int) -> Bool {
            let view = self.view!
            let x = PetalLayout.kanaTileX[tile]
            let y = PetalLayout.kanaTileY[tile]
            let side = PetalLayout.kanaTile
            let width = Double(view.width)
            let height = Double(view.height)
            for corner in 0..<4 {
                let u = corner % 2 == 1 ? side : 0.0
                let v = corner >= 2 ? side : 0.0
                let (px, py) = pose.apply(x + u, y + v)
                if !(px >= 0.0 && py >= 0.0 && px < width && py < height) { return false }
            }
            return true
        }

        /// How far (canvas units) the pose can miss a pilot centred `y` units
        /// down the square (negative above it): 4 units at the square's edge,
        /// growing by 10 over a band's 290-unit height.
        private func pilotReach(_ y: Double) -> Double {
            let beyond = y < 0.0 ? -y : y - PetalLayout.canvas
            return 4.0 + 10.0 * PetalNumeric.minNum(PetalNumeric.maxNum(beyond, 0.0) / 290.0, 1.2)
        }

        /// Fits the band's shift field to the found pilots: per axis an affine
        /// function of the tile centre (`x` scaled by 512 around the square's
        /// middle, `y` by 145 around the pilots' mean) by least squares, or
        /// their mean shift when fewer than three pilots fit one.
        private func fitShiftField() {
            let n = Double(foundCount)
            var sumY = 0.0
            for p in 0..<foundCount { sumY += foundCentreY[p] }
            fieldCentre = sumY / n
            var meanX = 0.0
            for p in 0..<foundCount { meanX += foundShiftX[p] }
            var meanY = 0.0
            for p in 0..<foundCount { meanY += foundShiftY[p] }
            fieldX = [meanX / n, 0.0, 0.0]
            fieldY = [meanY / n, 0.0, 0.0]
            if foundCount < 3 { return }
            var normal = [[Double]](repeating: [0.0, 0.0, 0.0], count: 3)
            var rightX = [0.0, 0.0, 0.0]
            var rightY = [0.0, 0.0, 0.0]
            for p in 0..<foundCount {
                let basis = [
                    1.0,
                    (foundCentreX[p] - PetalLayout.canvas / 2.0) / 512.0,
                    (foundCentreY[p] - fieldCentre) / 145.0,
                ]
                for i in 0..<3 {
                    for j in 0..<3 { normal[i][j] += basis[i] * basis[j] }
                    rightX[i] += basis[i] * foundShiftX[p]
                    rightY[i] += basis[i] * foundShiftY[p]
                }
            }
            guard let solvedX = Self.solve3(normal, rightX), let solvedY = Self.solve3(normal, rightY) else { return }
            fieldX = solvedX
            fieldY = solvedY
        }

        /// The shift of the field at the tile centre `(x, y)`.
        private func shiftAt(_ x: Double, _ y: Double) -> (Double, Double) {
            let b1 = (x - PetalLayout.canvas / 2.0) / 512.0
            let b2 = (y - fieldCentre) / 145.0
            return (
                fieldX[0] * 1.0 + fieldX[1] * b1 + fieldX[2] * b2,
                fieldY[0] * 1.0 + fieldY[1] * b1 + fieldY[2] * b2
            )
        }

        /// Solves `normal · s = right` by Gauss–Jordan elimination with partial
        /// pivoting; `nil` when a pivot is below `1e-9`. Of equal pivots the
        /// last wins, as the reference's `max_by` picks.
        private static func solve3(_ normal: [[Double]], _ right: [Double]) -> [Double]? {
            var m = normal
            var r = right
            for col in 0..<3 {
                var pivot = col
                for row in (col + 1)..<3
                where PetalNumeric.totalOrderKey(abs(m[row][col])) >= PetalNumeric.totalOrderKey(abs(m[pivot][col])) {
                    pivot = row
                }
                if abs(m[pivot][col]) < 1e-9 { return nil }
                m.swapAt(col, pivot)
                r.swapAt(col, pivot)
                for row in 0..<3 where row != col {
                    let factor = m[row][col] / m[col][col]
                    for k in col..<3 { m[row][k] -= factor * m[col][k] }
                    r[row] -= factor * r[col]
                }
            }
            return [r[0] / m[0][0], r[1] / m[1][1], r[2] / m[2][2]]
        }

        /// A level that varies linearly across the band, fitted by least
        /// squares to the levels of the light (`light`) or dark found pilots
        /// against `(x − 512) / 512`, as `(intercept, slope)`; a constant when
        /// the pilots do not spread, `nil` without such pilots.
        private func levelLine(light: Bool) -> (Double, Double)? {
            var xs: [Double] = []
            var ys: [Double] = []
            for p in 0..<foundCount where foundLight[p] == light {
                xs.append((foundCentreX[p] - PetalLayout.canvas / 2.0) / 512.0)
                ys.append(foundLevel[p])
            }
            if xs.isEmpty { return nil }
            let n = Double(xs.count)
            var sx = 0.0
            for x in xs { sx += x }
            let mx = sx / n
            var sy = 0.0
            for y in ys { sy += y }
            let my = sy / n
            var sxx = 0.0
            for x in xs { sxx += (x - mx) * (x - mx) }
            if sxx < 1e-6 { return (my, 0.0) }
            var sxy = 0.0
            for p in xs.indices { sxy += (xs[p] - mx) * (ys[p] - my) }
            let slope = sxy / sxx
            return (my - slope * mx, slope)
        }
    }
}

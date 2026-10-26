import XCTest
@testable import IrohaSwift

/// Ports of the `kana`, `glyphs` and card `layout` unit tests of `crates/iroha_petal`.
final class PetalKanaTests: XCTestCase {
    private typealias Support = PetalTestSupport

    func testLaneSizesAndIDsAreConsistent() {
        XCTAssertEqual(PetalKana.glyphWordLength, 149)
        XCTAssertEqual(PetalKana.polarityWordLength, 37)
        XCTAssertEqual(PetalLayout.kanaTiles.count, PetalLayout.kanaTileCount)
        XCTAssertEqual(PetalLayout.kanaDataTiles(0).count, PetalKana.dataTiles)
        for lane in PetalKanaLane.allCases {
            XCTAssertEqual(lane.dataLength, lane.atomCount * PetalLane.atomLength + PetalKana.checkLength, lane.name)
        }
        var ids = Set<UInt32>()
        for frame: UInt16 in [0, 1, 65_535] {
            for lane in PetalKanaLane.allCases {
                for k in 0..<lane.atomCount {
                    XCTAssertTrue(ids.insert(PetalKana.kanaFirstID(frame: frame, lane: lane) + UInt32(k)).inserted)
                }
            }
        }
        // above every turn atom id and below the u32 limit
        XCTAssertGreaterThanOrEqual(
            PetalKana.kanaFirstID(frame: 0, lane: .k0),
            PetalTurns.turnFirstID(frame: 65_535, sector: 3) + 4
        )
        XCTAssertLessThan(PetalKana.kanaFirstID(frame: 65_535, lane: .p1), UInt32.max - 16)
        XCTAssertEqual(PetalKanaLane.allCases.map(\.name), ["P0", "K0", "P1", "K1"])
        XCTAssertEqual(PetalKanaLane.allCases.map(\.code), [0x10, 0x11, 0x12, 0x13])
        XCTAssertEqual(PetalKanaLane(band: 1, glyph: true), .k1)
        XCTAssertNil(PetalKanaLane(band: 2, glyph: false))
    }

    func testWhiteningIsBalancedAndDistinctPerLane() {
        let all = PetalKanaLane.allCases.map(\.whitening)
        for (lane, whitening) in zip(PetalKanaLane.allCases, all) {
            XCTAssertEqual(whitening.count, lane.wordLength)
            let ones = whitening.reduce(0) { $0 + $1.nonzeroBitCount }
            let bits = whitening.count * 8
            XCTAssertTrue(ones > bits * 35 / 100 && ones < bits * 65 / 100, lane.name)
        }
        XCTAssertNotEqual(Array(all[1].prefix(37)), all[0])
        XCTAssertNotEqual(all[1], all[3])
    }

    func testSymbolsRoundtripThroughCodewords() throws {
        let header = PetalLaneHeader(tag: 0x5A, frame: 77)
        var words: [[UInt8]] = []
        var atoms: [[UInt8]] = []
        for lane in PetalKanaLane.allCases {
            let laneAtoms: [UInt8] = lane.isGlyph
                ? (0..<112).map { UInt8(truncatingIfNeeded: ($0 * 7) & 0xFF) ^ UInt8(lane.band) }
                : (0..<16).map { UInt8($0) ^ UInt8(lane.band) }
            atoms.append(laneAtoms)
            words.append(try PetalKana.encodeKanaLane(lane, data: PetalKana.kanaData(header, lane: lane, atoms: laneAtoms)))
        }
        var cells = try PetalFrameCells(
            d: [UInt8](repeating: 0, count: PetalLane.d.wordLength),
            fields: Array(repeating: [UInt8](repeating: 0, count: PetalLane.field(0).wordLength), count: PetalLayout.sectors)
        )
        try cells.setKana(words)
        for band in 0..<PetalLayout.kanaBands {
            let polarity = try XCTUnwrap(PetalKanaLane(band: band, glyph: false))
            let glyph = try XCTUnwrap(PetalKanaLane(band: band, glyph: true))
            XCTAssertEqual(cells.kanaPolarityWord(band: band), words[polarity.rawValue])
            XCTAssertEqual(cells.kanaGlyphWord(band: band), words[glyph.rawValue])
            let data = try PetalKana.decodeKanaLaneCounted(glyph, words[glyph.rawValue]).data
            let packet = try XCTUnwrap(PetalKana.kanaPacket(header, lane: glyph, data: data), "check matches")
            XCTAssertEqual(packet.firstID, PetalKana.kanaFirstID(frame: 77, lane: glyph))
            XCTAssertEqual(packet.atoms.flatMap { $0 }, atoms[glyph.rawValue])
            // the check refuses another frame, band or kind
            XCTAssertNil(PetalKana.kanaPacket(PetalLaneHeader(tag: 0x5A, frame: 78), lane: glyph, data: data))
            XCTAssertNil(PetalKana.kanaPacket(header, lane: try XCTUnwrap(PetalKanaLane(band: 1 - band, glyph: true)), data: data))
            let polarityData = try PetalKana.decodeKanaLaneCounted(polarity, words[polarity.rawValue]).data
            XCTAssertEqual(try XCTUnwrap(PetalKana.kanaPacket(header, lane: polarity, data: polarityData)).atoms, [atoms[polarity.rawValue]])
        }
        // pilots keep their symbols; the data tiles beyond the polarity codeword are light
        for (index, tile) in PetalLayout.kanaTiles.enumerated() {
            guard let pilot = tile.pilot else { continue }
            XCTAssertEqual(Int(cells.kanaGlyphs[index]), PetalLayout.kanaPilotSymbols[pilot].glyph)
            XCTAssertEqual(cells.kanaLight[index], PetalLayout.kanaPilotSymbols[pilot].light)
        }
        for band in 0..<PetalLayout.kanaBands {
            let data = PetalLayout.kanaDataTiles(band)
            XCTAssertTrue(cells.kanaLight[data[296]] && cells.kanaLight[data[297]])
        }
        // malformed words are refused
        XCTAssertThrowsError(try cells.setKana(Array(words.prefix(3))))
        XCTAssertThrowsError(try cells.setKana([words[1], words[0], words[2], words[3]]))
        XCTAssertThrowsError(try PetalKana.kanaData(header, lane: .p0, atoms: [1, 2, 3]))
        XCTAssertThrowsError(try PetalKana.encodeKanaLane(.k0, data: [UInt8](repeating: 0, count: 20)))
        XCTAssertThrowsError(try PetalKana.decodeKanaLaneCounted(.k0, [UInt8](repeating: 0, count: 37)))
    }

    func testTemplatesAreUnitAndTellGlyphsApart() {
        let side = PetalKana.grid
        for blur in PetalKana.blurs.indices {
            for glyph in 0..<PetalGlyphs.glyphCount {
                for light in [true, false] {
                    var norm = 0.0
                    for p in 0..<(side * side) {
                        let value = PetalKana.templateProbe(blur: blur, glyph: glyph, light: light, i: p % side, j: p / side)
                        norm += value * value
                    }
                    XCTAssertEqual(norm, 1.0, accuracy: 1e-9, "blur \(blur) glyph \(glyph) light \(light)")
                }
            }
        }
        // at the sharpest blur no two light templates correlate above 0.8
        for a in 0..<PetalGlyphs.glyphCount {
            for b in (a + 1)..<PetalGlyphs.glyphCount {
                var dot = 0.0
                for p in 0..<(side * side) {
                    dot += PetalKana.templateProbe(blur: 0, glyph: a, light: true, i: p % side, j: p / side)
                        * PetalKana.templateProbe(blur: 0, glyph: b, light: true, i: p % side, j: p / side)
                }
                XCTAssertLessThan(dot, 0.8, "glyphs \(a) and \(b)")
            }
        }
        // the coarse templates are unit too
        let coarse = PetalKana.coarseGrid
        for glyph in 0..<PetalGlyphs.glyphCount {
            for light in [true, false] {
                var norm = 0.0
                for p in 0..<(coarse * coarse) {
                    let value = PetalKana.coarseTemplateProbe(glyph: glyph, light: light, i: p % coarse, j: p / coarse)
                    norm += value * value
                }
                XCTAssertEqual(norm, 1.0, accuracy: 1e-9, "coarse glyph \(glyph) light \(light)")
            }
        }
        XCTAssertEqual(PetalKana.templateProbe(blur: 5, glyph: 0, light: true, i: 0, j: 0), 0)
        XCTAssertEqual(PetalKana.coarseTemplateProbe(glyph: 0, light: true, i: 6, j: 0), 0)
    }

    func testTheCardHolds306TilesPerBandAndEightPilotsEach() throws {
        XCTAssertEqual(PetalLayout.kanaBandHeight, 322)
        XCTAssertEqual(PetalLayout.cardHeight, 1668)
        let tiles = PetalLayout.kanaTiles
        for band in 0..<PetalLayout.kanaBands {
            let inBand = tiles.filter { $0.band == band }
            XCTAssertEqual(inBand.count, 306, "band \(band)")
            XCTAssertEqual(inBand.filter { $0.pilot != nil }.count, PetalLayout.kanaPilots)
            XCTAssertEqual(PetalLayout.kanaDataTiles(band).count, 306 - PetalLayout.kanaPilots)
            for (pilot, tile) in PetalLayout.kanaPilotTiles(band).enumerated() {
                XCTAssertEqual(tiles[tile].pilot, pilot)
            }
        }
        XCTAssertTrue(PetalLayout.kanaDataTiles(2).isEmpty)
        // the bands mirror each other about the square's middle
        for tile in tiles where tile.band == 0 {
            let mirror = try XCTUnwrap(
                tiles.first { $0.band == 1 && $0.column == tile.column && $0.row == PetalLayout.kanaRows - 1 - tile.row },
                "mirrored tile"
            )
            XCTAssertEqual(mirror.y + PetalLayout.kanaTile - PetalLayout.canvas + tile.y, 0, accuracy: 1e-9)
            XCTAssertEqual(mirror.pilot, tile.pilot)
        }
        // every tile centre maps back to its tile, and the square holds none
        for (index, tile) in tiles.enumerated() {
            XCTAssertEqual(PetalLayout.kanaTileAt(x: tile.center.x, y: tile.center.y)?.tile, index)
        }
        XCTAssertNil(PetalLayout.kanaTileAt(x: 512, y: 512))
        // tile centres keep their distance from the finders
        for tile in tiles {
            for finder in PetalLayout.finderCenters {
                let dx = tile.center.x - finder.x
                let dy = tile.center.y - finder.y
                XCTAssertGreaterThanOrEqual((dx * dx + dy * dy).squareRoot(), PetalLayout.kanaFinderClearance)
            }
        }
    }

    func testEveryGlyphHasInkInsideTheDesignGridAndStaysDistinct() {
        func coverage(_ glyph: Int) -> [Double] {
            let cell = PetalGlyphs.glyphGrid / 8.0
            return (0..<64).map { i in
                let u = Double(i % 8)
                let v = Double(i / 8)
                var inked = 0
                for sy in 0..<8 {
                    for sx in 0..<8 {
                        let x = (u + (Double(sx) + 0.5) / 8.0) * cell
                        let y = (v + (Double(sy) + 0.5) / 8.0) * cell
                        if PetalGlyphs.isInked(glyph, x: x, y: y) { inked += 1 }
                    }
                }
                return Double(inked) / 64.0
            }
        }
        for glyph in 0..<PetalGlyphs.glyphCount {
            XCTAssertGreaterThan(coverage(glyph).reduce(0, +), 6.0, "glyph \(glyph)")
            for stroke in PetalGlyphs.strokes(glyph) {
                for point in stroke {
                    XCTAssertTrue((2.0...30.0).contains(point.x) && (2.0...30.0).contains(point.y))
                }
            }
        }
        func feature(_ glyph: Int) -> [Double] {
            let values = coverage(glyph)
            let mean = values.reduce(0, +) / Double(values.count)
            let centred = values.map { $0 - mean }
            let norm = centred.reduce(0) { $0 + $1 * $1 }.squareRoot()
            return centred.map { $0 / norm }
        }
        for a in 0..<PetalGlyphs.glyphCount {
            for b in (a + 1)..<PetalGlyphs.glyphCount {
                let dot = zip(feature(a), feature(b)).reduce(0) { $0 + $1.0 * $1.1 }
                XCTAssertGreaterThan(1.0 - dot, 0.2, "glyphs \(a) and \(b)")
            }
        }
        XCTAssertEqual(PetalGlyphs.characters.count, PetalGlyphs.glyphCount)
        XCTAssertFalse(PetalGlyphs.isInked(16, x: 16, y: 16))
    }

    func testEveryGlyphStrokeLiesInsideItsRoundedTile() {
        // what lets a native canvas stroke the glyphs without clipping them to the tile
        let inset = (PetalLayout.kanaTile - PetalLayout.kanaGlyphBox) / 2.0
        let scale = PetalGlyphs.glyphGrid / PetalLayout.kanaGlyphBox
        for glyph in 0..<PetalGlyphs.glyphCount {
            for a in 0...270 {
                for b in 0...270 {
                    let u = -1.0 + Double(a) * 0.1
                    let v = -1.0 + Double(b) * 0.1
                    if PetalGlyphs.isInked(glyph, x: (u - inset) * scale, y: (v - inset) * scale) {
                        XCTAssertTrue(PetalLayout.kanaInside(u: u, v: v), "glyph \(glyph) inked outside its tile at (\(u), \(v))")
                    }
                }
            }
        }
    }

    func testAClearCardReadsEveryKanaLane() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(3_000, seed: 3), kind: 2)
        let image = try Support.renderCard(encoder, frame: 3, size: 640, supersample: 2)
        let decoded = try PetalDecoder.decode(image)
        XCTAssertEqual(decoded.laneLetters, "D0123")
        let header = PetalLaneHeader(tag: encoder.meta.tag, frame: 3)
        let truth = encoder.laneData(frame: 3)
        let results = PetalKana.readKana(image, frame: decoded, header: header)
        XCTAssertEqual(results.count, 4)
        for lane in PetalKanaLane.allCases {
            let result = try XCTUnwrap(results[lane.rawValue], lane.name)
            XCTAssertEqual(result.header, header)
            XCTAssertEqual(result.result.data, truth.kana(lane), lane.name)
            XCTAssertEqual(result.result.corrected, 0, lane.name)
        }
        // a skipped band is not read
        let skipped = PetalKana.readKana(image, frame: decoded, header: header, skip: [true, false])
        XCTAssertNil(skipped[0])
        XCTAssertNil(skipped[1])
        XCTAssertNotNil(skipped[2])
        // the check also matches the frame before the header's
        let previous = PetalLaneHeader(tag: encoder.meta.tag, frame: 4)
        XCTAssertEqual(PetalKana.readKana(image, frame: decoded, header: previous)[1]?.header, header)
        // every data tile reads its symbol
        let cells = encoder.cells(frame: 3)
        for band in 0..<PetalLayout.kanaBands {
            let reading = try XCTUnwrap(PetalKana.observeBand(image, frame: decoded, band: band))
            XCTAssertEqual(reading.pilots, PetalLayout.kanaPilots)
            XCTAssertTrue(PetalKana.blurs.contains(reading.blur))
            XCTAssertEqual(reading.tiles.count, PetalKana.dataTiles)
            for tile in reading.tiles {
                let tile = try XCTUnwrap(tile)
                XCTAssertEqual(tile.glyph, Int(cells.kanaGlyphs[tile.tile]), "tile \(tile.tile)")
                XCTAssertEqual(tile.light, cells.kanaLight[tile.tile], "tile \(tile.tile)")
            }
        }
        XCTAssertNil(PetalKana.observeBand(image, frame: decoded, band: 2))
        // a bare square shows no band
        let square = try Support.render(encoder, frame: 3, size: 512, supersample: 2)
        let squareFrame = try PetalDecoder.decode(square)
        XCTAssertTrue(PetalKana.readKana(square, frame: squareFrame, header: header).allSatisfy { $0 == nil })
    }
}

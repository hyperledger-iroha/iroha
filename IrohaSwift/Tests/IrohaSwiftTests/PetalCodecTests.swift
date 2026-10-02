import XCTest
@testable import IrohaSwift

/// Ports of the `crc`, `prng`, `rs`, `lanes`, `layout` and `glyphs` unit
/// tests of `crates/iroha_petal`.
final class PetalCodecTests: XCTestCase {
    // MARK: - CRC-32C and xorshift32

    func testCRC32CMatchesThePublishedCheckValue() {
        XCTAssertEqual(IrohaPeerCRC32CV1.checksum(Array("123456789".utf8)), 0xE306_9283)
        XCTAssertEqual(IrohaPeerCRC32CV1.checksum([UInt8]()), 0)
    }

    func testXorshiftMatchesTheReferenceSequence() {
        var rng = PetalXorshift32(seed: 1)
        XCTAssertEqual(rng.nextUInt32(), 270_369)
        XCTAssertEqual(rng.nextUInt32(), 67_634_689)
        XCTAssertEqual(rng.nextUInt32(), 2_647_435_461)
    }

    func testZeroSeedIsRemapped() {
        XCTAssertEqual(PetalXorshift32(seed: 0), PetalXorshift32(seed: 0xDEAD_BEEF))
    }

    // MARK: - Reed–Solomon

    func testReedSolomonMatchesTheQRHelloWorldCheckVector() throws {
        // QR Code version 1-M "HELLO WORLD": 16 data codewords, 10 EC codewords.
        let data: [UInt8] = [32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17]
        let expected: [UInt8] = [196, 35, 39, 119, 235, 215, 231, 226, 93, 23]
        let word = try PetalReedSolomon(parityLength: 10).encode(data)
        XCTAssertEqual(Array(word[..<16]), data)
        XCTAssertEqual(Array(word[16...]), expected)
    }

    func testReedSolomonCorrectsRandomErrorsAndErasuresUpToCapacity() throws {
        var rng = PetalTestLCG(state: 7)
        for (k, nsym) in [(16, 16), (60, 68), (12, 18), (13, 115)] {
            let rs = try PetalReedSolomon(parityLength: nsym)
            for _ in 0..<60 {
                let data = (0..<k).map { _ in rng.byte() }
                let clean = try rs.encode(data)
                let n = clean.count
                // pick f erasures and e errors with 2e + f <= nsym
                let f = rng.below(min(nsym, n - 1) + 1)
                let e = rng.below((nsym - f) / 2 + 1)
                var word = clean
                var positions = Array(0..<n)
                for i in 0..<(f + e) {
                    let j = i + rng.below(n - i)
                    positions.swapAt(i, j)
                }
                let erased = Array(positions[..<f])
                let errored = positions[f..<(f + e)]
                for p in erased { word[p] = rng.byte() }
                for p in errored { word[p] ^= rng.byte() | 1 }
                let corrected = try rs.decode(&word, erasures: erased)
                XCTAssertLessThanOrEqual(corrected, f + e)
                XCTAssertEqual(word, clean, "k=\(k) nsym=\(nsym) f=\(f) e=\(e)")
            }
        }
    }

    func testReedSolomonRejectsWordsBeyondCapacityWithoutReturningWrongData() throws {
        var rng = PetalTestLCG(state: 99)
        let rs = try PetalReedSolomon(parityLength: 16)
        var wrongAccepts = 0
        for _ in 0..<200 {
            let data = (0..<16).map { _ in rng.byte() }
            let clean = try rs.encode(data)
            var word = clean
            // 20 random errors is far beyond t = 8
            var positions = Array(0..<word.count)
            for i in 0..<20 {
                let j = i + rng.below(word.count - i)
                positions.swapAt(i, j)
            }
            for p in positions[..<20] { word[p] ^= rng.byte() | 1 }
            if (try? rs.decode(&word)) != nil {
                // a miscorrection must at least be a valid codeword
                XCTAssertTrue(rs.syndromes(word).allSatisfy { $0 == 0 })
                if word != clean { wrongAccepts += 1 }
            }
        }
        XCTAssertLessThanOrEqual(wrongAccepts, 2, "miscorrection rate too high")
    }

    func testReedSolomonRejectsMalformedArguments() throws {
        let rs = try PetalReedSolomon(parityLength: 4)
        var short = [UInt8](repeating: 0, count: 4)
        XCTAssertThrowsError(try rs.decode(&short)) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        var word = try rs.encode([1, 2, 3])
        XCTAssertThrowsError(try rs.decode(&word, erasures: [9])) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        XCTAssertThrowsError(try rs.decode(&word, erasures: [1, 1])) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        XCTAssertThrowsError(try rs.decode(&word, erasures: [-1])) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        XCTAssertThrowsError(try PetalReedSolomon(parityLength: 0))
        XCTAssertThrowsError(try PetalReedSolomon(parityLength: 255))
        XCTAssertThrowsError(try rs.encode([UInt8](repeating: 0, count: 252)))
    }

    // MARK: - Lanes

    func testLaneSizesAreConsistent() {
        XCTAssertEqual(PetalLane.p.wordLength, 32)
        XCTAssertEqual(PetalLane.k.wordLength, 128)
        XCTAssertEqual(PetalLane.d.wordLength, 30)
        XCTAssertEqual(PetalLane.p.dataLength, 19)
        XCTAssertEqual(PetalLane.k.dataLength, 83)
        XCTAssertEqual(PetalLane.d.dataLength, 19)
        for lane in PetalLane.decodeOrder {
            XCTAssertEqual(lane.dataLength, PetalLane.headerLength + lane.atomCount * PetalLane.atomLength)
        }
        XCTAssertEqual(PetalLane.decodeOrder, [.p, .d, .k])
        XCTAssertEqual(
            PetalLane.atomsPerFrame,
            PetalLane.p.atomCount + PetalLane.d.atomCount + PetalLane.k.atomCount
        )
        XCTAssertEqual(PetalLane.decodeOrder.map(\.letter), ["P", "D", "K"])
    }

    func testWhiteningIsDeterministicAndBalanced() {
        for lane in PetalLane.decodeOrder {
            let mask = lane.whitening
            XCTAssertEqual(mask, lane.whitening)
            XCTAssertEqual(mask.count, lane.wordLength)
            let ones = mask.reduce(0) { $0 + $1.nonzeroBitCount }
            let bits = mask.count * 8
            XCTAssertGreaterThan(ones, bits * 38 / 100, "\(lane)")
            XCTAssertLessThan(ones, bits * 62 / 100, "\(lane)")
        }
    }

    func testLanesRoundtripThroughCells() throws {
        let pData = (0..<19).map { UInt8($0) }
        let kData = (0..<83).map { UInt8($0) &* 37 }
        let dData = (0..<19).map { UInt8($0) ^ 0xA5 }
        let p = try PetalLane.p.encode(pData)
        let k = try PetalLane.k.encode(kData)
        let d = try PetalLane.d.encode(dData)
        let cells = try PetalFrameCells(p: p, k: k, d: d)
        XCTAssertEqual(cells.pWord, p)
        XCTAssertEqual(cells.kWord, k)
        XCTAssertEqual(cells.dWord, d)
        XCTAssertEqual(try PetalLane.p.decode(cells.pWord), pData)
        XCTAssertEqual(try PetalLane.k.decode(cells.kWord), kData)
        XCTAssertEqual(try PetalLane.d.decode(cells.dWord), dData)
    }

    func testAllZeroDataStillLightsRoughlyHalfTheCells() throws {
        let cells = try PetalFrameCells(
            p: PetalLane.p.encode([UInt8](repeating: 0, count: 19)),
            k: PetalLane.k.encode([UInt8](repeating: 0, count: 83)),
            d: PetalLane.d.encode([UInt8](repeating: 0, count: 19))
        )
        let lit = cells.light.filter { $0 }.count
        XCTAssertTrue((90...166).contains(lit), "\(lit) light tiles")
    }

    func testGateDotsAreAlwaysLitAndGuardsDark() throws {
        let cells = try PetalFrameCells(
            p: [UInt8](repeating: 0xFF, count: 32),
            k: [UInt8](repeating: 0xFF, count: 128),
            d: [UInt8](repeating: 0xFF, count: 30)
        )
        for (slot, role) in PetalLayout.slotRoles.enumerated() {
            switch role {
            case .gate, .data: XCTAssertTrue(cells.dots[slot])
            case .guard, .spare: XCTAssertFalse(cells.dots[slot])
            }
        }
    }

    func testCountedDecodingReportsTheRewrittenPositions() throws {
        let data = (0..<19).map { UInt8($0) }
        let clean = try PetalLane.p.encode(data)
        let exact = try PetalLane.p.decodeCounted(clean)
        XCTAssertEqual(exact.data, data)
        XCTAssertEqual(exact.corrected, 0)
        var damaged = clean
        for position in [0, 7, 19, 31] { damaged[position] ^= 0xC3 }
        let repaired = try PetalLane.p.decodeCounted(damaged)
        XCTAssertEqual(repaired.data, data)
        XCTAssertEqual(repaired.corrected, 4)
    }

    func testDecodeSurvivesBurstDamageToALane() throws {
        let kData = (0..<83).map { UInt8($0) }
        var word = try PetalLane.k.encode(kData)
        for index in 0..<(45 / 2) { word[index] ^= 0x5A }
        XCTAssertEqual(try PetalLane.k.decode(word), kData)
    }

    func testLaneCodecRejectsWrongLengths() {
        XCTAssertThrowsError(try PetalLane.p.encode([1, 2, 3])) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        XCTAssertThrowsError(try PetalLane.d.decode([UInt8](repeating: 0, count: 29))) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        XCTAssertThrowsError(try PetalFrameCells(p: [0], k: [], d: []))
    }

    // MARK: - Layout

    func testMaskHas256SymmetricTiles() {
        XCTAssertEqual(PetalLayout.tiles.count, 256)
        for row in PetalLayout.mask {
            let characters = Array(row)
            XCTAssertEqual(characters.count, PetalLayout.tileGrid)
            for column in 0..<(PetalLayout.tileGrid / 2) {
                XCTAssertEqual(characters[column], characters[PetalLayout.tileGrid - 1 - column], "row \(row) not mirrored")
            }
        }
        XCTAssertNotEqual(PetalLayout.mask.first, PetalLayout.mask.last, "mask must show top from bottom")
    }

    func testTilesAreRowMajorAndInsideTheCanvas() {
        var previous: PetalTilePosition?
        for (index, position) in PetalLayout.tiles.enumerated() {
            if let previous {
                XCTAssertTrue((position.row, position.column) > (previous.row, previous.column))
            }
            previous = position
            let center = PetalLayout.tileCenter(index)
            XCTAssertTrue((0..<PetalLayout.canvas).contains(center.x))
            XCTAssertTrue((0..<PetalLayout.canvas).contains(center.y))
        }
    }

    func testRingSlotsProvideExactlyTheLaneDCapacity() {
        let roles = PetalLayout.slotRoles
        let data = roles.filter { if case .data = $0 { return true } else { return false } }.count
        XCTAssertEqual(data, PetalLayout.dataBits)
        XCTAssertEqual(roles.filter { $0 == .gate }.count, 4 + 5 + 7)
        XCTAssertEqual(PetalLayout.dataSlots.count, PetalLayout.dataBits)
        // the two spare slots are the last non-reserved slots of the outer ring
        XCTAssertEqual(roles.filter { $0 == .spare }.count, 2)
    }

    func testGatesNeverTouchTheTop() {
        for (ring, n) in PetalLayout.ringSlots.enumerated() {
            let (gates, guards) = PetalLayout.gateSlots(ring: ring)
            for slot in gates + guards {
                XCTAssertGreaterThan(abs(slot - 3 * n / 4), 2, "ring \(ring) slot \(slot) near the top")
            }
        }
    }

    func testFindersAndRingsDoNotOverlap() {
        let outermost = PetalLayout.ringRadii[2] + PetalLayout.dotRadius
        for center in PetalLayout.finderCenters {
            let dx = center.x - PetalLayout.center
            let dy = center.y - PetalLayout.center
            XCTAssertGreaterThan((dx * dx + dy * dy).squareRoot() - PetalLayout.finderOuter, outermost + 20)
        }
        var farthest = 0.0
        for index in 0..<PetalLayout.tileCount {
            let center = PetalLayout.tileCenter(index)
            let h = PetalLayout.tileSize / 2
            for (x, y) in [(center.x - h, center.y - h), (center.x + h, center.y - h),
                           (center.x - h, center.y + h), (center.x + h, center.y + h)] {
                let dx = x - PetalLayout.center
                let dy = y - PetalLayout.center
                farthest = max(farthest, (dx * dx + dy * dy).squareRoot())
            }
        }
        XCTAssertLessThan(farthest + 10, PetalLayout.ringRadii[0] - PetalLayout.dotRadius)
    }

    func testSlotCentersSitOnTheirRings() {
        for (ring, slots) in PetalLayout.ringSlots.enumerated() {
            for slot in 0..<slots {
                let point = PetalLayout.slotCenter(ring: ring, slot: slot)
                let dx = point.x - PetalLayout.center
                let dy = point.y - PetalLayout.center
                XCTAssertEqual((dx * dx + dy * dy).squareRoot(), PetalLayout.ringRadii[ring], accuracy: 1e-3)
            }
        }
        let first = PetalLayout.slotCenter(ring: 0, slot: 0)
        XCTAssertEqual(first, PetalPoint(x: 872, y: 512))
        XCTAssertEqual(PetalLayout.splitSlot(80).ring, 1)
        XCTAssertEqual(PetalLayout.splitSlot(275).slot, 103)
    }

    func testFinderBlossomShape() {
        XCTAssertTrue(PetalLayout.finderLit(dx: 0, dy: 0))
        XCTAssertTrue(PetalLayout.finderLit(dx: 0, dy: -34), "upper petal centre")
        XCTAssertFalse(PetalLayout.finderLit(dx: 0, dy: -58), "notch at the upper petal tip")
        XCTAssertFalse(PetalLayout.finderLit(dx: 0, dy: 59), "gap between the two lower petals")
        XCTAssertFalse(PetalLayout.finderLit(dx: 70, dy: 0))
    }

    // MARK: - Glyphs

    func testCheckedInTemplatesMatchTheStrokeDefinitions() {
        XCTAssertEqual(PetalGlyphs.generateTemplates(), PetalGlyphs.templates)
    }

    func testEveryGlyphHasInkInsideTheDesignGrid() {
        XCTAssertEqual(PetalGlyphs.characters.count, PetalGlyphs.count)
        for (glyph, strokes) in PetalGlyphs.strokes.enumerated() {
            let total = PetalGlyphs.templates[glyph].reduce(0) { $0 + Int($1) }
            XCTAssertGreaterThan(total, 255 * 6, "glyph \(glyph) has too little ink")
            for stroke in strokes {
                for point in stroke {
                    XCTAssertTrue((2.0...30.0).contains(point.x) && (2.0...30.0).contains(point.y))
                }
            }
        }
    }

    func testGlyphsArePairwiseDistinctUnderBlur() {
        // Zero-mean cosine distance of the raw templates must stay well apart.
        func feature(_ glyph: Int) -> [Double] {
            let values = PetalGlyphs.templates[glyph].map(Double.init)
            let mean = values.reduce(0, +) / Double(values.count)
            let centered = values.map { $0 - mean }
            let norm = centered.reduce(0) { $0 + $1 * $1 }.squareRoot()
            return centered.map { $0 / norm }
        }
        for a in 0..<PetalGlyphs.count {
            for b in (a + 1)..<PetalGlyphs.count {
                let dot = zip(feature(a), feature(b)).reduce(0) { $0 + $1.0 * $1.1 }
                XCTAssertGreaterThan(1 - dot, 0.2, "glyphs \(a) and \(b) are too similar")
            }
        }
    }

    // MARK: - Numeric helpers

    func testTotalOrderKeyMatchesIEEETotalOrder() {
        let ordered: [Double] = [-.nan, -.infinity, -1, -0.0, 0.0, 1e-300, 1, .infinity, .nan]
        for (i, a) in ordered.enumerated() {
            for (j, b) in ordered.enumerated() {
                let less = PetalNumeric.totalOrderKey(a) < PetalNumeric.totalOrderKey(b)
                XCTAssertEqual(less, i < j, "\(a) vs \(b)")
                XCTAssertEqual(
                    a.isTotallyOrdered(belowOrEqualTo: b),
                    PetalNumeric.totalOrderKey(a) <= PetalNumeric.totalOrderKey(b)
                )
            }
        }
        XCTAssertTrue(PetalNumeric.clamp(.nan, 0, 1).isNaN)
        XCTAssertEqual(PetalNumeric.saturatingInt(.nan), 0)
        XCTAssertEqual(PetalNumeric.saturatingInt(.infinity), Int.max)
        XCTAssertEqual(PetalNumeric.saturatingInt(-.infinity), Int.min)
        XCTAssertEqual(PetalNumeric.saturatingInt(-3.7), -3)
    }
}

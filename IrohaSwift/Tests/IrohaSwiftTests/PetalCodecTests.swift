import XCTest
@testable import IrohaSwift

/// Ports of the `crc`, `prng`, `rs`, `lanes` and `layout` unit tests of
/// `crates/iroha_petal`.
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

    private func fieldData(_ seed: UInt8) -> [UInt8] {
        (0..<PetalLane.field(0).dataLength).map { (UInt8($0) &* 37) ^ seed }
    }

    func testLaneSizesAreConsistent() {
        XCTAssertEqual(PetalLane.d.wordLength, 30)
        XCTAssertEqual(PetalLane.d.dataLength, 19)
        XCTAssertEqual(PetalLane.d.parityLength, 11)
        for lane in PetalLane.fields {
            XCTAssertEqual(lane.wordLength, 105)
            XCTAssertEqual(lane.dataLength, 83)
            XCTAssertEqual(lane.parityLength, 22)
        }
        XCTAssertEqual(PetalLane.spareSectorCells, 4)
        XCTAssertEqual(PetalLane.atomsPerFrame, 21)
        XCTAssertEqual(
            PetalLane.atomsPerFrame,
            PetalLane.d.atomCount + PetalLane.fields.reduce(0) { $0 + $1.atomCount }
        )
        for lane in PetalLane.decodeOrder {
            XCTAssertEqual(lane.dataLength, PetalLane.headerLength + lane.atomCount * PetalLane.atomLength)
        }
        XCTAssertEqual(PetalLane.decodeOrder, [.d, .field(0), .field(1), .field(2), .field(3)])
        XCTAssertEqual(PetalLane.decodeOrder.map(\.letter), ["D", "0", "1", "2", "3"])
        XCTAssertEqual(PetalLane.field(2).sector, 2)
        XCTAssertNil(PetalLane.d.sector)
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
        XCTAssertNotEqual(PetalLane.field(0).whitening, PetalLane.field(1).whitening)
    }

    func testLanesRoundtripThroughCells() throws {
        let dData = (0..<19).map { UInt8($0) ^ 0xA5 }
        let d = try PetalLane.d.encode(dData)
        let fields = try (0..<PetalLayout.sectors).map { try PetalLane.field($0).encode(fieldData(UInt8($0))) }
        let cells = try PetalFrameCells(d: d, fields: fields)
        XCTAssertEqual(cells.dWord, d)
        XCTAssertEqual(try PetalLane.d.decode(cells.dWord), dData)
        for (sector, field) in fields.enumerated() {
            XCTAssertEqual(cells.fieldWord(sector: sector), field)
            XCTAssertEqual(try PetalLane.field(sector).decode(cells.fieldWord(sector: sector)), fieldData(UInt8(sector)))
        }
        // the spare cells at the end of every sector stay dark
        for sector in 0..<PetalLayout.sectors {
            for cell in PetalLayout.sectorCells(sector)[(105 * 8)...] {
                XCTAssertFalse(cells.petals[cell])
            }
        }
    }

    func testAllZeroDataStillLightsAboutHalfThePetals() throws {
        let zero = [UInt8](repeating: 0, count: 83)
        let cells = try PetalFrameCells(
            d: PetalLane.d.encode([UInt8](repeating: 0, count: 19)),
            fields: (0..<PetalLayout.sectors).map { try PetalLane.field($0).encode(zero) }
        )
        let lit = cells.petals.filter { $0 }.count
        XCTAssertTrue((1300...2050).contains(lit), "\(lit) lit petals")
    }

    func testGateDotsAreAlwaysLitAndGuardsDark() throws {
        let field = [UInt8](repeating: 0xFF, count: 105)
        let cells = try PetalFrameCells(d: [UInt8](repeating: 0xFF, count: 30), fields: [field, field, field, field])
        for (slot, role) in PetalLayout.slotRoles.enumerated() {
            switch role {
            case .gate, .data: XCTAssertTrue(cells.dots[slot])
            case .guard, .spare: XCTAssertFalse(cells.dots[slot])
            }
        }
    }

    func testCountedDecodingReportsTheRewrittenPositions() throws {
        let data = fieldData(9)
        let lane = PetalLane.field(2)
        let clean = try lane.encode(data)
        let exact = try lane.decodeCounted(clean)
        XCTAssertEqual(exact.data, data)
        XCTAssertEqual(exact.corrected, 0)
        var damaged = clean
        for position in [0, 7, 19, 31, 60, 104] { damaged[position] ^= 0xC3 }
        let repaired = try lane.decodeCounted(damaged)
        XCTAssertEqual(repaired.data, data)
        XCTAssertEqual(repaired.corrected, 6)
    }

    func testAWordDecodedUnderAnotherSectorIsRefused() throws {
        let word = try PetalLane.field(0).encode(fieldData(1))
        for sector in 1..<PetalLayout.sectors {
            XCTAssertThrowsError(try PetalLane.field(sector).decode(word), "sector \(sector)")
        }
        XCTAssertEqual(try PetalLane.field(0).decode(word), fieldData(1))
    }

    func testDecodeSurvivesBurstDamageToALane() throws {
        let data = fieldData(3)
        var word = try PetalLane.field(1).encode(data)
        for index in 0..<(22 / 2) { word[index] ^= 0x5A }
        XCTAssertEqual(try PetalLane.field(1).decode(word), data)
    }

    func testLaneCodecRejectsWrongLengths() {
        XCTAssertThrowsError(try PetalLane.field(0).encode([1, 2, 3])) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        XCTAssertThrowsError(try PetalLane.d.decode([UInt8](repeating: 0, count: 29))) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        XCTAssertThrowsError(try PetalFrameCells(d: [0], fields: []))
        let field = [UInt8](repeating: 0, count: 105)
        XCTAssertThrowsError(try PetalFrameCells(d: [UInt8](repeating: 0, count: 30), fields: [field, field, field]))
        XCTAssertThrowsError(try PetalFrameCells(d: [UInt8](repeating: 0, count: 30), fields: [field, field, field, [0]]))
    }

    // MARK: - Layout

    func testMaskIsMirrorSymmetricAndShowsWhichWayIsUp() {
        XCTAssertEqual(PetalLayout.mask.count, PetalLayout.maskGrid)
        for row in PetalLayout.mask {
            let characters = Array(row)
            XCTAssertEqual(characters.count, PetalLayout.maskGrid)
            for column in 0..<(PetalLayout.maskGrid / 2) {
                XCTAssertEqual(characters[column], characters[PetalLayout.maskGrid - 1 - column], "row \(row) not mirrored")
            }
        }
        XCTAssertNotEqual(PetalLayout.mask.first, PetalLayout.mask.last, "mask must show top from bottom")
    }

    func testBandsHoldEveryCellAndSplitIntoEqualSectors() {
        XCTAssertEqual(PetalLayout.bandOffsets[PetalLayout.bandCount], PetalLayout.fieldCells)
        var seen = [Bool](repeating: false, count: PetalLayout.fieldCells)
        for sector in 0..<PetalLayout.sectors {
            let cells = PetalLayout.sectorCells(sector)
            XCTAssertEqual(cells.count, PetalLayout.sectorCellCount)
            for cell in cells {
                XCTAssertFalse(seen[cell], "cell \(cell) in two sectors")
                seen[cell] = true
                // a cell sits in its own quarter turn (odd bands lean half a slot forward)
                let (band, slot) = PetalLayout.cellBandSlot(cell)
                let quarter = PetalLayout.bandSlots[band] / PetalLayout.sectors
                XCTAssertEqual(slot / quarter, sector)
                let turn = PetalLayout.cellAngle(cell) / (2 * Double.pi) * Double(PetalLayout.sectors)
                XCTAssertTrue(turn > Double(sector) - 1e-9 && turn < Double(sector) + 1)
            }
        }
        XCTAssertTrue(seen.allSatisfy { $0 })
    }

    func testBandSlotsFollowThePitchRule() {
        for (band, slots) in PetalLayout.bandSlots.enumerated() {
            let raw = 2 * Double.pi * PetalLayout.bandRadius(band) / 50
            XCTAssertEqual(slots, 4 * max(Int(raw.rounded()), 1), "band \(band)")
        }
    }

    func testCellCentresFollowTheBands() {
        for cell in 0..<PetalLayout.fieldCells {
            let (band, slot) = PetalLayout.cellBandSlot(cell)
            XCTAssertEqual(PetalLayout.bandOffsets[band] + slot, cell)
            let angle = 2 * Double.pi * (Double(slot) + PetalLayout.bandPhase(band)) / Double(PetalLayout.bandSlots[band])
            XCTAssertEqual(PetalLayout.cellAngle(cell), angle)
            let center = PetalLayout.cellCenter(cell)
            let radius = PetalLayout.bandRadius(band)
            XCTAssertEqual(center.x, 512 + radius * cos(angle), accuracy: 1e-9)
            XCTAssertEqual(center.y, 512 + radius * sin(angle), accuracy: 1e-9)
        }
        XCTAssertEqual(PetalLayout.cellBandSlot(PetalLayout.fieldCells - 1).band, PetalLayout.bandCount - 1)
    }

    func testTheFieldMapsOntoItselfUnderAQuarterTurn() {
        for cell in 0..<PetalLayout.fieldCells {
            let (band, slot) = PetalLayout.cellBandSlot(cell)
            let n = PetalLayout.bandSlots[band]
            let turned = PetalLayout.bandOffsets[band] + (slot + n / 4) % n
            let p = PetalLayout.cellCenter(cell)
            let q = PetalLayout.cellCenter(turned)
            // a quarter turn clockwise on screen about the centre
            XCTAssertEqual(q.x, 512 - (p.y - 512), accuracy: 1e-9, "cell \(cell)")
            XCTAssertEqual(q.y, 512 + (p.x - 512), accuracy: 1e-9, "cell \(cell)")
        }
    }

    func testPetalsStayClearOfTheirNeighboursAndTheRings() {
        // a petal reaches at most its half-length from its centre, whichever way it is turned
        let outer = PetalLayout.bandRadius(PetalLayout.bandCount - 1) + PetalLayout.petalHalfLength
        XCTAssertLessThan(outer + 10, PetalLayout.ringRadii[0] - PetalLayout.dotRadius)
        XCTAssertLessThan(outer, PetalLayout.backdropRadius)
        for cell in 0..<PetalLayout.fieldCells {
            let p = PetalLayout.cellCenter(cell)
            let band = PetalLayout.cellBandSlot(cell).band
            let near = PetalLayout.bandOffsets[max(band - 1, 0)]..<PetalLayout.bandOffsets[min(band + 2, PetalLayout.bandCount)]
            for other in near where other != cell {
                let q = PetalLayout.cellCenter(other)
                let distance = ((p.x - q.x) * (p.x - q.x) + (p.y - q.y) * (p.y - q.y)).squareRoot()
                XCTAssertGreaterThan(
                    distance,
                    PetalLayout.petalHalfLength + PetalLayout.petalRadius,
                    "cells \(cell) and \(other)"
                )
            }
        }
    }

    func testTheLightCellsDrawTheCharacter() {
        let light = (0..<PetalLayout.fieldCells).filter(PetalLayout.cellLight).count
        XCTAssertTrue((1200..<2400).contains(light), "\(light) light cells")
        XCTAssertTrue(PetalLayout.inMask(x: 512, y: 190), "the top bar")
        XCTAssertFalse(PetalLayout.inMask(x: 512, y: 350), "the gap under the second bar")
        XCTAssertFalse(PetalLayout.inMask(x: 181.9, y: 400), "left of the lattice")
        XCTAssertFalse(PetalLayout.inMask(x: 842, y: 400), "right of the lattice")
        XCTAssertFalse(PetalLayout.inMask(x: .nan, y: .nan))
        XCTAssertFalse(PetalLayout.inMask(x: .infinity, y: 400))
    }

    func testRingSlotsProvideExactlyTheLaneDCapacity() {
        let roles = PetalLayout.slotRoles
        XCTAssertEqual(roles.count, PetalLayout.totalSlots)
        XCTAssertEqual(PetalLayout.ringSlots.reduce(0, +), PetalLayout.totalSlots)
        let data = roles.filter { if case .data = $0 { return true } else { return false } }.count
        XCTAssertEqual(data, PetalLayout.dataBits)
        XCTAssertEqual(roles.filter { $0 == .gate }.count, 4 + 5 + 7)
        XCTAssertEqual(PetalLayout.dataSlots.count, PetalLayout.dataBits)
        XCTAssertEqual(roles.filter { $0 == .spare }.count, PetalLayout.totalSlots - PetalLayout.dataBits - 16 - 18)
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
    }

    /// `slot_center` of the release reference as `f32` bit patterns for slots
    /// (0, 5), (1, 30), (2, 77) and (2, 98): `f32::consts::TAU` is not
    /// `2 * Float.pi`, and the reference's fused sine and cosine move the last
    /// one by an ulp against separate `sinf` and `cosf`.
    private static let referenceSlotCenterBits: [[UInt32]] = [
        [0x445E_DDBC, 0x4423_621D],
        [0x43BC_0429, 0x4468_9DC0],
        [0x43C9_3A91, 0x4247_36F0],
        [0x4463_36D6, 0x437A_FBC0],
    ]

    func testSlotCentersSitOnTheirRings() {
        for (ring, slots) in PetalLayout.ringSlots.enumerated() {
            for slot in 0..<slots {
                let point = PetalLayout.slotCenter(ring: ring, slot: slot)
                let dx = point.x - PetalLayout.center
                let dy = point.y - PetalLayout.center
                XCTAssertEqual((dx * dx + dy * dy).squareRoot(), PetalLayout.ringRadii[ring], accuracy: 1e-3)
            }
        }
        XCTAssertEqual(PetalLayout.slotCenter(ring: 0, slot: 0), PetalPoint(x: 917, y: 512))
        let bits = [(0, 5), (1, 30), (2, 77), (2, 98)].map { ring, slot -> [UInt32] in
            let point = PetalLayout.slotCenter(ring: ring, slot: slot)
            return [Float(point.x).bitPattern, Float(point.y).bitPattern]
        }
        XCTAssertEqual(bits, Self.referenceSlotCenterBits)
        XCTAssertEqual(PetalLayout.splitSlot(88).ring, 1)
        XCTAssertEqual(PetalLayout.splitSlot(295).slot, 107)
    }

    func testFinderBlossomShape() {
        XCTAssertTrue(PetalLayout.finderLit(dx: 0, dy: 0))
        XCTAssertTrue(PetalLayout.finderLit(dx: 0, dy: -34), "upper petal centre")
        XCTAssertFalse(PetalLayout.finderLit(dx: 0, dy: -58), "notch at the upper petal tip")
        XCTAssertFalse(PetalLayout.finderLit(dx: 0, dy: 59), "gap between the two lower petals")
        XCTAssertFalse(PetalLayout.finderLit(dx: 70, dy: 0))
    }

    func testAPetalIsLitAtItsCentreAndNotchedAtItsTip() {
        XCTAssertTrue(PetalLayout.petalLit(dx: 0, dy: 0, angle: 0.3))
        let tip = PetalLayout.petalNotchDistance
        XCTAssertFalse(PetalLayout.petalLit(dx: tip * cos(0.3), dy: tip * sin(0.3), angle: 0.3))
        XCTAssertFalse(PetalLayout.petalLit(dx: 0, dy: 10, angle: 0))
        // half-length along the axis, half-width across it
        XCTAssertTrue(PetalLayout.petalLit(dx: 0, dy: 3.6, angle: 0))
        XCTAssertFalse(PetalLayout.petalLit(dx: 0, dy: 3.85, angle: 0))
        XCTAssertTrue(PetalLayout.petalLit(dx: 0, dy: 5.0, angle: Double.pi / 2))
        XCTAssertFalse(PetalLayout.petalLit(dx: 6.1, dy: 0, angle: 0))
        // the notch tells a petal from one turned by half a turn
        XCTAssertTrue(PetalLayout.petalLit(dx: 5.0, dy: 0, angle: 0))
        XCTAssertFalse(PetalLayout.petalLit(dx: 5.5, dy: 0, angle: 0))
        XCTAssertTrue(PetalLayout.petalLit(dx: -5.5, dy: 0, angle: 0))
        XCTAssertFalse(PetalLayout.petalLit(dx: -5.5, dy: 0, angle: Double.pi))
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

    func testFusedSineAndCosineFollowTheReference() {
        // the reference's optimiser computes a sine and a cosine of one angle in one call; at
        // the angle of field cell 708 that call's sine is an ulp below `sin`
        let angle = PetalLayout.cellAngle(708)
        XCTAssertEqual(angle.bitPattern, 0x3FE2_938D_3E48_A6E6)
        let (s, c) = PetalNumeric.sinCos(angle)
        XCTAssertEqual(s, sin(angle), accuracy: 2e-16)
        XCTAssertEqual(c, cos(angle), accuracy: 2e-16)
        #if canImport(Darwin)
        XCTAssertEqual(s.bitPattern, 0x3FE1_8CEA_F04E_4436)
        XCTAssertEqual(PetalLayout.cellCenter(708).y.bitPattern, 0x4083_1A28_86F9_D58E)
        #endif
        for k in 0..<64 {
            let a = Float(k) * 0.1
            let (sf, cf) = PetalNumeric.sinCos(a)
            XCTAssertEqual(sf, sin(a), accuracy: 1e-6)
            XCTAssertEqual(cf, cos(a), accuracy: 1e-6)
        }
    }
}

import XCTest
@testable import IrohaSwift

/// Ports of the `turns` unit tests of `crates/iroha_petal`: turn lane sizes
/// and ids, Gray labels, carrier orders, the turn lane codec and the carriers
/// that show it, the reader's tables, and the reader on rendered frames.
final class PetalTurnsTests: XCTestCase {
    private typealias Support = PetalTestSupport

    private func atoms(_ seed: UInt8) -> [UInt8] {
        (0..<UInt8(PetalTurns.atomCount * PetalLane.atomLength)).map { ($0 &* 29) ^ seed }
    }

    // MARK: - Lanes, ids and labels

    func testLaneSizesAreConsistent() {
        XCTAssertEqual(PetalTurns.wordLength, 90)
        XCTAssertEqual(PetalTurns.dataLength, 68)
        XCTAssertEqual(PetalTurns.dataLength, PetalTurns.atomCount * PetalLane.atomLength + PetalTurns.checkLength)
        XCTAssertEqual(1 << PetalTurns.bitsPerCarrier, PetalTurns.positions)
        XCTAssertLessThanOrEqual(PetalTurns.carriersPerSector, PetalLane.field(0).wordLength * 8)
        XCTAssertEqual(PetalTurns.turnFirstID(frame: 0, sector: 0), 1 << 31)
        XCTAssertEqual(PetalTurns.turnFirstID(frame: 1, sector: 3), (1 << 31) + 16 + 12)
        XCTAssertEqual(PetalTurns.turnFirstID(frame: .max, sector: 3) + 3, (1 << 31) + 16 * 65_535 + 15)
        // the base schedule stays far below the turn ids however long a stream runs
        XCTAssertLessThan(PetalStream.firstAtomID(frame: .max) + 21, PetalTurns.idBase)
    }

    func testGrayLabelsDifferInOneBitBetweenNeighbouringTurns() {
        for position in UInt8(0)..<UInt8(PetalTurns.positions) {
            XCTAssertEqual(PetalTurns.turnPosition(PetalTurns.turnLabel(position)), position)
            let next = (position + 1) % UInt8(PetalTurns.positions)
            XCTAssertEqual((PetalTurns.turnLabel(position) ^ PetalTurns.turnLabel(next)).nonzeroBitCount, 1)
        }
        XCTAssertEqual(PetalTurns.turnAngle(2), Double.pi / 2, accuracy: 1e-12)
    }

    func testCarrierOrdersShuffleEachSectorsCodewordCells() {
        let codeword = PetalLane.field(0).wordLength * 8
        for sector in 0..<PetalLayout.sectors {
            let order = PetalTurns.carrierOrder(sector)
            let cells = Array(PetalLayout.sectorCells(sector).prefix(codeword))
            XCTAssertNotEqual(order, cells, "sector \(sector)")
            XCTAssertEqual(order.sorted(), cells.sorted(), "sector \(sector)")
        }
        // the four shuffles differ
        func ranks(_ sector: Int) -> [Int?] {
            PetalTurns.carrierOrder(sector).map { PetalLayout.sectorCells(sector).firstIndex(of: $0) }
        }
        XCTAssertNotEqual(ranks(0), ranks(1))
    }

    func testASectorWithTooFewLitPetalsCarriesNothing() throws {
        let order = PetalTurns.carrierOrder(2)
        var petals = [Bool](repeating: false, count: PetalLayout.fieldCells)
        for cell in order.prefix(PetalTurns.carriersPerSector - 1) { petals[cell] = true }
        XCTAssertNil(PetalTurns.carriers(petals, sector: 2))
        petals[order[PetalTurns.carriersPerSector + 7]] = true
        let chosen = try XCTUnwrap(PetalTurns.carriers(petals, sector: 2), "enough lit petals")
        XCTAssertEqual(chosen.count, PetalTurns.carriersPerSector)
        XCTAssertEqual(chosen.last, order[PetalTurns.carriersPerSector + 7])
        // maps and sectors out of shape carry nothing
        XCTAssertNil(PetalTurns.carriers(Array(petals.prefix(100)), sector: 2))
        XCTAssertNil(PetalTurns.carriers(petals, sector: 4))
        XCTAssertNil(PetalTurns.carriers(petals, sector: -1))
    }

    func testTurnLanesRoundtripAndBindTheirHeader() throws {
        let header = PetalLaneHeader(tag: 7, frame: 300)
        let data = try PetalTurns.turnData(header, sector: 1, atoms: atoms(3))
        let word = try PetalTurns.encodeTurnLane(sector: 1, data: data)
        XCTAssertEqual(word.count, PetalTurns.wordLength)
        let (decoded, corrected) = try PetalTurns.decodeTurnLaneCounted(sector: 1, word)
        XCTAssertEqual(decoded, data)
        XCTAssertEqual(corrected, 0)
        let packet = try XCTUnwrap(PetalTurns.turnPacket(header, sector: 1, data: decoded), "check matches")
        XCTAssertEqual(packet.header, header)
        XCTAssertEqual(packet.firstID, PetalTurns.turnFirstID(frame: 300, sector: 1))
        XCTAssertEqual(packet.atoms.count, PetalTurns.atomCount)
        XCTAssertEqual(packet.atoms[2], Array(atoms(3)[32..<48]))
        // another frame, tag or sector does not match the check
        XCTAssertNil(PetalTurns.turnPacket(PetalLaneHeader(tag: 7, frame: 301), sector: 1, data: decoded))
        XCTAssertNil(PetalTurns.turnPacket(PetalLaneHeader(tag: 8, frame: 300), sector: 1, data: decoded))
        XCTAssertNil(PetalTurns.turnPacket(header, sector: 2, data: decoded))
        // a codeword whitened for another sector does not decode
        XCTAssertThrowsError(try PetalTurns.decodeTurnLaneCounted(sector: 0, word))
        // eleven wrong bytes are within the code's reach
        var damaged = word
        for position in [0, 9, 33, 44, 50, 61, 70, 80, 85, 88, 89] { damaged[position] ^= 0x5A }
        let repaired = try PetalTurns.decodeTurnLaneCounted(sector: 1, damaged)
        XCTAssertEqual(repaired.data, data)
        XCTAssertEqual(repaired.corrected, 11)
        // shapes the codec refuses
        XCTAssertThrowsError(try PetalTurns.turnData(header, sector: 1, atoms: [1, 2, 3])) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        XCTAssertThrowsError(try PetalTurns.encodeTurnLane(sector: 1, data: [1, 2, 3])) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        XCTAssertThrowsError(try PetalTurns.decodeTurnLaneCounted(sector: 1, Array(word.prefix(89)))) {
            XCTAssertEqual($0 as? PetalCodecError, .invalidShape)
        }
        XCTAssertNil(PetalTurns.turnPacket(header, sector: 1, data: Array(decoded.prefix(67))))
        XCTAssertNil(PetalTurns.turnPacket(header, sector: 4, data: decoded))
    }

    func testCarriersShowTheCodewordTwoBitsAtATime() throws {
        let fields = try (0..<PetalLayout.sectors).map { sector -> [UInt8] in
            let data = (0..<UInt8(PetalLane.field(0).dataLength)).map { ($0 &* 13) ^ UInt8(sector) }
            return try PetalLane.field(sector).encode(data)
        }
        var cells = try PetalFrameCells(d: [UInt8](repeating: 0, count: PetalLane.d.wordLength), fields: fields)
        XCTAssertTrue(cells.turns.allSatisfy { $0 == 0 })
        let words = try (0..<PetalLayout.sectors).map { sector in
            try PetalTurns.encodeTurnLane(
                sector: sector,
                data: try PetalTurns.turnData(PetalLaneHeader(tag: 1, frame: 2), sector: sector, atoms: atoms(UInt8(sector)))
            )
        }
        try cells.setTurns(words)
        for (sector, word) in words.enumerated() {
            XCTAssertEqual(cells.turnWord(sector: sector), word, "sector \(sector)")
            let chosen = try XCTUnwrap(PetalTurns.carriers(cells.petals, sector: sector))
            for (index, cell) in chosen.enumerated() {
                XCTAssertEqual(cells.turns[cell], PetalTurns.carrierPosition(word, index: index))
            }
        }
        // dark petals and lit petals beyond the carriers stay unturned
        var carried = 0
        for sector in 0..<PetalLayout.sectors {
            carried += try XCTUnwrap(PetalTurns.carriers(cells.petals, sector: sector)).filter { cells.turns[$0] != 0 }.count
        }
        let turned = cells.turns.filter { $0 != 0 }.count
        XCTAssertEqual(turned, carried)
        XCTAssertGreaterThan(turned, PetalLayout.sectors * PetalTurns.carriersPerSector * 6 / 10)
        // turning again replaces the earlier turns
        var again = cells
        try again.setTurns(words.reversed().map { $0 })
        try again.setTurns(words)
        XCTAssertEqual(again, cells)
        // words out of shape are refused
        XCTAssertThrowsError(try cells.setTurns(Array(words.prefix(3))))
        XCTAssertThrowsError(try cells.setTurns(words.map { Array($0.prefix(89)) }))
        XCTAssertNil(cells.turnWord(sector: 4))
    }

    // MARK: - The reader's tables

    func testATurnedPetalStaysOutOfItsNeighboursSamplingDiscs() {
        // the nearest petal centres (radially adjacent cells) are 11 units apart; a petal reaches
        // its half-length from its centre whichever way it is turned
        let tables = PetalTurns.Tables.shared
        var nearest = Double.greatestFiniteMagnitude
        for index in 0..<tables.nearCount {
            let along = tables.nearAlong[index]
            let across = tables.nearAcross[index]
            nearest = min(nearest, (along * along + across * across).squareRoot())
        }
        XCTAssertEqual(nearest, 11.0, accuracy: 1e-9)
        XCTAssertGreaterThan(nearest, PetalLayout.petalHalfLength + PetalLayout.petalRadius)
    }

    func testNeighboursAreEveryCellWithinNineteenUnitsInIndexOrder() {
        let tables = PetalTurns.Tables.shared
        XCTAssertEqual(tables.nearStart[PetalLayout.fieldCells], tables.nearCount)
        for cell in stride(from: 0, to: PetalLayout.fieldCells, by: 7) {
            let listed = (tables.nearStart[cell]..<tables.nearStart[cell + 1]).map { Int(tables.nearCell[$0]) }
            let c = PetalLayout.cellCenter(cell)
            let expected = (0..<PetalLayout.fieldCells).filter { other in
                let o = PetalLayout.cellCenter(other)
                return other != cell && (o.x - c.x) * (o.x - c.x) + (o.y - c.y) * (o.y - c.y) <= 361
            }
            XCTAssertEqual(listed, expected, "cell \(cell)")
        }
    }

    func testTemplatesHoldAUnitPetalAndKernelsTellTheTurnsApart() {
        func sharp(_ along: Double, _ across: Double) -> Double {
            PetalTurns.turnTemplate(blur: 0, along: along, across: across)
        }
        XCTAssertGreaterThan(sharp(0, 0), 0.99)
        XCTAssertTrue(sharp(0, 6) < 0.05 && sharp(8.5, 0) < 0.01)
        XCTAssertGreaterThan(sharp(4.5, 0), sharp(0, 3.4))
        // the notch: the tip of the petal is darker than its back
        XCTAssertLessThan(sharp(5.6, 0), sharp(-5.6, 0))
        XCTAssertEqual(sharp(9.5, 0), 0)
        XCTAssertEqual(PetalTurns.turnTemplate(blur: PetalTurns.blurs.count, along: 0, across: 0), 0)
        let points = PetalTurns.gridPoints
        for blur in PetalTurns.blurs.indices {
            let block = PetalTurns.Tables.shared.kernels + blur * PetalTurns.positions * points
            for k in 0..<PetalTurns.positions {
                // a window showing exactly the petal at turn k picks turn k
                let (position, lead) = PetalTurns.decide(block + k * points, block)
                XCTAssertEqual(Int(position), k, "blur \(blur)")
                XCTAssertGreaterThan(lead, 0, "blur \(blur) turn \(k)")
            }
        }
    }

    // MARK: - Reading rendered frames

    private func stream() throws -> PetalStreamEncoder {
        try PetalStreamEncoder(payload: Support.payload(6_000, seed: 5), kind: 2)
    }

    /// `frame` with its field lanes replaced.
    private func frame(_ decoded: PetalDecodedFrame, fields: [PetalLaneResult?]) -> PetalDecodedFrame {
        PetalDecodedFrame(
            pose: decoded.pose,
            rotation: decoded.rotation,
            mirrored: decoded.mirrored,
            d: decoded.d,
            fields: fields,
            inferredCorner: decoded.inferredCorner
        )
    }

    func testACleanRenderReadsEveryTurnLaneAndSkipsWhatItIsTold() throws {
        let encoder = try stream()
        let truth = encoder.laneData(frame: 6)
        let image = try Support.render(encoder, frame: 6, size: 768, supersample: 2)
        let decoded = try PetalDecoder.decode(image)
        XCTAssertEqual(decoded.laneLetters, "D0123")
        let turns = PetalTurns.readTurns(image, frame: decoded)
        for sector in 0..<PetalLayout.sectors {
            let result = try XCTUnwrap(turns[sector], "turn lane \(sector)")
            XCTAssertEqual(result.data, truth.turns[sector], "turn lane \(sector)")
            XCTAssertEqual(result.corrected, 0, "turn lane \(sector)")
        }
        let skipped = PetalTurns.readTurns(image, frame: decoded, skip: [true, false, true, true])
        XCTAssertEqual(skipped.map { $0 != nil }, [false, true, false, false])
        XCTAssertEqual(PetalTurns.readTurns(image, frame: decoded, skip: [true]).map { $0 != nil }, [false, true, true, true])
        // a sector whose field lane did not decode has no turn lane
        let partial = PetalTurns.readTurns(image, frame: frame(decoded, fields: [decoded.fields[0], decoded.fields[1], nil, decoded.fields[3]]))
        XCTAssertNil(partial[2])
        XCTAssertNotNil(partial[3])
        // the reader sees the drawn turn of every carrier
        let observed = try XCTUnwrap(PetalTurns.observedTurns(image, frame: decoded))
        let cells = encoder.cells(frame: 6)
        XCTAssertEqual(observed.carriers.count, PetalLayout.sectors * PetalTurns.carriersPerSector)
        XCTAssertLessThan(observed.blur, PetalTurns.blurs[PetalTurns.blurLimit])
        for carrier in observed.carriers {
            XCTAssertEqual(carrier.position, cells.turns[carrier.cell], "cell \(carrier.cell)")
            XCTAssertGreaterThan(carrier.lead, 0, "cell \(carrier.cell)")
        }
        // nothing decoded, nothing to read; lane data of another length names no lit petals
        let none = frame(decoded, fields: [nil, nil, nil, nil])
        XCTAssertNil(PetalTurns.observedTurns(image, frame: none))
        XCTAssertEqual(PetalTurns.readTurns(image, frame: none), [nil, nil, nil, nil])
        let short = PetalLaneResult(data: [decoded.fields[0]!.data[0], 0, 6], corrected: 0, erasures: 0)
        XCTAssertNil(PetalTurns.observedTurns(image, frame: frame(decoded, fields: [short, nil, nil, nil])))
        XCTAssertEqual(PetalTurns.readTurns(try PetalLuma(width: 0, height: 0), frame: decoded).compactMap { $0 }, [])
    }

    /// A 1280×720 capture of a 1024-pixel render of `frame`, turned by
    /// `rotation` degrees and tilted by `tilt` degrees about the horizontal
    /// axis (a homography; bilinear resampling, 2 × 2 samples per pixel), as
    /// large as fits the frame with a 4 px margin, at most 85 % of its height.
    /// The crate's camera simulator is not ported; this stands in for its
    /// sharp modern camera, without noise or lens effects.
    private func capture(_ encoder: PetalStreamEncoder, _ frame: UInt16, rotation: Double, tilt: Double) throws -> PetalLuma {
        let source = try Support.render(encoder, frame: frame, size: 1024, supersample: 2)
        let theta = rotation * Double.pi / 180
        let phi = tilt * Double.pi / 180
        let focal = 1_000.0
        let canvas = [(0.0, 0.0), (1024.0, 0.0), (1024.0, 1024.0), (0.0, 1024.0)]
        func corners(_ side: Double) -> [PetalPoint] {
            canvas.map { corner -> PetalPoint in
                let x = (corner.0 - 512) * side / 1024
                let y = (corner.1 - 512) * side / 1024
                let rx = x * cos(theta) - y * sin(theta)
                let ry = x * sin(theta) + y * cos(theta)
                let depth = focal + ry * sin(phi)
                return PetalPoint(x: 640 + focal * rx / depth, y: 360 + focal * ry * cos(phi) / depth)
            }
        }
        var side = 0.85 * 720
        while !corners(side).allSatisfy({ $0.x >= 4 && $0.y >= 4 && $0.x <= 1276 && $0.y <= 716 }) {
            side *= 0.97
        }
        let forward = try XCTUnwrap(PetalHomography.fit(
            from: canvas.map { PetalPoint(x: $0.0, y: $0.1) },
            to: corners(side)
        ))
        let inverse = try XCTUnwrap(forward.inverse())
        var pixels = [UInt8](repeating: 12, count: 1280 * 720)
        for y in 0..<720 {
            for x in 0..<1280 {
                var sum = 0.0
                for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let point = inverse.apply(PetalPoint(x: Double(x) + dx, y: Double(y) + dy))
                    let inside = point.x >= 0 && point.y >= 0 && point.x < 1024 && point.y < 1024
                    sum += inside ? source.sample(x: point.x, y: point.y) : 12
                }
                pixels[y * 1280 + x] = UInt8((sum / 4).rounded())
            }
        }
        return try PetalLuma(width: 1280, height: 720, pixels: pixels)
    }

    func testATurnedAndTiltedCaptureReadsTheTurnLanes() throws {
        let encoder = try stream()
        let truth = encoder.laneData(frame: 9)
        for (rotation, tilt) in [(17.0, 8.0), (205.0, -8.0)] {
            let image = try capture(encoder, 9, rotation: rotation, tilt: tilt)
            let decoded = try PetalDecoder.decode(image)
            XCTAssertEqual(decoded.laneLetters, "D0123", "rotation \(rotation)")
            let turns = PetalTurns.readTurns(image, frame: decoded)
            for (sector, field) in decoded.fields.enumerated() where field != nil {
                XCTAssertEqual(turns[sector]?.data, truth.turns[sector], "rotation \(rotation) sector \(sector)")
            }
        }
    }

    func testBrokenPosesAndOtherImagesNeverCrashTheReaderOrPass() throws {
        let encoder = try stream()
        let image = try Support.render(encoder, frame: 2, size: 512, supersample: 2)
        let decoded = try PetalDecoder.decode(image)
        var rng = PetalXorshift32(seed: 77)
        let noise = try PetalLuma(width: 512, height: 512, pixels: (0..<(512 * 512)).map { _ in rng.nextByte() })
        let small = try PetalLuma(width: 64, height: 48, pixels: [UInt8](repeating: 200, count: 64 * 48))
        for other in [noise, small] {
            XCTAssertEqual(PetalTurns.readTurns(other, frame: decoded), [nil, nil, nil, nil])
            XCTAssertNotNil(PetalTurns.observedTurns(other, frame: decoded))
        }
        let broken = [
            [Double](repeating: .nan, count: 9),
            [Double.infinity, 0, 0, 0, 1, 0, 0, 0, 1],
            [0, 0, 0, 0, 0, 0, 0, 0, 0],
            [1e12, 0, 0, 0, 1e12, 0, 0, 0, 1],
        ]
        for elements in broken {
            let pose = PetalPose.plain(try PetalHomography(elements: elements))
            let frame = PetalDecodedFrame(
                pose: pose,
                rotation: decoded.rotation,
                mirrored: decoded.mirrored,
                d: decoded.d,
                fields: decoded.fields,
                inferredCorner: nil
            )
            XCTAssertEqual(PetalTurns.readTurns(image, frame: frame), [nil, nil, nil, nil], "\(elements)")
            XCTAssertEqual(
                PetalTurns.observedTurns(image, frame: frame)?.carriers.count,
                PetalLayout.sectors * PetalTurns.carriersPerSector,
                "\(elements)"
            )
        }
    }

    func testReadersOnSeveralThreadsAgree() throws {
        let encoder = try stream()
        let image = try Support.render(encoder, frame: 6, size: 768, supersample: 2)
        let decoded = try PetalDecoder.decode(image)
        let expected = PetalTurns.readTurns(image, frame: decoded)
        XCTAssertFalse(expected.contains { $0 == nil })
        let results = UnsafeMutableBufferPointer<[PetalLaneResult?]>.allocate(capacity: 6)
        _ = results.initialize(from: repeatElement([], count: results.count))
        defer {
            _ = results.deinitialize()
            results.deallocate()
        }
        DispatchQueue.concurrentPerform(iterations: results.count) { index in
            results[index] = PetalTurns.readTurns(image, frame: decoded)
        }
        for result in results { XCTAssertEqual(result, expected) }
    }

    func testUnturnedPetalsNeverPassForATurnLane() throws {
        let encoder = try stream()
        let words = encoder.words(frame: 3)
        let cells = try PetalFrameCells(d: words.d, fields: words.fields)
        XCTAssertTrue(cells.turns.allSatisfy { $0 == 0 })
        let image = try PetalRenderer.renderSquare(cells, options: PetalRenderOptions(size: 768, supersample: 2)).luma()
        let decoded = try PetalDecoder.decode(image)
        XCTAssertEqual(decoded.laneLetters, "D0123", "the field lanes read without turns")
        XCTAssertTrue(PetalTurns.readTurns(image, frame: decoded).allSatisfy { $0 == nil })
    }
}

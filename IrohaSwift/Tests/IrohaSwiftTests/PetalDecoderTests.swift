import XCTest
@testable import IrohaSwift

/// Ports of the `decode` unit tests and `tests/captures.rs` of
/// `crates/iroha_petal`, plus the golden capture conformance suite.
final class PetalDecoderTests: XCTestCase {
    private typealias Support = PetalTestSupport

    private static let setupPayload: [UInt8] = (0..<300).map {
        UInt8(truncatingIfNeeded: (UInt32($0) &* 2_654_435_761) >> 11)
    }

    private func setup(_ frame: UInt16) throws -> (PetalStreamEncoder, PetalLuma) {
        let encoder = try PetalStreamEncoder(payload: Self.setupPayload, kind: 2)
        return (encoder, try Support.render(encoder, frame: frame, size: 768, supersample: 2))
    }

    // MARK: - Rendered frames

    func testCleanRenderDecodesEveryLane() throws {
        let (encoder, image) = try setup(5)
        let decoded = try PetalDecoder.decode(image)
        let expected = encoder.laneData(frame: 5)
        XCTAssertEqual(decoded.p?.data, expected.p)
        XCTAssertEqual(decoded.k?.data, expected.k)
        XCTAssertEqual(decoded.d?.data, expected.d)
        XCTAssertEqual(decoded.rotation, 0)
        XCTAssertFalse(decoded.mirrored)
        XCTAssertEqual(decoded.lanesOK, 3)
        XCTAssertEqual(decoded.laneLetters, "PKD")
        XCTAssertNil(decoded.beacon, "frame 5 carries an atom in lane D")
        XCTAssertEqual(decoded.atomPackets.map(\.atoms.count), [1, 5, 1])
        XCTAssertEqual(decoded.atomPackets.map(\.firstID), [33, 35, 34])

        // diagnostics agree with what was drawn
        XCTAssertEqual(PetalDecoder.observedCells(image, frame: decoded), encoder.cells(frame: 5))
        let error = try XCTUnwrap(PetalDecoder.tileMatchError(image, frame: decoded))
        XCTAssertLessThan(error, 1.0)
    }

    func testRotatedAndMirroredRendersDecodeWithTheRightOrientation() throws {
        let (encoder, image) = try setup(7)
        let expected = encoder.laneData(frame: 7)
        let n = image.width
        let cases: [(String, PetalLuma, Bool)] = [
            ("rot90", try Support.transform(image) { x, y in (y, n - 1 - x) }, false),
            ("rot180", try Support.transform(image) { x, y in (n - 1 - x, n - 1 - y) }, false),
            ("rot270", try Support.transform(image) { x, y in (n - 1 - y, x) }, false),
            ("mirror", try Support.transform(image) { x, y in (n - 1 - x, y) }, true),
            ("mirror-rot90", try Support.transform(image) { x, y in (y, x) }, true),
        ]
        var rotations: [String: Int] = [:]
        for (name, transformed, mirrored) in cases {
            let decoded = try PetalDecoder.decode(transformed)
            XCTAssertEqual(decoded.mirrored, mirrored, name)
            XCTAssertEqual(decoded.d?.data, expected.d, "\(name) lane D")
            XCTAssertEqual(decoded.p?.data, expected.p, "\(name) lane P")
            XCTAssertEqual(decoded.k?.data, expected.k, "\(name) lane K")
            rotations[name] = decoded.rotation
        }
        // `rotation` names the image corner (clockwise from top-left) that
        // holds the code's top-left finder.
        XCTAssertEqual(rotations, ["rot90": 1, "rot180": 2, "rot270": 3, "mirror": 1, "mirror-rot90": 0])

        // without mirror hypotheses a mirrored frame is never misread
        let mirrored = try Support.mirror(image)
        let strict = PetalDecodeOptions(tryMirrored: false)
        if let decoded = try? PetalDecoder.decode(mirrored, options: strict) {
            XCTAssertFalse(decoded.mirrored)
            if let p = decoded.p { XCTAssertEqual(p.data, expected.p) }
            if let k = decoded.k { XCTAssertEqual(k.data, expected.k) }
            if let d = decoded.d { XCTAssertEqual(d.data, expected.d) }
        }
    }

    func testDecodeWithAKnownHomographyReadsEveryLane() throws {
        let (encoder, image) = try setup(3)
        let scale = Double(image.width) / 1024
        let pose = try PetalHomography(elements: [scale, 0, 0, 0, scale, 0, 0, 0, 1])
        let decoded = try XCTUnwrap(PetalDecoder.decode(image, homography: pose))
        let expected = encoder.laneData(frame: 3)
        XCTAssertEqual(decoded.p?.data, expected.p)
        XCTAssertEqual(decoded.k?.data, expected.k)
        XCTAssertEqual(decoded.d?.data, expected.d)
        XCTAssertNil(PetalDecoder.decode(try PetalLuma(width: 768, height: 768), homography: pose))
    }

    func testCorrectedCountsRewrittenBytesNotJustErasures() throws {
        let data = (0..<19).map { UInt8($0) }
        var word = try PetalLane.p.encode(data)
        for position in [2, 11, 30] { word[position] ^= 0x5A }
        let confidence = [Double](repeating: 1, count: word.count)
        let result = try XCTUnwrap(PetalDecoder.decodeWithErasures(.p, word, confidence), "three errors fit")
        XCTAssertEqual(result.data, data)
        XCTAssertEqual(result.erasures, 0)
        XCTAssertEqual(result.corrected, 3)
        // with the damaged bytes flagged as least confident, they become erasures
        var flagged = [Double](repeating: 1, count: word.count)
        for position in [2, 11, 30] { flagged[position] = 0 }
        let erased = try XCTUnwrap(PetalDecoder.decodeWithErasures(.p, word, flagged))
        XCTAssertEqual(erased.data, data)
        XCTAssertGreaterThanOrEqual(erased.corrected, 3)
    }

    func testRandomWordsAreAlmostNeverAccepted() {
        // Reed–Solomon with erasures can accept a word that is not a transmission. Lane D has only
        // 11 parity bytes, so its schedule stops at five erasures; at seven it let through about one
        // random word in 250 (150 of these 40 000). Lane P stops at six for the same reason. The counts
        // are exact: the same xorshift32 words and byte-valued confidences (many ties, so the ranking
        // must be stable) reproduce the reference.
        var rng = PetalXorshift32(seed: 0x5EED)
        let trials = 40_000
        for (lane, expected) in [(PetalLane.d, 3), (PetalLane.p, 0)] {
            let length = lane.dataLength + lane.parityLength
            var accepted = 0
            for _ in 0..<trials {
                let word = (0..<length).map { _ in rng.nextByte() }
                let confidence = (0..<length).map { _ in Double(rng.nextByte()) }
                if PetalDecoder.decodeWithErasures(lane, word, confidence) != nil { accepted += 1 }
            }
            XCTAssertEqual(accepted, expected, "lane \(lane.letter) of \(trials) random words")
        }
    }

    func testOnlyLaneKUsesTwoThirdsOfItsParityAsErasures() throws {
        // damaged bytes: `flagged` of them marked least confident, two more hidden. With the extra
        // erasure step of the old schedule the decoder would repair them (2·2 + flagged parity
        // bytes); the capped schedule must refuse instead of risking a wrong codeword.
        for (lane, flagged) in [(PetalLane.d, 7), (PetalLane.p, 8)] {
            let data = (0..<lane.dataLength).map { UInt8($0) }
            let word = try lane.encode(data)
            var damaged = word
            var confidence = [Double](repeating: 1, count: word.count)
            for position in 0..<flagged {
                damaged[position] ^= 0xA5
                confidence[position] = 0
            }
            damaged[20] ^= 0x3C
            damaged[21] ^= 0x3C
            XCTAssertNil(PetalDecoder.decodeWithErasures(lane, damaged, confidence), "lane \(lane.letter)")
            // half the parity flagged plus one hidden error stays comfortably repairable
            damaged = word
            confidence = [Double](repeating: 1, count: word.count)
            for position in 0..<(lane.parityLength / 2) {
                damaged[position] ^= 0xA5
                confidence[position] = 0
            }
            damaged[20] ^= 0x3C
            let result = try XCTUnwrap(
                PetalDecoder.decodeWithErasures(lane, damaged, confidence),
                "lane \(lane.letter)"
            )
            XCTAssertEqual(result.data, data, "lane \(lane.letter)")
            XCTAssertLessThanOrEqual(result.erasures, lane.parityLength / 2, "lane \(lane.letter)")
        }
        // lane K keeps the two-thirds step: 30 flagged bytes plus 7 hidden errors need it
        // (2·7 + 30 = 44 of 45 parity bytes)
        let data = (0..<PetalLane.k.dataLength).map { UInt8(truncatingIfNeeded: $0) }
        var damaged = try PetalLane.k.encode(data)
        var confidence = [Double](repeating: 1, count: damaged.count)
        for position in 0..<30 {
            damaged[position] ^= 0xA5
            confidence[position] = 0
        }
        for position in 60..<67 { damaged[position] ^= 0x3C }
        let result = try XCTUnwrap(PetalDecoder.decodeWithErasures(.k, damaged, confidence), "30 erasures")
        XCTAssertEqual(result.data, data)
        XCTAssertEqual(result.erasures, 30)
    }

    func testEqualConfidencesAreErasedInPositionOrder() throws {
        // Every tile the normalised read erases has confidence exactly 0, so ties are the rule, and
        // the ranking must be stable or ports disagree about which bytes are erased. Three damaged
        // bytes at the front plus four hidden ones fit lane D only if exactly the first three
        // positions are erased (3 erasures + 4 errors = all 11 parity bytes): a step that erased
        // the last positions instead would see seven errors.
        let data = (0..<PetalLane.d.dataLength).map { UInt8($0) }
        var word = try PetalLane.d.encode(data)
        for position in Array(0..<3) + Array(20..<24) { word[position] ^= 0x5A }
        let confidence = [Double](repeating: 1, count: word.count)
        let result = try XCTUnwrap(PetalDecoder.decodeWithErasures(.d, word, confidence), "ties in order")
        XCTAssertEqual(result.data, data)
        XCTAssertEqual(result.erasures, 3)
        XCTAssertEqual(result.corrected, 7)
    }

    // MARK: - Level read and normalised read

    /// The exact canvas-to-pixel homography of the 768-pixel test renders.
    private func renderHomography() throws -> PetalHomography {
        let canonical = PetalLayout.finderCenters
        let scale = 768.0 / 1024.0
        let pixels = canonical.map { PetalPoint(x: $0.x * scale, y: $0.y * scale) }
        return try XCTUnwrap(PetalHomography.fit(from: canonical, to: pixels))
    }

    /// A clean 768-pixel render with the exact canvas-to-pixel homography and
    /// its raw patches.
    private func cleanPatches(
        _ frame: UInt16
    ) throws -> (encoder: PetalStreamEncoder, image: PetalLuma, homography: PetalHomography, patches: [Double]) {
        let (encoder, image) = try setup(frame)
        let h = try renderHomography()
        return (encoder, image, h, image.withView { PetalDecoder.samplePatches($0, h) })
    }

    /// Lane `P` and lane `K` as the given tile reads deliver them.
    private func tileLanes(_ reads: [PetalDecoder.TileRead]) -> (p: PetalLaneResult?, k: PetalLaneResult?) {
        let words = PetalDecoder.tileWords(reads)
        return (
            PetalDecoder.decodeWithErasures(.p, words.p, words.pConfidence),
            PetalDecoder.decodeWithErasures(.k, words.k, words.kConfidence)
        )
    }

    func testLevelAndNormalisedReadsAgreeOnACleanRender() throws {
        let (encoder, image, h, patches) = try cleanPatches(5)
        let expected = encoder.laneData(frame: 5)
        let sigmas = PetalDecodeOptions().templateSigmas
        let reference = try XCTUnwrap(image.withView { PetalDecoder.referenceLevels($0, h, inferred: nil) })
        for (name, reads) in [
            ("level", PetalDecoder.readTiles(patches, reference, sigmas)),
            ("normalised", PetalDecoder.readTilesNormalised(patches, sigmas)),
        ] {
            let lanes = tileLanes(reads)
            let p = try XCTUnwrap(lanes.p, "\(name) lane P")
            let k = try XCTUnwrap(lanes.k, "\(name) lane K")
            XCTAssertEqual(p.data, expected.p, name)
            XCTAssertEqual(p.corrected, 0, name)
            XCTAssertEqual(k.data, expected.k, name)
            XCTAssertEqual(k.corrected, 0, name)
        }
    }

    func testNormalisedReadCancelsGainAndOffsetPerTile() throws {
        let (encoder, image, h, patches) = try cleanPatches(5)
        let expected = encoder.laneData(frame: 5)
        let sigmas = PetalDecodeOptions().templateSigmas
        // every tile gets its own gain and offset, as under glare, shadows and saturation
        var distorted = patches
        for tile in 0..<PetalLayout.tileCount {
            let gain = 0.35 + 0.65 * (Double(tile * 37 % 101) / 100.0)
            let offset = 5.0 + Double(tile * 53 % 61)
            for index in tile * PetalDecoder.cells..<(tile + 1) * PetalDecoder.cells {
                distorted[index] = gain * patches[index] + offset
            }
        }
        let reference = try XCTUnwrap(image.withView { PetalDecoder.referenceLevels($0, h, inferred: nil) })
        XCTAssertNil(
            tileLanes(PetalDecoder.readTiles(distorted, reference, sigmas)).p,
            "the level read must not survive this distortion, or the test proves nothing"
        )
        let lanes = tileLanes(PetalDecoder.readTilesNormalised(distorted, sigmas))
        XCTAssertEqual(lanes.p?.data, expected.p)
        XCTAssertEqual(lanes.k?.data, expected.k)
    }

    func testNormalisedReadErasesTilesThatLostTheirContrast() throws {
        var patches = try cleanPatches(5).patches
        let cells = PetalDecoder.cells
        for index in 5 * cells..<6 * cells { patches[index] = 100 }
        for index in 9 * cells..<10 * cells { patches[index] = 30 }
        let reads = PetalDecoder.readTilesNormalised(patches, PetalDecodeOptions().templateSigmas)
        for tile in [5, 9] {
            XCTAssertLessThan(abs(reads[tile].polarityMargin), 1e-12, "tile \(tile)")
            XCTAssertLessThan(abs(reads[tile].glyphMargin), 1e-12, "tile \(tile)")
        }
        XCTAssertGreaterThan(reads[6].polarityMargin, 0)
        XCTAssertGreaterThan(reads[6].glyphMargin, 0)
    }

    func testPatchLevelsIgnoreTheExtremeCells() {
        var values = [Double](repeating: 10, count: PetalDecoder.cells)
        for index in 0..<PetalDecoder.cells / 2 { values[index] = 200 + Double(index % 3) }
        values[0] = 255 // one hot cell
        values[PetalDecoder.cells - 1] = 0 // one dead cell
        let (low, high) = values.withUnsafeBufferPointer { PetalDecoder.patchLevels($0) }
        XCTAssertEqual(low, 10, accuracy: 1e-12)
        XCTAssertTrue((200.0...202.0).contains(high))
        var scaled = values
        scaled.withUnsafeMutableBufferPointer { PetalDecoder.rescale($0, floor: 1.0) }
        XCTAssertTrue(scaled.allSatisfy { (-0.25...1.25).contains($0) })
        // a flat patch stays flat instead of dividing by nothing
        var flat = [Double](repeating: 7, count: PetalDecoder.cells)
        flat.withUnsafeMutableBufferPointer { PetalDecoder.rescale($0, floor: 1.0) }
        XCTAssertTrue(flat.allSatisfy { abs($0) < 1e-12 })
    }

    func testShadowedPartOfARenderDecodesThroughTheNormalisedRead() throws {
        let (encoder, image) = try setup(5)
        let expected = encoder.laneData(frame: 5)
        var pixels = image.pixels
        let width = image.width
        for row in 0..<image.height {
            for x in (width * 7 / 20)..<(width * 3 / 5) {
                pixels[row * width + x] = UInt8((Double(pixels[row * width + x]) * 0.3).rounded())
            }
        }
        let shadowed = try PetalLuma(width: width, height: image.height, pixels: pixels)
        // the finder levels cannot describe a step in the light: the level read loses lane K
        let h = try renderHomography()
        let sigmas = PetalDecodeOptions().templateSigmas
        let reference = try XCTUnwrap(shadowed.withView { PetalDecoder.referenceLevels($0, h, inferred: nil) })
        let patches = shadowed.withView { PetalDecoder.samplePatches($0, h) }
        let levelLanes = tileLanes(PetalDecoder.readTiles(patches, reference, sigmas))
        XCTAssertNil(levelLanes.k)
        let decoded = try PetalDecoder.decode(shadowed)
        XCTAssertEqual(decoded.p?.data, expected.p)
        XCTAssertEqual(decoded.k?.data, expected.k)

        // `readTileLanes` keeps what the level read delivered and only fills in what is missing
        let combined = PetalDecoder.readTileLanes(patches, reference, sigmas)
        if let levelP = levelLanes.p { XCTAssertEqual(combined.p, levelP) }
        XCTAssertEqual(combined.k?.data, expected.k)
        XCTAssertEqual(combined.p?.data, expected.p)
    }

    func testCleanFramesAreReadByTheLevelReadAlone() throws {
        let (encoder, image, h, patches) = try cleanPatches(5)
        let expected = encoder.laneData(frame: 5)
        let sigmas = PetalDecodeOptions().templateSigmas
        let reference = try XCTUnwrap(image.withView { PetalDecoder.referenceLevels($0, h, inferred: nil) })
        let level = tileLanes(PetalDecoder.readTiles(patches, reference, sigmas))
        let combined = PetalDecoder.readTileLanes(patches, reference, sigmas)
        XCTAssertEqual(level.p?.data, expected.p)
        XCTAssertEqual(level.k?.data, expected.k)
        XCTAssertEqual(combined.p, level.p)
        XCTAssertEqual(combined.k, level.k)
    }

    func testLaneDIsReadFromTheRingsUnderAKnownPose() throws {
        let (encoder, image, h, _) = try cleanPatches(5)
        let reference = try XCTUnwrap(image.withView { PetalDecoder.referenceLevels($0, h, inferred: nil) })
        let lane = try XCTUnwrap(image.withView { PetalDecoder.readLaneD($0, h, reference) })
        XCTAssertEqual(lane.data, encoder.laneData(frame: 5).d)
        XCTAssertEqual(lane.corrected, 0)
        // a mirrored image under the unmirrored pose reads the rings back to front
        let mirrored = try Support.mirror(image)
        let mirroredReference = try XCTUnwrap(mirrored.withView { PetalDecoder.referenceLevels($0, h, inferred: nil) })
        XCTAssertNil(mirrored.withView { PetalDecoder.readLaneD($0, h, mirroredReference) })
    }

    func testPatchLevelsOrderDamagedCellsLikeTheReference() {
        // positive NaN sorts above every number (IEEE total order), so up to
        // `patchCut` damaged cells cannot move the bright level
        var values = (0..<PetalDecoder.cells).map { Double($0) }
        for index in 0..<PetalDecoder.patchCut { values[index * 9] = .nan }
        var levels = values.withUnsafeBufferPointer { PetalDecoder.patchLevels($0) }
        XCTAssertTrue(levels.low.isFinite)
        XCTAssertTrue(levels.high.isFinite)
        values[PetalDecoder.cells - 1] = .nan // one more than the cut discards
        levels = values.withUnsafeBufferPointer { PetalDecoder.patchLevels($0) }
        XCTAssertTrue(levels.high.isNaN)
        XCTAssertTrue(levels.low.isFinite)
    }

    func testDegeneratePosesNeverCrashOrDecode() throws {
        let (_, image) = try setup(5)
        let poses: [[Double]] = [
            [Double.nan, 0, 0, 0, Double.nan, 0, 0, 0, 1],
            [0, 0, 0, 0, 0, 0, 0, 0, 0],
            [Double.infinity, 0, 0, 0, Double.infinity, 0, 0, 0, 1],
            [1e-12, 0, 0, 0, 1e-12, 0, 0, 0, 1],
        ]
        for elements in poses {
            let pose = try PetalHomography(elements: elements)
            if let frame = PetalDecoder.decode(image, homography: pose) {
                XCTAssertEqual(frame.lanesOK, 0, "pose \(elements)")
            }
        }
    }

    func testBlankFramesReportNoFinders() throws {
        XCTAssertThrowsError(try PetalDecoder.decode(try PetalLuma(width: 320, height: 240))) {
            XCTAssertEqual($0 as? PetalDecodeError, .noFinders)
        }
    }

    func testUnusableSizesAreRejectedWithoutWork() throws {
        for (width, height) in [(1, 1), (47, 400), (0, 0), (400, 47)] {
            XCTAssertThrowsError(try PetalDecoder.decode(try PetalLuma(width: width, height: height))) {
                XCTAssertEqual($0 as? PetalDecodeError, .unsupportedImage)
            }
        }
        let smallBudget = PetalDecodeOptions(maximumPixels: 1_000)
        XCTAssertThrowsError(try PetalDecoder.decode(try PetalLuma(width: 100, height: 100), options: smallBudget)) {
            XCTAssertEqual($0 as? PetalDecodeError, .unsupportedImage)
        }
        // one pixel beyond the default 12-megapixel budget
        let huge = try PetalLuma(width: 3_465, height: 3_464)
        XCTAssertThrowsError(try PetalDecoder.decode(huge)) {
            XCTAssertEqual($0 as? PetalDecodeError, .unsupportedImage)
        }
        XCTAssertNil(PetalDecoder.decode(try PetalLuma(width: 8, height: 8), homography: .identity))
        // a buffer that does not match the stated size cannot be built, so it never reaches the sampler
        XCTAssertThrowsError(try PetalLuma(width: 100, height: 100, pixels: [UInt8](repeating: 0, count: 5))) {
            XCTAssertEqual($0 as? PetalImageError, .invalidDimensions)
        }
        // the diagnostics refuse the images `decode` refuses
        let (_, rendered) = try setup(5)
        let decoded = try PetalDecoder.decode(rendered)
        let tiny = try PetalLuma(width: 8, height: 8)
        XCTAssertNil(PetalDecoder.observedCells(tiny, frame: decoded))
        XCTAssertNil(PetalDecoder.tileMatchError(tiny, frame: decoded))
        XCTAssertNil(PetalDecoder.observedCells(rendered, frame: decoded, options: smallBudget))
        XCTAssertNil(PetalDecoder.tileMatchError(rendered, frame: decoded, options: smallBudget))
        XCTAssertNotNil(PetalDecoder.observedCells(rendered, frame: decoded))
        XCTAssertNotNil(PetalDecoder.tileMatchError(rendered, frame: decoded))
    }

    func testGarbageImagesNeverPanicOrDecode() throws {
        var rng = PetalXorshift32(seed: 99)
        for (width, height) in [(64, 48), (257, 129), (320, 240), (480, 480)] {
            for style in 0..<4 {
                let pixels: [UInt8] = (0..<(width * height)).map { i in
                    switch style {
                    case 0: return rng.nextByte() // white noise
                    case 1: return UInt8((i % width) * 255 / width) // gradient
                    case 2: return (i / width / 8 + i % width / 8) % 2 == 0 ? 230 : 20 // checkerboard
                    default: return rng.nextUInt32() % 50 == 0 ? 255 : 0 // sparse specks
                    }
                }
                let image = try PetalLuma(width: width, height: height, pixels: pixels)
                XCTAssertThrowsError(try PetalDecoder.decode(image), "\(width)x\(height) style \(style)")
            }
        }
        // extreme aspect ratios, saturated and vertical-gradient frames
        for (width, height, fill) in [(48, 2_000, 255), (2_000, 48, 128), (48, 48, 0), (300, 200, 7)] {
            let pixels = (0..<(width * height)).map { i -> UInt8 in
                fill == 7 ? UInt8(i / width * 255 / height) : UInt8(fill)
            }
            let image = try PetalLuma(width: width, height: height, pixels: pixels)
            XCTAssertThrowsError(try PetalDecoder.decode(image), "\(width)x\(height) fill \(fill)")
        }
    }

    func testRandomBlobScenesNeverPanic() throws {
        // Scenes with several random bright ellipses (some finder-sized) on noise:
        // exercises the locator, quad selection and homography on degenerate layouts.
        var rng = PetalXorshift32(seed: 2024)
        for scene in 0..<60 {
            let width = 160 + Int(rng.nextUInt32() % 400)
            let height = 120 + Int(rng.nextUInt32() % 300)
            var pixels = (0..<(width * height)).map { _ in UInt8(rng.nextUInt32() % 40) }
            let blobs = 3 + rng.nextUInt32() % 8
            for _ in 0..<blobs {
                let cx = Double(Int(rng.nextUInt32()) % width)
                let cy = Double(Int(rng.nextUInt32()) % height)
                let rx = 6.0 + Double(rng.nextUInt32() % 40)
                let ry = 6.0 + Double(rng.nextUInt32() % 40)
                // only the ellipse's bounding box can pass the inside test
                let x0 = max(0, Int((cx - rx).rounded(.down)))
                let x1 = min(width - 1, Int((cx + rx).rounded(.up)))
                let y0 = max(0, Int((cy - ry).rounded(.down)))
                let y1 = min(height - 1, Int((cy + ry).rounded(.up)))
                guard x0 <= x1, y0 <= y1 else { continue }
                for y in y0...y1 {
                    for x in x0...x1 {
                        let dx = (Double(x) - cx) / rx
                        let dy = (Double(y) - cy) / ry
                        if dx * dx + dy * dy <= 1.0 { pixels[y * width + x] = 230 }
                    }
                }
            }
            let image = try PetalLuma(width: width, height: height, pixels: pixels)
            // must not crash; a lucky layout may locate finders but cannot yield lanes
            if let frame = try? PetalDecoder.decode(image) {
                XCTAssertEqual(frame.lanesOK, 0, "scene \(scene) produced lane data from blobs")
            }
        }
    }

    // MARK: - Hidden blossoms, the 天 and tracking

    /// A 768-pixel render of `frame` with the blossom of canonical corner
    /// `corner` painted over with background.
    private func hiddenBlossom(_ frame: UInt16, corner: Int) throws -> (PetalStreamEncoder, PetalLuma) {
        let (encoder, image) = try setup(frame)
        let n = image.width
        let scale = Double(n) / 1024
        let center = PetalLayout.finderCenters[corner]
        let cx = center.x * scale
        let cy = center.y * scale
        let radius = 75 * scale
        var pixels = image.pixels
        for y in 0..<n {
            for x in 0..<n {
                let dx = Double(x) + 0.5 - cx
                let dy = Double(y) + 0.5 - cy
                if dx * dx + dy * dy <= radius * radius { pixels[y * n + x] = 0 }
            }
        }
        return (encoder, try PetalLuma(width: n, height: n, pixels: pixels))
    }

    /// Shifts a luma image by whole pixels, filling with black.
    private func shifted(_ image: PetalLuma, _ dx: Int, _ dy: Int) throws -> PetalLuma {
        var pixels = [UInt8](repeating: 0, count: image.pixels.count)
        for y in 0..<image.height {
            for x in 0..<image.width {
                let sx = x - dx
                let sy = y - dy
                if sx >= 0, sy >= 0, sx < image.width, sy < image.height {
                    pixels[y * image.width + x] = image.pixels[sy * image.width + sx]
                }
            }
        }
        return try PetalLuma(width: image.width, height: image.height, pixels: pixels)
    }

    /// Places a luma image in the middle of a larger black frame.
    private func padded(_ image: PetalLuma, _ pad: Int) throws -> PetalLuma {
        let width = image.width + 2 * pad
        var pixels = [UInt8](repeating: 0, count: width * (image.height + 2 * pad))
        for y in 0..<image.height {
            let start = (y + pad) * width + pad
            pixels.replaceSubrange(start..<start + image.width, with: image.pixels[y * image.width..<(y + 1) * image.width])
        }
        return try PetalLuma(width: width, height: image.height + 2 * pad, pixels: pixels)
    }

    func testAHiddenBlossomIsInferredAndEveryLaneStillReads() throws {
        for corner in 0..<4 {
            let (encoder, image) = try hiddenBlossom(2, corner: corner)
            let decoded = try PetalDecoder.decode(image)
            let expected = encoder.laneData(frame: 2)
            XCTAssertEqual(decoded.inferredCorner, corner, "corner \(corner)")
            XCTAssertEqual(decoded.rotation, 0, "corner \(corner)")
            XCTAssertFalse(decoded.mirrored, "corner \(corner)")
            XCTAssertEqual(decoded.p?.data, expected.p, "corner \(corner) lane P")
            XCTAssertEqual(decoded.k?.data, expected.k, "corner \(corner) lane K")
            XCTAssertEqual(decoded.d?.data, expected.d, "corner \(corner) lane D")
            // the diagnostics read the levels of the inferred corner the same way
            XCTAssertEqual(PetalDecoder.observedCells(image, frame: decoded), encoder.cells(frame: 2), "corner \(corner)")
        }
    }

    func testTheInferredCornerIsReportedInCodeCoordinatesWhenMirrored() throws {
        // hide the top-right blossom of the code, then mirror the picture: the hidden
        // blossom appears top-left in the image but is still corner 1 of the code
        let (encoder, image) = try hiddenBlossom(3, corner: 1)
        let decoded = try PetalDecoder.decode(try Support.mirror(image))
        XCTAssertTrue(decoded.mirrored)
        XCTAssertEqual(decoded.inferredCorner, 1)
        XCTAssertEqual(decoded.d?.data, encoder.laneData(frame: 3).d)
    }

    func testALargeHiddenRegionNeverReadsWrongData() throws {
        // the whole bottom-right quarter is gone: rings and tiles with it
        let (encoder, image) = try setup(2)
        let n = image.width
        var pixels = image.pixels
        for y in (n * 3 / 4)..<n {
            for x in (n * 3 / 4)..<n { pixels[y * n + x] = 0 }
        }
        let expected = encoder.laneData(frame: 2)
        if let frame = try? PetalDecoder.decode(try PetalLuma(width: n, height: n, pixels: pixels)) {
            if let p = frame.p { XCTAssertEqual(p.data, expected.p) }
            if let k = frame.k { XCTAssertEqual(k.data, expected.k) }
            if let d = frame.d { XCTAssertEqual(d.data, expected.d) }
        }
    }

    func testTrackingFollowsASmallMovementAndGivesUpOnAJump() throws {
        let (encoder, rendered) = try setup(6)
        let image = try padded(rendered, 100)
        let first = try PetalDecoder.decode(image)
        let expected = encoder.laneData(frame: 6)
        let moved = try shifted(image, 9, -6)
        let followed = try XCTUnwrap(PetalDecoder.track(moved, previous: first), "tracks a 9 px move")
        XCTAssertEqual(followed.p?.data, expected.p)
        XCTAssertEqual(followed.k?.data, expected.k)
        XCTAssertEqual(followed.d?.data, expected.d)
        XCTAssertNil(followed.inferredCorner)
        XCTAssertEqual(followed.rotation, first.rotation)
        XCTAssertEqual(followed.mirrored, first.mirrored)
        // more than a finder diameter: tracking refuses, a full decode is needed
        let jumped = try shifted(image, 95, 0)
        XCTAssertNil(PetalDecoder.track(jumped, previous: first))
        XCTAssertNoThrow(try PetalDecoder.decode(jumped))
        // an image the decoder refuses is never tracked
        XCTAssertNil(PetalDecoder.track(try PetalLuma(width: 8, height: 8), previous: first))
    }

    func testTrackingSurvivesABlossomThatDisappears() throws {
        let (_, rendered) = try setup(4)
        let first = try PetalDecoder.decode(rendered)
        // the same code, slightly moved, now with the bottom-left blossom covered
        let (encoder, covered) = try hiddenBlossom(4, corner: 3)
        let moved = try shifted(covered, -5, 4)
        let followed = try XCTUnwrap(PetalDecoder.track(moved, previous: first), "tracks with three blossoms")
        XCTAssertEqual(followed.inferredCorner, 3)
        XCTAssertEqual(followed.d?.data, encoder.laneData(frame: 4).d)
    }

    /// A canvas-sized image with finder (lit) and reference-canvas (dark) levels
    /// painted where they are sampled.
    private func levelCard(_ levels: [(UInt8, UInt8)]) throws -> PetalLuma {
        var pixels = [UInt8](repeating: 0, count: 1024 * 1024)
        func paint(_ cx: Int, _ cy: Int, _ radius: Int, _ value: UInt8) {
            for y in (cy - radius)...(cy + radius) {
                for x in (cx - radius)...(cx + radius) { pixels[y * 1024 + x] = value }
            }
        }
        for (corner, (cx, cy)) in [(72, 72), (952, 72), (952, 952), (72, 952)].enumerated() {
            let (sx, sy) = (cx < 512 ? 1 : -1, cy < 512 ? 1 : -1)
            paint(cx, cy, 30, levels[corner].0)
            paint(cx + sx * 100, cy, 12, levels[corner].1)
            paint(cx, cy + sy * 100, 12, levels[corner].1)
        }
        return try PetalLuma(width: 1024, height: 1024, pixels: pixels)
    }

    func testAnInferredCornerNeedsContrastToo() throws {
        let h = try PetalHomography(elements: [1, 0, 0, 0, 1, 0, 0, 0, 1])
        // even light: the hidden corner (3) gets levels between the others'
        let even = try levelCard([(230, 30), (220, 25), (210, 20), (0, 0)])
        let reference = try XCTUnwrap(even.withView { PetalDecoder.referenceLevels($0, h, inferred: 3) })
        XCTAssertGreaterThanOrEqual(reference.lit[3] - reference.dark[3], 12.0)
        // the hidden corner's neighbours disagree (one dim, one veiled): the estimates cross
        let uneven = try levelCard([(60, 45), (250, 20), (200, 185), (0, 0)])
        XCTAssertNil(uneven.withView { PetalDecoder.referenceLevels($0, h, inferred: 3) })
        // with every corner seen, the same light is fine
        let seen = try levelCard([(60, 45), (250, 20), (200, 185), (240, 20)])
        XCTAssertNotNil(seen.withView { PetalDecoder.referenceLevels($0, h, inferred: nil) })
    }

    func testBrokenPosesAreRefusedWithoutPanicking() throws {
        let (_, image) = try setup(4)
        let previous = try PetalDecoder.decode(image)
        // a good pose that names a corner that does not exist
        XCTAssertNotNil(PetalDecoder.track(image, previous: previous))
        for corner in [4, 255, -1] {
            let named = PetalDecodedFrame(
                homography: previous.homography,
                rotation: previous.rotation,
                mirrored: previous.mirrored,
                p: previous.p,
                k: previous.k,
                d: previous.d,
                inferredCorner: corner
            )
            XCTAssertNil(PetalDecoder.track(image, previous: named), "\(corner)")
        }
        let nonFinite = [
            try PetalHomography(elements: [Double](repeating: .nan, count: 9)),
            try PetalHomography(elements: [.infinity, 0, 0, 0, 1, 0, 0, 0, 1]),
        ]
        for broken in nonFinite {
            XCTAssertNil(PetalDecoder.decode(image, homography: broken))
        }
        // the last one makes every finder far larger than the image
        let huge = try PetalHomography(elements: [50, 0, 0, 0, 50, 0, 0, 0, 1])
        for broken in nonFinite + [huge] {
            for inferred in [nil, 2] as [Int?] {
                let pose = PetalDecodedFrame(
                    homography: broken,
                    rotation: previous.rotation,
                    mirrored: previous.mirrored,
                    p: previous.p,
                    k: previous.k,
                    d: previous.d,
                    inferredCorner: inferred
                )
                XCTAssertNil(PetalDecoder.track(image, previous: pose), "\(broken.elements) \(String(describing: inferred))")
            }
        }
    }

    func testABlossomThatReappearsIsSeenAgain() throws {
        let (_, covered) = try hiddenBlossom(4, corner: 3)
        let first = try PetalDecoder.decode(covered)
        XCTAssertEqual(first.inferredCorner, 3)
        // the thumb moves away and the hand moves a little
        let (encoder, image) = try setup(4)
        let moved = try shifted(image, 4, -3)
        let followed = try XCTUnwrap(PetalDecoder.track(moved, previous: first), "tracks")
        XCTAssertNil(followed.inferredCorner)
        XCTAssertEqual(followed.d?.data, encoder.laneData(frame: 4).d)
        // still covered: still inferred
        let still = try shifted(covered, 4, -3)
        let stillFollowed = try XCTUnwrap(PetalDecoder.track(still, previous: first), "tracks")
        XCTAssertEqual(stillFollowed.inferredCorner, 3)
    }

    func testTheTianMaskTellsTheQuarterTurnsApart() throws {
        let (_, image) = try setup(5)
        let quad = try XCTUnwrap(PetalLocator.locate(image), "four finders")
        var byRotation = [Double](repeating: -Double.greatestFiniteMagnitude, count: 4)
        try image.withView { view in
            for hypothesis in PetalDecoder.hypotheses(quad, tryMirrored: true) {
                let reference = try XCTUnwrap(
                    PetalDecoder.referenceLevels(view, hypothesis.homography, inferred: nil),
                    "levels"
                )
                let score = PetalDecoder.maskScore(view, hypothesis.homography, reference)
                if !hypothesis.mirrored { byRotation[hypothesis.rotation] = score }
            }
        }
        // upright wins clearly over the three other quarter turns
        for rotation in 1..<4 {
            XCTAssertGreaterThan(byRotation[0], byRotation[rotation] + 0.1, "\(byRotation)")
        }
    }

    func testInferredLevelsAreTheParallelogramOfTheOtherThreeWithinTheirRange() throws {
        let (_, image) = try setup(5)
        let scale = 768.0 / 1024.0
        let pose = try PetalHomography(elements: [scale, 0, 0, 0, scale, 0, 0, 0, 1])
        try image.withView { view in
            let seen = try XCTUnwrap(PetalDecoder.referenceLevels(view, pose, inferred: nil))
            for corner in 0..<4 {
                let guessed = try XCTUnwrap(PetalDecoder.referenceLevels(view, pose, inferred: corner))
                let (n1, opposite, n2) = ((corner + 1) % 4, (corner + 2) % 4, (corner + 3) % 4)
                for (levels, all) in [(guessed.lit, seen.lit), (guessed.dark, seen.dark)] {
                    let others = [all[n1], all[opposite], all[n2]]
                    let expected = min(max(all[n1] + all[n2] - all[opposite], others.min()!), others.max()!)
                    XCTAssertEqual(levels[corner], expected, "corner \(corner)")
                    for index in 0..<4 where index != corner { XCTAssertEqual(levels[index], all[index]) }
                }
            }
        }
        XCTAssertEqual(PetalDecoder.canonicalCorner(2, rotation: 1, mirrored: false), 1)
        XCTAssertEqual(PetalDecoder.canonicalCorner(0, rotation: 1, mirrored: true), 1)
        XCTAssertEqual(PetalDecoder.canonicalCorner(3, rotation: 0, mirrored: true), 1)
    }

    func testFramesSurviveBeingShownSmallAndOffCentre() throws {
        // a 320 px render pasted into a larger dim frame at an odd offset
        let encoder = try PetalStreamEncoder(payload: Self.setupPayload, kind: 2)
        let code = try Support.render(encoder, frame: 8, size: 320, supersample: 2)
        let width = 640
        let height = 480
        var pixels = [UInt8](repeating: 12, count: width * height)
        for y in 0..<code.height {
            for x in 0..<code.width {
                pixels[(y + 97) * width + x + 211] = code.pixels[y * code.width + x]
            }
        }
        let decoded = try PetalDecoder.decode(try PetalLuma(width: width, height: height, pixels: pixels))
        let expected = encoder.laneData(frame: 8)
        XCTAssertEqual(decoded.p?.data, expected.p)
        XCTAssertEqual(decoded.d?.data, expected.d)
        XCTAssertEqual(decoded.beacon?.meta, encoder.meta, "frame 8 carries the beacon")
        if let k = decoded.k { XCTAssertEqual(k.data, expected.k) }
    }

    // MARK: - Golden captures (`fixtures/petal/petal_captures_v1.json`)

    private func captures() throws -> [String: Any] {
        try XCTUnwrap(Support.capturesFixture, "fixtures/petal/petal_captures_v1.json is missing")
    }

    func testGoldenCapturesDecodeAsRecorded() throws {
        let doc = try captures()
        var assembler = PetalStreamAssembler()
        let entries = try Support.objects(doc, "captures")
        XCTAssertEqual(entries.count, 11)
        for capture in entries {
            let name = try Support.string(capture, "name")
            let image = try Support.luma(of: capture)
            let started = DispatchTime.now().uptimeNanoseconds
            let decoded: PetalDecodedFrame
            do {
                decoded = try PetalDecoder.decode(image)
            } catch {
                XCTFail("\(name): \(error)")
                continue
            }
            let elapsed = Double(DispatchTime.now().uptimeNanoseconds - started) / 1e6
            let reference = try Support.string(capture, "reference_decoded")
            print(String(
                format: "petal capture %@ %dx%d: lanes %@ (reference %@) in %.1f ms",
                name, image.width, image.height, decoded.laneLetters, reference, elapsed
            ))
            XCTAssertEqual(decoded.mirrored, capture["mirrored"] as? Bool, "\(name): mirror flag")
            XCTAssertEqual(decoded.inferredCorner, try Support.optionalInteger(capture, "inferred_corner"), "\(name): inferred corner")
            let must = try Support.string(capture, "must_decode")
            let lanes: [(Character, PetalLaneResult?, String)] = [
                ("P", decoded.p, "p_data"),
                ("K", decoded.k, "k_data"),
                ("D", decoded.d, "d_data"),
            ]
            for (letter, lane, key) in lanes {
                if let lane {
                    XCTAssertEqual(lane.data, try Support.bytes(capture, key), "\(name): lane \(letter) data")
                } else {
                    XCTAssertFalse(must.contains(letter), "\(name): required lane \(letter) was not decoded")
                }
            }
            decoded.feed(&assembler)
        }
        // captures of different frames of the same stream accumulate in one assembler
        XCTAssertGreaterThan(assembler.progress.atomsReceived, 10)
        let payload = try Support.bytes(doc, "payload_hex")
        let encoder = try PetalStreamEncoder(payload: payload, kind: UInt8(try Support.integer(doc, "payload_kind")))
        XCTAssertEqual(assembler.progress.meta, encoder.meta)
    }

    func testGoldenCapturesReadExactlyTheReferenceLanes() throws {
        for capture in try Support.objects(try captures(), "captures") {
            let name = try Support.string(capture, "name")
            let decoded = try PetalDecoder.decode(try Support.luma(of: capture))
            XCTAssertEqual(decoded.laneLetters, try Support.string(capture, "reference_decoded"), name)
        }
    }

    func testGoldenTracksFollowThePoseIntoTheNextFrame() throws {
        let tracks = try Support.objects(try captures(), "tracks")
        XCTAssertEqual(tracks.count, 2)
        for pair in tracks {
            let name = try Support.string(pair, "name")
            let previous = try PetalDecoder.decode(try Support.luma(of: pair, key: "from_luma_zlib_base64"))
            let next = try Support.luma(of: pair, key: "to_luma_zlib_base64")
            let started = DispatchTime.now().uptimeNanoseconds
            let tracked = PetalDecoder.track(next, previous: previous)
            let elapsed = Double(DispatchTime.now().uptimeNanoseconds - started) / 1e6
            let followed = try XCTUnwrap(tracked, "\(name): tracking lost the code")
            let reference = try Support.string(pair, "reference_tracked")
            print(String(
                format: "petal track %@ %dx%d: lanes %@ (reference %@) in %.1f ms",
                name, next.width, next.height, followed.laneLetters, reference, elapsed
            ))
            XCTAssertEqual(followed.laneLetters, reference, name)
            let must = try Support.string(pair, "must_track")
            let lanes: [(Character, PetalLaneResult?, String)] = [
                ("P", followed.p, "p_data"),
                ("K", followed.k, "k_data"),
                ("D", followed.d, "d_data"),
            ]
            for (letter, lane, key) in lanes {
                if let lane {
                    XCTAssertEqual(lane.data, try Support.bytes(pair, key), "\(name): lane \(letter) data")
                } else {
                    XCTAssertFalse(must.contains(letter), "\(name): lane \(letter) lost")
                }
            }
            XCTAssertEqual(followed.inferredCorner, try Support.optionalInteger(pair, "inferred_corner"), "\(name): inferred corner")
            XCTAssertEqual(followed.rotation, previous.rotation, name)
            XCTAssertEqual(followed.mirrored, previous.mirrored, name)
        }
    }

    func testNegativeCapturesAreRejected() throws {
        let negatives = try Support.objects(try captures(), "negatives")
        XCTAssertEqual(negatives.count, 2)
        for negative in negatives {
            let name = try Support.string(negative, "name")
            let image = try Support.luma(of: negative)
            XCTAssertThrowsError(try PetalDecoder.decode(image), name)
        }
    }

    func testRecordedLaneDataIsAValidCodewordOfItsStream() throws {
        let doc = try captures()
        let payload = try Support.bytes(doc, "payload_hex")
        let encoder = try PetalStreamEncoder(payload: payload, kind: 2)
        for capture in try Support.objects(doc, "captures") {
            let data = try Support.bytes(capture, "p_data")
            XCTAssertEqual(try PetalLane.p.decode(try PetalLane.p.encode(data)), data)
            let frame = UInt16(try Support.integer(capture, "frame"))
            let expected = encoder.laneData(frame: frame)
            XCTAssertEqual(data, expected.p)
            XCTAssertEqual(try Support.bytes(capture, "k_data"), expected.k)
            XCTAssertEqual(try Support.bytes(capture, "d_data"), expected.d)
        }
    }

    func testSoftwareRendererReproducesTheCleanGoldenCapture() throws {
        // `clean-512` is the reference render (512 px, 3x3 supersampling) of
        // frame 5 of the `one-pass` stream, converted to luma.
        let doc = try captures()
        let capture = try XCTUnwrap(try Support.objects(doc, "captures").first {
            ($0["name"] as? String) == "clean-512"
        })
        let encoder = try PetalStreamEncoder(payload: try Support.bytes(doc, "payload_hex"), kind: 2)
        let rendered = try Support.render(encoder, frame: 5, size: 512, supersample: 3)
        let golden = try Support.luma(of: capture)
        XCTAssertEqual(rendered.width, golden.width)
        let mismatches = zip(rendered.pixels, golden.pixels).filter { $0 != $1 }.count
        XCTAssertEqual(mismatches, 0, "software render differs from the reference in \(mismatches) pixels")
    }
}

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
        let reference = try XCTUnwrap(image.withView { PetalDecoder.referenceLevels($0, h) })
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
        let reference = try XCTUnwrap(image.withView { PetalDecoder.referenceLevels($0, h) })
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
        let reference = try XCTUnwrap(shadowed.withView { PetalDecoder.referenceLevels($0, h) })
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
        let reference = try XCTUnwrap(image.withView { PetalDecoder.referenceLevels($0, h) })
        let level = tileLanes(PetalDecoder.readTiles(patches, reference, sigmas))
        let combined = PetalDecoder.readTileLanes(patches, reference, sigmas)
        XCTAssertEqual(level.p?.data, expected.p)
        XCTAssertEqual(level.k?.data, expected.k)
        XCTAssertEqual(combined.p, level.p)
        XCTAssertEqual(combined.k, level.k)
    }

    func testLaneDIsReadFromTheRingsUnderAKnownPose() throws {
        let (encoder, image, h, _) = try cleanPatches(5)
        let reference = try XCTUnwrap(image.withView { PetalDecoder.referenceLevels($0, h) })
        let lane = try XCTUnwrap(image.withView { PetalDecoder.readLaneD($0, h, reference) })
        XCTAssertEqual(lane.data, encoder.laneData(frame: 5).d)
        XCTAssertEqual(lane.corrected, 0)
        // a mirrored image under the unmirrored pose reads the rings back to front
        let mirrored = try Support.mirror(image)
        let mirroredReference = try XCTUnwrap(mirrored.withView { PetalDecoder.referenceLevels($0, h) })
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

    func testAValidCodeWithAMissingFinderIsNotMisread() throws {
        let (_, image) = try setup(2)
        let n = image.width
        var pixels = image.pixels
        // erase the bottom-right blossom
        for y in (n * 3 / 4)..<n {
            for x in (n * 3 / 4)..<n { pixels[y * n + x] = 0 }
        }
        XCTAssertThrowsError(try PetalDecoder.decode(try PetalLuma(width: n, height: n, pixels: pixels)))
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
        XCTAssertEqual(entries.count, 9)
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

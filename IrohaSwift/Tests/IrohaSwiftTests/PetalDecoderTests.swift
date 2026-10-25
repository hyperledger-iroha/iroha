import XCTest
@testable import IrohaSwift

/// Ports of the `decode` unit tests and `tests/captures.rs` of
/// `crates/iroha_petal`, plus the golden capture conformance suite.
final class PetalDecoderTests: XCTestCase {
    private typealias Support = PetalTestSupport

    /// The payload of the reference's decoder tests.
    private static let setupPayload: [UInt8] = (0..<3000).map {
        UInt8(truncatingIfNeeded: (UInt32($0) &* 2_654_435_761) >> 11)
    }

    private func setup(_ frame: UInt16) throws -> (PetalStreamEncoder, PetalLuma) {
        let encoder = try PetalStreamEncoder(payload: Self.setupPayload, kind: 2)
        return (encoder, try Support.render(encoder, frame: frame, size: 768, supersample: 2))
    }

    private func assertAllLanes(
        _ decoded: PetalDecodedFrame,
        _ truth: PetalLaneData,
        _ name: String,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertEqual(decoded.d?.data, truth.d, "\(name): lane D", file: file, line: line)
        for sector in 0..<PetalLayout.sectors {
            XCTAssertEqual(
                decoded.fields[sector]?.data,
                truth.fields[sector],
                "\(name): field lane \(sector)",
                file: file,
                line: line
            )
        }
    }

    /// The exact canvas-to-pixel pose of a `size`-pixel test render.
    private func renderPose(_ size: Int) throws -> PetalPose {
        let scale = Double(size) / 1024
        return PetalPose.plain(try PetalHomography(elements: [scale, 0, 0, 0, scale, 0, 0, 0, 1]))
    }

    // MARK: - Rendered frames

    func testCleanRenderDecodesEveryLane() throws {
        let (encoder, image) = try setup(5)
        let decoded = try PetalDecoder.decode(image)
        assertAllLanes(decoded, encoder.laneData(frame: 5), "clean")
        XCTAssertEqual(decoded.rotation, 0)
        XCTAssertFalse(decoded.mirrored)
        XCTAssertEqual(decoded.laneLetters, "D0123")
        XCTAssertEqual(decoded.lanesOK, 5)
        XCTAssertEqual(decoded.pose.distortion.bitPattern, 0.0.bitPattern)
        XCTAssertNil(decoded.beacon, "frame 5 carries an atom in lane D")
        XCTAssertEqual(decoded.atomPackets.map(\.atoms.count), [5, 5, 5, 5, 1])
        XCTAssertEqual(decoded.atomPackets.map(\.firstID), [103, 108, 113, 118, 123])
    }

    func testRotatedAndMirroredRendersDecodeWithTheRightOrientation() throws {
        let (encoder, image) = try setup(7)
        let truth = encoder.laneData(frame: 7)
        let n = image.width
        // mirrored hypotheses enumerate corners in the opposite direction, so the
        // unrotated mirror reports quarter-turn index 1
        let cases: [(String, PetalLuma, Int, Bool)] = [
            ("rot90", try Support.transform(image) { x, y in (y, n - 1 - x) }, 1, false),
            ("rot180", try Support.transform(image) { x, y in (n - 1 - x, n - 1 - y) }, 2, false),
            ("rot270", try Support.transform(image) { x, y in (n - 1 - y, x) }, 3, false),
            ("mirror", try Support.transform(image) { x, y in (n - 1 - x, y) }, 1, true),
            ("mirror-rot90", try Support.transform(image) { x, y in (y, x) }, 0, true),
        ]
        for (name, transformed, rotation, mirrored) in cases {
            let decoded = try PetalDecoder.decode(transformed)
            XCTAssertEqual(decoded.mirrored, mirrored, name)
            XCTAssertEqual(decoded.rotation, rotation, "\(name) rotation")
            assertAllLanes(decoded, truth, name)
        }
        // without mirror hypotheses a mirrored frame is never misread
        let strict = PetalDecodeOptions(tryMirrored: false)
        if let decoded = try? PetalDecoder.decode(try Support.mirror(image), options: strict) {
            XCTAssertFalse(decoded.mirrored)
            for lane in PetalLane.decodeOrder {
                if let result = decoded.lane(lane) { XCTAssertEqual(result.data, truth.lane(lane)) }
            }
        }
    }

    func testTheFieldAloneTellsTheOrientationWhenTheRingsAreGone() throws {
        // black out the ring band: no gates, no lane D; a wrong quarter turn reads
        // permuted petals that no field codeword accepts
        let (encoder, image) = try setup(9)
        let n = image.width
        let scale = Double(n) / 1024
        var pixels = image.pixels
        for y in 0..<n {
            for x in 0..<n {
                let dx = (Double(x) + 0.5) / scale - 512
                let dy = (Double(y) + 0.5) / scale - 512
                let r = (dx * dx + dy * dy).squareRoot()
                if (392.0...490.0).contains(r) { pixels[y * n + x] = 0 }
            }
        }
        let ringless = try PetalLuma(width: n, height: n, pixels: pixels)
        let rotated = try Support.transform(ringless) { x, y in (y, n - 1 - x) }
        let decoded = try PetalDecoder.decode(rotated)
        XCTAssertNil(decoded.d)
        XCTAssertEqual(decoded.rotation, 1)
        let truth = encoder.laneData(frame: 9)
        for sector in 0..<PetalLayout.sectors {
            XCTAssertEqual(decoded.fields[sector]?.data, truth.fields[sector], "sector \(sector)")
        }
    }

    func testDecodeWithAKnownPoseReadsEveryLane() throws {
        let (encoder, image) = try setup(3)
        let pose = try renderPose(image.width)
        let decoded = try XCTUnwrap(PetalDecoder.decode(image, pose: pose))
        assertAllLanes(decoded, encoder.laneData(frame: 3), "known pose")
        XCTAssertNil(PetalDecoder.decode(try PetalLuma(width: 768, height: 768), pose: pose))
    }

    func testCorrectedCountsRewrittenBytesNotJustErasures() throws {
        let data = (0..<83).map { UInt8($0) }
        let lane = PetalLane.field(1)
        var word = try lane.encode(data)
        for position in [2, 11, 30, 77] { word[position] ^= 0x5A }
        let confidence = [Double](repeating: 1, count: word.count)
        let result = try XCTUnwrap(PetalDecoder.decodeWithErasures(lane, word, confidence), "four errors fit")
        XCTAssertEqual(result.data, data)
        XCTAssertEqual(result.erasures, 0)
        XCTAssertEqual(result.corrected, 4)
        // with the damaged bytes flagged as least confident, they become erasures
        var flagged = [Double](repeating: 1, count: word.count)
        for position in [2, 11, 30, 77] { flagged[position] = 0 }
        let erased = try XCTUnwrap(PetalDecoder.decodeWithErasures(lane, word, flagged))
        XCTAssertEqual(erased.data, data)
        XCTAssertGreaterThanOrEqual(erased.corrected, 4)
    }

    func testRandomWordsAreAlmostNeverAccepted() {
        // Reed–Solomon with erasures can accept a word that is not a transmission. Lane D has only
        // 11 parity bytes, so its schedule stops at five erasures; a field lane has 22 and stops at
        // eleven. The counts are exact: the same xorshift32 words and byte-valued confidences (many
        // ties, so the ranking must be stable) reproduce the reference.
        var rng = PetalXorshift32(seed: 0x5EED)
        let trials = 40_000
        for (lane, expected) in [(PetalLane.d, 3), (PetalLane.field(0), 0)] {
            let length = lane.wordLength
            var accepted = 0
            for _ in 0..<trials {
                let word = (0..<length).map { _ in rng.nextByte() }
                let confidence = (0..<length).map { _ in Double(rng.nextByte()) }
                if PetalDecoder.decodeWithErasures(lane, word, confidence) != nil { accepted += 1 }
            }
            XCTAssertEqual(accepted, expected, "lane \(lane.letter) of \(trials) random words")
        }
    }

    func testErasuresStopAtHalfTheParity() throws {
        // damaged bytes: `flagged` of them marked least confident, two more hidden: beyond half
        // the parity the decoder must refuse instead of risking a wrong codeword
        for (lane, flagged) in [(PetalLane.d, 7), (PetalLane.field(2), 16)] {
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
            damaged[24] ^= 0x3C
            let result = try XCTUnwrap(
                PetalDecoder.decodeWithErasures(lane, damaged, confidence),
                "lane \(lane.letter)"
            )
            XCTAssertEqual(result.data, data, "lane \(lane.letter)")
            XCTAssertLessThanOrEqual(result.erasures, lane.parityLength / 2, "lane \(lane.letter)")
        }
    }

    func testEqualConfidencesAreErasedInPositionOrder() throws {
        // Ties must be broken by position or ports disagree about which bytes are erased.
        // Three damaged bytes at the front plus four hidden ones fit lane D only if exactly the
        // first three positions are erased (3 erasures + 4 errors = all 11 parity bytes).
        let data = (0..<PetalLane.d.dataLength).map { UInt8($0) }
        var word = try PetalLane.d.encode(data)
        for position in Array(0..<3) + Array(20..<24) { word[position] ^= 0x5A }
        let confidence = [Double](repeating: 1, count: word.count)
        let result = try XCTUnwrap(PetalDecoder.decodeWithErasures(.d, word, confidence), "ties in order")
        XCTAssertEqual(result.data, data)
        XCTAssertEqual(result.erasures, 3)
        XCTAssertEqual(result.corrected, 7)
    }

    func testTwoMeansSplitsTwoClustersAndSurvivesOne() {
        let values = [Double](repeating: 0.05, count: 40) + [Double](repeating: 0.9, count: 60)
        let threshold = PetalDecoder.twoMeans(values)
        XCTAssertTrue(threshold > 0.3 && threshold < 0.6, "\(threshold)")
        XCTAssertEqual(PetalDecoder.twoMeans([Double](repeating: 0.4, count: 10)), 0.4, accuracy: 1e-12)
    }

    // MARK: - The level read and the local read

    func testLocalLevelsRescaleEveryNeighbourhood() {
        let table = PetalDecoder.neighbourhoods
        XCTAssertEqual(table.starts.count, PetalLayout.fieldCells + 1)
        for cell in 0..<PetalLayout.fieldCells {
            let around = table.cells[table.starts[cell]..<table.starts[cell + 1]].map(Int.init)
            XCTAssertTrue(around.contains(cell), "cell \(cell) is its own neighbour")
            XCTAssertEqual(around, around.sorted(), "index order")
            XCTAssertGreaterThanOrEqual(around.count, 10, "cell \(cell)")
            let c = PetalLayout.cellCenter(cell)
            for other in around {
                let o = PetalLayout.cellCenter(other)
                XCTAssertLessThanOrEqual((o.x - c.x) * (o.x - c.x) + (o.y - c.y) * (o.y - c.y), 1024)
            }
        }
        // alternating dark and lit cells under any gain and offset become exactly 0 and 1
        for (gain, offset) in [(1.0, 0.0), (0.3, 0.45), (2.5, -0.7)] {
            let levels = (0..<PetalLayout.fieldCells).map { gain * ($0 % 2 == 0 ? 0.2 : 0.8) + offset }
            let local = PetalDecoder.localLevels(levels)
            for cell in 0..<PetalLayout.fieldCells {
                XCTAssertEqual(local[cell], cell % 2 == 0 ? 0.0 : 1.0, accuracy: 1e-9, "cell \(cell)")
            }
        }
        // a flat neighbourhood has no contrast: its span counts as 0.05
        let flat = PetalDecoder.localLevels([Double](repeating: 0.4, count: PetalLayout.fieldCells))
        XCTAssertTrue(flat.allSatisfy { $0 == 0 })
    }

    /// The 768-pixel render of frame 5 under light that varies over the canvas
    /// by `factor(x, y)` (canvas units), as shadows and glare do.
    private func shaded(_ factor: (Double, Double) -> Double) throws -> (PetalStreamEncoder, PetalLuma) {
        let (encoder, image) = try setup(5)
        let n = image.width
        let scale = Double(n) / 1024
        var pixels = image.pixels
        for y in 0..<n {
            for x in 0..<n {
                let light = factor((Double(x) + 0.5) / scale, (Double(y) + 0.5) / scale)
                pixels[y * n + x] = UInt8((Double(pixels[y * n + x]) * light).rounded())
            }
        }
        return (encoder, try PetalLuma(width: n, height: n, pixels: pixels))
    }

    /// The field levels of `image` under the exact render pose.
    private func renderLevels(_ image: PetalLuma) throws -> [Double] {
        let pose = try renderPose(image.width)
        return try XCTUnwrap(image.withView { view -> [Double]? in
            guard let reference = PetalDecoder.referenceLevels(view, pose, inferred: nil) else { return nil }
            return PetalDecoder.fieldLevels(view, pose, reference)
        })
    }

    private func smoothstep(_ t: Double) -> Double {
        let t = min(max(t, 0), 1)
        return t * t * (3 - 2 * t)
    }

    func testAShadowTheFinderLevelsCannotDescribeIsReadThroughTheLocalRead() throws {
        // a soft shadow over the middle of the code (30 % light) while the finders stay lit
        let (encoder, image) = try shaded { x, y in
            let r = ((x - 512) * (x - 512) + (y - 512) * (y - 512)).squareRoot()
            return 0.3 + 0.7 * self.smoothstep((r - 150) / 150)
        }
        let truth = encoder.laneData(frame: 5)
        let levels = try renderLevels(image)
        let level = PetalDecoder.readSectors(levels, skip: PetalDecoder.readAll).results
        XCTAssertTrue(level.allSatisfy { $0 == nil }, "the level read must lose every lane, or the test proves nothing")
        let local = PetalDecoder.readFields(levels, skip: PetalDecoder.readAll).results
        for sector in 0..<PetalLayout.sectors {
            XCTAssertEqual(local[sector]?.data, truth.fields[sector], "sector \(sector)")
        }
        assertAllLanes(try PetalDecoder.decode(image), truth, "shadowed")
    }

    func testTheLocalReadKeepsWhatTheLevelReadDelivered() throws {
        // a soft-edged shadow band (30 % light) along the main diagonal
        let (encoder, image) = try shaded { x, y in
            0.3 + 0.7 * self.smoothstep((abs(x - y) / 2.0.squareRoot() - 90) / 100)
        }
        let truth = encoder.laneData(frame: 5)
        let levels = try renderLevels(image)
        let level = PetalDecoder.readSectors(levels, skip: PetalDecoder.readAll).results
        XCTAssertTrue(level.contains { $0 != nil } && level.contains { $0 == nil }, "the level read must read some lanes")
        let combined = PetalDecoder.readFields(levels, skip: PetalDecoder.readAll).results
        for sector in 0..<PetalLayout.sectors {
            XCTAssertEqual(combined[sector]?.data, truth.fields[sector], "sector \(sector)")
            if let read = level[sector] { XCTAssertEqual(combined[sector], read, "sector \(sector) kept") }
        }
        // skipped sectors stay unread in both reads
        let skipped = PetalDecoder.readFields(levels, skip: [true, true, false, false]).results
        XCTAssertNil(skipped[0])
        XCTAssertNil(skipped[1])
        XCTAssertEqual(skipped[2]?.data, truth.fields[2])
        XCTAssertEqual(skipped[3]?.data, truth.fields[3])
    }

    func testLaneDIsReadFromTheRingsUnderAKnownPose() throws {
        let (encoder, image) = try setup(5)
        let pose = try renderPose(image.width)
        let reference = try XCTUnwrap(image.withView { PetalDecoder.referenceLevels($0, pose, inferred: nil) })
        let lane = try XCTUnwrap(image.withView { PetalDecoder.readLaneD($0, pose, reference) })
        XCTAssertEqual(lane.data, encoder.laneData(frame: 5).d)
        XCTAssertEqual(lane.corrected, 0)
        // a mirrored image under the unmirrored pose reads the rings back to front
        let mirrored = try Support.mirror(image)
        let mirroredReference = try XCTUnwrap(mirrored.withView { PetalDecoder.referenceLevels($0, pose, inferred: nil) })
        XCTAssertNil(mirrored.withView { PetalDecoder.readLaneD($0, pose, mirroredReference) })
    }

    // MARK: - Garbage and broken input

    func testDegeneratePosesNeverCrashOrDecode() throws {
        let (_, image) = try setup(5)
        let poses: [[Double]] = [
            [Double.nan, 0, 0, 0, Double.nan, 0, 0, 0, 1],
            [0, 0, 0, 0, 0, 0, 0, 0, 0],
            [Double.infinity, 0, 0, 0, Double.infinity, 0, 0, 0, 1],
            [1e-12, 0, 0, 0, 1e-12, 0, 0, 0, 1],
        ]
        for elements in poses {
            let pose = PetalPose.plain(try PetalHomography(elements: elements))
            if let frame = PetalDecoder.decode(image, pose: pose) {
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
        XCTAssertNil(PetalDecoder.decode(try PetalLuma(width: 8, height: 8), pose: .plain(.identity)))
        // a buffer that does not match the stated size cannot be built, so it never reaches the sampler
        XCTAssertThrowsError(try PetalLuma(width: 100, height: 100, pixels: [UInt8](repeating: 0, count: 5))) {
            XCTAssertEqual($0 as? PetalImageError, .invalidDimensions)
        }
        // the diagnostics refuse the images `decode` refuses
        let (_, rendered) = try setup(5)
        let decoded = try PetalDecoder.decode(rendered)
        let tiny = try PetalLuma(width: 8, height: 8)
        XCTAssertNil(PetalDecoder.observedCells(tiny, frame: decoded))
        XCTAssertNil(PetalDecoder.fieldEvidence(tiny, frame: decoded))
        XCTAssertNil(PetalDecoder.observedCells(rendered, frame: decoded, options: smallBudget))
        XCTAssertNotNil(PetalDecoder.observedCells(rendered, frame: decoded))
    }

    func testObservedCellsMatchTheDrawnCellsOfACleanRender() throws {
        let (encoder, image) = try setup(3)
        let decoded = try PetalDecoder.decode(image)
        XCTAssertEqual(PetalDecoder.observedCells(image, frame: decoded), encoder.cells(frame: 3))
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
        // exercises the locator, quad selection and pose fitting on degenerate layouts.
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

    // MARK: - Hidden blossoms, lens distortion and tracking

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
            XCTAssertEqual(decoded.inferredCorner, corner, "corner \(corner)")
            XCTAssertEqual(decoded.rotation, 0, "corner \(corner)")
            XCTAssertFalse(decoded.mirrored, "corner \(corner)")
            assertAllLanes(decoded, encoder.laneData(frame: 2), "corner \(corner)")
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
        // the whole bottom-right quarter is gone: rings and petals with it
        let (encoder, image) = try setup(2)
        let n = image.width
        var pixels = image.pixels
        for y in (n * 3 / 4)..<n {
            for x in (n * 3 / 4)..<n { pixels[y * n + x] = 0 }
        }
        let truth = encoder.laneData(frame: 2)
        if let frame = try? PetalDecoder.decode(try PetalLuma(width: n, height: n, pixels: pixels)) {
            for lane in PetalLane.decodeOrder {
                if let result = frame.lane(lane) { XCTAssertEqual(result.data, truth.lane(lane), "lane \(lane.letter)") }
            }
        }
    }

    /// The image warped by barrel distortion about its centre, as a cheap lens
    /// does: pixel `p` shows what an undistorted camera saw at
    /// `c + (p − c) / (1 + k r²/s²)`.
    private func barrel(_ image: PetalLuma, _ k: Double) throws -> PetalLuma {
        let width = image.width
        let height = image.height
        let cx = Double(width) / 2
        let cy = Double(height) / 2
        let scale = 0.8 * Double(max(width, height))
        let scale2 = scale * scale
        var pixels = [UInt8](repeating: 0, count: width * height)
        for y in 0..<height {
            for x in 0..<width {
                let dx = Double(x) + 0.5 - cx
                let dy = Double(y) + 0.5 - cy
                // invert p_d = c + u (1 + k |u|²/s²) for the undistorted offset u
                var ux = dx
                var uy = dy
                for _ in 0..<8 {
                    let f = 1.0 + k * (ux * ux + uy * uy) / scale2
                    ux = dx / f
                    uy = dy / f
                }
                pixels[y * width + x] = UInt8(image.sample(x: cx + ux, y: cy + uy).rounded())
            }
        }
        return try PetalLuma(width: width, height: height, pixels: pixels)
    }

    func testLensDistortionIsFoundWhenThePlainPoseMissesThePetals() throws {
        // a small, strongly distorted picture: 384 px of code with the corner blossoms far out
        let (encoder, image) = try setup(6)
        // downscale 768 → 384 by averaging 2×2 blocks, then frame it in 640×480
        var small = [UInt8](repeating: 0, count: 640 * 480)
        for y in 0..<384 {
            for x in 0..<384 {
                var sum: UInt32 = 0
                for (i, j) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    sum += UInt32(image.pixels[(2 * y + j) * 768 + 2 * x + i])
                }
                small[(y + 48) * 640 + x + 128] = UInt8((sum + 2) / 4)
            }
        }
        let distorted = try barrel(try PetalLuma(width: 640, height: 480, pixels: small), -0.12)
        // the plain pose misses petals at the edge of the field
        let quad = try XCTUnwrap(PetalLocator.locate(distorted), "four finders")
        let plainPose = try XCTUnwrap(PetalPose.fit(
            from: PetalLayout.finderCenters,
            to: quad.map { PetalPoint(x: $0.x, y: $0.y) },
            distortion: 0,
            width: distorted.width,
            height: distorted.height
        ))
        let plain = try XCTUnwrap(PetalDecoder.decode(distorted, pose: plainPose), "levels")
        XCTAssertTrue(plain.fields.contains { $0 == nil }, plain.laneLetters)
        let decoded = try PetalDecoder.decode(distorted)
        XCTAssertLessThan(decoded.pose.distortion, 0)
        assertAllLanes(decoded, encoder.laneData(frame: 6), "barrel")
    }

    func testTrackingFollowsASmallMovementAndGivesUpOnAJump() throws {
        let (encoder, rendered) = try setup(6)
        let image = try padded(rendered, 100)
        let first = try PetalDecoder.decode(image)
        let moved = try shifted(image, 9, -6)
        let followed = try XCTUnwrap(PetalDecoder.track(moved, previous: first), "tracks a 9 px move")
        assertAllLanes(followed, encoder.laneData(frame: 6), "tracked")
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

    func testBrokenPosesAreRefusedWithoutPanicking() throws {
        let (_, image) = try setup(4)
        let previous = try PetalDecoder.decode(image)
        XCTAssertNotNil(PetalDecoder.track(image, previous: previous))
        // a good pose that names a corner that does not exist
        for corner in [4, 255, -1] {
            let named = PetalDecodedFrame(
                pose: previous.pose,
                rotation: previous.rotation,
                mirrored: previous.mirrored,
                d: previous.d,
                fields: previous.fields,
                inferredCorner: corner
            )
            XCTAssertNil(PetalDecoder.track(image, previous: named), "\(corner)")
        }
        let nonFinite = [
            try PetalHomography(elements: [Double](repeating: .nan, count: 9)),
            try PetalHomography(elements: [.infinity, 0, 0, 0, 1, 0, 0, 0, 1]),
        ]
        for broken in nonFinite {
            XCTAssertNil(PetalDecoder.decode(image, pose: .plain(broken)))
        }
        // the last one makes every finder far larger than the image
        let huge = try PetalHomography(elements: [50, 0, 0, 0, 50, 0, 0, 0, 1])
        for broken in nonFinite + [huge] {
            for inferred in [nil, 2] as [Int?] {
                let pose = PetalDecodedFrame(
                    pose: .plain(broken),
                    rotation: previous.rotation,
                    mirrored: previous.mirrored,
                    d: previous.d,
                    fields: previous.fields,
                    inferredCorner: inferred
                )
                XCTAssertNil(PetalDecoder.track(image, previous: pose), "\(broken.elements) \(String(describing: inferred))")
            }
        }
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
        let pose = PetalPose.plain(.identity)
        // even light: the hidden corner (3) gets levels between the others'
        let even = try levelCard([(230, 30), (220, 25), (210, 20), (0, 0)])
        let reference = try XCTUnwrap(even.withView { PetalDecoder.referenceLevels($0, pose, inferred: 3) })
        XCTAssertGreaterThanOrEqual(reference.lit[3] - reference.dark[3], 12.0)
        // the hidden corner's neighbours disagree (one dim, one veiled): the estimates cross
        let uneven = try levelCard([(60, 45), (250, 20), (200, 185), (0, 0)])
        XCTAssertNil(uneven.withView { PetalDecoder.referenceLevels($0, pose, inferred: 3) })
        // with every corner seen, the same light is fine
        let seen = try levelCard([(60, 45), (250, 20), (200, 185), (240, 20)])
        XCTAssertNotNil(seen.withView { PetalDecoder.referenceLevels($0, pose, inferred: nil) })
    }

    func testInferredLevelsAreTheParallelogramOfTheOtherThreeWithinTheirRange() throws {
        let (_, image) = try setup(5)
        let pose = try renderPose(image.width)
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
        let truth = encoder.laneData(frame: 8)
        XCTAssertEqual(decoded.d?.data, truth.d)
        XCTAssertEqual(decoded.beacon?.meta, encoder.meta, "frame 8 carries the beacon")
        for sector in 0..<PetalLayout.sectors {
            if let field = decoded.fields[sector] { XCTAssertEqual(field.data, truth.fields[sector], "sector \(sector)") }
        }
    }

    // MARK: - Merging captures

    func testMergingOneCaptureReadsTheFieldLanesDecodeRead() throws {
        let (_, clean) = try setup(5)
        let (_, hidden) = try hiddenBlossom(2, corner: 3)
        for (name, image) in [("clean", clean), ("hidden corner", hidden)] {
            let frame = try PetalDecoder.decode(image)
            XCTAssertFalse(frame.fields.contains { $0 == nil }, name)
            let evidence = try XCTUnwrap(PetalDecoder.fieldEvidence(image, frame: frame), name)
            XCTAssertEqual(PetalDecoder.mergeFields([evidence]), frame.fields, name)
        }
    }

    func testAveragingCapturesCancelsTheirIndependentNoise() throws {
        let (encoder, image) = try setup(5)
        let truth = encoder.laneData(frame: 5)
        let frame = try PetalDecoder.decode(image)
        let clean = try XCTUnwrap(PetalDecoder.fieldEvidence(image, frame: frame))
        var rng = PetalXorshift32(seed: 9)
        // uniform noise of ±0.6 of the lit level per cell: about one bit in eight flips in one
        // capture, far more than a field codeword corrects
        let captures = (0..<4).map { _ in
            PetalFieldEvidence(levels: clean.levels.map { $0 + (Double(rng.nextByte()) - 127.5) / 213.0 })
        }
        for capture in captures {
            XCTAssertTrue(
                PetalDecoder.mergeFields([capture]).allSatisfy { $0 == nil },
                "one noisy capture must not read a field lane, or the test proves nothing"
            )
        }
        let merged = PetalDecoder.mergeFields(captures)
        for sector in 0..<PetalLayout.sectors {
            XCTAssertEqual(merged[sector]?.data, truth.fields[sector], "sector \(sector)")
        }
        // skipped sectors are not read
        let skipped = PetalDecoder.mergeFields(captures, skip: [true, false, true, true])
        XCTAssertNil(skipped[0])
        XCTAssertNotNil(skipped[1])
        XCTAssertNil(skipped[2])
    }

    func testEvidenceNeedsAUsableImageAndARealCorner() throws {
        let (_, image) = try setup(5)
        let frame = try PetalDecoder.decode(image)
        XCTAssertNil(PetalDecoder.fieldEvidence(try PetalLuma(width: 20, height: 20), frame: frame))
        let named = PetalDecodedFrame(
            pose: frame.pose,
            rotation: frame.rotation,
            mirrored: frame.mirrored,
            d: frame.d,
            fields: frame.fields,
            inferredCorner: 7
        )
        XCTAssertNil(PetalDecoder.fieldEvidence(image, frame: named))
        XCTAssertEqual(PetalDecoder.mergeFields([]), [nil, nil, nil, nil])
    }

    // MARK: - Golden captures (`fixtures/petal/petal_captures_v2.json`)

    private func captures() throws -> [String: Any] {
        try XCTUnwrap(Support.capturesFixture, "fixtures/petal/petal_captures_v2.json is missing")
    }

    /// The recorded data of every lane: lane `D`, then the four field lanes.
    private func truth(_ entry: [String: Any]) throws -> PetalLaneData {
        PetalLaneData(
            d: try Support.bytes(entry, "d_data"),
            fields: try XCTUnwrap(entry["field_data"] as? [String]).map(Support.hex)
        )
    }

    /// Checks every decoded lane against the recorded data and returns the lane letters.
    private func readLanes(_ name: String, _ frame: PetalDecodedFrame, _ truth: PetalLaneData) -> String {
        for lane in PetalLane.decodeOrder {
            if let result = frame.lane(lane) {
                XCTAssertEqual(result.data, truth.lane(lane), "\(name): lane \(lane.letter) data")
            }
        }
        return frame.laneLetters
    }

    func testGoldenCapturesDecodeAsRecorded() throws {
        let doc = try captures()
        var assembler = PetalStreamAssembler()
        let entries = try Support.objects(doc, "captures")
        XCTAssertEqual(entries.count, 12)
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
                format: "petal capture %@ %dx%d: lanes %@ (reference %@), distortion %.3f, in %.1f ms",
                name, image.width, image.height, decoded.laneLetters, reference, decoded.pose.distortion, elapsed
            ))
            XCTAssertEqual(decoded.mirrored, capture["mirrored"] as? Bool, "\(name): mirror flag")
            let read = readLanes(name, decoded, try truth(capture))
            let must = try Support.string(capture, "must_decode")
            XCTAssertTrue(must.allSatisfy { read.contains($0) }, "\(name): read \(read), must read \(must)")
            XCTAssertEqual(read, reference, "\(name): lanes read")
            XCTAssertEqual(
                decoded.inferredCorner,
                try Support.optionalInteger(capture, "inferred_corner"),
                "\(name): inferred corner"
            )
            decoded.feed(&assembler)
        }
        // captures of different frames of the same stream accumulate in one assembler
        XCTAssertGreaterThan(assembler.progress.atomsReceived, 100)
        let payload = try Support.bytes(doc, "payload_hex")
        let encoder = try PetalStreamEncoder(payload: payload, kind: UInt8(try Support.integer(doc, "payload_kind")))
        XCTAssertEqual(assembler.progress.meta, encoder.meta)
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
            let read = readLanes(name, followed, try truth(pair))
            let must = try Support.string(pair, "must_track")
            XCTAssertTrue(must.allSatisfy { read.contains($0) }, "\(name): tracked \(read), must track \(must)")
            XCTAssertEqual(read, reference, "\(name): lanes tracked")
            XCTAssertEqual(
                followed.inferredCorner,
                try Support.optionalInteger(pair, "inferred_corner"),
                "\(name): inferred corner"
            )
            XCTAssertEqual(followed.rotation, previous.rotation, name)
            XCTAssertEqual(followed.mirrored, previous.mirrored, name)
        }
    }

    func testNegativeCapturesAreRejected() throws {
        let negatives = try Support.objects(try captures(), "negatives")
        XCTAssertEqual(negatives.count, 2)
        for negative in negatives {
            let name = try Support.string(negative, "name")
            XCTAssertThrowsError(try PetalDecoder.decode(try Support.luma(of: negative)), name)
        }
    }

    func testRecordedLaneDataIsAValidCodewordOfItsStream() throws {
        let doc = try captures()
        let encoder = try PetalStreamEncoder(payload: try Support.bytes(doc, "payload_hex"), kind: 2)
        for capture in try Support.objects(doc, "captures") {
            let recorded = try truth(capture)
            for lane in PetalLane.decodeOrder {
                let data = recorded.lane(lane)
                XCTAssertEqual(try lane.decode(try lane.encode(data)), data)
            }
            XCTAssertEqual(recorded, encoder.laneData(frame: UInt16(try Support.integer(capture, "frame"))))
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

import XCTest
@testable import IrohaSwift

/// Ports of the `image`, `geometry`, `locate` and `render` unit tests of
/// `crates/iroha_petal`, plus draw-list checks.
final class PetalImageTests: XCTestCase {
    // MARK: - Luma

    func testBilinearSamplingInterpolatesBetweenPixelCentres() throws {
        let image = try PetalLuma(width: 2, height: 1, pixels: [0, 100])
        XCTAssertEqual(image.sample(x: 0.5, y: 0.5), 0, accuracy: 1e-9)
        XCTAssertEqual(image.sample(x: 1.5, y: 0.5), 100, accuracy: 1e-9)
        XCTAssertEqual(image.sample(x: 1.0, y: 0.5), 50, accuracy: 1e-9)
        XCTAssertEqual(image.sample(x: -5.0, y: 9.0), 0, accuracy: 1e-9)
        XCTAssertTrue(image.sample(x: .nan, y: 0.5).isNaN, "NaN propagates like the reference")
        XCTAssertEqual(try PetalLuma(width: 0, height: 0).sample(x: 1, y: 1), 0)
    }

    func testStridedPlanesDropThePadding() throws {
        let plane: [UInt8] = [1, 2, 9, 9, 3, 4, 9, 9]
        let image = try PetalLuma(width: 2, height: 2, bytesPerRow: 4, plane: plane)
        XCTAssertEqual(image.pixels, [1, 2, 3, 4])
        XCTAssertThrowsError(try PetalLuma(width: 2, height: 2, bytesPerRow: 1, plane: plane))
        XCTAssertThrowsError(try PetalLuma(width: 2, height: 3, bytesPerRow: 4, plane: plane))
        // the last row needs only `width` bytes
        XCTAssertEqual(try PetalLuma(width: 2, height: 2, bytesPerRow: 4, plane: Array(plane[..<6])).pixels, [1, 2, 3, 4])
    }

    func testLumaRejectsInconsistentDimensions() {
        XCTAssertThrowsError(try PetalLuma(width: 100, height: 100, pixels: [0, 0, 0, 0, 0])) {
            XCTAssertEqual($0 as? PetalImageError, .invalidDimensions)
        }
        XCTAssertThrowsError(try PetalLuma(width: -1, height: 4))
        XCTAssertThrowsError(try PetalLuma(width: Int.max, height: 2))
        XCTAssertNil(try PetalLuma(width: 2, height: 2).pixel(x: 2, y: 0))
    }

    func testRGBLumaUsesRec601Weights() {
        let rgb = PetalRGBImage(width: 3, height: 1, pixels: [255, 0, 0, 0, 255, 0, 0, 0, 255])
        XCTAssertEqual(rgb.luma().pixels, [76, 150, 29])
    }

    // MARK: - Homography

    func testFourPointsAreMappedExactly() throws {
        let source = [PetalPoint(x: 0, y: 0), PetalPoint(x: 1024, y: 0), PetalPoint(x: 1024, y: 1024), PetalPoint(x: 0, y: 1024)]
        let destination = [PetalPoint(x: 103.5, y: 40.25), PetalPoint(x: 590, y: 70), PetalPoint(x: 560, y: 420), PetalPoint(x: 80, y: 380)]
        let h = try XCTUnwrap(PetalHomography.fit(from: source, to: destination))
        for (s, d) in zip(source, destination) {
            let mapped = h.apply(s)
            XCTAssertEqual(mapped.x, d.x, accuracy: 1e-7)
            XCTAssertEqual(mapped.y, d.y, accuracy: 1e-7)
        }
    }

    func testInverseRoundtripsAndLeastSquaresAveragesNoise() throws {
        let truth = try PetalHomography(elements: [0.4, -0.1, 130.0, 0.12, 0.38, 60.0, 1e-4, -2e-5, 1.0])
        let source = (0..<30).map { PetalPoint(x: 50 + 31 * Double($0 % 6), y: 90 + 47 * Double($0 / 6)) }
        let destination = source.enumerated().map { index, point -> PetalPoint in
            let mapped = truth.apply(point)
            let jitter = index % 2 == 0 ? 0.05 : -0.05
            return PetalPoint(x: mapped.x + jitter, y: mapped.y - jitter)
        }
        let fit = try XCTUnwrap(PetalHomography.fit(from: source, to: destination))
        for point in source {
            let a = truth.apply(point)
            let b = fit.apply(point)
            XCTAssertEqual(a.x, b.x, accuracy: 0.1)
            XCTAssertEqual(a.y, b.y, accuracy: 0.1)
        }
        let inverse = try XCTUnwrap(truth.inverse())
        let back = inverse.apply(truth.apply(PetalPoint(x: 300, y: 200)))
        XCTAssertEqual(back.x, 300, accuracy: 1e-6)
        XCTAssertEqual(back.y, 200, accuracy: 1e-6)
        XCTAssertEqual(PetalHomography.identity.compose(truth), truth)
        XCTAssertEqual(truth.elements.count, 9)
    }

    func testDegenerateHomographyInputsAreRejected() {
        let p = [PetalPoint](repeating: PetalPoint(x: 1, y: 1), count: 4)
        XCTAssertNil(PetalHomography.fit(from: p, to: p))
        XCTAssertNil(PetalHomography.fit(from: Array(p[..<3]), to: Array(p[..<3])))
        XCTAssertNil(try PetalHomography(elements: [0, 0, 0, 0, 0, 0, 0, 0, 0]).inverse())
        XCTAssertThrowsError(try PetalHomography(elements: [1, 2, 3]))
    }

    // MARK: - Locator

    private func cleanFrameLuma(size: Int) throws -> PetalLuma {
        let encoder = try PetalStreamEncoder(payload: [UInt8](repeating: 9, count: 200), kind: 1)
        return try PetalTestSupport.render(encoder, frame: 1, size: size, supersample: 2)
    }

    func testFindsTheFourCornerBlossomsInACleanRender() throws {
        let quad = try XCTUnwrap(PetalLocator.locate(cleanFrameLuma(size: 512)))
        let expected = [(36.0, 36.0), (476.0, 36.0), (476.0, 476.0), (36.0, 476.0)]
        XCTAssertEqual(quad.count, 4)
        for (finder, (ex, ey)) in zip(quad, expected) {
            XCTAssertEqual(finder.x, ex, accuracy: 1.5, "\(finder)")
            XCTAssertEqual(finder.y, ey, accuracy: 1.5, "\(finder)")
            XCTAssertEqual(finder.size, 60, accuracy: 6, "size \(finder.size)")
        }
    }

    func testComponentsAreLabelledWithCorrectGeometry() {
        var mask = [Bool](repeating: false, count: 64)
        for y in 1..<4 {
            for x in 2..<6 { mask[y * 8 + x] = true }
        }
        mask[6 * 8 + 6] = true
        let components = PetalLocator.labelComponents(mask, width: 8, height: 8)
        XCTAssertEqual(components.count, 2)
        let big = components[0]
        XCTAssertEqual(big.area, 12)
        XCTAssertEqual([big.minX, big.maxX, big.minY, big.maxY], [2, 5, 1, 3])
        XCTAssertEqual(big.centroid.x, 4.0, accuracy: 1e-9)
        XCTAssertEqual(big.centroid.y, 2.5, accuracy: 1e-9)
        XCTAssertEqual(components[1].area, 1)
        XCTAssertEqual(PetalLocator.labelComponents(mask, width: 7, height: 8), [], "size mismatch")
    }

    func testLabellingMergesUShapedComponents() {
        // A "U": the two arms get different provisional labels and merge at the bottom.
        let rows = [
            "#...#",
            "#...#",
            "#####",
        ]
        let mask = rows.flatMap { $0.map { $0 == "#" } }
        let components = PetalLocator.labelComponents(mask, width: 5, height: 3)
        XCTAssertEqual(components.count, 1)
        XCTAssertEqual(components[0].area, 9)
    }

    func testOrderingIsClockwiseFromTheTopLeft() throws {
        func f(_ x: Double, _ y: Double) -> PetalFinder { PetalFinder(x: x, y: y, size: 10) }
        let quad = try XCTUnwrap(PetalLocator.orderClockwise([f(90, 90), f(10, 12), f(88, 8), f(12, 92)]))
        XCTAssertEqual(quad[0], f(10, 12))
        XCTAssertEqual(quad[1], f(88, 8))
        XCTAssertEqual(quad[2], f(90, 90))
        XCTAssertEqual(quad[3], f(12, 92))
        XCTAssertNil(PetalLocator.orderClockwise([f(0, 0), f(1, 1), f(2, 2), f(3, 3)]), "collinear")
    }

    func testTheLargestCandidatesWinWhenClutterPrecedesThem() throws {
        // twelve small decoys discovered before the four real finders
        var candidates = (0..<12).map { PetalFinder(x: 10 + 7 * Double($0), y: 5, size: 18) }
        candidates += [
            PetalFinder(x: 100, y: 100, size: 60),
            PetalFinder(x: 700, y: 110, size: 62),
            PetalFinder(x: 690, y: 520, size: 58),
            PetalFinder(x: 95, y: 510, size: 61),
        ]
        let quad = try XCTUnwrap(PetalLocator.selectQuad(candidates), "real finders found")
        XCTAssertEqual(quad.map { Int($0.x) }.sorted(), [95, 100, 690, 700])
        // the same holds without the size-class prefilter: decoys of finder size
        // discovered first must not push the real corners out of the ten combined
        var crowd = (0..<12).map { PetalFinder(x: 300 + 9 * Double($0), y: 300, size: 50) }
        crowd += [
            PetalFinder(x: 100, y: 100, size: 60),
            PetalFinder(x: 700, y: 110, size: 62),
            PetalFinder(x: 690, y: 520, size: 58),
            PetalFinder(x: 95, y: 510, size: 61),
        ]
        let crowded = try XCTUnwrap(PetalLocator.selectQuad(crowd))
        XCTAssertEqual(crowded.map { Int($0.x) }.sorted(), [95, 100, 690, 700])
    }

    func testThreeFindersFormingACornerInferTheFourth() throws {
        func blob(_ x: Double, _ y: Double) -> PetalFinder { PetalFinder(x: x, y: y, size: 60) }
        // top-left, top-right and bottom-left of a slightly rotated square, plus clutter
        var finders = [blob(100, 110), blob(540, 90), blob(120, 550)]
        finders += (0..<5).map { PetalFinder(x: 300 + 10 * Double($0), y: 300, size: 14) }
        let triple = try XCTUnwrap(PetalLocator.selectTriple(finders), "a corner of three")
        let fourth = triple.quad[triple.inferred]
        XCTAssertEqual(fourth.x, 560, accuracy: 1e-9)
        XCTAssertEqual(fourth.y, 530, accuracy: 1e-9)
        XCTAssertEqual(triple.inferred, 2, "the inferred corner is bottom-right in clockwise order")
        // three blossoms in a row are no corner
        XCTAssertNil(PetalLocator.selectTriple([blob(0, 0), blob(440, 0), blob(880, 0)]))
        XCTAssertNil(PetalLocator.selectTriple([blob(0, 0), blob(440, 0)]), "two blossoms are no corner")
    }

    func testASmallerBlobAtTheInferredCornerCompletesTheQuad() throws {
        // steep tilt: the far finder is under 0.55 of the largest, but it is where the
        // fourth corner belongs
        let finders = [
            PetalFinder(x: 100, y: 100, size: 64),
            PetalFinder(x: 540, y: 100, size: 60),
            PetalFinder(x: 100, y: 540, size: 62),
            PetalFinder(x: 520, y: 515, size: 30),
        ]
        let strong = PetalLocator.strongFinders(finders)
        XCTAssertEqual(strong.count, 3)
        let triple = try XCTUnwrap(PetalLocator.selectTriple(strong), "triple")
        let full = try XCTUnwrap(
            PetalLocator.completeTriple(finders, quad: triple.quad, missing: triple.inferred),
            "completed"
        )
        XCTAssertTrue(full.contains { abs($0.x - 520) < 1e-9 && abs($0.y - 515) < 1e-9 })
        // nothing near the parallelogram point: no completion
        XCTAssertNil(PetalLocator.completeTriple(strong, quad: triple.quad, missing: triple.inferred))
    }

    func testAHiddenBlossomYieldsAnInferredCandidate() throws {
        let clean = try cleanFrameLuma(size: 512)
        // paint over the bottom-left blossom (centre 36, 476 at this size)
        var pixels = clean.pixels
        for y in 420..<512 {
            for x in 0..<92 { pixels[y * 512 + x] = 0 }
        }
        let candidates = PetalLocator.locateCandidates(try PetalLuma(width: 512, height: 512, pixels: pixels))
        let inferred = try XCTUnwrap(candidates.first { $0.inferred != nil }, "an inferred candidate")
        let corner = inferred.corners[inferred.inferred ?? 0]
        XCTAssertEqual(corner.x, 36, accuracy: 4, "\(corner)")
        XCTAssertEqual(corner.y, 476, accuracy: 4, "\(corner)")
        // the lazy sequence yields the same sets in the same order
        let lazy = Array(PetalLocator.candidates(try PetalLuma(width: 512, height: 512, pixels: pixels)))
        XCTAssertEqual(lazy, candidates)
        // a clean frame: the first candidate is the seen quad that `locate` returns
        XCTAssertEqual(PetalLocator.candidates(clean).first { _ in true }?.corners, PetalLocator.locate(clean))
    }

    func testFollowingFindsAMovedBlossomAndRefusesALostOne() throws {
        let luma = try cleanFrameLuma(size: 512)
        let expected = PetalFinder(x: 48, y: 27, size: 60)
        let found = try XCTUnwrap(PetalLocator.follow(luma, expected: expected), "followed")
        XCTAssertEqual(found.x, 36, accuracy: 1.5, "\(found)")
        XCTAssertEqual(found.y, 36, accuracy: 1.5, "\(found)")
        XCTAssertEqual(found.size, 60)
        // nothing bright near the centre of the canvas corner gap
        XCTAssertNil(PetalLocator.follow(luma, expected: PetalFinder(x: 140, y: 36, size: 30)))
        // degenerate expectations are refused, never crash
        for odd in [
            PetalFinder(x: .nan, y: 36, size: 60),
            PetalFinder(x: 1e300, y: -1e300, size: 60),
            PetalFinder(x: 1e300, y: 36, size: 60),
            PetalFinder(x: -1e300, y: -1e300, size: 60),
            PetalFinder(x: 36, y: .infinity, size: 60),
            PetalFinder(x: 36, y: 36, size: .nan),
            PetalFinder(x: 36, y: 36, size: -5),
            PetalFinder(x: 36, y: 36, size: .infinity),
        ] {
            XCTAssertNil(PetalLocator.follow(luma, expected: odd), "\(odd)")
        }
        // a huge disc just covers the whole image
        _ = PetalLocator.follow(luma, expected: PetalFinder(x: 36, y: 36, size: 1e300))
    }

    func testABlankImageHasNoFinders() throws {
        XCTAssertNil(PetalLocator.locate(try PetalLuma(width: 200, height: 200)))
        XCTAssertEqual(PetalLocator.adaptiveBinarize(try PetalLuma(width: 0, height: 0), sensitivity: 0.12), [])
    }

    func testRefineCenterKeepsDegenerateFinders() throws {
        let image = try PetalLuma(width: 64, height: 64)
        let odd = PetalFinder(x: .nan, y: 3, size: .infinity)
        XCTAssertTrue(PetalLocator.refineCenter(image, finder: odd).x.isNaN)
        let outside = PetalFinder(x: 1_000, y: -1_000, size: 20)
        XCTAssertEqual(PetalLocator.refineCenter(image, finder: outside), outside)
        let far = PetalFinder(x: 9e18, y: -9e18, size: 1e18)
        XCTAssertEqual(PetalLocator.refineCenter(image, finder: far), far)
    }

    // MARK: - Software renderer

    private func cells(_ seed: UInt8) throws -> PetalFrameCells {
        let p = (0..<19).map { UInt8($0) &* 31 &+ seed }
        let k = (0..<83).map { (UInt8($0) &* 17) ^ seed }
        let d = (0..<19).map { (UInt8($0) &* 13) ^ seed }
        return try PetalFrameCells(p: PetalLane.p.encode(p), k: PetalLane.k.encode(k), d: PetalLane.d.encode(d))
    }

    func testFindersAreSolidBlossomsAndCornersAreOtherwiseBlack() throws {
        let image = try PetalRenderer.render(cells(1), options: PetalRenderOptions(size: 256, supersample: 2))
        func at(_ x: Int, _ y: Int) -> UInt8 { image.pixels[(y * 256 + x) * 3] }
        let scale = 256.0 / 1024.0
        let fx = PetalLayout.finderCenters[0].x * scale
        let fy = PetalLayout.finderCenters[0].y * scale
        XCTAssertGreaterThan(at(Int(fx), Int(fy)), 200, "core must be lit")
        XCTAssertGreaterThan(at(Int(fx), Int(fy - 34 * scale)), 200, "upper petal must be lit")
        XCTAssertGreaterThan(at(Int(fx + 20 * scale), Int(fy + 20 * scale)), 200, "blossom body must be lit")
        XCTAssertEqual(at(2, 255), 0)
    }

    func testLightTilesAreBrightAndDarkTilesAreMostlyBlack() throws {
        let frame = try cells(2)
        let image = try PetalRenderer.render(frame, options: PetalRenderOptions(size: 512, supersample: 2))
        let scale = 512.0 / 1024.0
        var lightMeans: [Double] = []
        var darkMeans: [Double] = []
        for tile in 0..<PetalLayout.tileCount {
            let center = PetalLayout.tileCenter(tile)
            let x0 = Int((center.x - 10) * scale)
            let y0 = Int((center.y - 10) * scale)
            var sum = 0
            for j in 0..<10 {
                for i in 0..<10 { sum += Int(image.pixels[((y0 + j) * 512 + x0 + i) * 3]) }
            }
            let mean = Double(sum) / 100
            if frame.light[tile] { lightMeans.append(mean) } else { darkMeans.append(mean) }
        }
        let light = lightMeans.reduce(0, +) / Double(lightMeans.count)
        let dark = darkMeans.reduce(0, +) / Double(darkMeans.count)
        // bold glyphs ink the middle of every tile, so only the ordering is stable
        XCTAssertGreaterThan(light, dark + 25, "light tiles average \(light), dark tiles \(dark)")
    }

    func testLitDotsAreDrawnAndUnlitSlotsAreBlack() throws {
        let frame = try cells(3)
        let image = try PetalRenderer.render(frame, options: PetalRenderOptions(size: 1024, supersample: 1))
        var checked = 0
        for (ring, slots) in PetalLayout.ringSlots.enumerated() {
            for slot in 0..<slots {
                let center = PetalLayout.slotCenter(ring: ring, slot: slot)
                let value = image.pixels[(Int(center.y) * 1024 + Int(center.x)) * 3]
                if frame.dots[PetalLayout.ringOffset(ring) + slot] {
                    XCTAssertGreaterThan(value, 150, "ring \(ring) slot \(slot) should be lit")
                } else {
                    XCTAssertEqual(value, 0, "ring \(ring) slot \(slot) should be dark")
                }
                checked += 1
            }
        }
        XCTAssertEqual(checked, PetalLayout.totalSlots)
    }

    func testRenderRejectsInvalidOptions() throws {
        let frame = try cells(4)
        for options in [PetalRenderOptions(size: 0), PetalRenderOptions(size: 64, supersample: 0),
                        PetalRenderOptions(size: 64, supersample: 5)] {
            XCTAssertThrowsError(try PetalRenderer.render(frame, options: options)) {
                XCTAssertEqual($0 as? PetalImageError, .invalidRenderOptions)
            }
        }
    }

    // MARK: - Draw list

    func testDrawListDescribesEveryElementOfTheFrame() throws {
        let frame = try cells(5)
        let list = PetalDrawList(cells: frame)
        XCTAssertEqual(list.palette, .standard)
        XCTAssertEqual(list.finders.count, 4)
        for (finder, center) in zip(list.finders, PetalLayout.finderCenters) {
            XCTAssertEqual(finder.core.center, center)
            XCTAssertEqual(finder.core.radius, 12)
            XCTAssertEqual(finder.petals.count, 5)
            XCTAssertEqual(finder.notches.count, 5)
            // the first petal points straight up and its notch sits at the tip
            XCTAssertEqual(finder.petals[0].center.x, center.x, accuracy: 1e-9)
            XCTAssertEqual(finder.petals[0].center.y, center.y - 34, accuracy: 1e-9)
            XCTAssertEqual(finder.notches[0].center.y, center.y - 60, accuracy: 1e-9)
            XCTAssertEqual(finder.petals[0].radius, 26)
            XCTAssertEqual(finder.notches[0].radius, 6)
        }
        XCTAssertEqual(list.tiles.count, 256)
        for tile in list.tiles {
            let center = PetalLayout.tileCenter(tile.index)
            XCTAssertEqual(tile.rect, PetalRect(x: center.x - 12.5, y: center.y - 12.5, width: 25, height: 25))
            XCTAssertEqual(tile.glyphBox, PetalRect(x: center.x - 11.5, y: center.y - 11.5, width: 23, height: 23))
            XCTAssertEqual(tile.cornerRadius, 3)
            XCTAssertEqual(tile.light, frame.light[tile.index])
            XCTAssertEqual(tile.glyph, Int(frame.glyph[tile.index]))
            XCTAssertEqual(tile.fill, tile.light ? PetalPalette.standard.light : nil)
            XCTAssertEqual(tile.ink, tile.light ? PetalPalette.standard.ink : PetalPalette.standard.pink)
            XCTAssertEqual(tile.strokeWidth, 6.5 / 32 * 23, accuracy: 1e-12)
            XCTAssertEqual(tile.strokes.count, PetalGlyphs.strokes[tile.glyph].count)
            for (polyline, design) in zip(tile.strokes, PetalGlyphs.strokes[tile.glyph]) {
                for (point, designPoint) in zip(polyline, design) {
                    XCTAssertEqual(point.x, tile.glyphBox.x + designPoint.x * 23 / 32, accuracy: 1e-9)
                    XCTAssertEqual(point.y, tile.glyphBox.y + designPoint.y * 23 / 32, accuracy: 1e-9)
                }
            }
        }
        XCTAssertEqual(list.dots.count, frame.dots.filter { $0 }.count)
        for dot in list.dots {
            let dx = dot.center.x - 512
            let dy = dot.center.y - 512
            let radius = (dx * dx + dy * dy).squareRoot()
            XCTAssertTrue(PetalLayout.ringRadii.contains { abs($0 - radius) < 1e-9 })
            XCTAssertEqual(dot.radius, 11)
        }
    }
}

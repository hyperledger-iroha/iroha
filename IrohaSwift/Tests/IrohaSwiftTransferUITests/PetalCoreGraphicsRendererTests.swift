#if canImport(CoreGraphics) && canImport(SwiftUI)
import CoreGraphics
import IrohaSwift
@testable import IrohaSwiftTransferUI
import SwiftUI
import XCTest
#if canImport(AppKit)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif

/// The CoreGraphics renderer and the SwiftUI player must draw frames the
/// Petal decoder reads back exactly, turn and kana lanes included.
final class PetalCoreGraphicsRendererTests: XCTestCase {
    private static let payload: [UInt8] = (0..<300).map {
        UInt8(truncatingIfNeeded: (UInt32($0) &* 2_654_435_761) >> 11)
    }

    /// Rec. 601 luma of `image`, read through an sRGB RGBA bitmap.
    private func luma(of image: CGImage) throws -> PetalLuma {
        let width = image.width
        let height = image.height
        var rgba = [UInt8](repeating: 0, count: width * height * 4)
        let space = try XCTUnwrap(CGColorSpace(name: CGColorSpace.sRGB))
        try rgba.withUnsafeMutableBytes { buffer in
            let context = try XCTUnwrap(CGContext(
                data: buffer.baseAddress,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width * 4,
                space: space,
                bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
            ))
            context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        }
        let pixels = (0..<(width * height)).map { index -> UInt8 in
            let r = UInt32(rgba[index * 4])
            let g = UInt32(rgba[index * 4 + 1])
            let b = UInt32(rgba[index * 4 + 2])
            return UInt8((299 * r + 587 * g + 114 * b + 500) / 1_000)
        }
        return try PetalLuma(width: width, height: height, pixels: pixels)
    }

    func testCoreGraphicsFramesDecodeEveryLane() throws {
        let encoder = try PetalStreamEncoder(payload: Self.payload, kind: 2)
        for frame in [UInt16(4), 5] {
            let list = PetalDrawList(cells: encoder.cells(frame: frame))
            let image = try XCTUnwrap(PetalCoreGraphicsRenderer.makeImage(list, size: 768))
            XCTAssertEqual(image.width, 768)
            XCTAssertEqual(image.height, PetalRenderer.cardHeightPixels(768))
            let captured = try luma(of: image)
            let decoded = try PetalDecoder.decode(captured)
            let expected = encoder.laneData(frame: frame)
            for lane in PetalLane.decodeOrder {
                XCTAssertEqual(decoded.lane(lane)?.data, expected.lane(lane), "frame \(frame) lane \(lane.letter)")
            }
            XCTAssertEqual(decoded.rotation, 0)
            XCTAssertFalse(decoded.mirrored)
            // the turned, notched petals read as the reference renderer draws them
            let turns = PetalTurns.readTurns(captured, frame: decoded)
            for sector in 0..<PetalLayout.sectors {
                XCTAssertEqual(turns[sector]?.data, expected.turns[sector], "frame \(frame) turn lane \(sector)")
            }
            // the katakana tiles, stroked glyphs and all, read as the reference renderer draws them
            let header = PetalLaneHeader(tag: encoder.meta.tag, frame: frame)
            let kana = PetalKana.readKana(captured, frame: decoded, header: header)
            for lane in PetalKanaLane.allCases {
                XCTAssertEqual(kana[lane.rawValue]?.result.data, expected.kana(lane), "frame \(frame) kana lane \(lane.name)")
            }
        }
    }

    func testThePetalOutlineIsTheNotchedEllipse() {
        let outline = PetalCoreGraphicsRenderer.petalOutline
        let a = PetalDrawList.petalHalfLength
        let b = PetalDrawList.petalHalfWidth
        let r = PetalDrawList.petalNotchRadius
        let d = PetalDrawList.petalNotchDistance
        var checked = 0
        for j in -100...100 {
            for i in -160...160 {
                let x = Double(i) * 0.05
                let y = Double(j) * 0.05
                // points within 0.01 units of either edge may fall either way
                let ellipse = (x / a) * (x / a) + (y / b) * (y / b)
                let notch = ((x - d) * (x - d) + y * y).squareRoot()
                if abs(ellipse.squareRoot() - 1) < 0.01 / b || abs(notch - r) < 0.01 { continue }
                let inside = outline.contains(CGPoint(x: x, y: y), using: .winding)
                XCTAssertEqual(inside, PetalLayout.petalLit(dx: x, dy: y, angle: 0), "(\(x), \(y))")
                checked += 1
            }
        }
        XCTAssertGreaterThan(checked, 60_000)
        // the notch is cut out of the tip, and the back of the petal is whole
        XCTAssertFalse(outline.contains(CGPoint(x: 5.5, y: 0)))
        XCTAssertTrue(outline.contains(CGPoint(x: 5.0, y: 0)))
        XCTAssertTrue(outline.contains(CGPoint(x: -5.5, y: 0)))
    }

    func testCoreGraphicsFrameMatchesTheSoftwareRendererGeometry() throws {
        let encoder = try PetalStreamEncoder(payload: Self.payload, kind: 2)
        let cells = encoder.cells(frame: 6)
        let vector = try luma(of: try XCTUnwrap(
            PetalCoreGraphicsRenderer.makeImage(PetalDrawList(cells: cells), size: 256)
        ))
        let software = try PetalRenderer.render(cells, options: PetalRenderOptions(size: 256, supersample: 4)).luma()
        XCTAssertEqual(vector.height, software.height)
        // Both rasterise the same geometry; only anti-aliased edges may differ.
        var large = 0
        var total = 0
        for (a, b) in zip(vector.pixels, software.pixels) {
            let difference = abs(Int(a) - Int(b))
            total += difference
            if difference > 96 { large += 1 }
        }
        XCTAssertLessThan(Double(total) / Double(vector.pixels.count), 6, "mean absolute difference")
        XCTAssertLessThan(large, vector.pixels.count / 200, "pixels that disagree outright")
        XCTAssertNil(PetalCoreGraphicsRenderer.makeImage(PetalDrawList(cells: cells), size: 0))
    }

    func testFrameScheduleAdvancesAndWraps() {
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: 0, framesPerSecond: 8), 0)
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: 0.124, framesPerSecond: 8), 0)
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: 0.125, framesPerSecond: 8), 1)
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: 10, framesPerSecond: 8, startFrame: 65_530), 74)
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: -3, framesPerSecond: 8, startFrame: 9), 9)
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: .nan, framesPerSecond: 8, startFrame: 9), 9)
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: .infinity, framesPerSecond: 8, startFrame: 9), 9)
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: 1, framesPerSecond: 1_000), 60, "rate is clamped to 60 fps")
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: 1, framesPerSecond: -5), 1, "rate is clamped to 1 fps")
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: 1, framesPerSecond: .nan), 12, "an invalid rate plays the default")
    }

    /// Render the actual SwiftUI hierarchy on every supported Apple deployment target.
    @MainActor
    private func hostedImage<Content: View>(of view: Content, size: CGSize) throws -> CGImage {
        let bounds = CGRect(origin: .zero, size: size)
        #if canImport(AppKit)
        let hostingView = NSHostingView(rootView: view)
        hostingView.frame = bounds
        hostingView.layoutSubtreeIfNeeded()
        let bitmap = try XCTUnwrap(hostingView.bitmapImageRepForCachingDisplay(in: bounds))
        hostingView.cacheDisplay(in: bounds, to: bitmap)
        return try XCTUnwrap(bitmap.cgImage)
        #elseif canImport(UIKit)
        let controller = UIHostingController(rootView: view)
        let window = UIWindow(frame: bounds)
        window.rootViewController = controller
        window.isHidden = false
        defer { window.isHidden = true }
        controller.view.frame = bounds
        controller.view.layoutIfNeeded()
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        let image = UIGraphicsImageRenderer(size: bounds.size, format: format).image { _ in
            XCTAssertTrue(controller.view.drawHierarchy(in: bounds, afterScreenUpdates: true))
        }
        return try XCTUnwrap(image.cgImage)
        #else
        #error("The SwiftUI renderer test requires a supported Apple hosting API")
        #endif
    }

    @MainActor
    func testSwiftUIFrameViewRendersADecodableFrame() throws {
        let encoder = try PetalStreamEncoder(payload: Self.payload, kind: 2)
        // a portrait frame the card fills
        let size = CGSize(width: 480, height: 782)
        let view = PetalFrameView(cells: encoder.cells(frame: 8)).frame(width: size.width, height: size.height)
        // Hosting renders on the package's macOS 12 / iOS 15 deployment targets.
        // Where available, also retain the independent ImageRenderer assertions.
        var images = [("hosted SwiftUI hierarchy", try hostedImage(of: view, size: size))]
        if #available(macOS 13.0, iOS 16.0, *) {
            let renderer = ImageRenderer(content: view)
            renderer.scale = 1
            images.append(("ImageRenderer", try XCTUnwrap(renderer.cgImage)))
        }
        let expected = encoder.laneData(frame: 8)
        for (name, image) in images {
            let captured = try luma(of: image)
            let decoded = try PetalDecoder.decode(captured)
            for lane in PetalLane.decodeOrder {
                XCTAssertEqual(decoded.lane(lane)?.data, expected.lane(lane), "\(name) lane \(lane.letter)")
            }
            XCTAssertEqual(decoded.beacon?.meta, encoder.meta, name)
            let turns = PetalTurns.readTurns(captured, frame: decoded)
            for sector in 0..<PetalLayout.sectors {
                XCTAssertEqual(turns[sector]?.data, expected.turns[sector], "\(name) turn lane \(sector)")
            }
            let kana = PetalKana.readKana(captured, frame: decoded, header: PetalLaneHeader(tag: encoder.meta.tag, frame: 8))
            for lane in PetalKanaLane.allCases {
                XCTAssertEqual(kana[lane.rawValue]?.result.data, expected.kana(lane), "\(name) kana lane \(lane.name)")
            }
        }
        _ = PetalStreamView(encoder: encoder, framesPerSecond: 10)
    }
}
#endif

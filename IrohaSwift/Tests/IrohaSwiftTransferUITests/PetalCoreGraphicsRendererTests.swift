#if canImport(CoreGraphics) && canImport(SwiftUI)
import CoreGraphics
import IrohaSwift
@testable import IrohaSwiftTransferUI
import SwiftUI
import XCTest

/// The CoreGraphics renderer and the SwiftUI player must draw frames the
/// Petal decoder reads back exactly.
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
            let decoded = try PetalDecoder.decode(try luma(of: image))
            let expected = encoder.laneData(frame: frame)
            XCTAssertEqual(decoded.p?.data, expected.p, "frame \(frame) lane P")
            XCTAssertEqual(decoded.k?.data, expected.k, "frame \(frame) lane K")
            XCTAssertEqual(decoded.d?.data, expected.d, "frame \(frame) lane D")
            XCTAssertEqual(decoded.rotation, 0)
            XCTAssertFalse(decoded.mirrored)
        }
    }

    func testCoreGraphicsFrameMatchesTheSoftwareRendererGeometry() throws {
        let encoder = try PetalStreamEncoder(payload: Self.payload, kind: 2)
        let cells = encoder.cells(frame: 6)
        let vector = try luma(of: try XCTUnwrap(
            PetalCoreGraphicsRenderer.makeImage(PetalDrawList(cells: cells), size: 256)
        ))
        let software = try PetalRenderer.render(cells, options: PetalRenderOptions(size: 256, supersample: 4)).luma()
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
        XCTAssertEqual(PetalStreamView.frameNumber(elapsed: 1, framesPerSecond: .nan), 8)
    }

    @MainActor
    func testSwiftUIFrameViewRendersADecodableFrame() throws {
        guard #available(macOS 13.0, iOS 16.0, *) else {
            throw XCTSkip("ImageRenderer needs macOS 13 or iOS 16")
        }
        let encoder = try PetalStreamEncoder(payload: Self.payload, kind: 2)
        let view = PetalFrameView(cells: encoder.cells(frame: 8)).frame(width: 640, height: 640)
        let renderer = ImageRenderer(content: view)
        renderer.scale = 1
        let image = try XCTUnwrap(renderer.cgImage)
        let decoded = try PetalDecoder.decode(try luma(of: image))
        let expected = encoder.laneData(frame: 8)
        XCTAssertEqual(decoded.p?.data, expected.p)
        XCTAssertEqual(decoded.d?.data, expected.d)
        XCTAssertEqual(decoded.beacon?.meta, encoder.meta)
        _ = PetalStreamView(encoder: encoder, framesPerSecond: 10)
    }
}
#endif

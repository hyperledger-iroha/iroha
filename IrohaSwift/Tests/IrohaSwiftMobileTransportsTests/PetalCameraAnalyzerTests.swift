#if canImport(AVFoundation) && canImport(CoreVideo)
import CoreVideo
import IrohaSwift
@testable import IrohaSwiftMobileTransports
import XCTest

private final class PetalOutcomeRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var values: [PetalScanOutcome] = []

    func record(_ outcome: PetalScanOutcome) {
        lock.lock()
        values.append(outcome)
        lock.unlock()
    }

    var outcomes: [PetalScanOutcome] {
        lock.lock()
        defer { lock.unlock() }
        return values
    }
}

/// Camera glue: pixel buffers in, Petal scan outcomes out.
final class PetalCameraAnalyzerTests: XCTestCase {
    private func pixelBuffer(_ format: OSType, width: Int, height: Int) throws -> CVPixelBuffer {
        var buffer: CVPixelBuffer?
        let attributes = [kCVPixelBufferIOSurfacePropertiesKey as String: [String: Any]()] as CFDictionary
        let status = CVPixelBufferCreate(kCFAllocatorDefault, width, height, format, attributes, &buffer)
        XCTAssertEqual(status, kCVReturnSuccess)
        return try XCTUnwrap(buffer)
    }

    /// A bi-planar full-range buffer whose Y plane holds `luma` (chroma neutral).
    private func yuvBuffer(_ luma: PetalLuma) throws -> CVPixelBuffer {
        let buffer = try pixelBuffer(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange, width: luma.width, height: luma.height)
        CVPixelBufferLockBaseAddress(buffer, [])
        defer { CVPixelBufferUnlockBaseAddress(buffer, []) }
        let y = try XCTUnwrap(CVPixelBufferGetBaseAddressOfPlane(buffer, 0)).assumingMemoryBound(to: UInt8.self)
        let stride = CVPixelBufferGetBytesPerRowOfPlane(buffer, 0)
        for row in 0..<luma.height {
            for column in 0..<luma.width {
                y[row * stride + column] = luma.pixels[row * luma.width + column]
            }
            // poison the row padding: it must never reach the decoder
            for column in luma.width..<stride { y[row * stride + column] = 0xFF }
        }
        let chroma = try XCTUnwrap(CVPixelBufferGetBaseAddressOfPlane(buffer, 1)).assumingMemoryBound(to: UInt8.self)
        let chromaBytes = CVPixelBufferGetBytesPerRowOfPlane(buffer, 1) * CVPixelBufferGetHeightOfPlane(buffer, 1)
        for index in 0..<chromaBytes { chroma[index] = 128 }
        return buffer
    }

    /// A 640×480 camera frame showing `frame` as a 400 px code.
    private func cameraLuma(_ encoder: PetalStreamEncoder, _ frame: UInt16) throws -> PetalLuma {
        let code = try PetalRenderer.render(
            encoder.cells(frame: frame),
            options: PetalRenderOptions(size: 400, supersample: 2)
        ).luma()
        var pixels = [UInt8](repeating: 10, count: 640 * 480)
        for y in 0..<code.height {
            for x in 0..<code.width {
                pixels[(y + 40) * 640 + x + 120] = code.pixels[y * code.width + x]
            }
        }
        return try PetalLuma(width: 640, height: 480, pixels: pixels)
    }

    func testBiPlanarBuffersYieldTheirYPlane() throws {
        let source = try PetalLuma(width: 6, height: 4, pixels: (0..<24).map { UInt8($0 * 10) })
        let luma = try PetalCameraFrame.luma(from: try yuvBuffer(source))
        XCTAssertEqual(luma, source)
    }

    func testBGRABuffersUseRec601Weights() throws {
        let buffer = try pixelBuffer(kCVPixelFormatType_32BGRA, width: 3, height: 2)
        CVPixelBufferLockBaseAddress(buffer, [])
        let base = try XCTUnwrap(CVPixelBufferGetBaseAddress(buffer)).assumingMemoryBound(to: UInt8.self)
        let stride = CVPixelBufferGetBytesPerRow(buffer)
        // red, green, blue / white, black, grey (BGRA byte order)
        let rows: [[[UInt8]]] = [
            [[0, 0, 255, 255], [0, 255, 0, 255], [255, 0, 0, 255]],
            [[255, 255, 255, 255], [0, 0, 0, 255], [100, 100, 100, 255]],
        ]
        for (y, row) in rows.enumerated() {
            for (x, pixel) in row.enumerated() {
                for (channel, value) in pixel.enumerated() { base[y * stride + x * 4 + channel] = value }
            }
        }
        CVPixelBufferUnlockBaseAddress(buffer, [])
        let luma = try PetalCameraFrame.luma(from: buffer)
        XCTAssertEqual(luma.pixels, [76, 150, 29, 255, 0, 100])
    }

    func testUnsupportedPixelFormatsAreRejected() throws {
        let buffer = try pixelBuffer(kCVPixelFormatType_32ARGB, width: 4, height: 4)
        XCTAssertThrowsError(try PetalCameraFrame.luma(from: buffer)) {
            XCTAssertEqual($0 as? PetalCameraFrameError, .unsupportedPixelFormat(kCVPixelFormatType_32ARGB))
        }
    }

    func testAnalyzerReceivesAStreamFromCameraBuffers() async throws {
        let data = (0..<400).map { UInt8(truncatingIfNeeded: $0 &* 7 &+ 3) }
        let encoder = try PetalStreamEncoder(payload: data, kind: 5)
        let recorder = PetalOutcomeRecorder()
        let analyzer = PetalCameraAnalyzer(onOutcome: { recorder.record($0) })
        var completed: PetalCompletedPayload?
        for frame in UInt16(0)..<30 {
            let outcome = try XCTUnwrap(try analyzer.analyze(
                try yuvBuffer(try cameraLuma(encoder, frame)),
                nowMilliseconds: UInt64(frame) * 125
            ))
            XCTAssertNil(outcome.error, "frame \(frame)")
            if let payload = outcome.completed {
                completed = payload
                break
            }
        }
        let payload = try XCTUnwrap(completed)
        XCTAssertEqual([UInt8](payload.payload), data)
        XCTAssertEqual(payload.meta.kind, 5)
        XCTAssertTrue(analyzer.isCompleted)
        XCTAssertTrue(analyzer.progress.complete)
        XCTAssertEqual(analyzer.stats.frames, UInt32(recorder.outcomes.count))
        XCTAssertNotNil(recorder.outcomes.last?.completed)

        // frames after completion are ignored until reset
        XCTAssertNil(try analyzer.analyze(try yuvBuffer(try cameraLuma(encoder, 1)), nowMilliseconds: 9_000))
        analyzer.finish()
        var streamed: [PetalScanOutcome] = []
        for await outcome in analyzer.outcomes { streamed.append(outcome) }
        XCTAssertEqual(streamed, recorder.outcomes)
        analyzer.reset()
        XCTAssertFalse(analyzer.isCompleted)
        XCTAssertNil(analyzer.progress.meta)
        let resumed = try XCTUnwrap(try analyzer.analyze(try yuvBuffer(try cameraLuma(encoder, 4)), nowMilliseconds: 9_500))
        XCTAssertEqual(resumed.progress.meta, encoder.meta)
    }
}
#endif

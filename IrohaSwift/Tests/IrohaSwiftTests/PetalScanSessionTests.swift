import XCTest
@testable import IrohaSwift

/// Ports of the `session` unit tests of `crates/iroha_petal`.
final class PetalScanSessionTests: XCTestCase {
    private typealias Support = PetalTestSupport

    /// A 640×480 camera-like frame showing `frame` as a 400 px code on a dim
    /// background, optionally mirrored like a front-camera preview.
    private func cameraFrame(_ encoder: PetalStreamEncoder, _ frame: UInt16, mirrored: Bool = false) throws -> PetalLuma {
        let code = try Support.render(encoder, frame: frame, size: 400, supersample: 2)
        let width = 640
        let height = 480
        var pixels = [UInt8](repeating: 9, count: width * height)
        for y in 0..<code.height {
            for x in 0..<code.width {
                let sourceX = mirrored ? code.width - 1 - x : x
                pixels[(y + 41) * width + x + 117] = code.pixels[y * code.width + sourceX]
            }
        }
        return try PetalLuma(width: width, height: height, pixels: pixels)
    }

    func testASessionReceivesAPayloadFromCameraFrames() throws {
        let data = Support.payload(500, seed: 3)
        let encoder = try PetalStreamEncoder(payload: data, kind: 2)
        var session = PetalScanSession()
        var done: PetalCompletedPayload?
        // join mid-stream and alternate mirrored previews
        for frame in UInt16(2)..<40 {
            let outcome = session.push(
                try cameraFrame(encoder, frame, mirrored: frame % 3 == 0),
                nowMilliseconds: UInt64(frame) * 125
            )
            XCTAssertNil(outcome.error, "frame \(frame)")
            if let completed = outcome.completed {
                done = completed
                break
            }
        }
        let completed = try XCTUnwrap(done)
        XCTAssertEqual([UInt8](completed.payload), data)
        XCTAssertEqual(completed.meta.kind, 2)
        let stats = session.stats
        XCTAssertGreaterThan(stats.readable, 0)
        XCTAssertGreaterThan(stats.laneD, 0)
        XCTAssertEqual(stats.frames, stats.located)
        XCTAssertTrue(session.progress.complete)
    }

    func testIdleSessionsForgetPartialStreams() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(4_000, seed: 3), kind: 1)
        var session = PetalScanSession(limits: PetalScanLimits(idleTimeoutMilliseconds: 1_000))
        let first = session.push(try cameraFrame(encoder, 0), nowMilliseconds: 0)
        XCTAssertNil(first.error)
        XCTAssertGreaterThan(session.progress.rank, 0)
        // a frame much later with nothing readable resets the session first
        let outcome = session.push(try PetalLuma(width: 640, height: 480), nowMilliseconds: 60_000)
        XCTAssertEqual(outcome.error, .noFinders)
        XCTAssertEqual(outcome.progress.rank, 0)
        XCTAssertNil(outcome.progress.meta)
    }

    func testAbsoluteTimeoutRestartsAStreamThatKeepsProgressing() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(4_000, seed: 4), kind: 1)
        var session = PetalScanSession(limits: PetalScanLimits(
            idleTimeoutMilliseconds: 1_000,
            absoluteTimeoutMilliseconds: 2_000
        ))
        XCTAssertNotNil(session.push(try cameraFrame(encoder, 0), nowMilliseconds: 0).progress.meta)
        let second = session.push(try cameraFrame(encoder, 1), nowMilliseconds: 900)
        XCTAssertNotNil(second.progress.meta)
        XCTAssertGreaterThan(second.progress.rank, 6)
        let third = session.push(try cameraFrame(encoder, 2), nowMilliseconds: 1_800)
        XCTAssertNotNil(third.progress.meta, "steady progress keeps the idle timer alive")
        // 2.5 s after the start the stream is dropped before the frame is read;
        // frame 3 carries no beacon, so its atoms wait for the next one.
        let fourth = session.push(try cameraFrame(encoder, 3), nowMilliseconds: 2_500)
        XCTAssertEqual(fourth.lanes, "PKD")
        XCTAssertNil(fourth.progress.meta)
        XCTAssertEqual(fourth.progress.rank, 0)
        let fifth = session.push(try cameraFrame(encoder, 4), nowMilliseconds: 2_600)
        XCTAssertEqual(fifth.progress.meta, encoder.meta)
        XCTAssertEqual(fifth.progress.rank, 7 + 6, "buffered atoms of frame 3 join the restarted stream")
    }

    func testLocatedCountsCodesThatWereSeenButCouldNotBeRead() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(100, seed: 3), kind: 1)
        let frame = try Support.render(encoder, frame: 1, size: 512, supersample: 2)
        // keep only the four blossoms: finders are located, no lane can be read
        let scale = 512.0 / 1024.0
        var pixels = frame.pixels
        for y in 0..<512 {
            for x in 0..<512 {
                let nearFinder = PetalLayout.finderCenters.contains { center in
                    let dx = Double(x) - center.x * scale
                    let dy = Double(y) - center.y * scale
                    return (dx * dx + dy * dy).squareRoot() < 34
                }
                if !nearFinder { pixels[y * 512 + x] = 0 }
            }
        }
        var session = PetalScanSession()
        let outcome = session.push(try PetalLuma(width: 512, height: 512, pixels: pixels), nowMilliseconds: 0)
        XCTAssertEqual(outcome.error, .noOrientation)
        XCTAssertEqual(session.stats.located, 1)
        XCTAssertEqual(session.stats.readable, 0)
        // a frame with no code at all is not "located"
        _ = session.push(try PetalLuma(width: 320, height: 240), nowMilliseconds: 100)
        XCTAssertEqual(session.stats.located, 1)
        XCTAssertEqual(session.stats.frames, 2)
    }

    func testUnreadableFramesDoNotDisturbProgress() throws {
        var session = PetalScanSession()
        let outcome = session.push(try PetalLuma(width: 320, height: 240), nowMilliseconds: 5)
        XCTAssertNil(outcome.completed)
        XCTAssertTrue(outcome.lanes.isEmpty)
        XCTAssertEqual(session.stats.frames, 1)
        XCTAssertEqual(session.stats.located, 0)
        let tiny = session.push(try PetalLuma(width: 8, height: 8), nowMilliseconds: 6)
        XCTAssertEqual(tiny.error, .unsupportedImage)
        XCTAssertEqual(session.stats.frames, 2)
        XCTAssertEqual(session.progress, PetalProgress())
    }

    func testResetDropsPartialState() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(900, seed: 5), kind: 7)
        var session = PetalScanSession()
        _ = session.push(try cameraFrame(encoder, 4), nowMilliseconds: 10)
        XCTAssertEqual(session.progress.meta, encoder.meta)
        session.reset()
        XCTAssertNil(session.progress.meta)
        XCTAssertEqual(session.stats.frames, 1, "statistics survive a reset")
    }
}

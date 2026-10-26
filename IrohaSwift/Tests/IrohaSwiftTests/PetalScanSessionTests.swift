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

    /// A 640×480 camera frame of a steady hand: `frame` rendered at 512 px,
    /// shown about 400 px wide and turned by `degrees` about the frame centre
    /// (bilinear resampling), on a dim background.
    private func turnedFrame(
        _ encoder: PetalStreamEncoder, _ frame: UInt16, degrees: Double, unturned: Bool = false
    ) throws -> PetalLuma {
        let cells: PetalFrameCells
        if unturned {
            // the frame's field and rings with every petal unturned: no turn lane reads
            let words = encoder.words(frame: frame)
            cells = try PetalFrameCells(d: words.d, fields: words.fields)
        } else {
            cells = encoder.cells(frame: frame)
        }
        let code = try PetalRenderer.renderSquare(cells, options: PetalRenderOptions(size: 512, supersample: 2)).luma()
        let width = 640
        let height = 480
        let angle = degrees * Double.pi / 180
        let scale = 400.0 / 512.0
        var pixels = [UInt8](repeating: 9, count: width * height)
        for y in 0..<height {
            for x in 0..<width {
                let u = Double(x) + 0.5 - 320
                let v = Double(y) + 0.5 - 240
                let sx = (cos(angle) * u + sin(angle) * v) / scale + 256
                let sy = (-sin(angle) * u + cos(angle) * v) / scale + 256
                if sx >= 0, sy >= 0, sx < 512, sy < 512 {
                    pixels[y * width + x] = UInt8(code.sample(x: sx, y: sy).rounded())
                }
            }
        }
        return try PetalLuma(width: width, height: height, pixels: pixels)
    }

    func testASteadyCameraIsTrackedAfterTheFirstFrame() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(300, seed: 3), kind: 2)
        var session = PetalScanSession()
        for frame in UInt16(0)..<6 {
            let outcome = session.push(try turnedFrame(encoder, frame, degrees: 8), nowMilliseconds: UInt64(frame) * 125)
            XCTAssertNil(outcome.error, "frame \(frame)")
        }
        XCTAssertEqual(session.stats.readable, 6)
        XCTAssertEqual(session.stats.tracked, 5, "every frame after the first follows the pose")
        XCTAssertEqual(session.stats.inferred, 0)
        // a pause longer than the tracking window forces a full search again
        _ = session.push(
            try turnedFrame(encoder, 6, degrees: 8),
            nowMilliseconds: 5 * 125 + PetalScanSession.trackWindowMilliseconds + 1
        )
        XCTAssertEqual(session.stats.tracked, 5)
        XCTAssertEqual(session.stats.readable, 7)
        // a reset forgets the pose as well
        session.reset()
        _ = session.push(try turnedFrame(encoder, 7, degrees: 8), nowMilliseconds: 5 * 125 + 600)
        XCTAssertEqual(session.stats.tracked, 5)
        XCTAssertEqual(session.stats.readable, 8)
    }

    func testAHiddenBlossomIsCountedAsInferred() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(300, seed: 3), kind: 2)
        let clean = try cameraFrame(encoder, 1)
        // a thumb over the code's bottom-right blossom (pasted at 117, 41, 400 px wide)
        var pixels = clean.pixels
        let cx = 117 + 952.0 * 400 / 1024
        let cy = 41 + 952.0 * 400 / 1024
        for y in 0..<clean.height {
            for x in 0..<clean.width {
                let dx = Double(x) + 0.5 - cx
                let dy = Double(y) + 0.5 - cy
                if dx * dx + dy * dy <= 30 * 30 { pixels[y * clean.width + x] = 9 }
            }
        }
        var session = PetalScanSession()
        let outcome = session.push(try PetalLuma(width: clean.width, height: clean.height, pixels: pixels), nowMilliseconds: 0)
        XCTAssertNil(outcome.error)
        XCTAssertEqual(session.stats.inferred, 1)
        XCTAssertEqual(session.stats.tracked, 0)
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
        XCTAssertGreaterThan(stats.turnLanes, 0)
        XCTAssertEqual(stats.frames, stats.located)
        XCTAssertTrue(session.progress.complete)
    }

    func testTheTurnLanesFinishAPayloadInFewerFrames() throws {
        let data = Support.payload(5_000, seed: 3)
        let encoder = try PetalStreamEncoder(payload: data, kind: 2)
        var session = PetalScanSession()
        var done: PetalCompletedPayload?
        for frame in UInt16(0)..<40 {
            let outcome = session.push(try turnedFrame(encoder, frame, degrees: 20), nowMilliseconds: UInt64(frame) * 100)
            if let completed = outcome.completed {
                done = completed
                break
            }
        }
        XCTAssertEqual(try XCTUnwrap(done).payload, Data(data))
        let stats = session.stats
        XCTAssertTrue(stats.readable > 0 && stats.laneD > 0 && stats.fieldLanes > 0)
        XCTAssertGreaterThan(stats.turnLanes, 0)
        // 5,000 bytes need 313 atoms: the field lanes and lane D alone carry them in 15 or 16
        // frames, the turn lanes on top in 9 or 10
        XCTAssertLessThanOrEqual(stats.frames, 11, "\(stats)")
    }

    func testATurnLaneIsReadOncePerShownFrame() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(3_000, seed: 3), kind: 2)
        var session = PetalScanSession()
        let first = session.push(try turnedFrame(encoder, 5, degrees: 12), nowMilliseconds: 0)
        XCTAssertEqual(first.lanes, "D0123")
        XCTAssertEqual(first.turned, "0123")
        // another capture of the same shown frame: its turn lanes were read already
        let again = session.push(try turnedFrame(encoder, 5, degrees: 12.5), nowMilliseconds: 33)
        XCTAssertEqual(again.lanes, "D0123")
        XCTAssertEqual(again.turned, "")
        XCTAssertEqual(session.stats.turnLanes, 4)
        // the next shown frame's turn lanes are new
        let next = session.push(try turnedFrame(encoder, 6, degrees: 12), nowMilliseconds: 83)
        XCTAssertEqual(next.turned, "0123")
        XCTAssertEqual(session.stats.turnTries, 8)
        XCTAssertEqual(session.stats.turnLanes, 8)
        XCTAssertEqual(session.tried.count, 8)
        session.reset()
        XCTAssertTrue(session.tried.isEmpty)
    }

    func testTriesThatReadNothingBackOff() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(3_000, seed: 3), kind: 2)
        var session = PetalScanSession()
        var triedAt: [UInt16] = []
        for frame in UInt16(1)...30 {
            let before = session.stats.turnTries
            let outcome = session.push(
                try turnedFrame(encoder, frame, degrees: 12, unturned: true),
                nowMilliseconds: UInt64(frame) * 83
            )
            XCTAssertTrue(outcome.lanes.hasSuffix("0123"), "frame \(frame)")
            XCTAssertEqual(outcome.turned, "")
            if session.stats.turnTries > before { triedAt.append(frame) }
        }
        // after the n-th try that reads nothing, the next min(n, 8) captures that would try are
        // passed over
        XCTAssertEqual(triedAt, [1, 3, 6, 10, 15, 21, 28])
        // a try that reads a lane ends the back-off
        session.turnWait = 0
        XCTAssertEqual(session.push(try turnedFrame(encoder, 31, degrees: 12), nowMilliseconds: 31 * 83).turned, "0123")
        XCTAssertEqual(session.turnMisses, 0)
        XCTAssertEqual(session.push(try turnedFrame(encoder, 32, degrees: 12), nowMilliseconds: 32 * 83).turned, "0123")
    }

    /// Card `frame` of `encoder` with its bands, filmed by a phone held upright: a landscape
    /// frame with the card turned a quarter.
    private func upright(_ encoder: PetalStreamEncoder, _ frame: UInt16) throws -> PetalLuma {
        try Support.turnedCamera(
            Support.renderCard(encoder, frame: frame, size: 640, supersample: 2),
            degrees: 92,
            width: 1120,
            height: 720
        )
    }

    /// A session whose work costs what `costs` says, however long it took.
    private func costed(_ costs: @escaping @Sendable (PetalWork) -> Double) -> PetalScanSession {
        PetalScanSession(workCost: { work, _ in costs(work) })
    }

    func testAnUprightCameraReadsBothBandsOncePerShownFrame() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(3_000, seed: 3), kind: 2)
        // a fast decoder: every capture has time to spare for the turn lanes too
        var session = costed { _ in 0 }
        let shown = try upright(encoder, 1)
        let first = session.push(shown, nowMilliseconds: 0)
        XCTAssertEqual(first.lanes, "D0123")
        XCTAssertEqual(first.kana, "P0K0P1K1")
        XCTAssertEqual(first.turned, "0123")
        // a second capture of the same shown frame tries no band and no turn lane again
        let again = session.push(shown, nowMilliseconds: 33)
        XCTAssertEqual(again.kana, "")
        XCTAssertEqual(again.turned, "")
        let next = session.push(try upright(encoder, 2), nowMilliseconds: 83)
        XCTAssertEqual(next.kana, "P0K0P1K1")
        XCTAssertEqual(next.turned, "0123")
        XCTAssertEqual(session.stats.kanaTries, 4)
        XCTAssertEqual(session.stats.kanaLanes, 8)
        XCTAssertEqual(session.stats.turnLanes, 8)
        XCTAssertEqual(session.stats.spareTurns, 2)
        XCTAssertEqual(
            session.kanaTried.last,
            PetalScanSession.KanaTry(header: PetalLaneHeader(tag: encoder.meta.tag, frame: 2), band: 1)
        )
        session.reset()
        XCTAssertTrue(session.kanaTried.isEmpty)
    }

    func testASlowDecoderLeavesTheTurnLanesToTheNextCapture() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(3_000, seed: 3), kind: 2)
        // a Pixel 6 held to its Cortex-A55 cores (the reference simulator's model): the bands
        // take the first capture's spare time
        var session = costed { work in
            switch work {
            case .search: 105
            case .track: 14.5
            case .merge: 10
            case .bands(let count): 15 * Double(count)
            case .turns: 75
            }
        }
        let shown = try upright(encoder, 1)
        let first = session.push(shown, nowMilliseconds: 0)
        XCTAssertEqual(first.kana, "P0K0P1K1")
        XCTAssertEqual(first.turned, "")
        let again = session.push(shown, nowMilliseconds: 33)
        XCTAssertEqual(again.kana, "")
        XCTAssertEqual(again.turned, "0123")
        let next = session.push(try upright(encoder, 2), nowMilliseconds: 83)
        XCTAssertEqual(next.kana, "P0K0P1K1")
        XCTAssertEqual(next.turned, "")
        XCTAssertEqual(session.stats.spareTurns, 0)
    }

    func testTheLastTurnTryTellsWhetherACaptureHasTimeToSpare() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(3_000, seed: 3), kind: 2)
        var session = costed { work in
            switch work {
            case .search, .track: 4
            case .bands(let count): 3 * Double(count)
            case .merge: 0
            case .turns: 30
            }
        }
        // 10 ms of work, and twice that for the turn lanes before any try: they fit
        let first = session.push(try upright(encoder, 1), nowMilliseconds: 0)
        XCTAssertEqual(first.kana, "P0K0P1K1")
        XCTAssertEqual(first.turned, "0123")
        // the try took 30 ms, which with the next capture's 10 ms does not fit
        let shown = try upright(encoder, 2)
        let next = session.push(shown, nowMilliseconds: 83)
        XCTAssertEqual(next.kana, "P0K0P1K1")
        XCTAssertEqual(next.turned, "")
        let again = session.push(shown, nowMilliseconds: 116)
        XCTAssertEqual(again.kana, "")
        XCTAssertEqual(again.turned, "0123")
        XCTAssertEqual(session.stats.spareTurns, 1)
    }

    func testBandTriesThatReadNothingBackOff() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(3_000, seed: 3), kind: 2)
        var session = PetalScanSession()
        var triedAt: [UInt16] = []
        // bare squares: every field lane reads, no band is in view
        for frame in UInt16(1)...30 {
            let before = session.stats.kanaTries
            let outcome = session.push(try turnedFrame(encoder, frame, degrees: 12), nowMilliseconds: UInt64(frame) * 83)
            XCTAssertTrue(outcome.lanes.hasSuffix("0123"), "frame \(frame)")
            XCTAssertEqual(outcome.kana, "")
            if session.stats.kanaTries > before { triedAt.append(frame) }
        }
        // after the n-th try that reads nothing, the next min(n, 8) captures that would try are
        // passed over; each try covers both bands
        XCTAssertEqual(triedAt, [1, 3, 6, 10, 15, 21, 28])
        XCTAssertEqual(Int(session.stats.kanaTries), 2 * triedAt.count)
        XCTAssertEqual(session.kanaMisses, 7)
    }

    func testTheSessionRemembersTheTurnLanesOfSixShownFrames() throws {
        let encoder = try PetalStreamEncoder(payload: Support.payload(3_000, seed: 3), kind: 2)
        var session = PetalScanSession()
        for frame in UInt16(1)...7 {
            let outcome = session.push(try turnedFrame(encoder, frame, degrees: 12), nowMilliseconds: UInt64(frame) * 83)
            XCTAssertEqual(outcome.turned, "0123", "frame \(frame)")
        }
        XCTAssertEqual(session.tried.count, PetalScanSession.turnMemory)
        XCTAssertEqual(session.tried.first?.header.frame, 2, "the oldest shown frame is forgotten")
        XCTAssertEqual(session.tried.last, PetalScanSession.TurnTry(header: PetalLaneHeader(tag: encoder.meta.tag, frame: 7), sector: 3))
        // a forgotten shown frame is read again
        XCTAssertEqual(session.push(try turnedFrame(encoder, 1, degrees: 12), nowMilliseconds: 700).turned, "0123")
    }

    func testAStreamSwitchForgetsTheTurnLanesRead() throws {
        let old = try PetalStreamEncoder(payload: Support.payload(3_000, seed: 3), kind: 2)
        let new = try PetalStreamEncoder(payload: Support.payload(3_000, seed: 4), kind: 2)
        var session = PetalScanSession()
        XCTAssertEqual(session.push(try turnedFrame(old, 4, degrees: 12), nowMilliseconds: 0).turned, "0123")
        XCTAssertFalse(session.tried.isEmpty)
        // two beacons of another stream replace it
        for (index, frame) in [UInt16(8), 12].enumerated() {
            _ = session.push(try turnedFrame(new, frame, degrees: 12), nowMilliseconds: 100 + UInt64(index) * 100)
        }
        XCTAssertEqual(session.progress.meta, new.meta)
        // only the turn lanes tried after the switch are remembered
        XCTAssertEqual(session.tried.count, 4)
        XCTAssertTrue(session.tried.allSatisfy { $0.header.tag == new.meta.tag && $0.header.frame == 12 })
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
        // the beacon frame's 20 atoms and frame 1's 21, each with 16 more from the turn lanes
        XCTAssertEqual(second.turned, "0123")
        XCTAssertEqual(second.progress.rank, (20 + 16) + (21 + 16))
        let third = session.push(try cameraFrame(encoder, 2), nowMilliseconds: 1_800)
        XCTAssertNotNil(third.progress.meta, "steady progress keeps the idle timer alive")
        // 2.5 s after the start the stream is dropped before the frame is read;
        // frame 3 carries no beacon, so its atoms wait for the next one.
        let fourth = session.push(try cameraFrame(encoder, 3), nowMilliseconds: 2_500)
        XCTAssertEqual(fourth.lanes, "D0123")
        XCTAssertNil(fourth.progress.meta)
        XCTAssertEqual(fourth.progress.rank, 0)
        XCTAssertEqual(fourth.turned, "0123", "a reset forgets the turn lanes tried")
        let fifth = session.push(try cameraFrame(encoder, 4), nowMilliseconds: 2_600)
        XCTAssertEqual(fifth.progress.meta, encoder.meta)
        XCTAssertEqual(fifth.progress.rank, (21 + 16) + (20 + 16), "buffered atoms of frame 3 join the restarted stream")
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

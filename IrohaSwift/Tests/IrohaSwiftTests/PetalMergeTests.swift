import Foundation
import XCTest
@testable import IrohaSwift

/// Capture averaging and session isolation (ports of the merge tests of
/// `crates/iroha_petal/src/session.rs` and the merge sets of
/// `tests/captures.rs`), including the reference's noisy and torn-camera
/// captures in the shared golden fixture.
final class PetalMergeTests: XCTestCase {
    private typealias Support = PetalTestSupport

    /// The `one-pass` stream of the golden captures.
    private func onePass() throws -> PetalStreamEncoder {
        try PetalStreamEncoder(payload: Support.payload(3000, seed: 2), kind: 2)
    }

    private func setup(_ number: UInt16 = 11) throws -> (PetalStreamEncoder, PetalLuma, PetalDecodedFrame) {
        let encoder = try onePass()
        let image = try Support.render(encoder, frame: number, size: 512, supersample: 2)
        return (encoder, image, try PetalDecoder.decode(image))
    }

    /// `pose` with its lanes replaced.
    private func frame(
        _ pose: PetalDecodedFrame,
        d: PetalLaneResult? = nil,
        fields: [PetalLaneResult?] = [nil, nil, nil, nil],
        inferredCorner: Int? = nil
    ) -> PetalDecodedFrame {
        PetalDecodedFrame(
            pose: pose.pose, rotation: pose.rotation, mirrored: pose.mirrored,
            d: d, fields: fields, inferredCorner: inferredCorner
        )
    }

    private func lane(_ data: [UInt8]) -> PetalLaneResult {
        PetalLaneResult(data: data, corrected: 0, erasures: 0)
    }

    /// A clean capture of frame `petals` whose decoded lanes are replaced by
    /// lane `D` of frame `rings`, as if the rings showed another frame than the
    /// petal field.
    private func ringsOfAnotherFrame(
        _ encoder: PetalStreamEncoder,
        petals: UInt16,
        rings: UInt16
    ) throws -> (PetalLuma, PetalDecodedFrame) {
        let image = try Support.render(encoder, frame: petals, size: 512, supersample: 2)
        let decoded = try PetalDecoder.decode(image)
        XCTAssertFalse(decoded.fields.contains { $0 == nil })
        return (image, frame(decoded, d: lane(encoder.laneData(frame: rings).d)))
    }

    func testOneEvidenceMatchesTheSingleCaptureAndTransformsStayInCodeCoordinates() throws {
        let (encoder, clean, _) = try setup()
        var shadow = clean.pixels
        for y in 0..<clean.height {
            for x in clean.width * 7 / 20..<clean.width * 3 / 5 {
                let i = y * clean.width + x
                shadow[i] = UInt8((Double(shadow[i]) * 0.3).rounded())
            }
        }
        var hidden = clean.pixels
        let center = PetalLayout.finderCenters[3]
        let scale = Double(clean.width) / 1024
        for y in 0..<clean.height {
            for x in 0..<clean.width {
                let dx = Double(x) + 0.5 - center.x * scale
                let dy = Double(y) + 0.5 - center.y * scale
                if dx * dx + dy * dy <= pow(75 * scale, 2) { hidden[y * clean.width + x] = 0 }
            }
        }
        let images = [
            clean,
            try PetalLuma(width: clean.width, height: clean.height, pixels: shadow),
            try PetalLuma(width: clean.width, height: clean.height, pixels: hidden),
            try Support.mirror(clean),
            try Support.transform(clean) { x, y in (y, clean.width - 1 - x) },
        ]
        var captures: [PetalFieldEvidence] = []
        for image in images {
            let decoded = try PetalDecoder.decode(image)
            let evidence = try XCTUnwrap(PetalDecoder.fieldEvidence(image, frame: decoded))
            XCTAssertEqual(PetalDecoder.mergeFields([evidence]), decoded.fields)
            captures.append(evidence)
        }
        let merged = PetalDecoder.mergeFields(captures)
        let truth = encoder.laneData(frame: 11)
        for sector in 0..<PetalLayout.sectors {
            XCTAssertEqual(merged[sector]?.data, truth.fields[sector], "sector \(sector)")
        }
    }

    func testEvidenceRequiresUsableImageAndCornerAndEmptyMergeReadsNothing() throws {
        let (_, image, pose) = try setup()
        XCTAssertNil(PetalDecoder.fieldEvidence(try PetalLuma(width: 20, height: 20), frame: pose))
        XCTAssertNil(PetalDecoder.fieldEvidence(image, frame: frame(pose, inferredCorner: 7)))
        XCTAssertNil(PetalDecoder.fieldEvidence(image, frame: frame(pose, inferredCorner: -1)))
        XCTAssertNil(PetalDecoder.fieldEvidence(image, frame: pose, options: PetalDecodeOptions(maximumPixels: 100)))
        XCTAssertEqual(PetalDecoder.mergeFields([]), [nil, nil, nil, nil])
        // a short skip list skips nothing beyond it
        let evidence = try XCTUnwrap(PetalDecoder.fieldEvidence(image, frame: pose))
        XCTAssertEqual(PetalDecoder.mergeFields([evidence], skip: [true]).map { $0 != nil }, [false, true, true, true])
    }

    func testUnkeyedTornAndShortHeadersAreNotHeld() throws {
        let (encoder, image, pose) = try setup()
        var session = PetalScanSession()
        for rejected in [
            frame(pose),
            frame(pose, fields: [lane([1, 2]), nil, nil, nil]),
            frame(pose, d: lane(encoder.laneData(frame: 10).d), fields: [nil, nil, lane(encoder.laneData(frame: 11).fields[2]), nil]),
        ] {
            XCTAssertNil(PetalScanSession.shownFrame(rejected))
            XCTAssertEqual(session.merge(image, frame: rejected), "")
        }
        XCTAssertTrue(session.held.isEmpty)
        XCTAssertEqual(session.stats.merges, 0)
    }

    func testAMergeNeverOffersAtomsOfAnotherShownFrame() throws {
        let encoder = try onePass()
        var session = PetalScanSession()
        let (image, rings) = try ringsOfAnotherFrame(encoder, petals: 11, rings: 10)
        for capture in 1...3 {
            XCTAssertEqual(session.merge(image, frame: rings), "", "capture \(capture)")
        }
        XCTAssertEqual(session.stats.merges, 2)
        XCTAssertEqual(session.stats.mergedLanes, 0)
        XCTAssertEqual(session.progress.atomsReceived, 0)
        // what was refused: the merged field lanes name frame 11, not frame 10
        let held = try XCTUnwrap(session.held.first)
        XCTAssertEqual(held.shown.frame, 10)
        let merged = PetalDecoder.mergeFields(held.evidence)
        let truth = encoder.laneData(frame: 11)
        for sector in 0..<PetalLayout.sectors {
            XCTAssertEqual(merged[sector]?.data, truth.fields[sector], "sector \(sector)")
        }
    }

    func testRecoveredLanesFillOnlyTheMissingSectors() throws {
        let (encoder, image, pose) = try setup()
        var session = PetalScanSession()
        let partial = frame(pose, d: pose.d, fields: [pose.fields[0], nil, nil, nil])
        XCTAssertEqual(session.merge(image, frame: partial), "")
        XCTAssertEqual(session.merge(image, frame: partial), "123")
        XCTAssertEqual(session.stats.mergedLanes, 3)
        XCTAssertEqual(session.stats.merges, 1)
        XCTAssertTrue(try XCTUnwrap(session.held.first).evidence.isEmpty)
        XCTAssertEqual(session.held[0].done, [true, true, true, true])
        let merges = session.stats.merges
        XCTAssertEqual(session.merge(image, frame: partial), "")
        XCTAssertEqual(session.stats.merges, merges, "a completed shown frame releases evidence and stops merging")
        // the recovered atoms wait for the beacon of their stream
        XCTAssertEqual(session.progress.atomsReceived, 0)
        let beacon = try PetalDecoder.decode(try Support.render(encoder, frame: 12, size: 512, supersample: 2))
        _ = session.absorb(frame(beacon, d: beacon.d))
        XCTAssertEqual(session.progress.meta, encoder.meta)
        XCTAssertEqual(session.progress.atomsReceived, 15, "the three merged lanes, replayed once the beacon arrives")
    }

    func testTornCapturesWhoseLanesNameDifferentFramesAreNotHeld() throws {
        let encoder = try onePass()
        var session = PetalScanSession()
        let (image, rings) = try ringsOfAnotherFrame(encoder, petals: 11, rings: 10)
        let torn = frame(rings, d: rings.d, fields: [nil, nil, lane(encoder.laneData(frame: 11).fields[2]), nil])
        XCTAssertEqual(session.merge(image, frame: torn), "")
        XCTAssertTrue(session.held.isEmpty)
    }

    func testTheSessionHoldsAtMostFourCapturesOfSixShownFrames() throws {
        let encoder = try onePass()
        var session = PetalScanSession()
        // refused merges never finish a shown frame, so its captures stay held
        let (image, rings) = try ringsOfAnotherFrame(encoder, petals: 11, rings: 10)
        for _ in 0..<(PetalScanSession.mergeCaptures + 2) { _ = session.merge(image, frame: rings) }
        XCTAssertEqual(session.held.count, 1)
        XCTAssertEqual(session.held[0].evidence.count, PetalScanSession.mergeCaptures)
        XCTAssertEqual(session.stats.merges, UInt32(PetalScanSession.mergeCaptures + 1))
        for number in 12..<12 + PetalScanSession.mergeWindow {
            _ = session.merge(image, frame: frame(rings, d: lane(encoder.laneData(frame: UInt16(number)).d)))
        }
        XCTAssertEqual(session.held.count, PetalScanSession.mergeWindow)
        XCTAssertEqual(session.held.map(\.shown.frame), (12..<18).map(UInt16.init))
        // a different stream tag gets its own shown-frame slot even with the same counter
        var other = encoder.laneData(frame: 17).d
        other[0] ^= 1
        _ = session.merge(image, frame: frame(rings, d: lane(other)))
        XCTAssertEqual(session.held.suffix(2).map(\.shown.frame), [17, 17])
        XCTAssertNotEqual(session.held[session.held.count - 2].shown.tag, session.held.last?.shown.tag)
        // sessions never share captures, and a reset forgets them
        var second = PetalScanSession()
        XCTAssertEqual(second.merge(image, frame: rings), "")
        XCTAssertEqual(second.stats.merges, 0)
        session.reset()
        XCTAssertTrue(session.held.isEmpty)
        XCTAssertEqual(session.merge(image, frame: rings), "")
        XCTAssertEqual(session.held[0].evidence.count, 1)
        XCTAssertEqual(second.held[0].evidence.count, 1)
    }

    func testSessionTimeoutsReleaseHeldEvidence() throws {
        let (encoder, image, pose) = try setup(0)
        let rings = frame(pose, d: lane(encoder.laneData(frame: 1).d))
        for limits in [
            PetalScanLimits(idleTimeoutMilliseconds: 10),
            PetalScanLimits(absoluteTimeoutMilliseconds: 10),
        ] {
            var session = PetalScanSession(limits: limits)
            XCTAssertNotNil(session.push(image, nowMilliseconds: 0).progress.meta)
            XCTAssertEqual(session.merge(image, frame: rings), "")
            XCTAssertFalse(session.held.flatMap(\.evidence).isEmpty)
            let expired = session.push(try PetalLuma(width: 320, height: 240), nowMilliseconds: 11)
            XCTAssertNil(expired.progress.meta)
            XCTAssertTrue(expired.merged.isEmpty)
            XCTAssertTrue(session.held.isEmpty)
        }
    }

    func testSwitchingStreamsWithTheSameShortTagDiscardsOldMergeState() throws {
        // full stream identities differ although the eight-bit tag and frame counter are identical
        func encoder(_ last: UInt8) throws -> PetalStreamEncoder {
            var payload = [UInt8](repeating: 0, count: 100)
            payload[3] = last
            return try PetalStreamEncoder(payload: payload, kind: 2)
        }
        let old = try encoder(49)
        let new = try encoder(128)
        XCTAssertEqual(old.meta.tag, 61)
        XCTAssertEqual(new.meta.tag, 61)
        XCTAssertNotEqual(old.meta, new.meta)
        let oldImage = try Support.render(old, frame: 0, size: 512, supersample: 2)
        let newImage = try Support.render(new, frame: 0, size: 512, supersample: 2)
        let oldFull = try PetalDecoder.decode(oldImage)
        let newDecoded = try PetalDecoder.decode(newImage)
        let newPartial = frame(newDecoded, d: newDecoded.d)
        for wasComplete in [false, true] {
            var session = PetalScanSession()
            let previous = wasComplete ? oldFull : frame(oldFull, d: oldFull.d)
            _ = session.absorb(previous)
            _ = session.merge(oldImage, frame: previous)
            XCTAssertFalse(session.held.isEmpty)
            XCTAssertEqual(session.held[0].done, [Bool](repeating: wasComplete, count: 4))
            XCTAssertEqual(session.held[0].evidence.isEmpty, wasComplete)
            _ = session.absorb(newPartial)
            XCTAssertEqual(session.progress.meta, old.meta, "one competing beacon does not switch streams")
            XCTAssertEqual(session.held.count, 1)
            _ = session.absorb(newPartial)
            XCTAssertEqual(session.progress.meta, new.meta)
            XCTAssertTrue(session.held.isEmpty, "an accepted metadata switch drops both evidence and done flags")
            XCTAssertEqual(session.merge(newImage, frame: newPartial), "")
            XCTAssertEqual(session.held[0].evidence.count, 1)
            XCTAssertTrue(session.held[0].done.allSatisfy { !$0 })
            XCTAssertEqual(session.merge(newImage, frame: newPartial), "0123")
            XCTAssertEqual(session.stats.mergedLanes, 4)
        }
    }

    func testTheFirstBeaconPreservesEvidenceReceivedBeforeIt() throws {
        let encoder = try onePass()
        let image = try Support.render(encoder, frame: 1, size: 512, supersample: 2)
        let decoded = try PetalDecoder.decode(image)
        let partial = frame(decoded, d: decoded.d)
        var session = PetalScanSession()
        _ = session.absorb(partial)
        _ = session.merge(image, frame: partial)
        XCTAssertNil(session.progress.meta)
        XCTAssertEqual(session.held[0].evidence.count, 1)
        let beaconImage = try Support.render(encoder, frame: 0, size: 512, supersample: 2)
        let beaconDecoded = try PetalDecoder.decode(beaconImage)
        _ = session.absorb(frame(beaconDecoded, d: beaconDecoded.d))
        XCTAssertEqual(session.progress.meta, encoder.meta)
        XCTAssertEqual(session.held[0].evidence.count, 1)
        XCTAssertEqual(session.merge(image, frame: partial), "0123")
    }

    // MARK: - Golden merge sets (`fixtures/petal/petal_captures_v5.json`)

    private func fixtures() throws -> [String: Any] {
        try XCTUnwrap(Support.capturesFixture, "fixtures/petal/petal_captures_v5.json is missing")
    }

    /// The recorded data of lane `D` and the four field lanes of a merge set.
    private struct Recorded {
        let d: [UInt8]
        let fields: [[UInt8]]

        func lane(_ lane: PetalLane) -> [UInt8] {
            switch lane {
            case .d: return d
            case .field(let sector): return fields[sector]
            }
        }
    }

    private func truth(_ set: [String: Any]) throws -> Recorded {
        Recorded(
            d: try Support.bytes(set, "d_data"),
            fields: try XCTUnwrap(set["field_data"] as? [String]).map(Support.hex)
        )
    }

    /// Decodes every capture of a merge set as recorded and pushes it through a
    /// fresh session (30 fps); returns the merge of their field evidence, the
    /// lanes the session merged and its counters.
    private func mergeSet(_ set: [String: Any]) throws -> (
        results: [PetalLaneResult?], merged: String, stats: PetalScanStats
    ) {
        let name = try Support.string(set, "name")
        let truth = try truth(set)
        var evidence: [PetalFieldEvidence] = []
        var session = PetalScanSession()
        var merged = ""
        for (index, capture) in try Support.objects(set, "captures").enumerated() {
            var entry = capture
            entry["width"] = set["width"]
            entry["height"] = set["height"]
            let image = try Support.luma(of: entry)
            let decoded = try PetalDecoder.decode(image)
            for lane in PetalLane.decodeOrder {
                if let result = decoded.lane(lane) {
                    XCTAssertEqual(result.data, truth.lane(lane), "\(name) capture \(index) lane \(lane.letter)")
                }
            }
            let read = decoded.laneLetters
            let must = try Support.string(capture, "must_decode")
            XCTAssertTrue(must.allSatisfy { read.contains($0) }, "\(name) capture \(index): read \(read), must read \(must)")
            XCTAssertEqual(read, try Support.string(capture, "reference_decoded"), "\(name) capture \(index): lanes read alone")
            evidence.append(try XCTUnwrap(PetalDecoder.fieldEvidence(image, frame: decoded)))
            merged += session.push(image, nowMilliseconds: UInt64(index) * 33).merged
        }
        return (PetalDecoder.mergeFields(evidence), merged, session.stats)
    }

    /// Checks merged field lanes against the recorded data and returns their sector digits.
    private func mergedLetters(_ name: String, _ results: [PetalLaneResult?], _ set: [String: Any]) throws -> String {
        let truth = try truth(set)
        var letters = ""
        for (sector, result) in results.enumerated() {
            if let result {
                XCTAssertEqual(result.data, truth.fields[sector], "\(name): merged lane \(sector) data")
                letters.append(PetalLane.field(sector).letter)
            }
        }
        return letters
    }

    func testGoldenMergeSetsRecoverLanesNoCaptureReadsAlone() throws {
        let sets = try Support.objects(fixtures(), "merges")
        XCTAssertFalse(sets.isEmpty)
        for set in sets {
            let name = try Support.string(set, "name")
            let result = try mergeSet(set)
            XCTAssertEqual(try mergedLetters(name, result.results, set), try Support.string(set, "reference_merged"), name)
            for letter in try Support.string(set, "must_merge") {
                XCTAssertTrue(result.merged.contains(letter), "\(name): the session did not recover lane \(letter) (merged \(result.merged))")
            }
            XCTAssertGreaterThan(result.stats.merges, 0, name)
            XCTAssertEqual(Int(result.stats.mergedLanes), result.merged.count, name)
        }
    }

    func testGoldenMixedSetsNeverOfferTheOtherFramesLanes() throws {
        let sets = try Support.objects(fixtures(), "mixed_merges")
        XCTAssertFalse(sets.isEmpty)
        for set in sets {
            let name = try Support.string(set, "name")
            let result = try mergeSet(set)
            // the merge itself reads the petal field, which shows another frame than the rings
            let merged = try mergedLetters(name, result.results, set)
            let ringsFrame = Array(try Support.bytes(set, "d_data")[1..<3])
            for lane in result.results.compactMap({ $0 }) {
                XCTAssertNotEqual(Array(lane.data[1..<3]), ringsFrame, "\(name): a merged lane names the rings' frame")
            }
            XCTAssertEqual(merged, try Support.string(set, "reference_merged"), name)
            XCTAssertGreaterThan(result.stats.merges, 0, "\(name): no merge was tried")
            XCTAssertEqual(result.merged, "", "\(name): the session offered another frame's lanes")
            XCTAssertEqual(result.stats.mergedLanes, 0, name)
        }
    }
}

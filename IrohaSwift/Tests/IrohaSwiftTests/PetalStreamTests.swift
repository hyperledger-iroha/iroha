import XCTest
@testable import IrohaSwift

/// Ports of the `fountain` and `stream` unit tests of `crates/iroha_petal`,
/// plus assembler limit and loss/reorder tests.
final class PetalStreamTests: XCTestCase {
    private func reassemble(_ atoms: [[UInt8]], _ length: Int) -> [UInt8] {
        Array(atoms.flatMap { $0 }.prefix(length))
    }

    // MARK: - Fountain

    func testSystematicAtomsAloneRecoverThePayload() throws {
        let data = PetalTestSupport.payload(100, seed: 5)
        let source = PetalFountain.splitPayload(data)
        var decoder = try PetalFountainDecoder(sourceAtoms: source.count)
        for (id, atom) in source.enumerated() {
            XCTAssertTrue(decoder.addEncoded(crc: 7, id: UInt32(id), atom: atom))
        }
        XCTAssertEqual(reassemble(try XCTUnwrap(decoder.solve()), 100), data)
    }

    func testRepairAtomsCoverForLostSystematicAtoms() throws {
        let data = PetalTestSupport.payload(1_000, seed: 9)
        let source = PetalFountain.splitPayload(data)
        let k = source.count
        let crc: UInt32 = 0x1234_5678
        var decoder = try PetalFountainDecoder(sourceAtoms: k)
        // lose every third systematic atom, then take repair atoms
        for id in (0..<UInt32(k)) where id % 3 != 0 {
            decoder.addEncoded(crc: crc, id: id, atom: try PetalFountain.encodeAtom(source: source, crc: crc, id: id))
        }
        var id = UInt32(k)
        var used = 0
        while !decoder.isComplete {
            decoder.addEncoded(crc: crc, id: id, atom: try PetalFountain.encodeAtom(source: source, crc: crc, id: id))
            id += 1
            used += 1
            XCTAssertLessThan(used, k, "decoder must converge")
            if used >= k { return }
        }
        let missing = (0..<k).filter { $0 % 3 == 0 }.count
        XCTAssertLessThanOrEqual(used, missing + 8, "needed \(used) repairs for \(missing) missing")
        XCTAssertEqual(reassemble(try XCTUnwrap(decoder.solve()), 1_000), data)
    }

    func testPureRepairStreamsDecodeWithSmallOverhead() throws {
        let data = PetalTestSupport.payload(5_000, seed: 11)
        let source = PetalFountain.splitPayload(data)
        let k = source.count
        let crc: UInt32 = 0xCAFE_F00D
        var totalOverhead = 0
        for trial in 0..<20 {
            var decoder = try PetalFountainDecoder(sourceAtoms: k)
            var id = UInt32(k + trial * 1_000)
            var received = 0
            while !decoder.isComplete {
                decoder.addEncoded(crc: crc, id: id, atom: try PetalFountain.encodeAtom(source: source, crc: crc, id: id))
                id += 1
                received += 1
                if received > 4 * k { return XCTFail("repair stream did not converge") }
            }
            totalOverhead += received - k
            XCTAssertEqual(reassemble(try XCTUnwrap(decoder.solve()), 5_000), data)
        }
        XCTAssertLessThanOrEqual(totalOverhead, 20 * 4, "average overhead \(Double(totalOverhead) / 20)")
    }

    func testDuplicateAndDependentAtomsDoNotRaiseRank() throws {
        let source = PetalFountain.splitPayload(PetalTestSupport.payload(60, seed: 3))
        var decoder = try PetalFountainDecoder(sourceAtoms: source.count)
        XCTAssertTrue(decoder.addEncoded(crc: 1, id: 0, atom: source[0]))
        XCTAssertFalse(decoder.addEncoded(crc: 1, id: 0, atom: source[0]))
        XCTAssertEqual(decoder.rank, 1)
        XCTAssertNil(decoder.solve())
    }

    func testRepairMasksSpanFarMoreThanThirtyTwoDimensions() throws {
        // Regression: an xorshift-derived mask is GF(2)-linear in a 32-bit seed
        // and can never exceed rank 32.
        let k = 200
        var decoder = try PetalFountainDecoder(sourceAtoms: k)
        for id in UInt32(k)..<UInt32(k + 400) {
            decoder.add(
                mask: PetalFountain.maskWords(sourceAtoms: k, crc: 5, id: id),
                atom: [UInt8](repeating: 0, count: 16)
            )
        }
        XCTAssertEqual(decoder.rank, k, "repair masks must reach full rank")
    }

    func testMasksAreNonzeroAndPaddedBitsAreClear() {
        for k in [1, 2, 31, 32, 33, 100] {
            for id in UInt32(0)..<200 {
                let mask = PetalFountain.maskWords(sourceAtoms: k, crc: 99, id: id)
                XCTAssertTrue(mask.contains { $0 != 0 })
                if k % 32 != 0 {
                    XCTAssertEqual(mask[mask.count - 1] >> UInt32(k % 32), 0)
                }
            }
        }
        XCTAssertEqual(PetalFountain.maskWords(sourceAtoms: 0, crc: 1, id: 1), [])
    }

    func testFountainSurvivesLossAndReordering() throws {
        let data = PetalTestSupport.payload(2_000, seed: 31)
        let source = PetalFountain.splitPayload(data)
        let k = source.count
        let crc = IrohaPeerCRC32CV1.checksum(data)
        // 3k encoded atoms, shuffled, with 40 % lost
        var ids = Array(UInt32(0)..<UInt32(3 * k))
        var rng = PetalXorshift32(seed: 4242)
        for index in stride(from: ids.count - 1, to: 0, by: -1) {
            ids.swapAt(index, Int(rng.nextUInt32() % UInt32(index + 1)))
        }
        var decoder = try PetalFountainDecoder(sourceAtoms: k)
        var offered = 0
        for id in ids where rng.nextUInt32() % 10 >= 4 {
            decoder.addEncoded(crc: crc, id: id, atom: try PetalFountain.encodeAtom(source: source, crc: crc, id: id))
            offered += 1
            if decoder.isComplete { break }
        }
        XCTAssertTrue(decoder.isComplete)
        XCTAssertLessThan(offered, k + 16)
        XCTAssertEqual(reassemble(try XCTUnwrap(decoder.solve()), data.count), data)
    }

    func testFountainRejectsMalformedInput() throws {
        XCTAssertThrowsError(try PetalFountainDecoder(sourceAtoms: 0))
        XCTAssertThrowsError(try PetalFountain.encodeAtom(source: [], crc: 0, id: 0))
        XCTAssertThrowsError(try PetalFountain.encodeAtom(source: [[1, 2]], crc: 0, id: 0))
        var decoder = try PetalFountainDecoder(sourceAtoms: 40)
        XCTAssertFalse(decoder.add(mask: [1], atom: [UInt8](repeating: 0, count: 16)))
        XCTAssertFalse(decoder.add(mask: [1, 0], atom: [UInt8](repeating: 0, count: 15)))
        XCTAssertFalse(decoder.add(mask: [0, 0], atom: [UInt8](repeating: 0, count: 16)))
        XCTAssertFalse(decoder.add(mask: [0, 0x100], atom: [UInt8](repeating: 0, count: 16)), "column beyond k")
        XCTAssertEqual(PetalFountain.splitPayload([1, 2, 3]), [[1, 2, 3] + [UInt8](repeating: 0, count: 13)])
    }

    // MARK: - Stream

    func testAtomIDsAreContiguousAcrossFrames() {
        var expected: UInt32 = 0
        for frame in UInt16(0)...70 {
            XCTAssertEqual(PetalStream.firstAtomID(frame: frame), expected, "frame \(frame)")
            expected += UInt32(PetalStream.atomsInFrame(frame))
        }
        XCTAssertEqual(PetalStream.atomsInFrame(0), 6)
        XCTAssertEqual(PetalStream.atomsInFrame(1), 7)
        XCTAssertEqual(PetalStream.laneFirstID(.k, frame: 4), PetalStream.firstAtomID(frame: 4) + 1)
        XCTAssertEqual(PetalStream.laneFirstID(.k, frame: 5), PetalStream.firstAtomID(frame: 5) + 2)
        // the frame counter wraps on a beacon frame, so ids repeat cleanly
        XCTAssertTrue(PetalStream.isBeaconFrame(0))
        XCTAssertEqual(65_536 % Int(PetalStream.beaconInterval), 0)
    }

    func testCleanStreamCompletesAfterOneSystematicPass() throws {
        let data = PetalTestSupport.payload(1_000, seed: 21)
        let encoder = try PetalStreamEncoder(payload: data, kind: 2)
        var assembler = PetalStreamAssembler()
        for frame in 0..<encoder.systematicFrames {
            try PetalTestSupport.feed(&assembler, encoder, frame: UInt16(frame), lanes: [.d, .p, .k])
        }
        let done = try XCTUnwrap(assembler.takeCompleted())
        XCTAssertEqual([UInt8](done.payload), data)
        XCTAssertEqual(done.meta.kind, 2)
        XCTAssertTrue(assembler.progress.complete)
        XCTAssertNil(assembler.takeCompleted(), "a payload is delivered exactly once")
    }

    func testAnySingleLaneIsEnoughGivenABeacon() throws {
        let data = PetalTestSupport.payload(400, seed: 22)
        let encoder = try PetalStreamEncoder(payload: data, kind: 1)
        for lane in [PetalLane.p, .k, .d] {
            var assembler = PetalStreamAssembler()
            try PetalTestSupport.feed(&assembler, encoder, frame: 0, lanes: [.d])
            for frame in UInt16(0)..<400 {
                try PetalTestSupport.feed(&assembler, encoder, frame: frame, lanes: [lane])
                if assembler.progress.complete { break }
            }
            XCTAssertEqual(assembler.takeCompleted().map { [UInt8]($0.payload) }, data, "\(lane) alone")
        }
    }

    func testAtomsSeenBeforeTheFirstBeaconAreNotLost() throws {
        let data = PetalTestSupport.payload(300, seed: 23)
        let encoder = try PetalStreamEncoder(payload: data, kind: 1)
        var assembler = PetalStreamAssembler()
        // frames 1..3: no beacon among them until frame 4
        for frame in UInt16(1)..<4 {
            try PetalTestSupport.feed(&assembler, encoder, frame: frame, lanes: [.p, .k, .d])
        }
        XCTAssertNil(assembler.progress.meta)
        for frame in UInt16(4)..<UInt16(encoder.systematicFrames + 4) {
            try PetalTestSupport.feed(&assembler, encoder, frame: frame, lanes: [.p, .k, .d])
        }
        XCTAssertEqual(assembler.takeCompleted().map { [UInt8]($0.payload) }, data)
    }

    func testJoiningMidStreamAndLosingFramesStillCompletes() throws {
        let data = PetalTestSupport.payload(2_500, seed: 24)
        let encoder = try PetalStreamEncoder(payload: data, kind: 3)
        var assembler = PetalStreamAssembler()
        var rng = PetalXorshift32(seed: 77)
        var frame: UInt16 = 15 // join late
        var shown = 0
        while !assembler.progress.complete {
            if rng.nextUInt32() % 10 < 6 {
                // 60 % of the frames are readable
                try PetalTestSupport.feed(&assembler, encoder, frame: frame, lanes: [.d, .p, .k])
            }
            frame = frame &+ 1
            shown += 1
            if shown >= 600 { return XCTFail("stream failed to complete") }
        }
        XCTAssertEqual(assembler.takeCompleted().map { [UInt8]($0.payload) }, data)
    }

    /// Port of `tests/streams.rs`: arbitrary sizes, join points, loss and lane subsets.
    func testRandomStreamsAlwaysCompleteAndNeverDeliverWrongData() throws {
        var rng = PetalXorshift32(seed: 0xC0FF_EE11)
        for trial in 0..<400 {
            // sizes cover K = 1, a few atoms, and a few hundred atoms
            let length: Int
            switch trial % 8 {
            case 0: length = 1 + Int(rng.nextUInt32() % 16)
            case 1: length = 17 + Int(rng.nextUInt32() % 100)
            default: length = 1 + Int(rng.nextUInt32() % 3_000)
            }
            let payload = (0..<length).map { _ in rng.nextByte() }
            let kind = rng.nextByte()
            let encoder = try PetalStreamEncoder(payload: payload, kind: kind)
            let lossPercent = rng.nextUInt32() % 70
            let lanes: [PetalLane]
            switch rng.nextUInt32() % 5 {
            case 0: lanes = [.p]
            case 1: lanes = [.d, .p]
            case 2: lanes = [.k, .d]
            default: lanes = [.p, .k, .d]
            }
            var assembler = PetalStreamAssembler()
            var frame = UInt16(truncatingIfNeeded: rng.nextUInt32() & 0xFFFF)
            var shown: UInt32 = 0
            let budget = 40 + 8 * (UInt32(length) / 13 + 2) * 100 / (100 - lossPercent)
            while !assembler.progress.complete {
                if rng.nextUInt32() % 100 >= lossPercent {
                    // only lane D carries the beacon, so a receiver must read it at
                    // least once; offer it on beacon frames whatever else is readable
                    var readable = lanes
                    if PetalStream.isBeaconFrame(frame) && !readable.contains(.d) { readable.append(.d) }
                    try PetalTestSupport.feed(&assembler, encoder, frame: frame, lanes: readable)
                }
                frame = frame &+ 1
                shown += 1
                if shown >= budget {
                    return XCTFail("trial \(trial): \(length) bytes, loss \(lossPercent) %, lanes \(lanes) exceeded \(budget) frames")
                }
            }
            let done = try XCTUnwrap(assembler.takeCompleted())
            XCTAssertEqual([UInt8](done.payload), payload, "trial \(trial)")
            XCTAssertEqual(done.meta.kind, kind)
        }
    }

    func testCounterWraparoundKeepsAtomIDsConsistent() throws {
        let payload = (0..<2_000).map { UInt8(truncatingIfNeeded: $0 * 7 + 3) }
        let encoder = try PetalStreamEncoder(payload: payload, kind: 1)
        var assembler = PetalStreamAssembler()
        // start a few frames before the 16-bit counter wraps and run across it
        var frame: UInt16 = 65_530
        for _ in 0..<200 {
            try PetalTestSupport.feed(&assembler, encoder, frame: frame, lanes: [.p, .k, .d])
            frame = frame &+ 1
            if assembler.progress.complete { break }
        }
        XCTAssertEqual(assembler.takeCompleted().map { [UInt8]($0.payload) }, payload)
    }

    func testADifferentStreamReplacesTheActiveOneAfterTwoBeacons() throws {
        let first = try PetalStreamEncoder(payload: PetalTestSupport.payload(100, seed: 1), kind: 1)
        let second = try PetalStreamEncoder(payload: PetalTestSupport.payload(100, seed: 2), kind: 1)
        var assembler = PetalStreamAssembler()
        try PetalTestSupport.feed(&assembler, first, frame: 0, lanes: [.d])
        XCTAssertEqual(assembler.progress.meta, first.meta)
        try PetalTestSupport.feed(&assembler, second, frame: 0, lanes: [.d])
        XCTAssertEqual(assembler.progress.meta, first.meta)
        try PetalTestSupport.feed(&assembler, second, frame: 4, lanes: [.d])
        XCTAssertEqual(assembler.progress.meta, second.meta)
    }

    func testAnInterleavedOldBeaconResetsTheSwitchCount() throws {
        let first = try PetalStreamEncoder(payload: PetalTestSupport.payload(100, seed: 1), kind: 1)
        let second = try PetalStreamEncoder(payload: PetalTestSupport.payload(100, seed: 2), kind: 1)
        var assembler = PetalStreamAssembler()
        try PetalTestSupport.feed(&assembler, first, frame: 0, lanes: [.d])
        try PetalTestSupport.feed(&assembler, second, frame: 0, lanes: [.d])
        try PetalTestSupport.feed(&assembler, first, frame: 4, lanes: [.d])
        try PetalTestSupport.feed(&assembler, second, frame: 8, lanes: [.d])
        XCTAssertEqual(assembler.progress.meta, first.meta, "sightings must be consecutive")
        try PetalTestSupport.feed(&assembler, second, frame: 12, lanes: [.d])
        XCTAssertEqual(assembler.progress.meta, second.meta)
    }

    func testOversizedBeaconsAreIgnored() throws {
        let encoder = try PetalStreamEncoder(payload: PetalTestSupport.payload(4_000, seed: 5), kind: 1)
        var assembler = PetalStreamAssembler(limits: PetalAssemblerLimits(maximumPayloadLength: 1_000))
        try PetalTestSupport.feed(&assembler, encoder, frame: 0, lanes: [.d])
        XCTAssertNil(assembler.progress.meta)
        var exact = PetalStreamAssembler(limits: PetalAssemblerLimits(maximumPayloadLength: 4_000))
        try PetalTestSupport.feed(&exact, encoder, frame: 0, lanes: [.d])
        XCTAssertEqual(exact.progress.meta, encoder.meta)
    }

    func testPendingAtomsAreBoundedAndKeepTheNewest() throws {
        let data = PetalTestSupport.payload(700, seed: 6)
        let encoder = try PetalStreamEncoder(payload: data, kind: 1)
        var assembler = PetalStreamAssembler(limits: PetalAssemblerLimits(maximumPendingAtoms: 5))
        // frames 1..3 without a beacon: 21 atoms, only the newest five are kept
        for frame in UInt16(1)..<4 {
            try PetalTestSupport.feed(&assembler, encoder, frame: frame, lanes: [.p, .d, .k])
        }
        try PetalTestSupport.feed(&assembler, encoder, frame: 4, lanes: [.d])
        let progress = assembler.progress
        XCTAssertEqual(progress.meta, encoder.meta)
        XCTAssertEqual(progress.atomsReceived, 5)
        XCTAssertEqual(progress.rank, 5)
    }

    func testAZeroPendingLimitBuffersNothing() throws {
        let encoder = try PetalStreamEncoder(payload: PetalTestSupport.payload(300, seed: 41), kind: 1)
        var assembler = PetalStreamAssembler(limits: PetalAssemblerLimits(maximumPendingAtoms: 0))
        // atoms before any beacon are dropped, not buffered
        for frame in UInt16(1)..<4 {
            try PetalTestSupport.feed(&assembler, encoder, frame: frame, lanes: [.p, .k, .d])
        }
        try PetalTestSupport.feed(&assembler, encoder, frame: 4, lanes: [.d])
        XCTAssertEqual(assembler.progress.rank, 0)
        XCTAssertEqual(assembler.progress.atomsReceived, 0)
        XCTAssertEqual(assembler.progress.meta, encoder.meta)
    }

    func testIntegrityFailuresAreCumulativeAcrossResets() throws {
        let data = PetalTestSupport.payload(40, seed: 42)
        let encoder = try PetalStreamEncoder(payload: data, kind: 1)
        var assembler = PetalStreamAssembler()
        try PetalTestSupport.feed(&assembler, encoder, frame: 0, lanes: [.d])
        // three wrong systematic atoms complete the elimination with a bad CRC
        assembler.push(atoms: PetalAtomPacket(
            header: PetalLaneHeader(tag: encoder.meta.tag, frame: 1),
            firstID: 0,
            atoms: [[UInt8]](repeating: [UInt8](repeating: 0x11, count: 16), count: 3)
        ))
        XCTAssertEqual(assembler.progress.integrityFailures, 1)
        XCTAssertNil(assembler.takeCompleted())
        assembler.reset()
        XCTAssertEqual(assembler.progress.integrityFailures, 1)
        XCTAssertEqual(assembler.progress.atomsReceived, 0)
    }

    func testAtomsOfAnotherStreamAreIgnoredOnceActive() throws {
        let first = try PetalStreamEncoder(payload: PetalTestSupport.payload(300, seed: 1), kind: 1)
        let second = try PetalStreamEncoder(payload: PetalTestSupport.payload(300, seed: 9), kind: 1)
        XCTAssertNotEqual(first.meta.tag, second.meta.tag)
        var assembler = PetalStreamAssembler()
        try PetalTestSupport.feed(&assembler, first, frame: 0, lanes: [.d])
        try PetalTestSupport.feed(&assembler, second, frame: 1, lanes: [.p, .k, .d])
        XCTAssertEqual(assembler.progress.atomsReceived, 0)
        assembler.reset()
        XCTAssertNil(assembler.progress.meta)
    }

    func testCorruptAtomsAreCaughtByThePayloadCRC() throws {
        let data = PetalTestSupport.payload(200, seed: 25)
        let encoder = try PetalStreamEncoder(payload: data, kind: 1)
        var assembler = PetalStreamAssembler()
        try PetalTestSupport.feed(&assembler, encoder, frame: 0, lanes: [.d, .k])
        // atom 0 arrives with a valid header but a wrong body
        assembler.push(atoms: PetalAtomPacket(
            header: PetalLaneHeader(tag: encoder.meta.tag, frame: 0),
            firstID: 0,
            atoms: [[UInt8](repeating: 0xEE, count: 16)]
        ))
        for frame in 1..<encoder.systematicFrames {
            try PetalTestSupport.feed(&assembler, encoder, frame: UInt16(frame), lanes: [.p, .k, .d])
        }
        XCTAssertNil(assembler.takeCompleted())
        XCTAssertEqual(assembler.progress.integrityFailures, 1)
        // clean repair frames after the reset recover the payload
        for frame in UInt16(100)..<600 {
            try PetalTestSupport.feed(&assembler, encoder, frame: frame, lanes: [.p, .k, .d])
            if assembler.progress.complete { break }
        }
        XCTAssertEqual(assembler.takeCompleted().map { [UInt8]($0.payload) }, data)
    }

    func testEncoderRejectsEmptyAndOversizedPayloads() {
        XCTAssertThrowsError(try PetalStreamEncoder(payload: [UInt8](), kind: 0)) {
            XCTAssertEqual($0 as? PetalStreamError, .emptyPayload)
        }
        XCTAssertThrowsError(try PetalStreamEncoder(payload: Data(), kind: 0)) {
            XCTAssertEqual($0 as? PetalStreamError, .emptyPayload)
        }
        XCTAssertThrowsError(
            try PetalStreamEncoder(payload: [UInt8](repeating: 0, count: PetalStream.maximumPayloadLength + 1), kind: 0)
        ) {
            XCTAssertEqual($0 as? PetalStreamError, .payloadTooLarge)
        }
    }

    func testBeaconRoundtripThroughLaneD() throws {
        let encoder = try PetalStreamEncoder(payload: PetalTestSupport.payload(77, seed: 9), kind: 3)
        let d = encoder.laneData(frame: 512).d
        guard case .beacon(let beacon) = try XCTUnwrap(PetalStream.parseDLane(d)) else {
            return XCTFail("beacon expected")
        }
        XCTAssertEqual(beacon.meta, encoder.meta)
        XCTAssertEqual(beacon.header.frame, 512)
        XCTAssertNil(PetalStream.parseDLane(Array(d[..<11])))
        var bad = d
        bad[3] = 0x20
        XCTAssertNil(PetalStream.parseDLane(bad))
        var empty = d
        empty[5] = 0
        empty[6] = 0
        empty[7] = 0
        XCTAssertNil(PetalStream.parseDLane(empty), "zero-length beacons are rejected")
        // non-beacon frames carry an atom instead
        guard case .atoms = try XCTUnwrap(PetalStream.parseDLane(encoder.laneData(frame: 513).d)) else {
            return XCTFail("atoms expected")
        }
        XCTAssertNil(PetalStream.parseAtomLane(.d, data: d))
        XCTAssertNil(PetalStream.parseAtomLane(.p, data: [1, 2, 3]))
    }

    func testDataAndByteArrayPayloadsEncodeIdentically() throws {
        let bytes = PetalTestSupport.payload(333, seed: 8)
        let fromBytes = try PetalStreamEncoder(payload: bytes, kind: 4)
        let fromData = try PetalStreamEncoder(payload: Data(bytes), kind: 4)
        XCTAssertEqual(fromBytes.meta, fromData.meta)
        XCTAssertEqual(fromBytes.cells(frame: 3), fromData.cells(frame: 3))
        XCTAssertEqual(fromBytes.meta.sourceAtoms, 21)
        XCTAssertEqual(fromBytes.systematicFrames, 4)
    }
}

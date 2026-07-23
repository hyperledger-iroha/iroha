import XCTest
@testable import IrohaSwift

final class KagemushaQRStreamTests: XCTestCase {
    func testMeasuredReleaseArchivesStayWithinTheStandardQRFrameBudget() {
        let samples: [(String, Int, Int)] = [
            ("request", 824, 6),
            ("acknowledgement", 471, 4),
            ("payment-depth-1-hop-1", 6_677, 35),
            ("payment-depth-8-hop-8", 6_848, 35),
            ("payment-depth-16-hop-8", 7_040, 36),
            ("payment-depth-32-hop-8", 7_424, 38),
            ("payment-depth-64-hop-8", 8_192, 41),
        ]
        let options = KagemushaQRStreamOptions.standard
        for (label, archiveBytes, expectedFrames) in samples {
            let dataFrames = (archiveBytes + options.chunkSize - 1) / options.chunkSize
            let parityFrames = (dataFrames + options.parityGroup - 1)
                / options.parityGroup
            XCTAssertEqual(1 + dataFrames + parityFrames, expectedFrames, label)
            XCTAssertLessThanOrEqual(
                archiveBytes,
                KagemushaPeerTransportContract.maximumArchiveBytes,
                label
            )
        }
    }

    func testEveryFrameRoundTripsForEveryPeerPayloadAndReassemblesOutOfOrder() throws {
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let request = try offer.project().request
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let acknowledgement = try KagemushaPeerTransportTestFixtures.acknowledgement(
            request: request,
            payment: payment
        )
        let payloads: [KagemushaPeerPayload] = [
            .receiveRequest(offer),
            .payment(payment),
            .acknowledgement(acknowledgement),
        ]

        for payload in payloads {
            let frameTexts = try KagemushaQRStreamCodec.encode(
                payload,
                options: .standard
            )
            let frames = try frameTexts.map(KagemushaQRStreamCodec.decodeFrameText)
            XCTAssertEqual(frames.map(text), frameTexts, "\(payload.kind)")

            let decoder = KagemushaQRStreamDecoder()
            var result: KagemushaQRDecodeResult?
            do {
                for (offset, frame) in frames.reversed().enumerated() {
                    let frameText = text(frame)
                    result = try decoder.ingest(frameText)
                    if offset.isMultiple(of: 3) {
                        result = try decoder.ingest(frameText)
                    }
                }
            } catch {
                if payload.kind == .payment,
                   !KagemushaRecursiveSpend.hasRequiredNativeSymbols {
                    XCTAssertEqual(error as? KagemushaQRStreamError, .invalidPayload)
                    continue
                }
                throw error
            }
            XCTAssertEqual(result?.payload, payload, "\(payload.kind)")
            XCTAssertEqual(result?.progress, 1, "\(payload.kind)")
        }
    }

    func testStreamRoundTripSupportsHeaderLastOutOfOrderAndDuplicates() throws {
        let payload = KagemushaPeerPayload.receiveRequest(
            try KagemushaPeerTransportTestFixtures.receiveRequest()
        )
        let frames = try KagemushaQRStreamCodec.encode(
            payload,
            options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 4)
        )
        XCTAssertGreaterThan(frames.count, 3)
        XCTAssertTrue(frames.allSatisfy { $0.hasPrefix("PKKQ1.") })

        let decoder = KagemushaQRStreamDecoder()
        var result: KagemushaQRDecodeResult?
        for frame in frames.reversed() {
            result = try decoder.ingest(frame)
        }
        result = try decoder.ingest(frames[0])
        XCTAssertEqual(result?.payload, payload)
        XCTAssertEqual(result?.payloadKind, .receiveRequest)
        XCTAssertEqual(result?.progress, 1)
    }

    func testOneMissingChunkPerParityGroupIsRecovered() throws {
        let payload = KagemushaPeerPayload.receiveRequest(
            try KagemushaPeerTransportTestFixtures.receiveRequest()
        )
        let frames = try KagemushaQRStreamCodec.encode(
            payload,
            options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 4)
        )
        let decoded = try frames.map(KagemushaQRStreamCodec.decodeFrameText)
        let missing = try XCTUnwrap(decoded.first { $0.kind == .data && $0.index == 1 })
        let decoder = KagemushaQRStreamDecoder()
        var result: KagemushaQRDecodeResult?
        for (text, frame) in zip(frames, decoded) where frame != missing {
            result = try decoder.ingest(text)
        }
        XCTAssertEqual(result?.payload, payload)
        XCTAssertEqual(result?.recoveredDataFrames, 1)
    }

    func testTwoMissingChunksInOneParityGroupRemainIncomplete() throws {
        let payload = KagemushaPeerPayload.receiveRequest(
            try KagemushaPeerTransportTestFixtures.receiveRequest()
        )
        let frames = try KagemushaQRStreamCodec.encode(
            payload,
            options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 4)
        )
        let decoded = try frames.map(KagemushaQRStreamCodec.decodeFrameText)
        let decoder = KagemushaQRStreamDecoder()
        var result: KagemushaQRDecodeResult?
        for (text, frame) in zip(frames, decoded)
            where !(frame.kind == .data && (frame.index == 0 || frame.index == 1)) {
            result = try decoder.ingest(text)
        }
        XCTAssertFalse(try XCTUnwrap(result).isComplete)
        XCTAssertLessThan(try XCTUnwrap(result).progress, 1)
    }

    func testMixedStreamsAreRejectedAndResetAllowsNewStream() throws {
        let first = try KagemushaQRStreamCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest(seed: 0x41)
        ))
        let second = try KagemushaQRStreamCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest(seed: 0x61)
        ))
        let decoder = KagemushaQRStreamDecoder()
        _ = try decoder.ingest(first[0])
        XCTAssertThrowsError(try decoder.ingest(second[0])) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .wrongStream)
        }
        decoder.reset()
        XCTAssertNoThrow(try decoder.ingest(second[0]))
    }

    func testConflictingDuplicateDataFrameIsRejectedEvenWithValidCRC() throws {
        let frames = try KagemushaQRStreamCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest()
        ))
        let originalText = try XCTUnwrap(frames.first {
            (try? KagemushaQRStreamCodec.decodeFrameText($0).kind) == .data
        })
        let original = try KagemushaQRStreamCodec.decodeFrameText(originalText)
        var bytes = original.payload
        bytes[0] ^= 0x80
        let conflicting = try KagemushaQRStreamFrame(
            kind: .data,
            streamID: original.streamID,
            index: original.index,
            total: original.total,
            payload: bytes
        )
        let decoder = KagemushaQRStreamDecoder()
        _ = try decoder.ingest(originalText)
        XCTAssertThrowsError(try decoder.ingest(text(conflicting))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .conflictingFrame)
        }

        let conflictingTotal = try KagemushaQRStreamFrame(
            kind: .data,
            streamID: original.streamID,
            index: original.index,
            total: original.total + 1,
            payload: original.payload
        )
        let totalDecoder = KagemushaQRStreamDecoder()
        _ = try totalDecoder.ingest(originalText)
        XCTAssertThrowsError(try totalDecoder.ingest(text(conflictingTotal))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .conflictingFrame)
        }
    }

    func testCorruptFrameChecksumAndNonCanonicalTextAreRejected() throws {
        let frameText = try KagemushaQRStreamCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest()
        ))[0]
        let body = String(frameText.dropFirst("PKKQ1.".count))
        var bytes = try XCTUnwrap(KagemushaPeerTextCodec.base64URLDecode(body))
        bytes[bytes.count - 5] ^= 1
        let corrupt = "PKKQ1." + KagemushaPeerTextCodec.base64URLEncode(bytes)
        XCTAssertThrowsError(try KagemushaQRStreamDecoder().ingest(corrupt)) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .checksumMismatch)
        }
        for invalid in [frameText + "=", " " + frameText, frameText + "\n", "pkkq1." + body] {
            XCTAssertThrowsError(try KagemushaQRStreamDecoder().ingest(invalid), invalid)
        }
    }

    func testForgedFullDigestFailsAfterOtherwiseCompleteStream() throws {
        let frames = try KagemushaQRStreamCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest()
        ), options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 4))
        let originals = try frames.map(KagemushaQRStreamCodec.decodeFrameText)
        let header = try XCTUnwrap(originals.first { $0.kind == .header })
        var headerPayload = header.payload
        headerPayload.replaceSubrange(14..<46, with: Data(repeating: 0x99, count: 32))
        let forgedStreamID = Data(repeating: 0x99, count: 16)
        let forged = try originals.map { frame in
            try KagemushaQRStreamFrame(
                kind: frame.kind,
                streamID: forgedStreamID,
                index: frame.index,
                total: frame.total,
                payload: frame.kind == .header ? headerPayload : frame.payload
            )
        }
        let decoder = KagemushaQRStreamDecoder()
        XCTAssertThrowsError(try forged.reduce(nil as KagemushaQRDecodeResult?) { _, frame in
            try decoder.ingest(text(frame))
        }) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .digestMismatch)
        }
    }

    func testForgedPayloadKindAndHeaderBoundsFailClosed() throws {
        let frames = try KagemushaQRStreamCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest()
        ), options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 4))
        let decoded = try frames.map(KagemushaQRStreamCodec.decodeFrameText)
        let header = try XCTUnwrap(decoded.first { $0.kind == .header })

        var forgedKindPayload = header.payload
        forgedKindPayload[1] = KagemushaPeerPayloadKind.payment.rawValue
        let forgedKindHeader = try KagemushaQRStreamFrame(
            kind: .header,
            streamID: header.streamID,
            index: 0,
            total: 1,
            payload: forgedKindPayload
        )
        let kindDecoder = KagemushaQRStreamDecoder()
        var kindError: Error?
        do {
            for frame in decoded {
                _ = try kindDecoder.ingest(text(frame.kind == .header ? forgedKindHeader : frame))
            }
        } catch { kindError = error }
        XCTAssertEqual(kindError as? KagemushaQRStreamError, .invalidPayload)

        var invalidBounds = header.payload
        invalidBounds[4] = 0
        invalidBounds[5] = 1
        let badHeader = try KagemushaQRStreamFrame(
            kind: .header,
            streamID: header.streamID,
            index: 0,
            total: 1,
            payload: invalidBounds
        )
        XCTAssertThrowsError(try KagemushaQRStreamDecoder().ingest(text(badHeader))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .invalidHeader)
        }
    }

    func testInvalidHeaderRollsBackInitialStreamSelection() throws {
        let firstFrames = try KagemushaQRStreamCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest(seed: 0x41)
        ))
        let firstHeader = try KagemushaQRStreamCodec.decodeFrameText(firstFrames[0])
        var invalidHeaderPayload = firstHeader.payload
        invalidHeaderPayload[3] = 1 // reserved byte must be zero
        let invalidHeader = try KagemushaQRStreamFrame(
            kind: .header,
            streamID: firstHeader.streamID,
            index: 0,
            total: 1,
            payload: invalidHeaderPayload
        )
        let secondPayload = KagemushaPeerPayload.receiveRequest(
            try KagemushaPeerTransportTestFixtures.receiveRequest(seed: 0x61)
        )
        let secondFrames = try KagemushaQRStreamCodec.encode(secondPayload)
        let decoder = KagemushaQRStreamDecoder()

        XCTAssertThrowsError(try decoder.ingest(text(invalidHeader))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .invalidHeader)
        }
        var result: KagemushaQRDecodeResult?
        for frame in secondFrames { result = try decoder.ingest(frame) }
        XCTAssertEqual(result?.payload, secondPayload)
    }

    func testInvalidSameStreamChunkLengthRollsBackBufferedFrame() throws {
        let payload = KagemushaPeerPayload.receiveRequest(
            try KagemushaPeerTransportTestFixtures.receiveRequest()
        )
        let texts = try KagemushaQRStreamCodec.encode(payload)
        let frames = try texts.map(KagemushaQRStreamCodec.decodeFrameText)
        let header = try XCTUnwrap(frames.first { $0.kind == .header })
        let original = try XCTUnwrap(frames.first {
            $0.kind == .data && $0.payload.count > 1
        })
        let wrongLength = try KagemushaQRStreamFrame(
            kind: .data,
            streamID: original.streamID,
            index: original.index,
            total: original.total,
            payload: original.payload.dropLast()
        )
        let decoder = KagemushaQRStreamDecoder()
        _ = try decoder.ingest(text(header))

        XCTAssertThrowsError(try decoder.ingest(text(wrongLength))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .malformedFrame)
        }
        var result: KagemushaQRDecodeResult?
        for (frameText, frame) in zip(texts, frames) where frame != header {
            result = try decoder.ingest(frameText)
        }
        XCTAssertEqual(result?.payload, payload)
    }

    func testConflictingSameStreamDuplicateDoesNotPoisonValidFrames() throws {
        let payload = KagemushaPeerPayload.receiveRequest(
            try KagemushaPeerTransportTestFixtures.receiveRequest()
        )
        let texts = try KagemushaQRStreamCodec.encode(payload)
        let frames = try texts.map(KagemushaQRStreamCodec.decodeFrameText)
        let header = try XCTUnwrap(frames.first { $0.kind == .header })
        let original = try XCTUnwrap(frames.first { $0.kind == .data })
        var forgedPayload = original.payload
        forgedPayload[0] ^= 0x80
        let conflict = try KagemushaQRStreamFrame(
            kind: .data,
            streamID: original.streamID,
            index: original.index,
            total: original.total,
            payload: forgedPayload
        )
        let decoder = KagemushaQRStreamDecoder()
        _ = try decoder.ingest(text(header))
        _ = try decoder.ingest(text(original))
        XCTAssertThrowsError(try decoder.ingest(text(conflict))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .conflictingFrame)
        }

        var result: KagemushaQRDecodeResult?
        for (frameText, frame) in zip(texts, frames)
            where frame != header && frame != original {
            result = try decoder.ingest(frameText)
        }
        XCTAssertEqual(result?.payload, payload)
    }

    func testFrameTextIsSizeBoundedBeforeBase64Decoding() throws {
        XCTAssertEqual(
            KagemushaQRStreamCodec.maximumFrameTextBytes,
            KagemushaPeerTransportContract.qrStreamTextPrefix.utf8.count
                + (KagemushaQRStreamFrame.maximumEncodedBytes * 4 + 2) / 3
        )
        let oversized = KagemushaPeerTransportContract.qrStreamTextPrefix
            + String(
                repeating: "A",
                count: KagemushaQRStreamCodec.maximumFrameTextBytes
                    - KagemushaPeerTransportContract.qrStreamTextPrefix.utf8.count
                    + 1
            )
        XCTAssertEqual(
            oversized.utf8.count,
            KagemushaQRStreamCodec.maximumFrameTextBytes + 1
        )
        XCTAssertThrowsError(try KagemushaQRStreamDecoder().ingest(oversized)) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .nonCanonicalFrame)
        }

        let valid = try KagemushaQRStreamCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest()
        ))
        XCTAssertTrue(valid.allSatisfy {
            $0.utf8.count <= KagemushaQRStreamCodec.maximumFrameTextBytes
        })
    }

    func testRepresentableFrameTotalIsAcceptedWithoutPreallocationAndResetAllowsValidStream() throws {
        let payload = KagemushaPeerPayload.receiveRequest(
            try KagemushaPeerTransportTestFixtures.receiveRequest()
        )
        let validTexts = try KagemushaQRStreamCodec.encode(payload)
        let validFrames = try validTexts.map(KagemushaQRStreamCodec.decodeFrameText)
        let dataFrame = try XCTUnwrap(validFrames.first { $0.kind == .data })
        let maximumRepresentableTotal = try KagemushaQRStreamFrame(
            kind: .data,
            streamID: dataFrame.streamID,
            index: dataFrame.index,
            total: Int(UInt16.max),
            payload: dataFrame.payload
        )
        let decoder = KagemushaQRStreamDecoder()

        XCTAssertNoThrow(try decoder.ingest(text(maximumRepresentableTotal)))
        decoder.reset()

        var result: KagemushaQRDecodeResult?
        for frameText in validTexts.reversed() {
            result = try decoder.ingest(frameText)
        }
        XCTAssertEqual(result?.payload, payload)
    }

    func testOptionsRejectCrashInducingAndResourceExhaustingValues() {
        for (chunk, parity) in [(0, 4), (63, 4), (513, 4), (64, 0), (64, 1), (64, 17)] {
            XCTAssertThrowsError(
                try KagemushaQRStreamOptions(chunkSize: chunk, parityGroup: parity),
                "chunk=\(chunk) parity=\(parity)"
            )
        }
    }

    private func text(_ frame: KagemushaQRStreamFrame) -> String {
        KagemushaPeerTransportContract.qrStreamTextPrefix
            + KagemushaPeerTextCodec.base64URLEncode(frame.encode())
    }
}

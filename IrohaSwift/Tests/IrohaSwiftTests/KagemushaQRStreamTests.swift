import XCTest
@testable import IrohaSwift

final class KagemushaQRStreamTests: XCTestCase {
    func testMeasuredReleaseArchivesStayWithinTheStandardQRFrameBudget() {
        let samples: [(String, Int, Int)] = [
            ("receive-offer", 12_435, 63),
            ("acknowledgement", 471, 4),
            ("payment-v4-peer-hop-1", 12_896, 65),
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
            XCTAssertLessThanOrEqual(
                expectedFrames,
                KagemushaQRStreamCodec.maximumStreamFrames,
                label
            )
        }
    }

    func testEveryFrameRoundTripsForEveryPeerPayloadAndReassemblesOutOfOrder() throws {
        try requireNativeTestCapability(
            KagemushaRecursiveSpend.hasRequiredNativeSymbols,
            "ABI-23 Kagemusha bridge is not linked in this test host"
        )
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let request = try offer.project(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ).request
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

            let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
            var result: KagemushaQRDecodeResult?
            result = try decoder.ingest(frameTexts[0])
            for (offset, frame) in frames.dropFirst().reversed().enumerated() {
                let frameText = text(frame)
                result = try decoder.ingest(frameText)
                if offset.isMultiple(of: 3) {
                    result = try decoder.ingest(frameText)
                }
            }
            XCTAssertEqual(result?.payload, payload, "\(payload.kind)")
            XCTAssertEqual(result?.progress, 1, "\(payload.kind)")
        }
    }

    func testDataBeforeHeaderIsRejectedThenHeaderFirstSupportsOutOfOrderAndDuplicates() throws {
        let payload = KagemushaPeerPayload.receiveRequest(
            try KagemushaPeerTransportTestFixtures.receiveRequest()
        )
        let frames = try KagemushaQRStreamCodec.encode(
            payload,
            options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 4)
        )
        XCTAssertGreaterThan(frames.count, 3)
        XCTAssertTrue(frames.allSatisfy { $0.hasPrefix("PKKQ1.") })

        let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
        XCTAssertThrowsError(try decoder.ingest(frames[1])) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .malformedFrame)
        }
        var result = try decoder.ingest(frames[0])
        for frame in frames.dropFirst().reversed() {
            result = try decoder.ingest(frame)
        }
        result = try decoder.ingest(frames[1])
        XCTAssertEqual(result.payload, payload)
        XCTAssertEqual(result.payloadKind, .receiveRequest)
        XCTAssertEqual(result.progress, 1)
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
        let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
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
        let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
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
        let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
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
        let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
        _ = try decoder.ingest(frames[0])
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
        let totalDecoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
        _ = try totalDecoder.ingest(frames[0])
        _ = try totalDecoder.ingest(originalText)
        XCTAssertThrowsError(try totalDecoder.ingest(text(conflictingTotal))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .malformedFrame)
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
        XCTAssertThrowsError(try KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1).ingest(corrupt)) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .checksumMismatch)
        }
        for invalid in [frameText + "=", " " + frameText, frameText + "\n", "pkkq1." + body] {
            XCTAssertThrowsError(try KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1).ingest(invalid), invalid)
        }
    }

    func testForgedFullDigestFailsAfterOtherwiseCompleteStream() throws {
        let frames = try KagemushaQRStreamCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest()
        ), options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 4))
        let originals = try frames.map(KagemushaQRStreamCodec.decodeFrameText)
        let header = try XCTUnwrap(originals.first { $0.kind == .header })
        var headerPayload = header.payload
        headerPayload.replaceSubrange(18..<50, with: Data(repeating: 0x99, count: 32))
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
        let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
        let forgedHeader = try XCTUnwrap(forged.first { $0.kind == .header })
        let forgedData = forged.filter { $0.kind == .data }
        _ = try decoder.ingest(text(forgedHeader))
        for frame in forgedData.dropLast() { _ = try decoder.ingest(text(frame)) }
        let finalFrame = try XCTUnwrap(forgedData.last)
        XCTAssertThrowsError(try decoder.ingest(text(finalFrame))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .digestMismatch)
        }
        XCTAssertThrowsError(try decoder.ingest(text(finalFrame))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .malformedFrame)
        }
        XCTAssertNoThrow(try decoder.ingest(frames[0]))
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
        let kindDecoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
        var kindError: Error?
        do {
            for frame in decoded {
                _ = try kindDecoder.ingest(text(frame.kind == .header ? forgedKindHeader : frame))
            }
        } catch { kindError = error }
        XCTAssertEqual(kindError as? KagemushaQRStreamError, .invalidPayload)
        let finalData = try XCTUnwrap(decoded.last { $0.kind == .data })
        XCTAssertThrowsError(try kindDecoder.ingest(text(finalData))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .malformedFrame)
        }
        XCTAssertNoThrow(try kindDecoder.ingest(text(header)))

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
        XCTAssertThrowsError(try KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1).ingest(text(badHeader))) { error in
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
        let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)

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
        let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
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
        let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
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
        XCTAssertThrowsError(try KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1).ingest(oversized)) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .nonCanonicalFrame)
        }

        let valid = try KagemushaQRStreamCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest()
        ))
        XCTAssertTrue(valid.allSatisfy {
            $0.utf8.count <= KagemushaQRStreamCodec.maximumFrameTextBytes
        })
    }

    func testHeaderDataAndParityFramesMatchCrossSDKGoldenVectors() throws {
        let expectedEnvelope = try XCTUnwrap(Data(hexString:
            "010202000040000000010000000100000003" +
            "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"
        ))
        let expectedFrame = try XCTUnwrap(Data(hexString:
            "4b510100039058c6f2c0cb492c533b0a4d14ef77" +
            "00000000000000010032" +
            "010202000040000000010000000100000003" +
            "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81" +
            "4807f6d1"
        ))
        let expectedText =
            "PKKQ1.S1EBAAOQWMbywMtJLFM7Ck0U73cAAAAAAAAAAQAy" +
            "AQICAABAAAAAAQAAAAEAAAADA5BYxvLAy0ksUzsKTRTvd8wPeKvMztUofYShogEc-4FIB_bR"
        let expectedDataFrame = try XCTUnwrap(Data(hexString:
            "4b510101039058c6f2c0cb492c533b0a4d14ef77" +
            "000000000000000100030102033f206e96"
        ))
        let expectedDataText =
            "PKKQ1.S1EBAQOQWMbywMtJLFM7Ck0U73cAAAAAAAAAAQADAQIDPyBulg"
        let expectedParityFrame = try XCTUnwrap(Data(hexString:
            "4b510102039058c6f2c0cb492c533b0a4d14ef77" +
            "00000000000000010040010203" + String(repeating: "00", count: 61) +
            "06035fc2"
        ))
        let expectedParityText =
            "PKKQ1.S1EBAgOQWMbywMtJLFM7Ck0U73cAAAAAAAAAAQBAAQIDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" +
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAYDX8I"

        let envelope = try KagemushaQRStreamEnvelope(
            kind: .payment,
            payload: Data([1, 2, 3]),
            options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 2)
        )
        let envelopeBytes = envelope.encode()
        XCTAssertEqual(envelopeBytes.count, KagemushaQRStreamEnvelope.encodedLength)
        XCTAssertEqual(envelopeBytes, expectedEnvelope)

        let frame = try KagemushaQRStreamFrame(
            kind: .header,
            streamID: envelope.streamID,
            index: 0,
            total: 1,
            payload: envelopeBytes
        )
        let frameBytes = frame.encode()
        XCTAssertEqual(
            frameBytes.count,
            KagemushaQRStreamFrame.fixedOverhead + KagemushaQRStreamEnvelope.encodedLength
        )
        XCTAssertEqual(frameBytes, expectedFrame)
        XCTAssertEqual(text(frame), expectedText)
        XCTAssertEqual(try KagemushaQRStreamCodec.decodeFrameText(expectedText), frame)

        let dataFrame = try KagemushaQRStreamFrame(
            kind: .data,
            streamID: envelope.streamID,
            index: 0,
            total: 1,
            payload: Data([1, 2, 3])
        )
        XCTAssertEqual(dataFrame.encode(), expectedDataFrame)
        XCTAssertEqual(text(dataFrame), expectedDataText)
        XCTAssertEqual(
            try KagemushaQRStreamCodec.decodeFrameText(expectedDataText),
            dataFrame
        )

        let parityFrame = try KagemushaQRStreamFrame(
            kind: .parity,
            streamID: envelope.streamID,
            index: 0,
            total: 1,
            payload: Data([1, 2, 3]) + Data(repeating: 0, count: 61)
        )
        XCTAssertEqual(parityFrame.encode(), expectedParityFrame)
        XCTAssertEqual(text(parityFrame), expectedParityText)
        XCTAssertEqual(
            try KagemushaQRStreamCodec.decodeFrameText(expectedParityText),
            parityFrame
        )
    }

    func testStreamFrameCapIsPreflightedAndMismatchedTotalDoesNotPoisonStream() throws {
        let archive = Data((0..<129).map { UInt8(truncatingIfNeeded: $0 * 17 + 3) })
        let options = try KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 4)
        let envelope = try KagemushaQRStreamEnvelope(
            kind: .receiveRequest,
            payload: archive,
            options: options
        )
        let header = try KagemushaQRStreamFrame(
            kind: .header,
            streamID: envelope.streamID,
            index: 0,
            total: 1,
            payload: envelope.encode()
        )
        let dataFrame = try KagemushaQRStreamFrame(
            kind: .data,
            streamID: envelope.streamID,
            index: 0,
            total: envelope.dataChunks,
            payload: archive.prefix(options.chunkSize)
        )
        let boundaryFrame = try KagemushaQRStreamFrame(
            kind: .data,
            streamID: dataFrame.streamID,
            index: KagemushaQRStreamCodec.maximumStreamFrames - 2,
            total: KagemushaQRStreamCodec.maximumStreamFrames - 1,
            payload: dataFrame.payload
        )
        let decodedBoundaryFrame = try KagemushaQRStreamFrame.decode(boundaryFrame.encode())
        XCTAssertEqual(decodedBoundaryFrame.index, 4_094)
        XCTAssertEqual(decodedBoundaryFrame.total, 4_095)
        XCTAssertThrowsError(try KagemushaQRStreamFrame(
            kind: .data,
            streamID: dataFrame.streamID,
            index: 0,
            total: KagemushaQRStreamCodec.maximumStreamFrames,
            payload: dataFrame.payload
        )) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .malformedFrame)
        }

        let decoder = KagemushaQRStreamDecoder(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
        XCTAssertThrowsError(try decoder.ingest(text(dataFrame))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .malformedFrame)
        }
        _ = try decoder.ingest(text(header))
        let mismatchedTotal = try KagemushaQRStreamFrame(
            kind: .data,
            streamID: dataFrame.streamID,
            index: dataFrame.index,
            total: dataFrame.total + 1,
            payload: dataFrame.payload
        )

        XCTAssertThrowsError(try decoder.ingest(text(mismatchedTotal))) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .malformedFrame)
        }

        let result = try decoder.ingest(text(dataFrame))
        XCTAssertFalse(result.isComplete)
        XCTAssertEqual(result.receivedDataFrames, 1)
        XCTAssertEqual(result.totalDataFrames, envelope.dataChunks)

        let boundaryBytes = 3_854 * options.chunkSize
        XCTAssertEqual(
            try KagemushaQRStreamCodec.preflightStreamFrameCount(
                payloadBytes: boundaryBytes,
                options: try KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 16)
            ),
            KagemushaQRStreamCodec.maximumStreamFrames
        )
        XCTAssertThrowsError(try KagemushaQRStreamCodec.preflightStreamFrameCount(
            payloadBytes: 3_855 * options.chunkSize,
            options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 16)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaQRStreamError,
                .tooManyFrames(actual: 4_097, maximum: 4_096)
            )
        }
        let boundaryArchive = Data(repeating: 0x5A, count: boundaryBytes)
        let boundaryEnvelope = try KagemushaQRStreamEnvelope(
            kind: .payment,
            payload: boundaryArchive,
            options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 16)
        )
        XCTAssertEqual(boundaryEnvelope.dataChunks, 3_854)
        XCTAssertEqual(boundaryEnvelope.parityChunks, 241)
        XCTAssertEqual(
            try KagemushaQRStreamEnvelope.decode(boundaryEnvelope.encode()),
            boundaryEnvelope
        )
        XCTAssertThrowsError(try KagemushaQRStreamEnvelope(
            kind: .payment,
            payload: Data(repeating: 0x5A, count: 3_855 * options.chunkSize),
            options: KagemushaQRStreamOptions(chunkSize: 64, parityGroup: 16)
        )) { error in
            XCTAssertEqual(
                error as? KagemushaQRStreamError,
                .tooManyFrames(actual: 4_097, maximum: 4_096)
            )
        }

        var overCapEnvelopeBytes = boundaryEnvelope.encode()
        overCapEnvelopeBytes[9] = 0x0F
        overCapEnvelopeBytes[17] = 0xC0
        XCTAssertThrowsError(try KagemushaQRStreamEnvelope.decode(overCapEnvelopeBytes)) { error in
            XCTAssertEqual(error as? KagemushaQRStreamError, .invalidHeader)
        }
    }

    func testTemporaryFrameZeroizationDoesNotMutateCallerBuffers() throws {
        let streamID = Data(repeating: 0x41, count: 16)
        let payload = Data([1, 2, 3])
        var frame = try KagemushaQRStreamFrame(
            kind: .data,
            streamID: streamID,
            index: 0,
            total: 1,
            payload: payload
        )
        frame.zeroize()
        XCTAssertTrue(frame.streamID.allSatisfy { $0 == 0 })
        XCTAssertTrue(frame.payload.allSatisfy { $0 == 0 })
        XCTAssertEqual(streamID, Data(repeating: 0x41, count: 16))
        XCTAssertEqual(payload, Data([1, 2, 3]))
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

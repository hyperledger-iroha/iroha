import XCTest
@testable import IrohaSwift

final class OfflineCashQRStreamV1Tests: XCTestCase {
    func testSmallCanonicalPeerTextRoundTripsAsOneTypedFrame() throws {
        let text = peerText(kind: .receiveRequest, rawArchiveBytes: 49)
        let frames = try OfflineCashQRStreamCodecV1.encodePeerText(
            text,
            kind: .receiveRequest
        )
        XCTAssertEqual(frames.count, 1)
        let frame = try OfflineCashQRStreamCodecV1.decodeFrameText(try XCTUnwrap(frames.first))
        XCTAssertEqual(frame.profile, .offlineCashV1)
        XCTAssertEqual(frame.payloadKind, .receiveRequest)

        let result = try OfflineCashQRStreamDecoderV1(
            expectedKind: .receiveRequest
        ).ingest(try XCTUnwrap(frames.first))
        XCTAssertTrue(result.isComplete)
        XCTAssertEqual(result.completedPeerText, text)
        XCTAssertEqual(result.kind, .receiveRequest)
        XCTAssertEqual(result.streamID.count, 16)
        XCTAssertEqual(result.fractionComplete, 1)
    }

    func testMaximumPeerTextRoundTripsThroughReorderedAnimatedFrames() throws {
        let text = peerText(kind: .payment, rawArchiveBytes: 7_936)
        XCTAssertEqual(text.utf8.count, OfflineCashPeerAdapterV1.maximumPaymentTextBytes)
        let frames = try OfflineCashQRStreamCodecV1.encodePeerText(text, kind: .payment)
        XCTAssertGreaterThan(frames.count, 1)
        let header = try XCTUnwrap(frames.first {
            (try? OfflineCashQRStreamCodecV1.decodeFrameText($0).frameKind) == .header
        })
        let reordered = [header] + frames.reversed().filter { $0 != header }
        let decoder = OfflineCashQRStreamDecoderV1(expectedKind: .payment)
        var completed: OfflineCashQRStreamProgressV1? = nil
        for frame in reordered {
            let update = try decoder.ingest(frame)
            if update.isComplete {
                completed = update
                break
            }
        }
        let result = try XCTUnwrap(completed)
        XCTAssertEqual(result.completedPeerText, text)
        XCTAssertEqual(result.kind, .payment)
        XCTAssertEqual(result.streamID.count, 16)
    }

    func testParityRecoversOneMissingDataFramePerPair() throws {
        let text = peerText(kind: .payment, rawArchiveBytes: 2_048)
        let frames = try OfflineCashQRStreamCodecV1.encodePeerText(
            text,
            kind: .payment,
            options: OfflineCashQRStreamOptionsV1(compressionPolicy: .disabled)
        )
        let filtered = try frames.filter { text in
            let frame = try OfflineCashQRStreamCodecV1.decodeFrameText(text)
            return !(frame.frameKind == .data && frame.index % 2 == 1)
        }
        let decoder = OfflineCashQRStreamDecoderV1(expectedKind: .payment)
        var completed: OfflineCashQRStreamProgressV1? = nil
        for frame in filtered {
            let update = try decoder.ingest(frame)
            if update.isComplete {
                completed = update
                break
            }
        }
        let result = try XCTUnwrap(completed)
        XCTAssertEqual(result.completedPeerText, text)
        XCTAssertGreaterThan(result.recoveredDataFrames, 0)
        XCTAssertEqual(result.kind, .payment)
    }

    func testBoundsKindAndIntegrityFailClosed() throws {
        XCTAssertThrowsError(
            try OfflineCashQRStreamCodecV1.encodePeerText("kgm2:AA=", kind: .payment)
        )
        let oversized = peerText(kind: .payment, rawArchiveBytes: 7_937)
        XCTAssertGreaterThan(
            oversized.utf8.count,
            OfflineCashPeerAdapterV1.maximumPaymentTextBytes
        )
        XCTAssertThrowsError(
            try OfflineCashQRStreamCodecV1.encodePeerText(oversized, kind: .payment)
        )

        let frame = try XCTUnwrap(
            OfflineCashQRStreamCodecV1.encodePeerText(
                peerText(kind: .receiveRequest, rawArchiveBytes: 49),
                kind: .receiveRequest
            ).first
        )
        XCTAssertThrowsError(
            try OfflineCashQRStreamDecoderV1(expectedKind: .payment).ingest(frame)
        )
        let replacement = frame.dropLast(2).last == "0" ? "1" : "0"
        let corrupted = String(frame.dropLast(2)) + replacement + ":"
        XCTAssertThrowsError(try OfflineCashQRStreamCodecV1.decodeFrameText(corrupted))
    }

    func testDuplicateProgressAndExplicitQuarantineExposeStreamContext() throws {
        let text = peerText(kind: .payment, rawArchiveBytes: 1_024)
        let frames = try OfflineCashQRStreamCodecV1.encodePeerText(
            text,
            kind: .payment,
            options: OfflineCashQRStreamOptionsV1(compressionPolicy: .disabled)
        )
        let header = try XCTUnwrap(frames.first {
            (try? OfflineCashQRStreamCodecV1.decodeFrameText($0).frameKind) == .header
        })
        let decoder = OfflineCashQRStreamDecoderV1(expectedKind: .payment)
        let accepted = try decoder.ingest(header, atUptime: 10)
        let duplicate = try decoder.ingest(header, atUptime: 11)
        XCTAssertFalse(accepted.isDuplicate)
        XCTAssertTrue(duplicate.isDuplicate)
        XCTAssertEqual(duplicate.kind, .payment)
        XCTAssertEqual(duplicate.streamID, accepted.streamID)
        try decoder.quarantine(streamID: duplicate.streamID, atUptime: 12)
        XCTAssertThrowsError(try decoder.ingest(header, atUptime: 13))
    }

    private func peerText(
        kind: IrohaPeerWireKindV1,
        rawArchiveBytes: Int
    ) -> String {
        let schema: String
        let alignment: Int
        switch kind {
        case .receiveRequest:
            schema = "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentRequestV1"
            alignment = 8
        case .payment:
            schema = "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentV1"
            alignment = 8
        case .acknowledgement:
            schema =
                "iroha_data_model::offline::offline_cash_v1::OfflineCashAcknowledgementV1"
            alignment = 1
        }
        let padding = noritoHeaderPaddingLength(payloadAlignment: alignment) ?? 0
        let payloadBytes = rawArchiveBytes - NoritoHeader.encodedLength - padding
        precondition(payloadBytes > 0)
        let archive = noritoEncode(
            typeName: schema,
            payload: deterministicBytes(count: payloadBytes, seed: UInt32(rawArchiveBytes)),
            flags: NoritoHeader.compactLen,
            payloadAlignment: alignment
        )
        return OfflineCashPeerAdapterV1.textPrefix
            + archive.base64EncodedString()
                .replacingOccurrences(of: "+", with: "-")
                .replacingOccurrences(of: "/", with: "_")
                .replacingOccurrences(of: "=", with: "")
    }

    private func deterministicBytes(count: Int, seed: UInt32) -> Data {
        var state = seed
        return Data((0..<count).map { _ in
            state = 1_664_525 &* state &+ 1_013_904_223
            return UInt8(truncatingIfNeeded: state >> 24)
        })
    }
}

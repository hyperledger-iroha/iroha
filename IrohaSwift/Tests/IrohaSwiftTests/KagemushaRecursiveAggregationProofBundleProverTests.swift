import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveAggregationProofBundleProverTests: XCTestCase {
    func testRejectsEmptyRecordBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: Data(),
                    pallasOpenEnvelopesArchive: validKagemushaNoritoArchive()
                )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .emptyRecordBundleArchive
            )
        }
    }

    func testRejectsEmptyPallasOpenEnvelopesArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validKagemushaNoritoArchive(),
                    pallasOpenEnvelopesArchive: Data()
                )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .emptyPallasOpenEnvelopesArchive
            )
        }
    }

    func testRejectsEmptyNativeOutput() {
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validKagemushaNoritoArchive(),
                    pallasOpenEnvelopesArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    Data()
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .proofRejected
            )
        }
    }

    func testRejectsMalformedInputArchivesBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveAggregationProofBundleProver
                    .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                        recordBundleArchive: malformedArchive,
                        pallasOpenEnvelopesArchive: validArchive,
                        bridgeAvailable: false
                    ) {
                        XCTFail("native recursive aggregation body must not run for malformed record bundles")
                        return nil
                    },
                "record bundle \(label) should be rejected"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveAggregationProofBundleProverError,
                    .invalidRecordBundleArchive
                )
            }
        }
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveAggregationProofBundleProver
                    .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                        recordBundleArchive: validArchive,
                        pallasOpenEnvelopesArchive: malformedArchive,
                        bridgeAvailable: false
                    ) {
                        XCTFail("native recursive aggregation body must not run for malformed Pallas openings")
                        return nil
                    },
                "Pallas openings \(label) should be rejected"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveAggregationProofBundleProverError,
                    .invalidPallasOpenEnvelopesArchive
                )
            }
        }
    }

    func testRejectsOversizedInputArchivesBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        let oversizedArchive = Data(
            repeating: 0x7f,
            count: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1
        )
        let oversizedMessage =
            "must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes"
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: oversizedArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native recursive aggregation body must not run for oversized record bundles")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .oversizedRecordBundleArchive
            )
            XCTAssertTrue(error.localizedDescription.contains(oversizedMessage))
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: oversizedArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native recursive aggregation body must not run for oversized Pallas openings")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .oversizedPallasOpenEnvelopesArchive
            )
            XCTAssertTrue(error.localizedDescription.contains(oversizedMessage))
        }
    }

    func testRejectsEmptyPayloadInputArchivesBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        let emptyPayloadArchive = emptyPayloadKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: emptyPayloadArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native recursive aggregation body must not run for empty record-bundle payloads")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .emptyRecordBundlePayload
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: emptyPayloadArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native recursive aggregation body must not run for empty Pallas payloads")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .emptyPallasOpenEnvelopesPayload
            )
        }
    }

    func testRejectsMalformedNativeOutput() {
        let validArchive = validKagemushaNoritoArchive()
        for (label, nativeOutput) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveAggregationProofBundleProver
                    .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                        recordBundleArchive: validArchive,
                        pallasOpenEnvelopesArchive: validArchive,
                        bridgeAvailable: true
                    ) {
                        nativeOutput
                    },
                "native output \(label) should be rejected"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveAggregationProofBundleProverError,
                    .invalidProofBundleArchive
                )
            }
        }
    }

    private func malformedKagemushaNoritoArchives(_ validArchive: Data) -> [(String, Data)] {
        var compressed = validArchive
        compressed[22] = 0x01
        var unsupportedFlags = validArchive
        unsupportedFlags[39] = NoritoHeader.varintOffsets
        var invalidFieldBitset = validArchive
        invalidFieldBitset[39] = NoritoHeader.fieldBitset
        return [
            ("truncated", Data([0x01])),
            ("compressed", compressed),
            ("unsupported flags", unsupportedFlags),
            ("invalid field bitset", invalidFieldBitset),
            (
                "nonzero header padding",
                kagemushaNoritoFrameWithHeaderPadding(validArchive, padding: Data([0x7f]))
            ),
            (
                "excessive header padding",
                kagemushaNoritoFrameWithHeaderPadding(
                    validArchive,
                    padding: Data(repeating: 0, count: 65)
                )
            ),
        ]
    }

    private func kagemushaNoritoFrameWithHeaderPadding(_ archive: Data, padding: Data) -> Data {
        var padded = archive
        padded.insert(contentsOf: padding, at: NoritoHeader.encodedLength)
        return padded
    }

    func testRejectsEmptyPayloadNativeOutput() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: true
                ) {
                    emptyPayloadKagemushaNoritoArchive()
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .emptyProofBundlePayload
            )
        }
    }

    func testReturnsValidNativeOutput() throws {
        let archive = validKagemushaNoritoArchive()
        let output = try KagemushaRecursiveAggregationProofBundleProver
            .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                recordBundleArchive: archive,
                pallasOpenEnvelopesArchive: archive,
                bridgeAvailable: true
            ) {
                archive
            }

        XCTAssertEqual(output, archive)
    }

    func testNilNativeOutputIsBridgeUnavailable() {
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validKagemushaNoritoArchive(),
                    pallasOpenEnvelopesArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .bridgeUnavailable
            )
        }
    }
}

private func validKagemushaNoritoArchive() -> Data {
    noritoEncode(
        typeName: "KagemushaRecursiveAggregationProofBundleArchiveV1",
        payload: Data([0xa5, 0x5a, 0x11])
    )
}

private func emptyPayloadKagemushaNoritoArchive() -> Data {
    noritoEncode(
        typeName: "KagemushaRecursiveAggregationProofBundleArchiveV1",
        payload: Data()
    )
}

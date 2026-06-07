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
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: Data([0x01, 0x02]),
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native recursive aggregation body must not run for malformed record bundles")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .invalidRecordBundleArchive
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: Data([0x01, 0x02]),
                    bridgeAvailable: false
                ) {
                    XCTFail("native recursive aggregation body must not run for malformed Pallas openings")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .invalidPallasOpenEnvelopesArchive
            )
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
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: true
                ) {
                    Data([0x01])
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .invalidProofBundleArchive
            )
        }
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

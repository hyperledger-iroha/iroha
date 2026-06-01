import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveAggregationProofBundleProverTests: XCTestCase {
    func testRejectsEmptyRecordBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: Data(),
                    pallasOpenEnvelopesArchive: Data([0x01])
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
                    recordBundleArchive: Data([0x01]),
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
                    recordBundleArchive: Data([0x01]),
                    pallasOpenEnvelopesArchive: Data([0x02]),
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

    func testNilNativeOutputIsBridgeUnavailable() {
        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: Data([0x01]),
                    pallasOpenEnvelopesArchive: Data([0x02]),
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

    func testRejectsMalformedArchivesWhenBridgeIsAvailable() throws {
        guard KagemushaRecursiveAggregationProofBundleProver.isNativeAvailable else {
            throw XCTSkip("Native Kagemusha recursive aggregation proof-bundle prover is unavailable.")
        }

        XCTAssertThrowsError(
            try KagemushaRecursiveAggregationProofBundleProver
                .proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: Data([0x01, 0x02]),
                    pallasOpenEnvelopesArchive: Data([0x03, 0x04])
                )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveAggregationProofBundleProverError,
                .proofRejected
            )
        }
    }
}

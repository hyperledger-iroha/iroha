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

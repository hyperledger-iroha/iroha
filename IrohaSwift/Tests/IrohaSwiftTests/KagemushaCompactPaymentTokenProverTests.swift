import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaCompactPaymentTokenProverTests: XCTestCase {
    func testRejectsEmptyRecordBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive: Data())
        ) { error in
            XCTAssertEqual(
                error as? KagemushaCompactPaymentTokenProverError,
                .emptyRecordBundleArchive
            )
        }
    }

    func testRejectsMalformedRecordBundleArchiveWhenBridgeIsAvailable() throws {
        guard KagemushaCompactPaymentTokenProver.isNativeAvailable else {
            throw XCTSkip("Native Kagemusha compact-token prover is unavailable.")
        }

        XCTAssertThrowsError(
            try KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive: Data([0x01, 0x02]))
        ) { error in
            XCTAssertEqual(
                error as? KagemushaCompactPaymentTokenProverError,
                .proofRejected
            )
        }
    }
}

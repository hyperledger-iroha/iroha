import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveSpendProverTests: XCTestCase {
    func testPreferredModeDefaultsToRecursiveWhenAvailable() {
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredMode(recursiveSpendAvailable: true),
            .recursiveSpendV1
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendProver.preferredMode(recursiveSpendAvailable: false),
            .checkedPrefoldV1
        )
    }

    func testRejectsEmptyRequestArchivesBeforeBridgeCall() {
        let helpers: [(String, (Data) throws -> Data)] = [
            ("init", KagemushaRecursiveSpendProver.initSpend),
            ("append", KagemushaRecursiveSpendProver.appendSpend),
            ("verify", KagemushaRecursiveSpendProver.verifySpend),
            ("redeem", KagemushaRecursiveSpendProver.redeemSpend)
        ]

        for (label, helper) in helpers {
            XCTAssertThrowsError(try helper(Data()), "helper \(label) should reject empty archives") { error in
                XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .emptyRequestArchive)
            }
        }
    }

    func testRejectsEmptyNativeOutput() {
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: Data([0x01]),
                bridgeAvailable: true
            ) { _ in
                Data()
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
        }
    }

    func testNilNativeOutputIsBridgeUnavailable() {
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendProver.call(
                requestArchive: Data([0x01]),
                bridgeAvailable: true
            ) { _ in
                nil
            }
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .bridgeUnavailable)
        }
    }

    func testRejectsMalformedArchivesWhenBridgeIsAvailable() throws {
        guard KagemushaRecursiveSpendProver.isNativeAvailable else {
            throw XCTSkip("Native Kagemusha recursive spend prover is unavailable.")
        }

        let helpers: [(String, (Data) throws -> Data)] = [
            ("init", KagemushaRecursiveSpendProver.initSpend),
            ("append", KagemushaRecursiveSpendProver.appendSpend),
            ("verify", KagemushaRecursiveSpendProver.verifySpend),
            ("redeem", KagemushaRecursiveSpendProver.redeemSpend)
        ]

        for (label, helper) in helpers {
            XCTAssertThrowsError(try helper(Data([0x01, 0x02])), "helper \(label) should reject malformed archives") { error in
                XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
            }
        }
    }
}

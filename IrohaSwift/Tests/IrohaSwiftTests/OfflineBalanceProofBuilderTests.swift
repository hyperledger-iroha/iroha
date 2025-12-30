import XCTest
@testable import IrohaSwift

final class OfflineBalanceProofBuilderTests: XCTestCase {
    func testCommitmentAndProofGeneration() throws {
        #if canImport(Darwin)
        do {
            let artifacts = try OfflineBalanceProofBuilder.advanceCommitment(
                chainId: "swift-testnet",
                claimedDelta: "5",
                resultingValue: "5",
                initialCommitmentHex: String(repeating: "0", count: 64),
                initialBlindingHex: String(repeating: "0", count: 64),
                resultingBlindingHex: "01" + String(repeating: "0", count: 62)
            )
            XCTAssertEqual(artifacts.resultingCommitment.count, OfflineBalanceProofBuilder.commitmentLength)
            XCTAssertEqual(artifacts.proof.count, OfflineBalanceProofBuilder.proofLength)
            XCTAssertEqual(artifacts.resultingCommitmentHex.count, 64)
            XCTAssertEqual(artifacts.proofHex.count, OfflineBalanceProofBuilder.proofLength * 2)
        } catch OfflineBalanceProofError.bridgeUnavailable {
            throw XCTSkip("Offline balance proof bridge unavailable on this platform")
        }
        #else
        throw XCTSkip("Offline balance proof bridge unavailable on this platform")
        #endif
    }
}

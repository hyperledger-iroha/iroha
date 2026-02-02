import XCTest
import Security
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

    /// Verifies that advanceCommitment accepts raw random 32-byte blindings.
    /// Before the bridge fix (from_canonical_bytes → from_bytes_mod_order),
    /// ~87% of random blindings would be rejected with status -306.
    func testAdvanceCommitmentAcceptsRandomBlindings() throws {
        #if canImport(Darwin)
        let zeroHex = String(repeating: "0", count: 64)
        for i in 0..<20 {
            let blindingHex = try randomHex32()
            do {
                let artifacts = try OfflineBalanceProofBuilder.advanceCommitment(
                    chainId: "sora",
                    claimedDelta: "5",
                    resultingValue: "5",
                    initialCommitmentHex: zeroHex,
                    initialBlindingHex: zeroHex,
                    resultingBlindingHex: blindingHex
                )
                XCTAssertEqual(artifacts.resultingCommitment.count, 32, "Iteration \(i)")
                XCTAssertEqual(artifacts.proof.count, OfflineBalanceProofBuilder.proofLength, "Iteration \(i)")
            } catch OfflineBalanceProofError.bridgeUnavailable {
                throw XCTSkip("Offline balance proof bridge unavailable on this platform")
            } catch {
                XCTFail("Iteration \(i) failed with blinding \(blindingHex.prefix(16))...: \(error)")
            }
        }
        #else
        throw XCTSkip("Offline balance proof bridge unavailable on this platform")
        #endif
    }

    /// Two sequential spends with random blindings.
    func testTwoSequentialSpends() throws {
        #if canImport(Darwin)
        let zeroHex = String(repeating: "0", count: 64)
        let blinding1 = try randomHex32()
        let blinding2 = try randomHex32()

        // First: initialize with balance 10 (from zero: delta=10, result=10)
        let first: OfflineBalanceProofBuilder.Artifacts
        do {
            first = try OfflineBalanceProofBuilder.advanceCommitment(
                chainId: "sora",
                claimedDelta: "10",
                resultingValue: "10",
                initialCommitmentHex: zeroHex,
                initialBlindingHex: zeroHex,
                resultingBlindingHex: blinding1
            )
        } catch OfflineBalanceProofError.bridgeUnavailable {
            throw XCTSkip("Offline balance proof bridge unavailable on this platform")
        }

        // Second: add 3 more (from 10: delta=3, result=13)
        let second = try OfflineBalanceProofBuilder.advanceCommitment(
            chainId: "sora",
            claimedDelta: "3",
            resultingValue: "13",
            initialCommitmentHex: first.resultingCommitmentHex,
            initialBlindingHex: blinding1,
            resultingBlindingHex: blinding2
        )

        XCTAssertEqual(second.resultingCommitment.count, 32)
        XCTAssertNotEqual(second.resultingCommitmentHex, first.resultingCommitmentHex)
        #else
        throw XCTSkip("Offline balance proof bridge unavailable on this platform")
        #endif
    }

    // MARK: - Helpers

    private func randomHex32() throws -> String {
        var data = Data(count: 32)
        let status = data.withUnsafeMutableBytes { buf -> Int32 in
            guard let base = buf.baseAddress else { return errSecAllocate }
            return SecRandomCopyBytes(kSecRandomDefault, buf.count, base)
        }
        guard status == errSecSuccess else {
            throw NSError(domain: "test", code: Int(status))
        }
        return data.map { String(format: "%02x", $0) }.joined()
    }
}

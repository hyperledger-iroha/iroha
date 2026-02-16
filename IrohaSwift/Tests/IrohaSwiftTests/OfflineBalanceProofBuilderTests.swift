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

    // MARK: - deriveResultingBlinding → advanceCommitment round-trip

    /// Verifies that deriveResultingBlinding produces a valid blinding
    /// that advanceCommitment accepts and generates a correct proof.
    /// This is the exact flow used in offline spend on iOS.
    func testDeriveResultingBlindingRoundTrip() throws {
        #if canImport(Darwin)
        let zeroHex = String(repeating: "0", count: 64)
        let initialBlinding = try randomHex32()
        let certificateId = try randomHex32()
        let counter: UInt64 = 1

        // Step 1: build initial commitment (topUp equivalent: delta=50, result=50)
        let initial: OfflineBalanceProofBuilder.Artifacts
        do {
            initial = try OfflineBalanceProofBuilder.advanceCommitment(
                chainId: "sora",
                claimedDelta: "50",
                resultingValue: "50",
                initialCommitmentHex: zeroHex,
                initialBlindingHex: zeroHex,
                resultingBlindingHex: initialBlinding
            )
        } catch OfflineBalanceProofError.bridgeUnavailable {
            throw XCTSkip("Offline balance proof bridge unavailable on this platform")
        }
        XCTAssertEqual(initial.resultingCommitment.count, 32)
        XCTAssertEqual(initial.proof.count, OfflineBalanceProofBuilder.proofLength)

        // Step 2: derive resulting blinding deterministically (the key operation under test)
        let derivedBlinding: String
        do {
            derivedBlinding = try OfflineBalanceProofBuilder.deriveResultingBlinding(
                initialBlindingHex: initialBlinding,
                certificateIdHex: certificateId,
                counter: counter
            )
        } catch OfflineBalanceProofError.bridgeUnavailable {
            throw XCTSkip("offlineBlindingFromSeed bridge unavailable on this platform")
        }

        // Verify derived blinding is valid hex of correct length
        XCTAssertEqual(derivedBlinding.count, 64, "Derived blinding must be 32 bytes (64 hex chars)")
        XCTAssertNotNil(Data(hexString: derivedBlinding), "Derived blinding must be valid hex")
        XCTAssertNotEqual(derivedBlinding, initialBlinding, "Derived blinding must differ from initial")

        // Step 3: use derived blinding in advanceCommitment (offline spend: delta=4, result=54)
        let spend = try OfflineBalanceProofBuilder.advanceCommitment(
            chainId: "sora",
            claimedDelta: "4",
            resultingValue: "54",
            initialCommitmentHex: initial.resultingCommitmentHex,
            initialBlindingHex: initialBlinding,
            resultingBlindingHex: derivedBlinding
        )
        XCTAssertEqual(spend.resultingCommitment.count, 32)
        XCTAssertEqual(spend.proof.count, OfflineBalanceProofBuilder.proofLength)
        XCTAssertNotEqual(spend.resultingCommitmentHex, initial.resultingCommitmentHex)
        #else
        throw XCTSkip("Offline balance proof bridge unavailable on this platform")
        #endif
    }

    /// Two sequential offline spends using deriveResultingBlinding with incrementing counter.
    /// Mirrors the real flow: topUp → spend#1(counter=1) → spend#2(counter=2).
    func testTwoSequentialSpendsWithDerivedBlinding() throws {
        #if canImport(Darwin)
        let zeroHex = String(repeating: "0", count: 64)
        let initialBlinding = try randomHex32()
        let certificateId = try randomHex32()

        // topUp: delta=100, result=100
        let topUp: OfflineBalanceProofBuilder.Artifacts
        do {
            topUp = try OfflineBalanceProofBuilder.advanceCommitment(
                chainId: "sora",
                claimedDelta: "100",
                resultingValue: "100",
                initialCommitmentHex: zeroHex,
                initialBlindingHex: zeroHex,
                resultingBlindingHex: initialBlinding
            )
        } catch OfflineBalanceProofError.bridgeUnavailable {
            throw XCTSkip("Offline balance proof bridge unavailable on this platform")
        }

        // Spend #1: delta=10, cumulative spent=10, resultingValue = limit + spent = 100+10 = 110
        let blinding1 = try OfflineBalanceProofBuilder.deriveResultingBlinding(
            initialBlindingHex: initialBlinding,
            certificateIdHex: certificateId,
            counter: 1
        )
        let spend1 = try OfflineBalanceProofBuilder.advanceCommitment(
            chainId: "sora",
            claimedDelta: "10",
            resultingValue: "110",
            initialCommitmentHex: topUp.resultingCommitmentHex,
            initialBlindingHex: initialBlinding,
            resultingBlindingHex: blinding1
        )
        XCTAssertEqual(spend1.proof.count, OfflineBalanceProofBuilder.proofLength)

        // Spend #2: delta=5, cumulative spent=15, resultingValue = 100+15 = 115
        // NOTE: initialBlinding for spend#2 is blinding1 (result of spend#1)
        let blinding2 = try OfflineBalanceProofBuilder.deriveResultingBlinding(
            initialBlindingHex: blinding1,
            certificateIdHex: certificateId,
            counter: 2
        )
        let spend2 = try OfflineBalanceProofBuilder.advanceCommitment(
            chainId: "sora",
            claimedDelta: "5",
            resultingValue: "115",
            initialCommitmentHex: spend1.resultingCommitmentHex,
            initialBlindingHex: blinding1,
            resultingBlindingHex: blinding2
        )
        XCTAssertEqual(spend2.proof.count, OfflineBalanceProofBuilder.proofLength)
        XCTAssertNotEqual(spend2.resultingCommitmentHex, spend1.resultingCommitmentHex)

        // Verify all blindings are distinct
        XCTAssertNotEqual(initialBlinding, blinding1)
        XCTAssertNotEqual(blinding1, blinding2)
        XCTAssertNotEqual(initialBlinding, blinding2)
        #else
        throw XCTSkip("Offline balance proof bridge unavailable on this platform")
        #endif
    }

    /// deriveResultingBlinding must be deterministic: same inputs → same output.
    func testDeriveResultingBlindingIsDeterministic() throws {
        #if canImport(Darwin)
        let initialBlinding = try randomHex32()
        let certificateId = try randomHex32()
        let counter: UInt64 = 42

        let result1: String
        do {
            result1 = try OfflineBalanceProofBuilder.deriveResultingBlinding(
                initialBlindingHex: initialBlinding,
                certificateIdHex: certificateId,
                counter: counter
            )
        } catch OfflineBalanceProofError.bridgeUnavailable {
            throw XCTSkip("offlineBlindingFromSeed bridge unavailable on this platform")
        }

        let result2 = try OfflineBalanceProofBuilder.deriveResultingBlinding(
            initialBlindingHex: initialBlinding,
            certificateIdHex: certificateId,
            counter: counter
        )

        XCTAssertEqual(result1, result2, "deriveResultingBlinding must be deterministic")
        #else
        throw XCTSkip("Offline balance proof bridge unavailable on this platform")
        #endif
    }

    /// Different counters must produce different blindings.
    func testDeriveResultingBlindingDifferentCounters() throws {
        #if canImport(Darwin)
        let initialBlinding = try randomHex32()
        let certificateId = try randomHex32()

        let b1: String
        do {
            b1 = try OfflineBalanceProofBuilder.deriveResultingBlinding(
                initialBlindingHex: initialBlinding,
                certificateIdHex: certificateId,
                counter: 1
            )
        } catch OfflineBalanceProofError.bridgeUnavailable {
            throw XCTSkip("offlineBlindingFromSeed bridge unavailable on this platform")
        }

        let b2 = try OfflineBalanceProofBuilder.deriveResultingBlinding(
            initialBlindingHex: initialBlinding,
            certificateIdHex: certificateId,
            counter: 2
        )

        let b3 = try OfflineBalanceProofBuilder.deriveResultingBlinding(
            initialBlindingHex: initialBlinding,
            certificateIdHex: certificateId,
            counter: 3
        )

        XCTAssertNotEqual(b1, b2, "Different counters must produce different blindings")
        XCTAssertNotEqual(b2, b3, "Different counters must produce different blindings")
        XCTAssertNotEqual(b1, b3, "Different counters must produce different blindings")
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

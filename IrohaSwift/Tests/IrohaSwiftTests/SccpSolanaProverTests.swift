import Foundation
import XCTest
@testable import IrohaSwift

final class SccpSolanaProverTests: XCTestCase {
    func testBuildsSolanaSccpProofRequest() throws {
        let request = try buildSolanaSccpProofRequest(Self.sampleWitness())

        XCTAssertEqual(request.version, 1)
        XCTAssertEqual(request.backend, sccpSolanaRecursiveProofBackendV1)
        XCTAssertEqual(request.sourceDomain, sccpDomainSolana)
        XCTAssertEqual(request.targetDomain, sccpDomainSora)
        XCTAssertEqual(request.mainnetGenesisHash, sccpSolanaMainnetGenesisHash)
        XCTAssertEqual(request.publicInputs.messageId, "0x" + String(repeating: "dd", count: 32))
        XCTAssertTrue(request.witnessHash.hasPrefix("0x"))
        XCTAssertEqual(request.witnessHash.count, 66)
    }

    func testRequiresSourceEventDigest() {
        let witness = SolanaSccpWitnessInput(
            finalizedSlot: 321,
            blockhash: "9xQeWvG816bUx9EPfYdLSdJH7Gq2Xv3yQPG8mD3kAcL7",
            bankHash: String(repeating: "aa", count: 32),
            transactionStatusRoot: String(repeating: "bb", count: 32),
            messageProofHash: String(repeating: "cc", count: 32),
            transactionSignature: "5eykt4Signature111111111111111111111111111111",
            emitterProgramId: "Bridge111111111111111111111111111111111111",
            messageId: String(repeating: "dd", count: 32),
            payloadHash: String(repeating: "ee", count: 32),
            commitmentRoot: String(repeating: "12", count: 32),
            sourceEventDigest: ""
        )

        XCTAssertThrowsError(try normalizeSolanaSccpWitness(witness)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("sourceEventDigest"))
        }
    }

    func testBuildsSolanaMessageProofHashFromInclusionWitness() throws {
        let branch = [Data(repeating: 0x56, count: 32)]
        let hash = try solanaSccpMessageProofHash(
            sourceEventDigest: String(repeating: "34", count: 32),
            transactionStatusRoot: String(repeating: "bb", count: 32),
            inclusionBranch: branch
        )

        XCTAssertTrue(hash.hasPrefix("0x"))
        XCTAssertEqual(hash.count, 66)
        XCTAssertGreaterThan(
            try canonicalSolanaSccpMessageProofBytes(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionStatusRoot: String(repeating: "bb", count: 32),
                inclusionBranch: branch
            ).count,
            0
        )
        XCTAssertThrowsError(
            try solanaSccpMessageProofHash(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionStatusRoot: String(repeating: "bb", count: 32),
                inclusionBranch: [Data(repeating: 0xab, count: 31)]
            )
        )
    }

    func testProverRequiresLinkedProofEngine() async throws {
        let prover = SolanaSccpProver()

        do {
            _ = try await prover.prove(Self.sampleWitness())
            XCTFail("expected localProverUnavailable")
        } catch let error as SolanaSccpProverError {
            XCTAssertEqual(error, .localProverUnavailable)
        }
    }

    func testProverWrapsExternalProofBytes() async throws {
        let prover = SolanaSccpProver { request in
            XCTAssertEqual(request.backend, sccpSolanaRecursiveProofBackendV1)
            return Data([1, 2, 3, 4])
        }

        let result = try await prover.prove(Self.sampleWitness())

        XCTAssertEqual(result.proofBytes, Data([1, 2, 3, 4]))
        XCTAssertEqual(result.proofBase64, "AQIDBA==")
        XCTAssertTrue(result.envelopeHash.hasPrefix("0x"))
        XCTAssertEqual(result.envelopeHash.count, 66)
    }

    func testBuildsTonMessageBodyBoc() throws {
        let body = try buildTonSccpMessageBodyBoc(Self.sampleTonMessageBodyInput())

        XCTAssertEqual(Array(body.prefix(4)), [0xb5, 0xee, 0x9c, 0x72])
        XCTAssertGreaterThan(body.count, try canonicalTonSccpPublicInputsBytes(Self.sampleTonPublicInputs()).count)

        let submission = try buildTonSccpSubmission(Self.sampleTonMessageBodyInput())
        XCTAssertEqual(submission.envelopeEncoding, sccpTonMessageBodyBocV1)
        XCTAssertEqual(submission.messageBodyBoc, body)
        XCTAssertTrue(submission.messageBodyBocHex.hasPrefix("0xb5ee9c72"))
    }

    func testTonProverRequiresLinkedProofEngine() async throws {
        let prover = TonSccpProver()

        do {
            _ = try await prover.prove(Self.sampleTonProofRequestInput())
            XCTFail("expected localProverUnavailable")
        } catch let error as TonSccpProverError {
            XCTAssertEqual(error, .localProverUnavailable)
        }
    }

    func testTonProverWrapsExternalProofBytes() async throws {
        let prover = TonSccpProver { request in
            XCTAssertEqual(request.backend, sccpTonContractProofBackendV1)
            return Data([1, 2, 3, 4])
        }

        let result = try await prover.prove(Self.sampleTonProofRequestInput())

        XCTAssertEqual(result.proofBytes, Data([1, 2, 3, 4]))
        XCTAssertEqual(result.proofBase64, "AQIDBA==")
        XCTAssertTrue(result.requestHash.hasPrefix("0x"))
        XCTAssertEqual(result.requestHash.count, 66)
    }

    private static func sampleWitness() -> SolanaSccpWitnessInput {
        SolanaSccpWitnessInput(
            finalizedSlot: 321,
            blockhash: "9xQeWvG816bUx9EPfYdLSdJH7Gq2Xv3yQPG8mD3kAcL7",
            bankHash: String(repeating: "aa", count: 32),
            transactionStatusRoot: String(repeating: "bb", count: 32),
            messageProofHash: String(repeating: "cc", count: 32),
            transactionSignature: "5eykt4Signature111111111111111111111111111111",
            emitterProgramId: "Bridge111111111111111111111111111111111111",
            messageId: String(repeating: "dd", count: 32),
            payloadHash: String(repeating: "ee", count: 32),
            commitmentRoot: String(repeating: "12", count: 32),
            sourceEventDigest: String(repeating: "34", count: 32)
        )
    }

    private static func sampleTonMessageBodyInput() -> TonSccpMessageBodyInput {
        TonSccpMessageBodyInput(
            publicInputs: sampleTonPublicInputs(),
            proofBytes: Data([1, 2, 3, 4]),
            bundleBytes: Data([5, 6, 7]),
            statementHash: String(repeating: "bb", count: 32),
            destinationBindingHash: String(repeating: "56", count: 32),
            metadataBytes: Data([8, 9])
        )
    }

    private static func sampleTonProofRequestInput() -> TonSccpProofRequestInput {
        TonSccpProofRequestInput(
            publicInputs: sampleTonPublicInputs(),
            bundleBytes: Data([5, 6, 7])
        )
    }

    private static func sampleTonPublicInputs() -> TonSccpPublicInputsInput {
        TonSccpPublicInputsInput(
            messageId: String(repeating: "dd", count: 32),
            payloadHash: String(repeating: "ee", count: 32),
            commitmentRoot: String(repeating: "12", count: 32),
            finalityHeight: 19,
            finalityBlockHash: String(repeating: "aa", count: 32)
        )
    }
}

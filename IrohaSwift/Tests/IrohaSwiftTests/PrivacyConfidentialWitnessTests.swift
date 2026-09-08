import Foundation
import XCTest
@testable import IrohaSwift

final class PrivacyConfidentialWitnessTests: XCTestCase {
    func testTypedTransferWitnessEncodesWithoutGenericProofRequest() throws {
        let witness = try transferWitness()
        let archive = try RetiredPrivacyConfidentialWitnessCodecs.encodeTransferWitness(witness)
        XCTAssertGreaterThan(archive.count, NoritoHeader.encodedLength)
        XCTAssertEqual(
            Array(archive[6..<22]),
            Array(noritoSchemaHash(
                forTypeName: RetiredPrivacyConfidentialWitnessCodecs.privacyConfidentialWitnessV1WireName
            ))
        )
    }

    func testTypedUnshieldWitnessEncodes() throws {
        let witness = try unshieldWitness()
        let archive = try RetiredPrivacyConfidentialWitnessCodecs.encodeUnshieldWitness(witness)
        XCTAssertGreaterThan(archive.count, NoritoHeader.encodedLength)
    }

    func testTransferWitnessRejectsPublicAmountAndMissingOutput() throws {
        let input = try note()
        let publicAmount = try RetiredPrivacyConfidentialWitnessV1(
            networkId: TestNetworkIds.canonical,
            assetDefinitionId: "xor#taira",
            spendKey: bytes(1),
            treeCommitments: [bytes(2)],
            inputs: [input],
            transferOutputs: [],
            unshieldChange: [],
            publicAmount: "1",
            rootHint: bytes(3)
        )
        XCTAssertThrowsError(
            try RetiredPrivacyConfidentialWitnessCodecs.encodeTransferWitness(publicAmount)
        )
    }

    func testWitnessRejectsDuplicateLeafAndRho() throws {
        let input = try note()
        XCTAssertThrowsError(
            try RetiredPrivacyConfidentialWitnessV1(
                networkId: TestNetworkIds.canonical,
                assetDefinitionId: "xor#taira",
                spendKey: bytes(1),
                treeCommitments: [bytes(2)],
                inputs: [input, input],
                transferOutputs: [try output()],
                unshieldChange: [],
                publicAmount: "0",
                rootHint: bytes(3)
            )
        )
    }

    func testWitnessRejectsNonCanonicalNumbersAndWrongDigestLengths() {
        XCTAssertEqual(
            try RetiredPrivacyConfidentialWitnessCodecs.canonicalPublicAmount(
                "0",
                field: "publicAmount"
            ),
            "0"
        )
        for value in ["00", "01", "-1"] {
            XCTAssertThrowsError(
                try RetiredPrivacyConfidentialWitnessCodecs.canonicalPublicAmount(
                    value,
                    field: "publicAmount"
                )
            )
        }
        XCTAssertThrowsError(
            try RetiredPrivacyConfidentialNoteWitnessV1(
                amount: "01",
                rho: bytes(4),
                diversifier: bytes(5),
                leafIndex: 0
            )
        )
        XCTAssertThrowsError(
            try RetiredPrivacyConfidentialNoteWitnessV1(
                amount: "1",
                rho: Data(repeating: 4, count: 31),
                diversifier: bytes(5),
                leafIndex: 0
            )
        )
    }

    func testRetiredWitnessArchivesAreRejectedByCurrentTypedCarrier() throws {
        let transfer = try RetiredPrivacyConfidentialWitnessCodecs.encodeTransferWitness(transferWitness())
        let unshield = try RetiredPrivacyConfidentialWitnessCodecs.encodeUnshieldWitness(unshieldWitness())
        for archive in [transfer, unshield] {
            XCTAssertThrowsError(try PrivacyExact12FixtureCodecV1.decodeCanonicalArchive(archive))
        }
    }

    private func transferWitness() throws -> RetiredPrivacyConfidentialWitnessV1 {
        try RetiredPrivacyConfidentialWitnessV1(
            networkId: TestNetworkIds.canonical,
            assetDefinitionId: "xor#taira",
            spendKey: bytes(1),
            treeCommitments: [bytes(2)],
            inputs: [try note()],
            transferOutputs: [try output()],
            unshieldChange: [],
            publicAmount: "0",
            rootHint: bytes(3)
        )
    }

    private func unshieldWitness() throws -> RetiredPrivacyConfidentialWitnessV1 {
        try RetiredPrivacyConfidentialWitnessV1(
            networkId: TestNetworkIds.canonical,
            assetDefinitionId: "xor#taira",
            spendKey: bytes(1),
            treeCommitments: [bytes(2)],
            inputs: [try note()],
            transferOutputs: [],
            unshieldChange: [
                try RetiredPrivacyConfidentialUnshieldChangeWitnessV1(
                    amount: "2",
                    rho: bytes(7)
                ),
            ],
            publicAmount: "3",
            rootHint: bytes(3)
        )
    }

    private func note() throws -> RetiredPrivacyConfidentialNoteWitnessV1 {
        try RetiredPrivacyConfidentialNoteWitnessV1(
            amount: "5",
            rho: bytes(4),
            diversifier: bytes(5),
            leafIndex: 0
        )
    }

    private func output() throws -> RetiredPrivacyConfidentialTransferOutputWitnessV1 {
        try RetiredPrivacyConfidentialTransferOutputWitnessV1(
            amount: "5",
            rho: bytes(6),
            ownerTag: bytes(7)
        )
    }

    private func bytes(_ byte: UInt8) -> Data {
        Data(repeating: byte, count: 32)
    }
}

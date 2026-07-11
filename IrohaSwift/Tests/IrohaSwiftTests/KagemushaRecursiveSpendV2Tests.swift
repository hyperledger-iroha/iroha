import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveSpendV2Tests: XCTestCase {
    func testSplitIntentConservesFractionalAtomicValue() throws {
        let input = try note(seed: 0x10, amount: "625")
        let recipient = try note(seed: 0x20, amount: "210")
        let change = try note(seed: 0x30, amount: "415")
        let split = try KagemushaRecursiveSpendSplitIntentV2(
            chainID: input.chainID,
            assetDefinitionID: input.assetDefinitionID,
            inputNote: input,
            parentBranchPath: try branchPath(),
            assetScale: 2,
            transferAmount: KagemushaScaledAmount(atomicUnits: "210", scale: 2),
            recipientOutput: recipient,
            changeOutput: change,
            recipientRequestDigest: Data(repeating: 0x41, count: 32),
            parentLineageDigest: Data(repeating: 0x42, count: 32),
            operationID: Data(repeating: 0x43, count: 32)
        )
        XCTAssertEqual(split.transferAmount.displayDecimal, "2.1")
        XCTAssertEqual(split.changeOutput?.amount.atomicUnits, "415")
    }

    func testSplitIntentRejectsLossOverlapAndMissingChange() throws {
        let input = try note(seed: 0x10, amount: "625")
        let recipient = try note(seed: 0x20, amount: "210")
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendSplitIntentV2(
                chainID: input.chainID,
                assetDefinitionID: input.assetDefinitionID,
                inputNote: input,
                parentBranchPath: try branchPath(),
                assetScale: 2,
                transferAmount: KagemushaScaledAmount(atomicUnits: "210", scale: 2),
                recipientOutput: recipient,
                changeOutput: nil,
                recipientRequestDigest: Data(repeating: 0x41, count: 32),
                parentLineageDigest: Data(repeating: 0x42, count: 32),
                operationID: Data(repeating: 0x43, count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendV2Error, .invalidField("changeOutput"))
        }

        let wrongChange = try note(seed: 0x30, amount: "414")
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendSplitIntentV2(
                chainID: input.chainID,
                assetDefinitionID: input.assetDefinitionID,
                inputNote: input,
                parentBranchPath: try branchPath(),
                assetScale: 2,
                transferAmount: KagemushaScaledAmount(atomicUnits: "210", scale: 2),
                recipientOutput: recipient,
                changeOutput: wrongChange,
                recipientRequestDigest: Data(repeating: 0x41, count: 32),
                parentLineageDigest: Data(repeating: 0x42, count: 32),
                operationID: Data(repeating: 0x43, count: 32)
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendV2Error,
                .invalidField("changeOutput.amount")
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendSplitIntentV2(
                chainID: input.chainID,
                assetDefinitionID: input.assetDefinitionID,
                inputNote: input,
                parentBranchPath: try branchPath(),
                assetScale: 9,
                transferAmount: KagemushaScaledAmount(atomicUnits: "210", scale: 2),
                recipientOutput: recipient,
                changeOutput: try note(seed: 0x30, amount: "415"),
                recipientRequestDigest: Data(repeating: 0x41, count: 32),
                parentLineageDigest: Data(repeating: 0x42, count: 32),
                operationID: Data(repeating: 0x43, count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendV2Error, .invalidField("split.context"))
        }
    }

    func testProofSurfaceFailsClosed() {
        XCTAssertFalse(KagemushaRecursiveSpendV2.isProofBackendAvailable)
        XCTAssertEqual(KagemushaRecursiveSpendV2.requiredNativeBridgeAbiVersion, 17)
        XCTAssertEqual(
            NativeBridgeError.fromStatus(-314),
            .kagemushaRecursiveSpendV2Unavailable
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendV2.splitIntentWireName,
            "iroha_data_model::offline::model::KagemushaRecursiveSpendSplitIntentV2"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendV2.branchWireName,
            "iroha_data_model::offline::model::KagemushaRecursiveSpendBranchV2"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendV2.splitResultWireName,
            "iroha_data_model::offline::model::KagemushaRecursiveSpendSplitResultV2"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendV2.requiredProofSymbols,
            [
                "connect_norito_kagemusha_recursive_spend_init_v2",
                "connect_norito_kagemusha_recursive_spend_topup_v2",
                "connect_norito_kagemusha_recursive_spend_append_v2",
                "connect_norito_kagemusha_recursive_spend_redeem_change_v2",
                "connect_norito_kagemusha_recursive_spend_verify_v2",
                "connect_norito_kagemusha_recursive_spend_redeem_v2",
            ]
        )
        XCTAssertEqual(KagemushaRecursiveSpendV2.requiredProtocolSymbols.count, 15)
        XCTAssertEqual(
            KagemushaRecursiveSpendV2.requiredNativeSymbols,
            KagemushaRecursiveSpendV2.requiredProofSymbols
                + KagemushaRecursiveSpendV2.requiredProtocolSymbols
        )
        XCTAssertEqual(Set(KagemushaRecursiveSpendV2.requiredNativeSymbols).count, 21)
        XCTAssertThrowsError(try KagemushaRecursiveSpendV2.ensureProofBackendAvailable()) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendV2Error, .proofBackendUnavailable)
        }
        XCTAssertFalse(KagemushaRecursiveSpendLineageWitnessVerifier.isNativeAvailable)
        XCTAssertEqual(
            KagemushaRecursiveSpendLineageWitnessVerifier.requiredNativeSymbol,
            "connect_norito_kagemusha_recursive_spend_lineage_witness_verify"
        )
    }

    private func note(
        seed: UInt8,
        amount: String
    ) throws -> KagemushaSpendableNoteDescriptorV2 {
        var assetBytes = Data(repeating: 0, count: 16)
        assetBytes[6] = 0x40
        assetBytes[8] = 0x80
        let assetDefinitionID = try XCTUnwrap(AssetDefinitionAddress.encode(uuidBytes: assetBytes))
        return try KagemushaSpendableNoteDescriptorV2(
            chainID: "test-chain",
            assetDefinitionID: assetDefinitionID,
            noteCommitment: Data(repeating: seed, count: 32),
            spendNullifier: Data(repeating: seed &+ 1, count: 32),
            amount: KagemushaScaledAmount(atomicUnits: amount, scale: 2)
        )
    }

    private func branchPath() throws -> KagemushaRecursiveSpendBranchPathV2 {
        try .root(Data(repeating: 0x44, count: 32))
    }
}

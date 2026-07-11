import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveSpendV2Tests: XCTestCase {
    func testSplitIntentConservesFractionalAtomicValue() throws {
        let input = try note(seed: 0x10, amount: "625")
        let recipient = try note(seed: 0x20, amount: "210")
        let change = try note(seed: 0x30, amount: "415")
        let split = try KagemushaRecursiveSpendSplitIntentV2(
            chainID: testChainID,
            assetDefinitionID: try testAssetDefinitionID(),
            inputNote: input,
            parentBranchPath: try testParentBranchPath(),
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
                chainID: testChainID,
                assetDefinitionID: try testAssetDefinitionID(),
                inputNote: input,
                parentBranchPath: try testParentBranchPath(),
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
                chainID: testChainID,
                assetDefinitionID: try testAssetDefinitionID(),
                inputNote: input,
                parentBranchPath: try testParentBranchPath(),
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
                chainID: testChainID,
                assetDefinitionID: try testAssetDefinitionID(),
                inputNote: input,
                parentBranchPath: try testParentBranchPath(),
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
        XCTAssertEqual(KagemushaRecursiveSpendV2.requiredNativeSymbols.count, 21)
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
        try KagemushaSpendableNoteDescriptorV2(
            chainID: testChainID,
            assetDefinitionID: testAssetDefinitionID(),
            noteCommitment: Data(repeating: seed, count: 32),
            spendNullifier: Data(repeating: seed &+ 1, count: 32),
            amount: KagemushaScaledAmount(atomicUnits: amount, scale: 2)
        )
    }

    private var testChainID: String { "kagemusha-v2-test-chain" }

    private func testAssetDefinitionID() throws -> String {
        let uuidBytes = Data([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x46, 0x77,
            0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
        ])
        return try XCTUnwrap(AssetDefinitionAddress.encode(uuidBytes: uuidBytes))
    }

    private func testParentBranchPath() throws -> KagemushaRecursiveSpendBranchPathV2 {
        try .root(Data(repeating: 0x44, count: 32))
    }
}

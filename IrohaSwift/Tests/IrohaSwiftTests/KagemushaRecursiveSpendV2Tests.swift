import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveSpendV2Tests: XCTestCase {
    func testSplitIntentConservesFractionalAtomicValue() throws {
        let input = try note(seed: 0x10, amount: "625")
        let recipient = try note(seed: 0x20, amount: "210")
        let change = try note(seed: 0x30, amount: "415")
        let split = try KagemushaRecursiveSpendSplitIntentV2(
            inputNote: input,
            assetScale: 2,
            transferAmount: KagemushaScaledAmount(atomicUnits: "210", scale: 2),
            recipientOutput: recipient,
            changeOutput: change,
            recipientRequestDigest: Data(repeating: 0x41, count: 32),
            parentLineageDigest: Data(repeating: 0x42, count: 32),
            operationID: Data(repeating: 0x43, count: 32)
        )
        XCTAssertEqual(split.transferAmount.displayDecimal, "2.1")
        XCTAssertEqual(split.changeOutput?.amount, "415")
    }

    func testSplitIntentRejectsLossOverlapAndMissingChange() throws {
        let input = try note(seed: 0x10, amount: "625")
        let recipient = try note(seed: 0x20, amount: "210")
        XCTAssertThrowsError(
            try KagemushaRecursiveSpendSplitIntentV2(
                inputNote: input,
                assetScale: 2,
                transferAmount: KagemushaScaledAmount(atomicUnits: "210", scale: 2),
                recipientOutput: recipient,
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
                inputNote: input,
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
                inputNote: input,
                assetScale: 9,
                transferAmount: KagemushaScaledAmount(atomicUnits: "210", scale: 2),
                recipientOutput: recipient,
                changeOutput: try note(seed: 0x30, amount: "415"),
                recipientRequestDigest: Data(repeating: 0x41, count: 32),
                parentLineageDigest: Data(repeating: 0x42, count: 32),
                operationID: Data(repeating: 0x43, count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendV2Error, .invalidField("amount.scale"))
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
        XCTAssertEqual(KagemushaRecursiveSpendV2.requiredNativeSymbols.count, 5)
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
    ) throws -> KagemushaRecursiveSpendableNoteDescriptor {
        try KagemushaRecursiveSpendableNoteDescriptor(
            noteCommitment: Data(repeating: seed, count: 32),
            spendNullifier: Data(repeating: seed &+ 1, count: 32),
            amount: amount
        )
    }
}

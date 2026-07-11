import XCTest
@testable import IrohaSwift

final class ConfidentialUnshieldRedeemNativeTests: XCTestCase {
    func testRealNativeUnshieldProofBuildsRustDecodableRedeemAttachment() async throws {
        let environment = ProcessInfo.processInfo.environment
        let recordBytes: Data?
        if let recordPath = environment["IROHA_SWIFT_UNSHIELD_V3_RECORD_PATH"],
           !recordPath.isEmpty {
            recordBytes = try Data(contentsOf: URL(fileURLWithPath: recordPath))
        } else if let recordBase64 = environment["IROHA_SWIFT_UNSHIELD_V3_RECORD_NORITO_BASE64"],
                  !recordBase64.isEmpty,
                  let decoded = Data(base64Encoded: recordBase64),
                  decoded.base64EncodedString() == recordBase64 {
            recordBytes = decoded
        } else {
            recordBytes = nil
        }
        guard let recordBytes, !recordBytes.isEmpty else {
            throw XCTSkip("Canonical Rust unshield-v3 verifier record was not provided.")
        }
        guard PrivacyNativeBridge.isNativeAvailable else {
            XCTFail("The focused native lane must provide a loadable privacy bridge.")
            return
        }

        let chainId = "fc56984b-2be7-431d-840e-21514d1883f0"
        let assetDefinitionId = "xor#universal"
        let spendKey = Data(repeating: 0xA1, count: 32)
        let inputRho = Data(repeating: 0xA2, count: 32)
        let changeRho = Data(repeating: 0xA3, count: 32)
        let diversifier = try ConfidentialOwnerTag.deriveDiversifier(
            Data("unshield-v3-input".utf8)
        )
        let ownerTag = try ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(
            spendKey,
            diversifier: diversifier
        )
        let opening = try ConfidentialNoteOpening(
            rho: inputRho,
            spendKey: spendKey,
            ownerTag: ownerTag,
            asset: assetDefinitionId,
            chainId: chainId,
            amount: "9"
        )
        let inputCommitment = try ConfidentialNoteCommitment.deriveFromOpening(opening)
        let pathProvider = try LocalZkAssetMerklePathProvider(
            rootHistory: [],
            commitmentHistory: [inputCommitment]
        )
        let path = try await pathProvider.getMerklePathForCommitment(
            asset: assetDefinitionId,
            commitment: inputCommitment
        )
        let witness = try PrivacyConfidentialWitnessV1(
            chainId: chainId,
            assetDefinitionId: assetDefinitionId,
            spendKey: spendKey,
            treeCommitments: [inputCommitment],
            inputs: [
                try PrivacyConfidentialNoteWitnessV1(
                    amount: "9",
                    rho: inputRho,
                    diversifier: diversifier,
                    leafIndex: 0
                )
            ],
            transferOutputs: [],
            unshieldChange: [
                try PrivacyConfidentialUnshieldChangeWitnessV1(
                    amount: "4",
                    rho: changeRho
                )
            ],
            publicAmount: "5",
            rootHint: path.rootAtHeight
        )
        let proofRequest = try PrivacyConfidentialWitnessCodecs
            .buildConfidentialUnshieldProofRequestV1(witness: witness)
        let proofOutput = try PrivacyNativeBridge.buildConfidentialUnshieldProofV3(
            requestArchive: proofRequest
        )
        if let proofOutputPath = environment["IROHA_SWIFT_UNSHIELD_PROOF_OUTPUT"],
           !proofOutputPath.isEmpty {
            try proofOutput.write(to: URL(fileURLWithPath: proofOutputPath), options: .atomic)
        }
        let verifierRecord = try KagemushaRecursiveSpendVerifierRecordRef(
            verifierKeyId: "halo2/ipa:vk_unshield",
            recordBytes: recordBytes
        )
        let attachment = try KagemushaRecursiveSpendRequestCodecs.buildRedeemProofAttachment(
            unshieldProofOutputArchive: proofOutput,
            unshieldVerifierRecord: verifierRecord
        )
        let attachmentPayload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            attachment,
            schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName,
            field: "attachment"
        )
        XCTAssertFalse(attachmentPayload.isEmpty)

        if let outputPath = environment["IROHA_SWIFT_UNSHIELD_ATTACHMENT_OUT"],
           !outputPath.isEmpty {
            try attachment.write(to: URL(fileURLWithPath: outputPath), options: .atomic)
        }
    }
}

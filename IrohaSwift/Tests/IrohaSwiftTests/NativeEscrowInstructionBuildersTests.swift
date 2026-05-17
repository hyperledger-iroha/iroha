import XCTest
@testable import IrohaSwift

final class NativeEscrowInstructionBuildersTests: XCTestCase {
    private func object(from payload: NoritoJSON) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: payload.data, options: []) as? [String: Any])
    }

    func testOpenAssetEscrowPayloadShape() throws {
        let payload = try NativeEscrowInstructionBuilders.openAssetEscrow(
            escrowId: "escrow-hash",
            assetDefinition: "xor#wonderland",
            amount: "42.5",
            evidenceHashes: ["hash-a", "hash-b"]
        )
        let root = try object(from: payload)
        let inner = try XCTUnwrap(root["OpenAssetEscrow"] as? [String: Any])

        XCTAssertEqual(inner["escrow_id"] as? String, "escrow-hash")
        XCTAssertEqual(inner["asset_definition"] as? String, "xor#wonderland")
        XCTAssertEqual(inner["amount"] as? String, "42.5")
        XCTAssertEqual(inner["evidence_hashes"] as? [String], ["hash-a", "hash-b"])
    }

    func testLifecyclePayloadShapes() throws {
        let accept = try object(from: NativeEscrowInstructionBuilders.acceptAssetEscrow(escrowId: "escrow-hash"))
        let markPaid = try object(from: NativeEscrowInstructionBuilders.markEscrowPaymentSent(escrowId: "escrow-hash"))
        let release = try object(from: NativeEscrowInstructionBuilders.releaseAssetEscrow(escrowId: "escrow-hash"))
        let cancel = try object(from: NativeEscrowInstructionBuilders.cancelAssetEscrow(escrowId: "escrow-hash"))

        XCTAssertEqual((accept["AcceptAssetEscrow"] as? [String: Any])?["escrow_id"] as? String,
                       "escrow-hash")
        XCTAssertEqual((markPaid["MarkEscrowPaymentSent"] as? [String: Any])?["escrow_id"] as? String,
                       "escrow-hash")
        XCTAssertEqual((release["ReleaseAssetEscrow"] as? [String: Any])?["escrow_id"] as? String,
                       "escrow-hash")
        XCTAssertEqual((cancel["CancelAssetEscrow"] as? [String: Any])?["escrow_id"] as? String,
                       "escrow-hash")
    }

    func testDisputePayloadShapesAndPermissionConstant() throws {
        let dispute = try object(from: NativeEscrowInstructionBuilders.openEscrowDispute(
            escrowId: "escrow-hash",
            evidenceHashes: ["party-evidence"]
        ))
        let resolve = try object(from: NativeEscrowInstructionBuilders.resolveEscrowDispute(
            escrowId: "escrow-hash",
            buyerAmount: "30",
            sellerAmount: "12",
            evidenceHashes: ["judgement"]
        ))
        let disputeInner = try XCTUnwrap(dispute["OpenEscrowDispute"] as? [String: Any])
        let resolveInner = try XCTUnwrap(resolve["ResolveEscrowDispute"] as? [String: Any])

        XCTAssertEqual(disputeInner["evidence_hashes"] as? [String], ["party-evidence"])
        XCTAssertEqual(resolveInner["buyer_amount"] as? String, "30")
        XCTAssertEqual(resolveInner["seller_amount"] as? String, "12")
        XCTAssertEqual(resolveInner["evidence_hashes"] as? [String], ["judgement"])
        XCTAssertEqual(NativeEscrowPermissions.canResolveEscrowDispute, "CanResolveEscrowDispute")
    }

    func testAnonymousEscrowPayloadShapes() throws {
        let proof: [String: Any] = [
            "backend": "halo2/ipa",
            "proof": "proof-bytes",
            "vk_ref": ["backend": "halo2/ipa", "name": "vk_escrow"],
        ]
        let open = try object(from: NativeEscrowInstructionBuilders.openAnonymousAssetEscrow(
            escrowId: "anonymous-escrow",
            assetDefinition: "xor#wonderland",
            fundingNullifiers: ["n1", "n2"],
            escrowCommitment: "escrow-note",
            proof: proof,
            rootHint: "root",
            evidenceHashes: ["receipt"]
        ))
        let release = try object(from: NativeEscrowInstructionBuilders.releaseAnonymousAssetEscrow(
            escrowId: "anonymous-escrow",
            escrowNullifiers: ["escrow-nullifier"],
            buyerOutputCommitments: ["buyer-note"],
            proof: proof
        ))
        let resolve = try object(from: NativeEscrowInstructionBuilders.resolveAnonymousEscrowDispute(
            escrowId: "anonymous-escrow",
            escrowNullifiers: ["escrow-nullifier"],
            buyerOutputCommitments: ["buyer-note"],
            sellerOutputCommitments: ["seller-note"],
            proof: proof,
            evidenceHashes: ["judgement"]
        ))

        let openInner = try XCTUnwrap(open["OpenAnonymousAssetEscrow"] as? [String: Any])
        let releaseInner = try XCTUnwrap(release["ReleaseAnonymousAssetEscrow"] as? [String: Any])
        let resolveInner = try XCTUnwrap(resolve["ResolveAnonymousEscrowDispute"] as? [String: Any])

        XCTAssertEqual(openInner["funding_nullifiers"] as? [String], ["n1", "n2"])
        XCTAssertEqual(openInner["escrow_commitment"] as? String, "escrow-note")
        XCTAssertEqual(openInner["root_hint"] as? String, "root")
        XCTAssertEqual(openInner["evidence_hashes"] as? [String], ["receipt"])
        XCTAssertEqual((openInner["proof"] as? [String: Any])?["backend"] as? String, "halo2/ipa")
        XCTAssertEqual(releaseInner["buyer_output_commitments"] as? [String], ["buyer-note"])
        XCTAssertEqual(resolveInner["seller_output_commitments"] as? [String], ["seller-note"])
        XCTAssertEqual(resolveInner["evidence_hashes"] as? [String], ["judgement"])
    }

    func testIrohaSDKConvenienceHelpersProduceSamePayloads() throws {
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        let direct = try NativeEscrowInstructionBuilders.resolveEscrowDispute(
            escrowId: "escrow-hash",
            buyerAmount: "1",
            sellerAmount: "2"
        )
        let viaSDK = try sdk.buildResolveEscrowDispute(
            escrowId: "escrow-hash",
            buyerAmount: "1",
            sellerAmount: "2"
        )

        XCTAssertEqual(viaSDK.data, direct.data)
    }

    func testRejectsBlankValues() {
        XCTAssertThrowsError(try NativeEscrowInstructionBuilders.acceptAssetEscrow(escrowId: " ")) { error in
            XCTAssertEqual(error as? NativeEscrowInstructionBuilderError,
                           .invalidValue(field: "escrowId"))
        }
        XCTAssertThrowsError(try NativeEscrowInstructionBuilders.openEscrowDispute(
            escrowId: "escrow-hash",
            evidenceHashes: ["ok", " "]
        )) { error in
            XCTAssertEqual(error as? NativeEscrowInstructionBuilderError,
                           .invalidEvidenceHash(index: 1))
        }
    }

    func testAnonymousEscrowRejectsLegacyInlineVerifyingKeyField() {
        for field in ["vk_inline", "vkInline", "verifyingKeyInline", "verifying_key_inline"] {
            let proof: [String: Any] = [
                "backend": "halo2/ipa",
                "proof": "proof-bytes",
                "vk_ref": ["backend": "halo2/ipa", "name": "vk_escrow"],
                field: ["backend": "halo2/ipa", "bytes_b64": "AQID"],
            ]
            XCTAssertThrowsError(try NativeEscrowInstructionBuilders.openAnonymousAssetEscrow(
                escrowId: "anonymous-escrow",
                assetDefinition: "xor#wonderland",
                fundingNullifiers: ["n1"],
                escrowCommitment: "escrow-note",
                proof: proof
            )) { error in
                XCTAssertEqual(error as? NativeEscrowInstructionBuilderError,
                               .invalidValue(field: "proof.\(field)"))
            }
        }
    }

    func testAnonymousEscrowRejectsMissingVerifyingKeyReference() {
        let proof: [String: Any] = [
            "backend": "halo2/ipa",
            "proof": "proof-bytes",
            "vk_commitment": Array(repeating: 0, count: 32),
        ]
        XCTAssertThrowsError(try NativeEscrowInstructionBuilders.openAnonymousAssetEscrow(
            escrowId: "anonymous-escrow",
            assetDefinition: "xor#wonderland",
            fundingNullifiers: ["n1"],
            escrowCommitment: "escrow-note",
            proof: proof
        )) { error in
            XCTAssertEqual(error as? NativeEscrowInstructionBuilderError,
                           .invalidValue(field: "proof.vk_ref"))
        }
    }

    func testAnonymousEscrowRejectsNonObjectVerifyingKeyReference() {
        let proof: [String: Any] = [
            "backend": "halo2/ipa",
            "proof": "proof-bytes",
            "vk_ref": "halo2/ipa:vk_escrow",
        ]
        XCTAssertThrowsError(try NativeEscrowInstructionBuilders.openAnonymousAssetEscrow(
            escrowId: "anonymous-escrow",
            assetDefinition: "xor#wonderland",
            fundingNullifiers: ["n1"],
            escrowCommitment: "escrow-note",
            proof: proof
        )) { error in
            XCTAssertEqual(error as? NativeEscrowInstructionBuilderError,
                           .invalidValue(field: "proof.vk_ref"))
        }
    }

    func testAnonymousEscrowRejectsIncompleteVerifyingKeyReference() {
        for (vkRef, field) in [
            (["name": "vk_escrow"], "proof.vk_ref.backend"),
            (["backend": "halo2/ipa"], "proof.vk_ref.name"),
            (["backend": "   ", "name": "vk_escrow"], "proof.vk_ref.backend"),
            (["backend": "halo2/ipa", "name": "   "], "proof.vk_ref.name"),
        ] {
            let proof: [String: Any] = [
                "backend": "halo2/ipa",
                "proof": "proof-bytes",
                "vk_ref": vkRef,
            ]
            XCTAssertThrowsError(try NativeEscrowInstructionBuilders.openAnonymousAssetEscrow(
                escrowId: "anonymous-escrow",
                assetDefinition: "xor#wonderland",
                fundingNullifiers: ["n1"],
                escrowCommitment: "escrow-note",
                proof: proof
            )) { error in
                XCTAssertEqual(error as? NativeEscrowInstructionBuilderError,
                               .invalidValue(field: field))
            }
        }
    }

    func testAnonymousEscrowRejectsVerifyingKeyBackendMismatch() {
        let proof: [String: Any] = [
            "backend": "halo2/ipa",
            "proof": "proof-bytes",
            "vk_ref": ["backend": "stark/fri-v1", "name": "vk_escrow"],
        ]
        XCTAssertThrowsError(try NativeEscrowInstructionBuilders.openAnonymousAssetEscrow(
            escrowId: "anonymous-escrow",
            assetDefinition: "xor#wonderland",
            fundingNullifiers: ["n1"],
            escrowCommitment: "escrow-note",
            proof: proof
        )) { error in
            XCTAssertEqual(error as? NativeEscrowInstructionBuilderError,
                           .invalidValue(field: "proof.vk_ref.backend"))
        }
    }

    func testAnonymousEscrowRejectsVerifyingKeyReferenceShadowField() {
        let proof: [String: Any] = [
            "backend": "halo2/ipa",
            "proof": "proof-bytes",
            "vk_ref": ["backend": "halo2/ipa", "name": "vk_escrow"],
            "vk_reference": ["backend": "halo2/ipa", "name": "shadow"],
        ]
        XCTAssertThrowsError(try NativeEscrowInstructionBuilders.openAnonymousAssetEscrow(
            escrowId: "anonymous-escrow",
            assetDefinition: "xor#wonderland",
            fundingNullifiers: ["n1"],
            escrowCommitment: "escrow-note",
            proof: proof
        )) { error in
            XCTAssertEqual(error as? NativeEscrowInstructionBuilderError,
                           .invalidValue(field: "proof.vk_reference"))
        }
    }

    func testAnonymousEscrowRejectsNestedVerifyingKeyReferenceShadowField() {
        let proof: [String: Any] = [
            "backend": "halo2/ipa",
            "proof": "proof-bytes",
            "vk_ref": [
                "backend": "halo2/ipa",
                "name": "vk_escrow",
                "vk_reference": "shadow",
            ],
        ]
        XCTAssertThrowsError(try NativeEscrowInstructionBuilders.openAnonymousAssetEscrow(
            escrowId: "anonymous-escrow",
            assetDefinition: "xor#wonderland",
            fundingNullifiers: ["n1"],
            escrowCommitment: "escrow-note",
            proof: proof
        )) { error in
            XCTAssertEqual(error as? NativeEscrowInstructionBuilderError,
                           .invalidValue(field: "proof.vk_ref.vk_reference"))
        }
    }
}

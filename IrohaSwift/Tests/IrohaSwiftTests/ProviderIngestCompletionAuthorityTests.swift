import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

/// Structural SDK authority checks; canonical cross-language proof bytes remain Rust-owned.
final class ProviderIngestCompletionAuthorityTests: XCTestCase {
    private func accounts() throws -> (String, String) {
        let ownerKey = try XCTUnwrap(Data(hexString:
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"))
        let signerKey = try XCTUnwrap(Data(hexString:
            "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29"))
        return (
            try AccountAddress.fromAccount(publicKey: ownerKey).toI105(networkPrefix: 753),
            try AccountAddress.fromAccount(publicKey: signerKey).toI105(networkPrefix: 753)
        )
    }

    func testMusubiAuthorityRequiresSignerAndBindingUsesThatSigner() throws {
        let (owner, signer) = try accounts()
        XCTAssertNotEqual(owner, signer)
        let policy = try MusubiProviderIngestCompletionSignerPolicyV1(
            policyID: [UInt8](repeating: 1, count: 32), revision: 1,
            predecessorDigest: nil, policyDigest: [UInt8](repeating: 2, count: 32)
        )
        let authority = try MusubiProviderIngestCompletionAuthorityV1(
            providerOwner: owner, completionSigner: signer, signerPolicy: policy
        )
        let bytes = try JSONEncoder().encode(authority)
        XCTAssertEqual(try JSONDecoder().decode(MusubiProviderIngestCompletionAuthorityV1.self, from: bytes), authority)
        let digest = try MusubiDigest32V1(bytes: [UInt8](repeating: 3, count: 32))
        func binding(_ completedBy: String) throws -> MusubiProviderBundleVerificationBindingV1 {
            try MusubiProviderBundleVerificationBindingV1(
                networkId: NetworkId(bytes: Data(repeating: 7, count: 32)),
                providerID: digest, completedBy: completedBy, completionAuthority: authority,
                replicationOrder: digest, assignmentRevision: 1, completionEpoch: 1,
                finalizedAnchor: MusubiProviderIngestFinalizedAnchorV1(height: 1, blockHash: [UInt8](repeating: 5, count: 32)),
                archiveID: digest, bundleDigest: digest, descriptorDigest: digest,
                semanticReleaseManifestDigest: digest, verificationLockDigest: digest, sourceTreeDigest: digest
            )
        }
        XCTAssertEqual(try binding(signer).completedBy, signer)
        XCTAssertEqual(try binding(signer).completionAuthority.providerOwner, owner)
        XCTAssertThrowsError(try binding(owner))
        // The builder checks controller membership, not the attestation signature. These are
        // real signatures over a test challenge; native fixture tests retain protocol parity.
        let signerKey = try Curve25519.Signing.PrivateKey(rawRepresentation: Data(repeating: 0, count: 32))
        let ownerSeed = try XCTUnwrap(Data(hexString:
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"))
        let ownerKey = try Curve25519.Signing.PrivateKey(rawRepresentation: ownerSeed)
        XCTAssertEqual(try AccountAddress.fromAccount(publicKey: signerKey.publicKey.rawRepresentation)
            .toI105(networkPrefix: 753), signer)
        XCTAssertEqual(try AccountAddress.fromAccount(publicKey: ownerKey.publicKey.rawRepresentation)
            .toI105(networkPrefix: 753), owner)
        let challenge = Data("provider completion controller selection".utf8)
        func approval(_ key: Curve25519.Signing.PrivateKey) throws -> MusubiProviderBundleVerificationApprovalV1 {
            let signature = try key.signature(for: challenge)
            XCTAssertTrue(key.publicKey.isValidSignature(signature, for: challenge))
            return try MusubiProviderBundleVerificationApprovalV1(
                publicKey: CanonicalNorito.publicKeyMultihash(
                    algorithm: .ed25519, payload: key.publicKey.rawRepresentation),
                signature: signature.hexEncodedString().uppercased())
        }
        let payload = try MusubiProviderBundleVerificationPayloadV1(binding: binding(signer))
        let selectedAttestation = try MusubiProviderBundleVerificationAttestationV1(
            payload: payload, approvals: [approval(signerKey)])
        XCTAssertNoThrow(try RegisterMusubiProviderBundleAttestationV1(
            attestation: selectedAttestation, expectedLocationRevision: 1))
        let ownerAttestation = try MusubiProviderBundleVerificationAttestationV1(
            payload: payload, approvals: [approval(ownerKey)])
        XCTAssertThrowsError(try RegisterMusubiProviderBundleAttestationV1(
            attestation: ownerAttestation, expectedLocationRevision: 1))
        let replacements: [Any?] = [nil, NSNull(), "invalid"]
        for replacement in replacements {
            var object = try XCTUnwrap(JSONSerialization.jsonObject(with: bytes) as? [String: Any])
            object["completion_signer"] = replacement
            XCTAssertThrowsError(try JSONDecoder().decode(
                MusubiProviderIngestCompletionAuthorityV1.self,
                from: JSONSerialization.data(withJSONObject: object)
            ))
        }
        var extra = try XCTUnwrap(JSONSerialization.jsonObject(with: bytes) as? [String: Any])
        extra["extra"] = 1
        XCTAssertThrowsError(try JSONDecoder().decode(
            MusubiProviderIngestCompletionAuthorityV1.self,
            from: JSONSerialization.data(withJSONObject: extra)
        ))
    }

    func testReplicationArgumentsRequireTheWholeDistinctAuthority() throws {
        let (owner, signer) = try accounts()
        let authority = try SorafsProviderIngestCompletionAuthorityV1(
            providerOwner: owner, completionSigner: signer,
            signerPolicy: SorafsProviderIngestCompletionSignerPolicyV1(
                policyId: String(repeating: "11", count: 32), revision: 1,
                predecessorDigest: nil, policyDigest: String(repeating: "22", count: 32)
            )
        )
        let instruction = try SorafsReplicationInstructionBuilders.completeReplicationOrder(
            orderId: String(repeating: "33", count: 32), providerId: String(repeating: "44", count: 32),
            completionEpoch: 2, expectedAuthority: authority, expectedAssignmentRevision: 1,
            finalizedAnchor: SorafsProviderIngestFinalizedAnchorV1(height: 3, blockHash: String(repeating: "55", count: 32))
        )
        _ = try SorafsReplicationInstructionBuilders.decode(instruction)
        let root = try XCTUnwrap(JSONSerialization.jsonObject(with: instruction.data) as? [String: Any])
        let body = try XCTUnwrap(root["CompleteReplicationOrder"] as? [String: Any])
        let original = try XCTUnwrap(body["expected_authority"] as? [String: Any])
        XCTAssertEqual(original["provider_owner"] as? String, owner)
        XCTAssertEqual(original["completion_signer"] as? String, signer)
        let replacements: [Any?] = [nil, NSNull(), "invalid"]
        for replacement in replacements {
            var changed = original
            changed["completion_signer"] = replacement
            var changedBody = body
            changedBody["expected_authority"] = changed
            XCTAssertThrowsError(try SorafsReplicationInstructionBuilders.decode(
                NoritoJSON.fromJSONObject(["CompleteReplicationOrder": changedBody])
            ))
        }
        var changed = original
        changed["extra"] = 1
        var changedBody = body
        changedBody["expected_authority"] = changed
        XCTAssertThrowsError(try SorafsReplicationInstructionBuilders.decode(
            NoritoJSON.fromJSONObject(["CompleteReplicationOrder": changedBody])
        ))
    }
}

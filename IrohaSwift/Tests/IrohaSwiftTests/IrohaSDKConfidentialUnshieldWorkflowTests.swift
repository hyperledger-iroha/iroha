import Foundation
import XCTest
@testable import IrohaSwift

final class IrohaSDKConfidentialUnshieldWorkflowTests: XCTestCase {
    @available(iOS 15.0, macOS 12.0, *)
    func testOrchestratorFetchesExactRecordBeforeBuildingProofAndAttachment() async throws {
        let verifierKeyId = try ToriiVerifyingKeyId(
            backend: "halo2/ipa",
            name: "unshield-v3"
        )
        let recordNorito = Self.recordNorito()
        let detail = try Self.verifyingKeyDetail(
            id: verifierKeyId,
            recordNorito: recordNorito
        )
        let witness = try Self.unshieldWitness()
        let expectedProofRequest = try PrivacyConfidentialWitnessCodecs
            .buildConfidentialUnshieldProofRequestV1(witness: witness)
        let proofOutput = Data([0x91, 0x92, 0x93])
        let attachment = Data([0xa1, 0xa2])
        var calls: [String] = []

        let result = try await IrohaSDK
            .orchestrateKagemushaConfidentialUnshieldRedeemProofAttachment(
                witness: witness,
                verifierKeyId: verifierKeyId,
                blockHeight: 42,
                fetchVerifierKey: { requestedId in
                    calls.append("fetch")
                    XCTAssertEqual(requestedId, verifierKeyId)
                    return detail
                },
                buildProof: { request in
                    calls.append("build")
                    XCTAssertEqual(request, expectedProofRequest)
                    return proofOutput
                },
                buildAttachment: { output, record, blockHeight in
                    calls.append("attach")
                    XCTAssertEqual(output, proofOutput)
                    XCTAssertEqual(record.verifierKeyId, "halo2/ipa:unshield-v3")
                    XCTAssertEqual(record.recordBytes, recordNorito)
                    XCTAssertEqual(blockHeight, 42)
                    return attachment
                }
            )

        XCTAssertEqual(result, attachment)
        XCTAssertEqual(calls, ["fetch", "build", "attach"])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testOrchestratorRejectsCrossWiredToriiDetailBeforeProofConstruction() async throws {
        let requestedId = try ToriiVerifyingKeyId(
            backend: "halo2/ipa",
            name: "unshield-v3"
        )
        let returnedId = try ToriiVerifyingKeyId(
            backend: "halo2/ipa",
            name: "different-vk"
        )
        let detail = try Self.verifyingKeyDetail(
            id: returnedId,
            recordNorito: Self.recordNorito()
        )
        var buildWasCalled = false

        do {
            _ = try await IrohaSDK
                .orchestrateKagemushaConfidentialUnshieldRedeemProofAttachment(
                    witness: Self.unshieldWitness(),
                    verifierKeyId: requestedId,
                    blockHeight: nil,
                    fetchVerifierKey: { _ in detail },
                    buildProof: { _ in
                        buildWasCalled = true
                        return Data([0x01])
                    },
                    buildAttachment: { _, _, _ in
                        XCTFail("attachment builder must not run for a cross-wired Torii detail")
                        return Data()
                    }
                )
            XCTFail("cross-wired verifier detail must be rejected")
        } catch let error as ToriiClientError {
            guard case let .invalidPayload(message) = error else {
                return XCTFail("unexpected Torii error: \(error)")
            }
            XCTAssertEqual(
                message,
                "verifying-key detail identifier does not match the requested verifier key"
            )
        }

        XCTAssertFalse(buildWasCalled)
    }

    private static func recordNorito() -> Data {
        noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            payload: Data([0x01]),
            flags: NoritoHeader.compactLen
        )
    }

    private static func verifyingKeyDetail(
        id: ToriiVerifyingKeyId,
        recordNorito: Data
    ) throws -> ToriiVerifyingKeyDetail {
        let payload = """
        {
          "id": { "backend": "\(id.backend)", "name": "\(id.name)" },
          "record_norito_base64": "\(recordNorito.base64EncodedString())",
          "record": {
            "version": 3,
            "circuit_id": "halo2/pasta/ipa/anon-unshield-2in-1change-merkle16-poseidon-diversified",
            "owner_manifest_id": "confidential-v3",
            "namespace": "offline_kagemusha",
            "backend": "halo2/ipa",
            "curve": "pallas",
            "public_inputs_schema_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "commitment": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "vk_len": 96,
            "max_proof_bytes": 196608,
            "status": "Active"
          }
        }
        """.data(using: .utf8)!
        return try JSONDecoder().decode(ToriiVerifyingKeyDetail.self, from: payload)
    }

    private static func unshieldWitness() throws -> PrivacyConfidentialWitnessV1 {
        let input = try PrivacyConfidentialNoteWitnessV1(
            amount: "10",
            rho: Data(repeating: 0x22, count: 32),
            diversifier: Data(repeating: 0x33, count: 32),
            leafIndex: 0
        )
        let change = try PrivacyConfidentialUnshieldChangeWitnessV1(
            amount: "5",
            rho: Data(repeating: 0x44, count: 32)
        )
        return try PrivacyConfidentialWitnessV1(
            chainId: "chain",
            assetDefinitionId: "asset",
            spendKey: Data(repeating: 0x11, count: 32),
            treeCommitments: [Data(repeating: 0x55, count: 32)],
            inputs: [input],
            transferOutputs: [],
            unshieldChange: [change],
            publicAmount: "5",
            rootHint: Data(repeating: 0x66, count: 32)
        )
    }
}

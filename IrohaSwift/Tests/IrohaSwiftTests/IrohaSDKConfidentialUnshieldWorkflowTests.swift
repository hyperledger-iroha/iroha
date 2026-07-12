import CryptoKit
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
        let recordNorito = try Self.recordNorito()
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
            recordNorito: try Self.recordNorito()
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

    private static func recordNorito() throws -> Data {
        let backend = KagemushaRecursiveSpendProver.recursiveAggregationProofBackend
        let verifierKey = Data(repeating: 0x77, count: 96)
        let schemaHash = IrohaHash.hash(
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()
        )
        let commitment = verifierKeyCommitment(backend: backend, bytes: verifierKey)

        var keyWriter = OfflineCompactNoritoWriter()
        keyWriter.writeField(OfflineCompactNorito.encodeString(backend))
        keyWriter.writeField(byteVec(verifierKey))

        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeUInt32(3))
        writer.writeField(OfflineCompactNorito.encodeString(
            KagemushaRecursiveSpendRequestCodecs.confidentialUnshieldV3CircuitId
        ))
        writer.writeField(option(OfflineCompactNorito.encodeString("confidential-v3")))
        writer.writeField(OfflineCompactNorito.encodeString("offline_kagemusha"))
        writer.writeField(OfflineCompactNorito.encodeUInt32(
            VerifyingKeyBackendTag.halo2IpaPasta.rawValue
        ))
        writer.writeField(OfflineCompactNorito.encodeString("pallas"))
        writer.writeField(schemaHash)
        writer.writeField(commitment)
        writer.writeField(OfflineCompactNorito.encodeUInt32(UInt32(verifierKey.count)))
        writer.writeField(OfflineCompactNorito.encodeUInt32(196_608))
        writer.writeField(option(nil))
        writer.writeField(option(nil))
        writer.writeField(option(nil))
        writer.writeField(option(nil))
        writer.writeField(option(nil))
        writer.writeField(option(keyWriter.data))
        writer.writeField(OfflineCompactNorito.encodeUInt32(1))
        return noritoEncode(
            typeName: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            payload: writer.data,
            flags: NoritoHeader.compactLen
        )
    }

    private static func verifyingKeyDetail(
        id: ToriiVerifyingKeyId,
        recordNorito: Data
    ) throws -> ToriiVerifyingKeyDetail {
        let backend = KagemushaRecursiveSpendProver.recursiveAggregationProofBackend
        let verifierKey = Data(repeating: 0x77, count: 96)
        let schemaHash = IrohaHash.hash(
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()
        )
        let commitment = verifierKeyCommitment(backend: backend, bytes: verifierKey)
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
            "public_inputs_schema_hash": "\(schemaHash.hexLowercased())",
            "commitment": "\(commitment.hexLowercased())",
            "vk_len": 96,
            "max_proof_bytes": 196608,
            "status": "Active",
            "key": {
              "backend": "halo2/ipa",
              "bytes_b64": "\(verifierKey.base64EncodedString())"
            }
          }
        }
        """.data(using: .utf8)!
        return try JSONDecoder().decode(ToriiVerifyingKeyDetail.self, from: payload)
    }

    private static func verifierKeyCommitment(backend: String, bytes: Data) -> Data {
        var preimage = Data("iroha:zk:v1:vk".utf8)
        appendUInt64BE(UInt64(backend.utf8.count), to: &preimage)
        preimage.append(Data(backend.utf8))
        appendUInt64BE(UInt64(bytes.count), to: &preimage)
        preimage.append(bytes)
        return Data(SHA256.hash(data: preimage))
    }

    private static func byteVec(_ bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    private static func option(_ payload: Data?) -> Data {
        var writer = OfflineCompactNoritoWriter()
        guard let payload else {
            writer.writeUInt8(0)
            return writer.data
        }
        writer.writeUInt8(1)
        writer.writeField(payload)
        return writer.data
    }

    private static func appendUInt64BE(_ value: UInt64, to data: inout Data) {
        for shift in stride(from: 56, through: 0, by: -8) {
            data.append(UInt8((value >> UInt64(shift)) & 0xff))
        }
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

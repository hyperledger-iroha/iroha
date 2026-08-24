import Foundation
import XCTest
@testable import IrohaSwift

func tcMakeSampleManifestRaw(storageTicket: String = String(repeating: "aa", count: 32)) -> [String: ToriiJSONValue] {
    let manifestBytes = Data("sample-manifest".utf8).base64EncodedString()
    return [
        "storage_ticket": .string(storageTicket),
        "client_blob_id": .string(String(repeating: "bb", count: 32)),
        "blob_hash": .string(String(repeating: "cc", count: 32)),
        "manifest_hash": .string(String(repeating: "ff", count: 32)),
        "chunk_root": .string(String(repeating: "dd", count: 32)),
        "lane_id": .number(1),
        "epoch": .number(2),
        "manifest_len": .number(16),
        "manifest_norito": .string(manifestBytes),
        "manifest": .object([
            "chunking": .object([
                "namespace": .string("sorafs"),
                "name": .string("sf1"),
                "semver": .string("1.0.0")
            ])
        ]),
        "chunk_plan": .object([
            "schema": .string("sorafs.chunk_fetch_plan.v1"),
            "payload_digest_blake3_hex": .string(String(repeating: "cc", count: 32)),
            "chunk_fetch_specs": .array([
                .object([
                    "chunk_index": .number(0),
                    "offset": .number(0),
                    "length": .number(4),
                    "digest_blake3": .string(String(repeating: "ee", count: 32))
                ])
            ])
        ])
    ]
}

func tcMakeSampleManifestBundle(storageTicket: String = String(repeating: "aa", count: 32)) throws -> ToriiDaManifestBundle {
    try ToriiDaManifestBundle(raw: tcMakeSampleManifestRaw(storageTicket: storageTicket))
}

func tcMakeGatewayFetchResult() -> SorafsGatewayFetchResult {
    let report = SorafsGatewayFetchReport(
        chunkCount: 1,
        providerReports: [],
        chunkReceipts: [],
        scoreboard: nil
    )
    return SorafsGatewayFetchResult(
        payload: Data([0x01, 0x02]),
        report: report,
        reportJSON: #"{"chunk_count":1}"#
    )
}

@available(iOS 15.0, macOS 12.0, *)
enum TcHelperError: Error {
    case invalidHashEncoding
    case invalidPayloadEncoding
}

func tcMakePipelineEnvelope(hashHex: String, marker: UInt8) throws -> SignedTransactionEnvelope {
    guard let hashData = Data(hexString: hashHex) else {
        throw TcHelperError.invalidHashEncoding
    }
    let payload = Data([marker, marker ^ 0xFF, 0xA5])
    return SignedTransactionEnvelope(norito: payload,
                                     signedTransaction: payload,
                                     payload: nil,
                                     transactionHash: hashData)
}

func tcLoadDaProofFixture() throws -> (manifest: Data, payload: Data, blobHashHex: String) {
    let fixtureRoot = tcRepositoryRootURL()
        .appendingPathComponent("fixtures/da/reconstruct/rs_parity_v1", isDirectory: true)
    let manifestHexURL = fixtureRoot.appendingPathComponent("manifest.norito.hex")
    let manifestJSONURL = fixtureRoot.appendingPathComponent("manifest.json")
    let payloadURL = fixtureRoot.appendingPathComponent("payload.bin")

    let manifestHex = try String(contentsOf: manifestHexURL, encoding: .utf8)
        .trimmingCharacters(in: .whitespacesAndNewlines)
    let manifestData = try XCTUnwrap(
        Data(hexString: manifestHex),
        "failed to decode DA manifest fixture"
    )
    let payloadData = try Data(contentsOf: payloadURL)
    let manifestJSONData = try Data(contentsOf: manifestJSONURL)
    let manifestObject = try XCTUnwrap(
        try JSONSerialization.jsonObject(with: manifestJSONData) as? [String: Any],
        "DA manifest fixture must be a JSON object"
    )
    let blobArray = try XCTUnwrap(
        manifestObject["blob_hash"] as? [[NSNumber]],
        "blob_hash fixture missing"
    )
    let blobBytes = try XCTUnwrap(blobArray.first, "blob_hash fixture is empty")
    let blobHex = blobBytes.reduce(into: "") { partialResult, value in
        partialResult.append(String(format: "%02x", value.uint8Value))
    }
    return (manifestData, payloadData, blobHex)
}

private func tcRepositoryRootURL() -> URL {
    URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // ToriiClientDaTestSupport.swift
        .deletingLastPathComponent() // IrohaSwiftTests
        .deletingLastPathComponent() // Tests
        .deletingLastPathComponent() // IrohaSwift
}

func tcMakeStubProofSummary() -> ToriiDaProofSummary {
    let proof = ToriiDaProofRecord(
        origin: "explicit",
        leafIndex: 0,
        chunkIndex: 0,
        segmentIndex: 0,
        leafOffset: 0,
        leafLength: 32,
        segmentOffset: 0,
        segmentLength: 32,
        chunkOffset: 0,
        chunkLength: 32,
        payloadLength: 32,
        chunkDigestHex: "aa",
        chunkRootHex: "bb",
        segmentDigestHex: "cc",
        leafDigestHex: "dd",
        leafBytes: Data(),
        segmentLeavesHex: [],
        chunkSegmentsHex: [],
        chunkCount: 1,
        chunkMerklePathHex: [],
        verified: true
    )
    return ToriiDaProofSummary(
        blobHashHex: "aa",
        chunkRootHex: "bb",
        porRootHex: "cc",
        leafCount: 1,
        segmentCount: 1,
        chunkCount: 1,
        sampleCount: 0,
        sampleSeed: 0,
        proofCount: 1,
        proofs: [proof]
    )
}

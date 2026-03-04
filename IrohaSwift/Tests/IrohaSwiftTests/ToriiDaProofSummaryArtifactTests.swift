import Foundation
@testable import IrohaSwift
import XCTest

final class ToriiDaProofSummaryArtifactTests: XCTestCase {
    func testArtifactEncodingMatchesCliSchema() throws {
        let summary = makeStubProofSummary()
        let artifact = ToriiDaProofSummaryArtifact(
            summary: summary,
            manifestPath: "/tmp/manifest.json",
            payloadPath: "/tmp/payload.bin"
        )
        let data = try artifact.encoded(prettyPrinted: false, sortedKeys: true)
        let json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: data, options: []) as? [String: Any]
        )

        XCTAssertEqual(json["blob_hash"] as? String, summary.blobHashHex.lowercased())
        XCTAssertEqual(json["chunk_root"] as? String, summary.chunkRootHex.lowercased())
        XCTAssertEqual(json["por_root"] as? String, summary.porRootHex.lowercased())
        XCTAssertEqual(json["manifest_path"] as? String, "/tmp/manifest.json")
        XCTAssertEqual(json["payload_path"] as? String, "/tmp/payload.bin")
        XCTAssertEqual(numberValue(json["leaf_count"]), Optional(summary.leafCount))
        XCTAssertEqual(numberValue(json["segment_count"]), Optional(summary.segmentCount))
        XCTAssertEqual(numberValue(json["chunk_count"]), Optional(summary.chunkCount))
        XCTAssertEqual(numberValue(json["sample_seed"]), Optional(summary.sampleSeed))

        let proofs = try XCTUnwrap(json["proofs"] as? [[String: Any]])
        XCTAssertEqual(proofs.count, 1)
        let proof = try XCTUnwrap(proofs.first)
        XCTAssertEqual(proof["origin"] as? String, "sorafs_gateway")
        XCTAssertEqual(proof["leaf_bytes_b64"] as? String, artifact.proofs[0].leafBytesBase64)
        XCTAssertEqual(proof["chunk_digest"] as? String, "0a0b")
        XCTAssertEqual(proof["segment_leaves"] as? [String], ["aa", "bb"])
        XCTAssertEqual(proof["chunk_segments"] as? [String], ["cc"])
        XCTAssertEqual(proof["chunk_roots"] as? [String], ["dd"])
        XCTAssertEqual(proof["verified"] as? Bool, true)
    }

    func testEmitterGeneratesSummaryAndWritesFile() throws {
        let summary = makeStubProofSummary()
        let generator = StubDaProofSummaryGenerator(summary: summary)
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("da-proof-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true, attributes: nil)
        defer { try? FileManager.default.removeItem(at: tempDir) }
        let outputURL = tempDir.appendingPathComponent("artifact.json")

        let result = try DaProofSummaryArtifactEmitter.emit(
            manifestBytes: Data([0xAA]),
            payloadBytes: Data([0xBB]),
            proofOptions: ToriiDaProofSummaryOptions(sampleCount: 4, sampleSeed: 42),
            manifestPath: "manifest.json",
            payloadPath: "payload.bin",
            outputURL: outputURL,
            prettyPrinted: false,
            sortedKeys: true,
            generator: generator
        )

        XCTAssertEqual(result.summary, summary)
        XCTAssertEqual(result.artifact.manifestPath, "manifest.json")
        XCTAssertEqual(generator.invocations.count, 1)
        XCTAssertEqual(generator.invocations.first?.options.sampleCount, 4)
        XCTAssertTrue(FileManager.default.fileExists(atPath: outputURL.path))

        let written = try Data(contentsOf: outputURL)
        XCTAssertEqual(written.last, 0x0A)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: written, options: []) as? [String: Any])
        XCTAssertEqual(numberValue(json["proof_count"]), Optional(1 as UInt64))
    }
}

private func numberValue(_ value: Any?) -> UInt64? {
    guard let number = value as? NSNumber else {
        return nil
    }
    return number.uint64Value
}

private func makeStubProofSummary() -> ToriiDaProofSummary {
    let record = ToriiDaProofRecord(
        origin: "sorafs_gateway",
        leafIndex: 0,
        chunkIndex: 0,
        segmentIndex: 0,
        leafOffset: 0,
        leafLength: 1024,
        segmentOffset: 0,
        segmentLength: 2048,
        chunkOffset: 0,
        chunkLength: 4096,
        payloadLength: 4096,
        chunkDigestHex: "0A0B",
        chunkRootHex: "0C0D",
        segmentDigestHex: "0E0F",
        leafDigestHex: "0102",
        leafBytes: Data([0xDE, 0xAD, 0xBE, 0xEF]),
        segmentLeavesHex: ["AA", "BB"],
        chunkSegmentsHex: ["CC"],
        chunkRootsHex: ["DD"],
        verified: true
    )
    return ToriiDaProofSummary(
        blobHashHex: "deadbeef",
        chunkRootHex: "abcd",
        porRootHex: "f00d",
        leafCount: 64,
        segmentCount: 4,
        chunkCount: 2,
        sampleCount: 8,
        sampleSeed: 99,
        proofCount: 1,
        proofs: [record]
    )
}

private final class StubDaProofSummaryGenerator: @unchecked Sendable, DaProofSummaryGenerating {
    struct Invocation {
        let manifest: Data
        let payload: Data
        let options: ToriiDaProofSummaryOptions
    }

    let summary: ToriiDaProofSummary
    private(set) var invocations: [Invocation] = []

    init(summary: ToriiDaProofSummary) {
        self.summary = summary
    }

    func makeProofSummary(manifest: Data,
                          payload: Data,
                          options: ToriiDaProofSummaryOptions) throws -> ToriiDaProofSummary {
        invocations.append(Invocation(manifest: manifest, payload: payload, options: options))
        return summary
    }
}

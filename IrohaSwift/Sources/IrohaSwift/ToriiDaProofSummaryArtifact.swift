import Foundation

/// CLI-compatible Proof-of-Retrievability summary artefact.
public struct ToriiDaProofSummaryArtifact: Encodable, Sendable, Equatable {
    public struct ProofRecord: Encodable, Sendable, Equatable {
        public var origin: String
        public var leafIndex: UInt64
        public var chunkIndex: UInt64
        public var segmentIndex: UInt64
        public var leafOffset: UInt64
        public var leafLength: UInt64
        public var segmentOffset: UInt64
        public var segmentLength: UInt64
        public var chunkOffset: UInt64
        public var chunkLength: UInt64
        public var payloadLength: UInt64
        public var chunkDigest: String
        public var chunkRoot: String
        public var segmentDigest: String
        public var leafDigest: String
        public var leafBytesBase64: String
        public var segmentLeaves: [String]
        public var chunkSegments: [String]
        public var chunkRoots: [String]
        public var verified: Bool

        public init(from record: ToriiDaProofRecord) {
            origin = record.origin
            leafIndex = UInt64(record.leafIndex)
            chunkIndex = UInt64(record.chunkIndex)
            segmentIndex = UInt64(record.segmentIndex)
            leafOffset = record.leafOffset
            leafLength = UInt64(record.leafLength)
            segmentOffset = record.segmentOffset
            segmentLength = UInt64(record.segmentLength)
            chunkOffset = record.chunkOffset
            chunkLength = UInt64(record.chunkLength)
            payloadLength = record.payloadLength
            chunkDigest = record.chunkDigestHex.lowercased()
            chunkRoot = record.chunkRootHex.lowercased()
            segmentDigest = record.segmentDigestHex.lowercased()
            leafDigest = record.leafDigestHex.lowercased()
            leafBytesBase64 = record.leafBytes.base64EncodedString()
            segmentLeaves = record.segmentLeavesHex.map { $0.lowercased() }
            chunkSegments = record.chunkSegmentsHex.map { $0.lowercased() }
            chunkRoots = record.chunkRootsHex.map { $0.lowercased() }
            verified = record.verified
        }

        private enum CodingKeys: String, CodingKey {
            case origin
            case leafIndex = "leaf_index"
            case chunkIndex = "chunk_index"
            case segmentIndex = "segment_index"
            case leafOffset = "leaf_offset"
            case leafLength = "leaf_length"
            case segmentOffset = "segment_offset"
            case segmentLength = "segment_length"
            case chunkOffset = "chunk_offset"
            case chunkLength = "chunk_length"
            case payloadLength = "payload_len"
            case chunkDigest = "chunk_digest"
            case chunkRoot = "chunk_root"
            case segmentDigest = "segment_digest"
            case leafDigest = "leaf_digest"
            case leafBytesBase64 = "leaf_bytes_b64"
            case segmentLeaves = "segment_leaves"
            case chunkSegments = "chunk_segments"
            case chunkRoots = "chunk_roots"
            case verified
        }
    }

    public var manifestPath: String?
    public var payloadPath: String?
    public var blobHash: String
    public var chunkRoot: String
    public var porRoot: String
    public var leafCount: UInt64
    public var segmentCount: UInt64
    public var chunkCount: UInt64
    public var sampleCount: UInt64
    public var sampleSeed: UInt64
    public var proofCount: UInt64
    public var proofs: [ProofRecord]

    public init(summary: ToriiDaProofSummary, manifestPath: String? = nil, payloadPath: String? = nil) {
        self.manifestPath = manifestPath
        self.payloadPath = payloadPath
        blobHash = summary.blobHashHex.lowercased()
        chunkRoot = summary.chunkRootHex.lowercased()
        porRoot = summary.porRootHex.lowercased()
        leafCount = summary.leafCount
        segmentCount = summary.segmentCount
        chunkCount = summary.chunkCount
        sampleCount = summary.sampleCount
        sampleSeed = summary.sampleSeed
        proofCount = UInt64(summary.proofs.count)
        proofs = summary.proofs.map(ProofRecord.init)
    }

    private enum CodingKeys: String, CodingKey {
        case manifestPath = "manifest_path"
        case payloadPath = "payload_path"
        case blobHash = "blob_hash"
        case chunkRoot = "chunk_root"
        case porRoot = "por_root"
        case leafCount = "leaf_count"
        case segmentCount = "segment_count"
        case chunkCount = "chunk_count"
        case sampleCount = "sample_count"
        case sampleSeed = "sample_seed"
        case proofCount = "proof_count"
        case proofs
    }

    public func encoded(prettyPrinted: Bool = true, sortedKeys: Bool = true) throws -> Data {
        let encoder = JSONEncoder()
        var formatting: JSONEncoder.OutputFormatting = []
        if prettyPrinted {
            formatting.insert(.prettyPrinted)
        }
        if sortedKeys {
            formatting.insert(.sortedKeys)
        }
        encoder.outputFormatting = formatting
        return try encoder.encode(self)
    }
}

public struct ToriiDaProofSummaryArtifactEmitResult: Sendable, Equatable {
    public let summary: ToriiDaProofSummary
    public let artifact: ToriiDaProofSummaryArtifact
    public let outputURL: URL?
}

/// Convenience helpers for emitting PoR artefacts without the CLI.
public enum DaProofSummaryArtifactEmitter {
    public static func emit(
        summary: ToriiDaProofSummary? = nil,
        manifestBytes: Data? = nil,
        payloadBytes: Data? = nil,
        proofOptions: ToriiDaProofSummaryOptions? = nil,
        manifestPath: String? = nil,
        payloadPath: String? = nil,
        outputURL: URL? = nil,
        prettyPrinted: Bool = true,
        sortedKeys: Bool = true,
        generator: DaProofSummaryGenerating = NativeDaProofSummaryGenerator.shared,
        fileManager: FileManager = .default
    ) throws -> ToriiDaProofSummaryArtifactEmitResult {
        let resolvedSummary: ToriiDaProofSummary
        if let summary {
            resolvedSummary = summary
        } else {
            guard let manifestBytes, let payloadBytes else {
                throw ToriiClientError.invalidPayload(
                    "emitDaProofSummaryArtifact requires either a summary or manifestBytes + payloadBytes"
                )
            }
            guard !manifestBytes.isEmpty else {
                throw ToriiClientError.invalidPayload("manifestBytes must not be empty")
            }
            guard !payloadBytes.isEmpty else {
                throw ToriiClientError.invalidPayload("payloadBytes must not be empty")
            }
            let options = proofOptions ?? ToriiDaProofSummaryOptions()
            resolvedSummary = try generator.makeProofSummary(
                manifest: manifestBytes,
                payload: payloadBytes,
                options: options
            )
        }

        let artifact = ToriiDaProofSummaryArtifact(
            summary: resolvedSummary,
            manifestPath: manifestPath,
            payloadPath: payloadPath
        )

        if let outputURL {
            var encoded = try artifact.encoded(prettyPrinted: prettyPrinted, sortedKeys: sortedKeys)
            if let newline = "\n".data(using: .utf8) {
                encoded.append(newline)
            }
            let directoryURL = outputURL.deletingLastPathComponent()
            let directoryPath = directoryURL.path
            if !directoryPath.isEmpty && directoryPath != "." {
                try fileManager.createDirectory(at: directoryURL, withIntermediateDirectories: true, attributes: nil)
            }
            try encoded.write(to: outputURL, options: .atomic)
        }

        return ToriiDaProofSummaryArtifactEmitResult(
            summary: resolvedSummary,
            artifact: artifact,
            outputURL: outputURL
        )
    }
}

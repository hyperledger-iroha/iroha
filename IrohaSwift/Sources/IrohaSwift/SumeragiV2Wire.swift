// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import Foundation

/// Fail-closed errors produced by the canonical Sumeragi v2 Norito decoder.
public enum SumeragiV2WireError: Error, LocalizedError, Equatable, Sendable {
    case invalid(String)

    public var errorDescription: String? {
        switch self {
        case .invalid(let reason):
            return "Invalid Sumeragi v2 wire value: \(reason)"
        }
    }
}

/// A 32-byte Iroha hash used by the Sumeragi v2 wire models.
public struct SumeragiV2Hash: Equatable, Hashable, Sendable {
    public let bytes: Data

    public init(_ bytes: Data) throws {
        guard bytes.count == 32 else {
            throw SumeragiV2WireError.invalid("Iroha hash must contain 32 bytes")
        }
        guard let last = bytes.last, (last & 1) == 1 else {
            throw SumeragiV2WireError.invalid("Iroha hash low bit must be set")
        }
        self.bytes = bytes
    }
}

/// Exact bare-Norito payload of an Iroha `PeerId`.
public struct SumeragiV2PeerIDPayload: Equatable, Hashable, Sendable {
    public let bytes: Data

    public init(_ bytes: Data) throws {
        guard !bytes.isEmpty else {
            throw SumeragiV2WireError.invalid("PeerId payload must not be empty")
        }
        self.bytes = bytes
    }
}

/// Canonical Norito representation of an Iroha chain identifier.
public struct SumeragiV2ChainID: Equatable, Hashable, Sendable {
    public let value: String

    public init(_ value: String) {
        self.value = value
    }

    public func encode() -> Data {
        sumeragiV2Struct(sumeragiV2String(value))
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(sumeragiV2DecodeString(reader.field("chain ID value")))
        try reader.finish("chain ID")
        return value
    }
}

/// Typed hash of the immutable height context.
public struct SumeragiV2HeightContextID: Equatable, Sendable {
    public let hash: SumeragiV2Hash

    public init(hash: SumeragiV2Hash) {
        self.hash = hash
    }

    public func encode() -> Data {
        sumeragiV2Struct(hash.bytes)
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(hash: SumeragiV2Hash(reader.field("context hash")))
        try reader.finish("height context id")
        return value
    }
}

/// Round identity under a frozen height context.
public struct SumeragiV2ConsensusRound: Equatable, Sendable {
    public let contextID: SumeragiV2HeightContextID
    public let height: UInt64
    public let view: UInt64

    public init(contextID: SumeragiV2HeightContextID, height: UInt64, view: UInt64) {
        self.contextID = contextID
        self.height = height
        self.view = view
    }

    public func encode() -> Data {
        sumeragiV2Struct(contextID.encode(), sumeragiV2U64(height), sumeragiV2U64(view))
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            contextID: SumeragiV2HeightContextID.decode(reader.field("round context")),
            height: sumeragiV2DecodeU64(reader.field("round height")),
            view: sumeragiV2DecodeU64(reader.field("round view"))
        )
        try reader.finish("round")
        return value
    }
}

/// Global Sumeragi v2 voting phase.
public enum SumeragiV2GlobalPhase: UInt32, Equatable, Sendable {
    case prepare = 1
    case commit = 2

    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }

    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown global phase \(tag)")
        }
        return value
    }
}

/// Exact block and payload subject authenticated by consensus votes.
public struct SumeragiV2BlockSubject: Equatable, Sendable {
    public let parentBlockHash: SumeragiV2Hash?
    public let blockHash: SumeragiV2Hash
    public let payloadHash: SumeragiV2Hash

    public init(
        parentBlockHash: SumeragiV2Hash?,
        blockHash: SumeragiV2Hash,
        payloadHash: SumeragiV2Hash
    ) {
        self.parentBlockHash = parentBlockHash
        self.blockHash = blockHash
        self.payloadHash = payloadHash
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            sumeragiV2Option(parentBlockHash?.bytes),
            blockHash.bytes,
            payloadHash.bytes
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let parent = try sumeragiV2DecodeOption(reader.field("subject parent")) {
            try SumeragiV2Hash($0)
        }
        let value = try Self(
            parentBlockHash: parent,
            blockHash: SumeragiV2Hash(reader.field("subject block")),
            payloadHash: SumeragiV2Hash(reader.field("subject payload"))
        )
        try reader.finish("block subject")
        return value
    }
}

/// Prepare or Commit vote.
public struct SumeragiV2Vote: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let phase: SumeragiV2GlobalPhase
    public let subject: SumeragiV2BlockSubject
    public let signer: UInt32
    public let signature: Data

    public init(
        round: SumeragiV2ConsensusRound,
        phase: SumeragiV2GlobalPhase,
        subject: SumeragiV2BlockSubject,
        signer: UInt32,
        signature: Data
    ) {
        self.round = round
        self.phase = phase
        self.subject = subject
        self.signer = signer
        self.signature = signature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            round.encode(), phase.encode(), subject.encode(), sumeragiV2U32(signer),
            sumeragiV2ByteVector(signature)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("vote round")),
            phase: SumeragiV2GlobalPhase.decode(reader.field("vote phase")),
            subject: SumeragiV2BlockSubject.decode(reader.field("vote subject")),
            signer: sumeragiV2DecodeU32(reader.field("vote signer")),
            signature: sumeragiV2DecodeByteVector(reader.field("vote signature"))
        )
        try reader.finish("vote")
        return value
    }
}

/// Stable reference to a quorum certificate.
public struct SumeragiV2QuorumCertificateRef: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let phase: SumeragiV2GlobalPhase
    public let subject: SumeragiV2BlockSubject

    public init(
        round: SumeragiV2ConsensusRound,
        phase: SumeragiV2GlobalPhase,
        subject: SumeragiV2BlockSubject
    ) {
        self.round = round
        self.phase = phase
        self.subject = subject
    }

    public func encode() -> Data {
        sumeragiV2Struct(round.encode(), phase.encode(), subject.encode())
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("qc ref round")),
            phase: SumeragiV2GlobalPhase.decode(reader.field("qc ref phase")),
            subject: SumeragiV2BlockSubject.decode(reader.field("qc ref subject"))
        )
        try reader.finish("quorum certificate ref")
        return value
    }
}

/// Aggregate Prepare or Commit certificate.
public struct SumeragiV2QuorumCertificate: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let phase: SumeragiV2GlobalPhase
    public let subject: SumeragiV2BlockSubject
    public let signers: [UInt32]
    public let aggregateSignature: Data

    public init(
        round: SumeragiV2ConsensusRound,
        phase: SumeragiV2GlobalPhase,
        subject: SumeragiV2BlockSubject,
        signers: [UInt32],
        aggregateSignature: Data
    ) throws {
        try sumeragiV2RequireIncreasing(signers, label: "quorum certificate signers")
        self.round = round
        self.phase = phase
        self.subject = subject
        self.signers = signers
        self.aggregateSignature = aggregateSignature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            round.encode(), phase.encode(), subject.encode(),
            sumeragiV2Vector(signers, encode: sumeragiV2U32),
            sumeragiV2ByteVector(aggregateSignature)
        )
    }

    public var reference: SumeragiV2QuorumCertificateRef {
        SumeragiV2QuorumCertificateRef(round: round, phase: phase, subject: subject)
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("qc round")),
            phase: SumeragiV2GlobalPhase.decode(reader.field("qc phase")),
            subject: SumeragiV2BlockSubject.decode(reader.field("qc subject")),
            signers: sumeragiV2DecodeVector(reader.field("qc signers"), decode: sumeragiV2DecodeU32),
            aggregateSignature: sumeragiV2DecodeByteVector(reader.field("qc signature"))
        )
        try reader.finish("quorum certificate")
        return value
    }
}

/// One durable timeout vote.
public struct SumeragiV2TimeoutVote: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let highestPrepareQC: SumeragiV2QuorumCertificate?
    public let signer: UInt32
    public let signature: Data

    public init(
        round: SumeragiV2ConsensusRound,
        highestPrepareQC: SumeragiV2QuorumCertificate?,
        signer: UInt32,
        signature: Data
    ) {
        self.round = round
        self.highestPrepareQC = highestPrepareQC
        self.signer = signer
        self.signature = signature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            round.encode(), sumeragiV2Option(highestPrepareQC?.encode()),
            sumeragiV2U32(signer), sumeragiV2ByteVector(signature)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("timeout vote round")),
            highestPrepareQC: sumeragiV2DecodeOption(reader.field("timeout vote high qc"), decode: SumeragiV2QuorumCertificate.decode),
            signer: sumeragiV2DecodeU32(reader.field("timeout vote signer")),
            signature: sumeragiV2DecodeByteVector(reader.field("timeout vote signature"))
        )
        try reader.finish("timeout vote")
        return value
    }
}

/// Aggregate timeout signatures sharing one highest PrepareQC.
public struct SumeragiV2TimeoutVoteGroup: Equatable, Sendable {
    public let highestPrepareQC: SumeragiV2QuorumCertificate?
    public let signers: [UInt32]
    public let aggregateSignature: Data

    public init(
        highestPrepareQC: SumeragiV2QuorumCertificate?,
        signers: [UInt32],
        aggregateSignature: Data
    ) throws {
        guard !signers.isEmpty else {
            throw SumeragiV2WireError.invalid("timeout vote group has no signers")
        }
        try sumeragiV2RequireIncreasing(signers, label: "timeout group signers")
        self.highestPrepareQC = highestPrepareQC
        self.signers = signers
        self.aggregateSignature = aggregateSignature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            sumeragiV2Option(highestPrepareQC?.encode()),
            sumeragiV2Vector(signers, encode: sumeragiV2U32),
            sumeragiV2ByteVector(aggregateSignature)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            highestPrepareQC: sumeragiV2DecodeOption(reader.field("timeout group high qc"), decode: SumeragiV2QuorumCertificate.decode),
            signers: sumeragiV2DecodeVector(reader.field("timeout group signers"), decode: sumeragiV2DecodeU32),
            aggregateSignature: sumeragiV2DecodeByteVector(reader.field("timeout group signature"))
        )
        try reader.finish("timeout vote group")
        return value
    }
}

/// Certified transition out of one timed-out view.
public struct SumeragiV2TimeoutCertificate: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let groups: [SumeragiV2TimeoutVoteGroup]

    public init(round: SumeragiV2ConsensusRound, groups: [SumeragiV2TimeoutVoteGroup]) throws {
        guard !groups.isEmpty else {
            throw SumeragiV2WireError.invalid("timeout certificate has no groups")
        }
        var seen = Set<UInt32>()
        for signer in groups.flatMap(\.signers) {
            guard seen.insert(signer).inserted else {
                throw SumeragiV2WireError.invalid("timeout certificate signer groups overlap")
            }
        }
        self.round = round
        self.groups = groups
    }

    public func encode() -> Data {
        sumeragiV2Struct(round.encode(), sumeragiV2Vector(groups) { $0.encode() })
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("tc round")),
            groups: sumeragiV2DecodeVector(reader.field("tc groups"), decode: SumeragiV2TimeoutVoteGroup.decode)
        )
        try reader.finish("timeout certificate")
        return value
    }
}

/// Stable reference to a timeout certificate.
public struct SumeragiV2TimeoutCertificateRef: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let highestPrepareQC: SumeragiV2QuorumCertificateRef?
    public let certificateHash: SumeragiV2Hash

    public init(
        round: SumeragiV2ConsensusRound,
        highestPrepareQC: SumeragiV2QuorumCertificateRef?,
        certificateHash: SumeragiV2Hash
    ) {
        self.round = round
        self.highestPrepareQC = highestPrepareQC
        self.certificateHash = certificateHash
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            round.encode(), sumeragiV2Option(highestPrepareQC?.encode()), certificateHash.bytes
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("tc ref round")),
            highestPrepareQC: sumeragiV2DecodeOption(reader.field("tc ref high qc"), decode: SumeragiV2QuorumCertificateRef.decode),
            certificateHash: SumeragiV2Hash(reader.field("tc ref hash"))
        )
        try reader.finish("timeout certificate ref")
        return value
    }
}

/// View-zero parent CommitQC justification.
public struct SumeragiV2ParentCommitJustification: Equatable, Sendable {
    public let certificate: SumeragiV2QuorumCertificate?

    public init(certificate: SumeragiV2QuorumCertificate?) {
        self.certificate = certificate
    }

    public func encode() -> Data { sumeragiV2Struct(sumeragiV2Option(certificate?.encode())) }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            certificate: sumeragiV2DecodeOption(reader.field("parent certificate"), decode: SumeragiV2QuorumCertificate.decode)
        )
        try reader.finish("parent justification")
        return value
    }
}

/// Later-view timeout justification.
public struct SumeragiV2TimeoutJustification: Equatable, Sendable {
    public let timeoutCertificate: SumeragiV2TimeoutCertificate
    public let highestPrepareQC: SumeragiV2QuorumCertificate?

    public init(
        timeoutCertificate: SumeragiV2TimeoutCertificate,
        highestPrepareQC: SumeragiV2QuorumCertificate?
    ) {
        self.timeoutCertificate = timeoutCertificate
        self.highestPrepareQC = highestPrepareQC
    }

    public func encode() -> Data {
        sumeragiV2Struct(timeoutCertificate.encode(), sumeragiV2Option(highestPrepareQC?.encode()))
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            timeoutCertificate: SumeragiV2TimeoutCertificate.decode(reader.field("timeout justification tc")),
            highestPrepareQC: sumeragiV2DecodeOption(reader.field("timeout justification high qc"), decode: SumeragiV2QuorumCertificate.decode)
        )
        try reader.finish("timeout justification")
        return value
    }
}

/// Proposal justification union in Rust declaration order.
public enum SumeragiV2ProposalJustification: Equatable, Sendable {
    case parentCommit(SumeragiV2ParentCommitJustification)
    case timeout(SumeragiV2TimeoutJustification)

    public func encode() -> Data {
        switch self {
        case .parentCommit(let value): return sumeragiV2Enum(0, value.encode())
        case .timeout(let value): return sumeragiV2Enum(1, value.encode())
        }
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let tag = try reader.u32("proposal justification")
        let payload = try reader.compactField("proposal justification payload")
        try reader.finish("proposal justification")
        switch tag {
        case 0: return try .parentCommit(SumeragiV2ParentCommitJustification.decode(payload))
        case 1: return try .timeout(SumeragiV2TimeoutJustification.decode(payload))
        default: throw SumeragiV2WireError.invalid("unknown proposal justification \(tag)")
        }
    }
}

/// Deterministic data-availability payload encoding.
public enum SumeragiV2PayloadEncoding: UInt32, Equatable, Sendable {
    case plain = 0
    case reedSolomon16 = 1

    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }

    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown payload encoding \(tag)")
        }
        return value
    }
}

/// Payload chunking limits frozen for one height.
public struct SumeragiV2DataAvailabilityLayout: Equatable, Sendable {
    public let encoding: SumeragiV2PayloadEncoding
    public let chunkSizeBytes: UInt32
    public let dataShards: UInt16
    public let parityShards: UInt16
    public let maxPayloadSizeBytes: UInt64
    public let maxChunkCount: UInt32

    public init(
        encoding: SumeragiV2PayloadEncoding,
        chunkSizeBytes: UInt32,
        dataShards: UInt16,
        parityShards: UInt16,
        maxPayloadSizeBytes: UInt64,
        maxChunkCount: UInt32
    ) {
        self.encoding = encoding
        self.chunkSizeBytes = chunkSizeBytes
        self.dataShards = dataShards
        self.parityShards = parityShards
        self.maxPayloadSizeBytes = maxPayloadSizeBytes
        self.maxChunkCount = maxChunkCount
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            encoding.encode(), sumeragiV2U32(chunkSizeBytes), sumeragiV2U16(dataShards),
            sumeragiV2U16(parityShards), sumeragiV2U64(maxPayloadSizeBytes),
            sumeragiV2U32(maxChunkCount)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            encoding: SumeragiV2PayloadEncoding.decode(reader.field("da encoding")),
            chunkSizeBytes: sumeragiV2DecodeU32(reader.field("da chunk size")),
            dataShards: sumeragiV2DecodeU16(reader.field("da data shards")),
            parityShards: sumeragiV2DecodeU16(reader.field("da parity shards")),
            maxPayloadSizeBytes: sumeragiV2DecodeU64(reader.field("da max payload")),
            maxChunkCount: sumeragiV2DecodeU32(reader.field("da max chunks"))
        )
        try reader.finish("data availability layout")
        return value
    }
}

/// Manifest committing to one exact canonical block body.
public struct SumeragiV2PayloadManifest: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let subject: SumeragiV2BlockSubject
    public let payloadSizeBytes: UInt64
    public let layout: SumeragiV2DataAvailabilityLayout
    public let chunkHashes: [SumeragiV2Hash]
    public let chunkRoot: SumeragiV2Hash

    public init(
        round: SumeragiV2ConsensusRound,
        subject: SumeragiV2BlockSubject,
        payloadSizeBytes: UInt64,
        layout: SumeragiV2DataAvailabilityLayout,
        chunkHashes: [SumeragiV2Hash],
        chunkRoot: SumeragiV2Hash
    ) throws {
        guard !chunkHashes.isEmpty else {
            throw SumeragiV2WireError.invalid("payload manifest has no chunk hashes")
        }
        self.round = round
        self.subject = subject
        self.payloadSizeBytes = payloadSizeBytes
        self.layout = layout
        self.chunkHashes = chunkHashes
        self.chunkRoot = chunkRoot
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            round.encode(), subject.encode(), sumeragiV2U64(payloadSizeBytes), layout.encode(),
            sumeragiV2Vector(chunkHashes) { $0.bytes }, chunkRoot.bytes
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("manifest round")),
            subject: SumeragiV2BlockSubject.decode(reader.field("manifest subject")),
            payloadSizeBytes: sumeragiV2DecodeU64(reader.field("manifest size")),
            layout: SumeragiV2DataAvailabilityLayout.decode(reader.field("manifest layout")),
            chunkHashes: sumeragiV2DecodeVector(reader.field("manifest hashes"), decode: SumeragiV2Hash.init),
            chunkRoot: SumeragiV2Hash(reader.field("manifest root"))
        )
        try reader.finish("payload manifest")
        return value
    }
}

/// One authenticated encoded payload chunk.
public struct SumeragiV2PayloadChunk: Equatable, Sendable {
    public let manifestHash: SumeragiV2Hash
    public let index: UInt32
    public let bytes: Data
    public let sender: UInt32
    public let signature: Data

    public init(
        manifestHash: SumeragiV2Hash,
        index: UInt32,
        bytes: Data,
        sender: UInt32,
        signature: Data
    ) {
        self.manifestHash = manifestHash
        self.index = index
        self.bytes = bytes
        self.sender = sender
        self.signature = signature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            manifestHash.bytes, sumeragiV2U32(index), sumeragiV2ByteVector(bytes),
            sumeragiV2U32(sender), sumeragiV2ByteVector(signature)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            manifestHash: SumeragiV2Hash(reader.field("chunk manifest")),
            index: sumeragiV2DecodeU32(reader.field("chunk index")),
            bytes: sumeragiV2DecodeByteVector(reader.field("chunk bytes")),
            sender: sumeragiV2DecodeU32(reader.field("chunk sender")),
            signature: sumeragiV2DecodeByteVector(reader.field("chunk signature"))
        )
        try reader.finish("payload chunk")
        return value
    }
}

/// Signed proposal for one consensus round.
public struct SumeragiV2Proposal: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let proposer: UInt32
    public let subject: SumeragiV2BlockSubject
    public let manifest: SumeragiV2PayloadManifest
    public let justification: SumeragiV2ProposalJustification
    public let signature: Data

    public init(
        round: SumeragiV2ConsensusRound,
        proposer: UInt32,
        subject: SumeragiV2BlockSubject,
        manifest: SumeragiV2PayloadManifest,
        justification: SumeragiV2ProposalJustification,
        signature: Data
    ) {
        self.round = round
        self.proposer = proposer
        self.subject = subject
        self.manifest = manifest
        self.justification = justification
        self.signature = signature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            round.encode(), sumeragiV2U32(proposer), subject.encode(), manifest.encode(),
            justification.encode(), sumeragiV2ByteVector(signature)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("proposal round")),
            proposer: sumeragiV2DecodeU32(reader.field("proposal proposer")),
            subject: SumeragiV2BlockSubject.decode(reader.field("proposal subject")),
            manifest: SumeragiV2PayloadManifest.decode(reader.field("proposal manifest")),
            justification: SumeragiV2ProposalJustification.decode(reader.field("proposal justification")),
            signature: sumeragiV2DecodeByteVector(reader.field("proposal signature"))
        )
        try reader.finish("proposal")
        return value
    }
}

/// Authenticated request for a body covered by a quorum certificate.
public struct SumeragiV2CertifiedBodyRequest: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let subject: SumeragiV2BlockSubject
    public let certificate: SumeragiV2QuorumCertificate
    public let requester: SumeragiV2PeerIDPayload
    public let signature: Data

    public init(
        round: SumeragiV2ConsensusRound,
        subject: SumeragiV2BlockSubject,
        certificate: SumeragiV2QuorumCertificate,
        requester: SumeragiV2PeerIDPayload,
        signature: Data
    ) {
        self.round = round
        self.subject = subject
        self.certificate = certificate
        self.requester = requester
        self.signature = signature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            round.encode(), subject.encode(), certificate.encode(), requester.bytes,
            sumeragiV2ByteVector(signature)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("body request round")),
            subject: SumeragiV2BlockSubject.decode(reader.field("body request subject")),
            certificate: SumeragiV2QuorumCertificate.decode(reader.field("body request certificate")),
            requester: SumeragiV2PeerIDPayload(reader.field("body request requester")),
            signature: sumeragiV2DecodeByteVector(reader.field("body request signature"))
        )
        try reader.finish("certified body request")
        return value
    }
}

/// Authenticated response carrying a certified body.
public struct SumeragiV2CertifiedBodyResponse: Equatable, Sendable {
    public let requestHash: SumeragiV2Hash
    public let manifest: SumeragiV2PayloadManifest
    public let body: Data
    public let responder: UInt32
    public let signature: Data

    public init(
        requestHash: SumeragiV2Hash,
        manifest: SumeragiV2PayloadManifest,
        body: Data,
        responder: UInt32,
        signature: Data
    ) {
        self.requestHash = requestHash
        self.manifest = manifest
        self.body = body
        self.responder = responder
        self.signature = signature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            requestHash.bytes, manifest.encode(), sumeragiV2ByteVector(body),
            sumeragiV2U32(responder), sumeragiV2ByteVector(signature)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            requestHash: SumeragiV2Hash(reader.field("body response request hash")),
            manifest: SumeragiV2PayloadManifest.decode(reader.field("body response manifest")),
            body: sumeragiV2DecodeByteVector(reader.field("body response body")),
            responder: sumeragiV2DecodeU32(reader.field("body response responder")),
            signature: sumeragiV2DecodeByteVector(reader.field("body response signature"))
        )
        try reader.finish("certified body response")
        return value
    }
}

/// Signed request for the durable CommitQC of one exact height context.
public struct SumeragiV2CommitCertificateRequest: Equatable, Sendable {
    private static let signatureDomain =
        Data("iroha:sumeragi:v2:commit-certificate-request".utf8)

    public let protocolVersion: UInt16
    public let chainID: SumeragiV2ChainID
    public let contextID: SumeragiV2HeightContextID
    public let height: UInt64
    public let requester: SumeragiV2PeerIDPayload
    public let signature: Data

    public init(
        protocolVersion: UInt16 = SumeragiV2ConsensusMessage.protocolVersion,
        chainID: SumeragiV2ChainID,
        contextID: SumeragiV2HeightContextID,
        height: UInt64,
        requester: SumeragiV2PeerIDPayload,
        signature: Data
    ) throws {
        guard protocolVersion == SumeragiV2ConsensusMessage.protocolVersion else {
            throw SumeragiV2WireError.invalid(
                "unsupported commit-certificate request protocol version \(protocolVersion)"
            )
        }
        guard !signature.isEmpty else {
            throw SumeragiV2WireError.invalid("commit-certificate request signature is missing")
        }
        self.protocolVersion = protocolVersion
        self.chainID = chainID
        self.contextID = contextID
        self.height = height
        self.requester = requester
        self.signature = signature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            sumeragiV2U16(protocolVersion), chainID.encode(), contextID.encode(),
            sumeragiV2U64(height), requester.bytes, sumeragiV2ByteVector(signature)
        )
    }

    /// Exact domain-separated bytes authenticated by the requester.
    public func signaturePreimage() -> Data {
        var output = Self.signatureDomain
        output.append(sumeragiV2Struct(
            sumeragiV2U16(protocolVersion), chainID.encode(), contextID.encode(),
            sumeragiV2U64(height), requester.bytes, sumeragiV2ByteVector(Data())
        ))
        return output
    }

    /// Iroha hash identifying this exact signed request.
    public func requestHash() throws -> SumeragiV2Hash {
        try SumeragiV2Hash(IrohaHash.hash(encode()))
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            protocolVersion: sumeragiV2DecodeU16(reader.field("commit request protocol")),
            chainID: SumeragiV2ChainID.decode(reader.field("commit request chain ID")),
            contextID: SumeragiV2HeightContextID.decode(reader.field("commit request context")),
            height: sumeragiV2DecodeU64(reader.field("commit request height")),
            requester: SumeragiV2PeerIDPayload(reader.field("commit request requester")),
            signature: sumeragiV2DecodeByteVector(reader.field("commit request signature"))
        )
        try reader.finish("commit-certificate request")
        return value
    }
}

/// Signed response carrying the durable CommitQC for an exact request.
public struct SumeragiV2CommitCertificateResponse: Equatable, Sendable {
    private static let signatureDomain =
        Data("iroha:sumeragi:v2:commit-certificate-response".utf8)

    public let requestHash: SumeragiV2Hash
    public let certificate: SumeragiV2QuorumCertificate
    public let responder: SumeragiV2PeerIDPayload
    public let signature: Data

    public init(
        requestHash: SumeragiV2Hash,
        certificate: SumeragiV2QuorumCertificate,
        responder: SumeragiV2PeerIDPayload,
        signature: Data
    ) throws {
        guard certificate.phase == .commit else {
            throw SumeragiV2WireError.invalid(
                "commit-certificate response must carry a CommitQC"
            )
        }
        guard !signature.isEmpty else {
            throw SumeragiV2WireError.invalid("commit-certificate response signature is missing")
        }
        self.requestHash = requestHash
        self.certificate = certificate
        self.responder = responder
        self.signature = signature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            requestHash.bytes, certificate.encode(), responder.bytes,
            sumeragiV2ByteVector(signature)
        )
    }

    /// Exact domain-separated bytes authenticated by the responder.
    public func signaturePreimage() -> Data {
        var output = Self.signatureDomain
        output.append(sumeragiV2Struct(
            sumeragiV2U16(SumeragiV2ConsensusMessage.protocolVersion),
            requestHash.bytes, certificate.encode(), responder.bytes
        ))
        return output
    }

    /// Fail closed unless this response answers the exact request under its height context.
    /// Responder and aggregate-signature verification remains the caller's responsibility.
    public func validate(against request: SumeragiV2CommitCertificateRequest) throws {
        guard requestHash == (try request.requestHash()) else {
            throw SumeragiV2WireError.invalid(
                "commit-certificate response does not answer the exact signed request"
            )
        }
        guard certificate.round.contextID == request.contextID else {
            throw SumeragiV2WireError.invalid(
                "commit-certificate response uses a different height context"
            )
        }
        guard certificate.round.height == request.height else {
            throw SumeragiV2WireError.invalid(
                "commit-certificate response uses a different height"
            )
        }
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            requestHash: SumeragiV2Hash(reader.field("commit response request hash")),
            certificate: SumeragiV2QuorumCertificate.decode(reader.field("commit response certificate")),
            responder: SumeragiV2PeerIDPayload(reader.field("commit response responder")),
            signature: sumeragiV2DecodeByteVector(reader.field("commit response signature"))
        )
        try reader.finish("commit-certificate response")
        return value
    }
}

/// Canonical v2 network payload union in Rust declaration order.
public enum SumeragiV2ConsensusPayload: Equatable, Sendable {
    case proposal(SumeragiV2Proposal)
    case vote(SumeragiV2Vote)
    case quorumCertificate(SumeragiV2QuorumCertificate)
    case timeoutVote(SumeragiV2TimeoutVote)
    case timeoutCertificate(SumeragiV2TimeoutCertificate)
    case payloadManifest(SumeragiV2PayloadManifest)
    case payloadChunk(SumeragiV2PayloadChunk)
    case certifiedBodyRequest(SumeragiV2CertifiedBodyRequest)
    case certifiedBodyResponse(SumeragiV2CertifiedBodyResponse)
    case commitCertificateRequest(SumeragiV2CommitCertificateRequest)
    case commitCertificateResponse(SumeragiV2CommitCertificateResponse)

    public func encode() -> Data {
        switch self {
        case .proposal(let value): return sumeragiV2Enum(0, value.encode())
        case .vote(let value): return sumeragiV2Enum(1, value.encode())
        case .quorumCertificate(let value): return sumeragiV2Enum(2, value.encode())
        case .timeoutVote(let value): return sumeragiV2Enum(3, value.encode())
        case .timeoutCertificate(let value): return sumeragiV2Enum(4, value.encode())
        case .payloadManifest(let value): return sumeragiV2Enum(5, value.encode())
        case .payloadChunk(let value): return sumeragiV2Enum(6, value.encode())
        case .certifiedBodyRequest(let value): return sumeragiV2Enum(7, value.encode())
        case .certifiedBodyResponse(let value): return sumeragiV2Enum(8, value.encode())
        case .commitCertificateRequest(let value): return sumeragiV2Enum(9, value.encode())
        case .commitCertificateResponse(let value): return sumeragiV2Enum(10, value.encode())
        }
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let tag = try reader.u32("consensus payload")
        let payload = try reader.compactField("consensus payload value")
        try reader.finish("consensus payload")
        switch tag {
        case 0: return try .proposal(SumeragiV2Proposal.decode(payload))
        case 1: return try .vote(SumeragiV2Vote.decode(payload))
        case 2: return try .quorumCertificate(SumeragiV2QuorumCertificate.decode(payload))
        case 3: return try .timeoutVote(SumeragiV2TimeoutVote.decode(payload))
        case 4: return try .timeoutCertificate(SumeragiV2TimeoutCertificate.decode(payload))
        case 5: return try .payloadManifest(SumeragiV2PayloadManifest.decode(payload))
        case 6: return try .payloadChunk(SumeragiV2PayloadChunk.decode(payload))
        case 7: return try .certifiedBodyRequest(SumeragiV2CertifiedBodyRequest.decode(payload))
        case 8: return try .certifiedBodyResponse(SumeragiV2CertifiedBodyResponse.decode(payload))
        case 9: return try .commitCertificateRequest(SumeragiV2CommitCertificateRequest.decode(payload))
        case 10: return try .commitCertificateResponse(SumeragiV2CommitCertificateResponse.decode(payload))
        default: throw SumeragiV2WireError.invalid("unknown consensus payload \(tag)")
        }
    }
}

/// Explicitly versioned live-consensus envelope.
public struct SumeragiV2ConsensusMessage: Equatable, Sendable {
    public static let protocolVersion: UInt16 = 2

    public let version: UInt16
    public let payload: SumeragiV2ConsensusPayload

    public init(payload: SumeragiV2ConsensusPayload) {
        version = Self.protocolVersion
        self.payload = payload
    }

    fileprivate init(version: UInt16, payload: SumeragiV2ConsensusPayload) throws {
        guard version == Self.protocolVersion else {
            throw SumeragiV2WireError.invalid("unsupported consensus protocol version \(version)")
        }
        self.version = version
        self.payload = payload
    }

    public func encode() -> Data {
        sumeragiV2Struct(sumeragiV2U16(version), payload.encode())
    }

    /// Decode and require the canonical compact-length bare-Norito representation.
    public static func decodeCanonical(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            version: sumeragiV2DecodeU16(reader.field("message version")),
            payload: SumeragiV2ConsensusPayload.decode(reader.field("message payload"))
        )
        try reader.finish("consensus message")
        guard value.encode() == data else {
            throw SumeragiV2WireError.invalid("non-canonical consensus message")
        }
        return value
    }
}

/// Reducer phase reported by the compact Sumeragi v2 status endpoint.
public enum SumeragiV2StatusPhase: UInt32, Equatable, Sendable {
    case awaitingProposal = 0
    case reconstructingPayload = 1
    case validatingPayload = 2
    case prepare = 3
    case commit = 4
    case pendingApply = 5

    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }

    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown status phase \(tag)")
        }
        return value
    }
}

/// Local body state reported by the compact Sumeragi v2 status endpoint.
public enum SumeragiV2BodyState: UInt32, Equatable, Sendable {
    case missing = 0
    case reconstructing = 1
    case stored = 2
    case validated = 3
    case pendingApply = 4
    case applied = 5

    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }

    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown body state \(tag)")
        }
        return value
    }
}

/// Compact protocol-v2-only `/v1/sumeragi/status` payload.
public struct SumeragiV2Status: Equatable, Sendable {
    public let protocolVersion: UInt16
    public let nodeFingerprint: SumeragiV2Hash
    public let buildFingerprint: SumeragiV2Hash
    public let configFingerprint: SumeragiV2Hash
    public let heightContextID: SumeragiV2HeightContextID
    public let height: UInt64
    public let view: UInt64
    public let phase: SumeragiV2StatusPhase
    public let leader: UInt32
    public let lockedPrepareQC: SumeragiV2QuorumCertificateRef?
    public let highestPrepareQC: SumeragiV2QuorumCertificateRef?
    public let lastTimeoutCertificate: SumeragiV2TimeoutCertificateRef?
    public let bodyState: SumeragiV2BodyState
    public let pendingPersistenceID: UInt64?
    public let lastCommittedHeight: UInt64
    public let lastCommittedSubject: SumeragiV2BlockSubject?

    public init(
        protocolVersion: UInt16 = SumeragiV2ConsensusMessage.protocolVersion,
        nodeFingerprint: SumeragiV2Hash,
        buildFingerprint: SumeragiV2Hash,
        configFingerprint: SumeragiV2Hash,
        heightContextID: SumeragiV2HeightContextID,
        height: UInt64,
        view: UInt64,
        phase: SumeragiV2StatusPhase,
        leader: UInt32,
        lockedPrepareQC: SumeragiV2QuorumCertificateRef?,
        highestPrepareQC: SumeragiV2QuorumCertificateRef?,
        lastTimeoutCertificate: SumeragiV2TimeoutCertificateRef?,
        bodyState: SumeragiV2BodyState,
        pendingPersistenceID: UInt64?,
        lastCommittedHeight: UInt64,
        lastCommittedSubject: SumeragiV2BlockSubject?
    ) throws {
        guard protocolVersion == SumeragiV2ConsensusMessage.protocolVersion else {
            throw SumeragiV2WireError.invalid("unsupported status protocol version \(protocolVersion)")
        }
        self.protocolVersion = protocolVersion
        self.nodeFingerprint = nodeFingerprint
        self.buildFingerprint = buildFingerprint
        self.configFingerprint = configFingerprint
        self.heightContextID = heightContextID
        self.height = height
        self.view = view
        self.phase = phase
        self.leader = leader
        self.lockedPrepareQC = lockedPrepareQC
        self.highestPrepareQC = highestPrepareQC
        self.lastTimeoutCertificate = lastTimeoutCertificate
        self.bodyState = bodyState
        self.pendingPersistenceID = pendingPersistenceID
        self.lastCommittedHeight = lastCommittedHeight
        self.lastCommittedSubject = lastCommittedSubject
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            sumeragiV2U16(protocolVersion), nodeFingerprint.bytes, buildFingerprint.bytes,
            configFingerprint.bytes, heightContextID.encode(), sumeragiV2U64(height),
            sumeragiV2U64(view), phase.encode(), sumeragiV2U32(leader),
            sumeragiV2Option(lockedPrepareQC?.encode()),
            sumeragiV2Option(highestPrepareQC?.encode()),
            sumeragiV2Option(lastTimeoutCertificate?.encode()), bodyState.encode(),
            sumeragiV2Option(pendingPersistenceID.map(sumeragiV2U64)),
            sumeragiV2U64(lastCommittedHeight),
            sumeragiV2Option(lastCommittedSubject?.encode())
        )
    }

    /// Decode and require the canonical compact-length bare-Norito representation.
    public static func decodeCanonical(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            protocolVersion: sumeragiV2DecodeU16(reader.field("status version")),
            nodeFingerprint: SumeragiV2Hash(reader.field("status node")),
            buildFingerprint: SumeragiV2Hash(reader.field("status build")),
            configFingerprint: SumeragiV2Hash(reader.field("status config")),
            heightContextID: SumeragiV2HeightContextID.decode(reader.field("status context")),
            height: sumeragiV2DecodeU64(reader.field("status height")),
            view: sumeragiV2DecodeU64(reader.field("status view")),
            phase: SumeragiV2StatusPhase.decode(reader.field("status phase")),
            leader: sumeragiV2DecodeU32(reader.field("status leader")),
            lockedPrepareQC: sumeragiV2DecodeOption(reader.field("status lock"), decode: SumeragiV2QuorumCertificateRef.decode),
            highestPrepareQC: sumeragiV2DecodeOption(reader.field("status high qc"), decode: SumeragiV2QuorumCertificateRef.decode),
            lastTimeoutCertificate: sumeragiV2DecodeOption(reader.field("status last tc"), decode: SumeragiV2TimeoutCertificateRef.decode),
            bodyState: SumeragiV2BodyState.decode(reader.field("status body state")),
            pendingPersistenceID: sumeragiV2DecodeOption(reader.field("status persistence"), decode: sumeragiV2DecodeU64),
            lastCommittedHeight: sumeragiV2DecodeU64(reader.field("status committed height")),
            lastCommittedSubject: sumeragiV2DecodeOption(reader.field("status committed subject"), decode: SumeragiV2BlockSubject.decode)
        )
        try reader.finish("Sumeragi v2 status")
        guard value.encode() == data else {
            throw SumeragiV2WireError.invalid("non-canonical Sumeragi v2 status")
        }
        return value
    }
}

private struct SumeragiV2Reader {
    private let data: Data
    private var offset = 0

    init(_ data: Data) {
        self.data = data
    }

    mutating func u8(_ label: String) throws -> UInt8 {
        guard offset < data.count else { throw SumeragiV2WireError.invalid("\(label) is truncated") }
        defer { offset += 1 }
        return data[data.startIndex + offset]
    }

    mutating func u16(_ label: String) throws -> UInt16 {
        try integer(label, byteCount: 2)
    }

    mutating func u32(_ label: String) throws -> UInt32 {
        try integer(label, byteCount: 4)
    }

    mutating func u64(_ label: String) throws -> UInt64 {
        try integer(label, byteCount: 8)
    }

    mutating func compactField(_ label: String) throws -> Data {
        let length = try varint(label)
        guard length <= UInt64(Int.max) else {
            throw SumeragiV2WireError.invalid("\(label) length exceeds platform range")
        }
        return try read(Int(length), label: label)
    }

    mutating func field(_ label: String) throws -> Data {
        try compactField(label)
    }

    mutating func finish(_ label: String) throws {
        guard offset == data.count else {
            throw SumeragiV2WireError.invalid("\(label) contains trailing bytes")
        }
    }

    private mutating func integer<T: FixedWidthInteger>(
        _ label: String,
        byteCount: Int
    ) throws -> T {
        let bytes = try read(byteCount, label: label)
        var value: T = 0
        bytes.withUnsafeBytes { source in
            guard let base = source.baseAddress else { return }
            memcpy(&value, base, byteCount)
        }
        return T(littleEndian: value)
    }

    private mutating func read(_ count: Int, label: String) throws -> Data {
        guard count >= 0, offset <= data.count - count else {
            throw SumeragiV2WireError.invalid("\(label) is truncated")
        }
        let start = data.startIndex + offset
        offset += count
        return Data(data[start..<(start + count)])
    }

    private mutating func varint(_ label: String) throws -> UInt64 {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        var count = 0
        while true {
            let byte = try u8(label)
            count += 1
            guard count <= 10, shift < 64 else {
                throw SumeragiV2WireError.invalid("\(label) varint overflows u64")
            }
            if shift == 63, (byte & 0x7e) != 0 {
                throw SumeragiV2WireError.invalid("\(label) varint overflows u64")
            }
            value |= UInt64(byte & 0x7f) << shift
            if (byte & 0x80) == 0 {
                guard sumeragiV2Varint(value).count == count else {
                    throw SumeragiV2WireError.invalid("\(label) uses a non-canonical varint")
                }
                return value
            }
            shift += 7
        }
    }
}

private func sumeragiV2Struct(_ fields: Data...) -> Data {
    var output = Data()
    for field in fields {
        output.append(sumeragiV2Varint(UInt64(field.count)))
        output.append(field)
    }
    return output
}

private func sumeragiV2Enum(_ tag: UInt32, _ payload: Data) -> Data {
    var output = sumeragiV2U32(tag)
    output.append(sumeragiV2Varint(UInt64(payload.count)))
    output.append(payload)
    return output
}

private func sumeragiV2U16(_ value: UInt16) -> Data { sumeragiV2Integer(value) }
private func sumeragiV2U32(_ value: UInt32) -> Data { sumeragiV2Integer(value) }
private func sumeragiV2U64(_ value: UInt64) -> Data { sumeragiV2Integer(value) }

private func sumeragiV2Integer<T: FixedWidthInteger>(_ value: T) -> Data {
    var littleEndian = value.littleEndian
    return withUnsafeBytes(of: &littleEndian) { Data($0) }
}

private func sumeragiV2ByteVector(_ bytes: Data) -> Data {
    var output = sumeragiV2U64(UInt64(bytes.count))
    output.append(bytes)
    return output
}

private func sumeragiV2String(_ value: String) -> Data {
    let bytes = Data(value.utf8)
    var output = sumeragiV2Varint(UInt64(bytes.count))
    output.append(bytes)
    return output
}

private func sumeragiV2Option(_ payload: Data?) -> Data {
    guard let payload else { return Data([0]) }
    var output = Data([1])
    output.append(sumeragiV2Varint(UInt64(payload.count)))
    output.append(payload)
    return output
}

private func sumeragiV2Vector<T>(_ values: [T], encode: (T) -> Data) -> Data {
    var output = sumeragiV2U64(UInt64(values.count))
    for value in values {
        let payload = encode(value)
        output.append(sumeragiV2Varint(UInt64(payload.count)))
        output.append(payload)
    }
    return output
}

private func sumeragiV2Varint(_ value: UInt64) -> Data {
    var remaining = value
    var output = Data()
    while remaining >= 0x80 {
        output.append(UInt8(remaining & 0x7f) | 0x80)
        remaining >>= 7
    }
    output.append(UInt8(remaining))
    return output
}

private func sumeragiV2DecodeU16(_ data: Data) throws -> UInt16 {
    var reader = SumeragiV2Reader(data)
    let value = try reader.u16("u16")
    try reader.finish("u16")
    return value
}

private func sumeragiV2DecodeU32(_ data: Data) throws -> UInt32 {
    var reader = SumeragiV2Reader(data)
    let value = try reader.u32("u32")
    try reader.finish("u32")
    return value
}

private func sumeragiV2DecodeU64(_ data: Data) throws -> UInt64 {
    var reader = SumeragiV2Reader(data)
    let value = try reader.u64("u64")
    try reader.finish("u64")
    return value
}

private func sumeragiV2DecodeByteVector(_ data: Data) throws -> Data {
    var reader = SumeragiV2Reader(data)
    let length = try reader.u64("byte vector length")
    guard length <= UInt64(Int.max) else {
        throw SumeragiV2WireError.invalid("byte vector length exceeds platform range")
    }
    let value = try reader.rawBytes(Int(length), label: "byte vector")
    try reader.finish("byte vector")
    return value
}

private func sumeragiV2DecodeString(_ data: Data) throws -> String {
    var reader = SumeragiV2Reader(data)
    let bytes = try reader.compactField("string bytes")
    try reader.finish("string")
    guard let value = String(data: bytes, encoding: .utf8) else {
        throw SumeragiV2WireError.invalid("string is not valid UTF-8")
    }
    return value
}

private func sumeragiV2DecodeOption<T>(
    _ data: Data,
    decode: (Data) throws -> T
) throws -> T? {
    var reader = SumeragiV2Reader(data)
    let tag = try reader.u8("option")
    if tag == 0 {
        try reader.finish("option")
        return nil
    }
    guard tag == 1 else { throw SumeragiV2WireError.invalid("invalid Option tag \(tag)") }
    let value = try decode(reader.compactField("option payload"))
    try reader.finish("option")
    return value
}

private func sumeragiV2DecodeVector<T>(
    _ data: Data,
    decode: (Data) throws -> T
) throws -> [T] {
    var reader = SumeragiV2Reader(data)
    let count = try reader.u64("vector count")
    guard count <= UInt64(Int.max) else {
        throw SumeragiV2WireError.invalid("vector count exceeds platform range")
    }
    var values: [T] = []
    values.reserveCapacity(Int(count))
    for _ in 0..<Int(count) {
        values.append(try decode(reader.compactField("vector element")))
    }
    try reader.finish("vector")
    return values
}

private func sumeragiV2RequireIncreasing(_ values: [UInt32], label: String) throws {
    for (left, right) in zip(values, values.dropFirst()) where left >= right {
        throw SumeragiV2WireError.invalid("\(label) are not strictly increasing")
    }
}

private extension SumeragiV2Reader {
    mutating func rawBytes(_ count: Int, label: String) throws -> Data {
        var bytes = Data()
        bytes.reserveCapacity(count)
        for _ in 0..<count { bytes.append(try u8(label)) }
        return bytes
    }
}

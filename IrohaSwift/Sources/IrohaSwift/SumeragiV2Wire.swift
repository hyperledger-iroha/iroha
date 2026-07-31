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

/// An arbitrary 32-byte protocol value without Iroha hash bit constraints.
public struct SumeragiV2Bytes32: Equatable, Hashable, Sendable {
    public let bytes: Data

    public init(_ bytes: Data) throws {
        guard bytes.count == 32 else {
            throw SumeragiV2WireError.invalid("protocol value must contain 32 bytes")
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

/// Exact merge-ledger entry identity authenticated by global finality.
public struct SumeragiV2MergeCarrierCommitment: Equatable, Sendable {
    public static let canonicalVersion: UInt16 = 1

    public let version: UInt16
    public let entryHash: SumeragiV2Hash

    public init(version: UInt16 = Self.canonicalVersion, entryHash: SumeragiV2Hash) throws {
        guard version == Self.canonicalVersion else {
            throw SumeragiV2WireError.invalid(
                "merge-carrier commitment has an unsupported version"
            )
        }
        self.version = version
        self.entryHash = entryHash
    }

    public func encode() -> Data {
        sumeragiV2Struct(sumeragiV2U16(version), entryHash.bytes)
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            version: sumeragiV2DecodeU16(reader.field("merge carrier version")),
            entryHash: SumeragiV2Hash(reader.field("merge carrier entry hash"))
        )
        try reader.finish("merge carrier commitment")
        return value
    }
}

/// Deterministic state-transition commitment authenticated by consensus votes.
public struct SumeragiV2ExecutionCommitment: Equatable, Sendable {
    /// Maximum number of Kagemusha top-up anchors committed by one block.
    public static let maximumTopUpAnchorCount: UInt32 = 16
    /// Canonical Native AMX application-manifest wire version.
    public static let canonicalNativeAmxApplicationManifestVersion: UInt16 = 1
    /// Maximum participant route/incarnation leaves committed by one global block.
    public static let maximumNativeAmxApplicationManifestLeafCount: UInt32 = 1024

    public let parentStateRoot: SumeragiV2Hash
    public let postStateRoot: SumeragiV2Hash
    public let ordinaryWritesRoot: SumeragiV2Hash
    public let topUpAnchorRoot: SumeragiV2Hash?
    public let topUpAnchorCount: UInt32
    public let nativeAmxApplicationManifestVersion: UInt16
    public let nativeAmxApplicationManifestRoot: SumeragiV2Hash
    public let nativeAmxApplicationManifestCount: UInt32
    public let mergeCarrier: SumeragiV2MergeCarrierCommitment?
    public let executedBlockWireLen: UInt64
    public let executedBlockWireHash: SumeragiV2Hash

    public init(
        parentStateRoot: SumeragiV2Hash,
        postStateRoot: SumeragiV2Hash,
        ordinaryWritesRoot: SumeragiV2Hash,
        topUpAnchorRoot: SumeragiV2Hash?,
        topUpAnchorCount: UInt32,
        nativeAmxApplicationManifestVersion: UInt16,
        nativeAmxApplicationManifestRoot: SumeragiV2Hash,
        nativeAmxApplicationManifestCount: UInt32,
        mergeCarrier: SumeragiV2MergeCarrierCommitment? = nil,
        executedBlockWireLen: UInt64,
        executedBlockWireHash: SumeragiV2Hash
    ) throws {
        guard (topUpAnchorCount == 0) == (topUpAnchorRoot == nil) else {
            throw SumeragiV2WireError.invalid(
                "execution commitment top-up count and root presence disagree"
            )
        }
        guard topUpAnchorCount <= Self.maximumTopUpAnchorCount else {
            throw SumeragiV2WireError.invalid("execution commitment has too many top-up anchors")
        }
        if let topUpAnchorRoot {
            let expectedPostStateRoot = Self.topUpPostStateRoot(
                count: topUpAnchorCount,
                ordinaryWritesRoot: ordinaryWritesRoot,
                topUpAnchorRoot: topUpAnchorRoot
            )
            guard postStateRoot.bytes == expectedPostStateRoot else {
                throw SumeragiV2WireError.invalid(
                    "execution commitment post-state root does not match its top-up projection"
                )
            }
        }
        guard nativeAmxApplicationManifestVersion ==
            Self.canonicalNativeAmxApplicationManifestVersion else {
            throw SumeragiV2WireError.invalid(
                "execution commitment has an unsupported Native AMX application-manifest version"
            )
        }
        guard nativeAmxApplicationManifestCount <=
            Self.maximumNativeAmxApplicationManifestLeafCount else {
            throw SumeragiV2WireError.invalid(
                "execution commitment has too many Native AMX application-manifest leaves"
            )
        }
        let emptyManifestRoot = Self.nativeAmxApplicationManifestEmptyRootBytes()
        guard (nativeAmxApplicationManifestCount == 0) ==
            (nativeAmxApplicationManifestRoot.bytes == emptyManifestRoot) else {
            throw SumeragiV2WireError.invalid(
                "execution commitment Native AMX application-manifest count and root disagree"
            )
        }
        guard executedBlockWireLen != 0 else {
            throw SumeragiV2WireError.invalid(
                "execution commitment executed block wire length must be non-zero"
            )
        }

        self.parentStateRoot = parentStateRoot
        self.postStateRoot = postStateRoot
        self.ordinaryWritesRoot = ordinaryWritesRoot
        self.topUpAnchorRoot = topUpAnchorRoot
        self.topUpAnchorCount = topUpAnchorCount
        self.nativeAmxApplicationManifestVersion = nativeAmxApplicationManifestVersion
        self.nativeAmxApplicationManifestRoot = nativeAmxApplicationManifestRoot
        self.nativeAmxApplicationManifestCount = nativeAmxApplicationManifestCount
        self.mergeCarrier = mergeCarrier
        self.executedBlockWireLen = executedBlockWireLen
        self.executedBlockWireHash = executedBlockWireHash
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            parentStateRoot.bytes,
            postStateRoot.bytes,
            ordinaryWritesRoot.bytes,
            sumeragiV2Option(topUpAnchorRoot?.bytes),
            sumeragiV2U32(topUpAnchorCount),
            sumeragiV2U16(nativeAmxApplicationManifestVersion),
            nativeAmxApplicationManifestRoot.bytes,
            sumeragiV2U32(nativeAmxApplicationManifestCount),
            sumeragiV2Option(mergeCarrier?.encode()),
            sumeragiV2U64(executedBlockWireLen),
            executedBlockWireHash.bytes
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            parentStateRoot: SumeragiV2Hash(reader.field("execution commitment parent state")),
            postStateRoot: SumeragiV2Hash(reader.field("execution commitment post state")),
            ordinaryWritesRoot: SumeragiV2Hash(
                reader.field("execution commitment ordinary writes")
            ),
            topUpAnchorRoot: sumeragiV2DecodeOption(
                reader.field("execution commitment top-up root"),
                decode: SumeragiV2Hash.init
            ),
            topUpAnchorCount: sumeragiV2DecodeU32(
                reader.field("execution commitment top-up count")
            ),
            nativeAmxApplicationManifestVersion: sumeragiV2DecodeU16(
                reader.field("execution commitment Native AMX application-manifest version")
            ),
            nativeAmxApplicationManifestRoot: SumeragiV2Hash(
                reader.field("execution commitment Native AMX application-manifest root")
            ),
            nativeAmxApplicationManifestCount: sumeragiV2DecodeU32(
                reader.field("execution commitment Native AMX application-manifest count")
            ),
            mergeCarrier: sumeragiV2DecodeOption(
                reader.field("execution commitment merge carrier"),
                decode: SumeragiV2MergeCarrierCommitment.decode
            ),
            executedBlockWireLen: sumeragiV2DecodeU64(
                reader.field("execution commitment executed block wire length")
            ),
            executedBlockWireHash: SumeragiV2Hash(
                reader.field("execution commitment executed block wire hash")
            )
        )
        try reader.finish("execution commitment")
        return value
    }

    private static func topUpPostStateRoot(
        count: UInt32,
        ordinaryWritesRoot: SumeragiV2Hash,
        topUpAnchorRoot: SumeragiV2Hash
    ) -> Data {
        var preimage = Data("iroha:kagemusha:v2:post-state-root".utf8)
        preimage.append(0)
        preimage.append(sumeragiV2U32(count))
        preimage.append(ordinaryWritesRoot.bytes)
        preimage.append(topUpAnchorRoot.bytes)
        return IrohaHash.hash(preimage)
    }

    /// Canonical root bytes for a global block with no separate Native AMX applications.
    public static func nativeAmxApplicationManifestEmptyRootBytes() -> Data {
        IrohaHash.hash(
            Data("iroha:sumeragi:v2:native-amx-application-manifest:v1:empty".utf8)
        )
    }
}

/// Prepare or Commit vote.
public struct SumeragiV2Vote: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let proposalRound: SumeragiV2ConsensusRound
    public let phase: SumeragiV2GlobalPhase
    public let subject: SumeragiV2BlockSubject
    public let executionCommitment: SumeragiV2ExecutionCommitment
    public let signer: UInt32
    public let signature: Data

    public init(
        round: SumeragiV2ConsensusRound,
        proposalRound: SumeragiV2ConsensusRound,
        phase: SumeragiV2GlobalPhase,
        subject: SumeragiV2BlockSubject,
        executionCommitment: SumeragiV2ExecutionCommitment,
        signer: UInt32,
        signature: Data
    ) throws {
        guard proposalRound == round else {
            throw SumeragiV2WireError.invalid(
                "Prepare/Commit vote proposal round must match its round"
            )
        }
        self.round = round
        self.proposalRound = proposalRound
        self.phase = phase
        self.subject = subject
        self.executionCommitment = executionCommitment
        self.signer = signer
        self.signature = signature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            round.encode(), proposalRound.encode(), phase.encode(), subject.encode(),
            executionCommitment.encode(), sumeragiV2U32(signer),
            sumeragiV2ByteVector(signature)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("vote round")),
            proposalRound: SumeragiV2ConsensusRound.decode(
                reader.field("vote proposal round")
            ),
            phase: SumeragiV2GlobalPhase.decode(reader.field("vote phase")),
            subject: SumeragiV2BlockSubject.decode(reader.field("vote subject")),
            executionCommitment: SumeragiV2ExecutionCommitment.decode(
                reader.field("vote execution commitment")
            ),
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
    public let proposalRound: SumeragiV2ConsensusRound
    public let phase: SumeragiV2GlobalPhase
    public let subject: SumeragiV2BlockSubject
    public let executionCommitment: SumeragiV2ExecutionCommitment

    public init(
        round: SumeragiV2ConsensusRound,
        proposalRound: SumeragiV2ConsensusRound,
        phase: SumeragiV2GlobalPhase,
        subject: SumeragiV2BlockSubject,
        executionCommitment: SumeragiV2ExecutionCommitment
    ) throws {
        guard proposalRound == round else {
            throw SumeragiV2WireError.invalid(
                "Prepare/Commit certificate reference proposal round must match its round"
            )
        }
        self.init(
            validatedRound: round,
            proposalRound: proposalRound,
            phase: phase,
            subject: subject,
            executionCommitment: executionCommitment
        )
    }

    fileprivate init(
        validatedRound round: SumeragiV2ConsensusRound,
        proposalRound: SumeragiV2ConsensusRound,
        phase: SumeragiV2GlobalPhase,
        subject: SumeragiV2BlockSubject,
        executionCommitment: SumeragiV2ExecutionCommitment
    ) {
        self.round = round
        self.proposalRound = proposalRound
        self.phase = phase
        self.subject = subject
        self.executionCommitment = executionCommitment
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            round.encode(), proposalRound.encode(), phase.encode(), subject.encode(),
            executionCommitment.encode()
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("qc ref round")),
            proposalRound: SumeragiV2ConsensusRound.decode(
                reader.field("qc ref proposal round")
            ),
            phase: SumeragiV2GlobalPhase.decode(reader.field("qc ref phase")),
            subject: SumeragiV2BlockSubject.decode(reader.field("qc ref subject")),
            executionCommitment: SumeragiV2ExecutionCommitment.decode(
                reader.field("qc ref execution commitment")
            )
        )
        try reader.finish("quorum certificate ref")
        return value
    }
}

/// Aggregate Prepare or Commit certificate.
public struct SumeragiV2QuorumCertificate: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let proposalRound: SumeragiV2ConsensusRound
    public let phase: SumeragiV2GlobalPhase
    public let subject: SumeragiV2BlockSubject
    public let executionCommitment: SumeragiV2ExecutionCommitment
    public let signers: [UInt32]
    public let aggregateSignature: Data

    public init(
        round: SumeragiV2ConsensusRound,
        proposalRound: SumeragiV2ConsensusRound,
        phase: SumeragiV2GlobalPhase,
        subject: SumeragiV2BlockSubject,
        executionCommitment: SumeragiV2ExecutionCommitment,
        signers: [UInt32],
        aggregateSignature: Data
    ) throws {
        guard proposalRound == round else {
            throw SumeragiV2WireError.invalid(
                "Prepare/Commit certificate proposal round must match its round"
            )
        }
        try sumeragiV2RequireIncreasing(signers, label: "quorum certificate signers")
        self.round = round
        self.proposalRound = proposalRound
        self.phase = phase
        self.subject = subject
        self.executionCommitment = executionCommitment
        self.signers = signers
        self.aggregateSignature = aggregateSignature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            round.encode(), proposalRound.encode(), phase.encode(), subject.encode(),
            executionCommitment.encode(), sumeragiV2Vector(signers, encode: sumeragiV2U32),
            sumeragiV2ByteVector(aggregateSignature)
        )
    }

    public var reference: SumeragiV2QuorumCertificateRef {
        SumeragiV2QuorumCertificateRef(
            validatedRound: round,
            proposalRound: proposalRound,
            phase: phase,
            subject: subject,
            executionCommitment: executionCommitment
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("qc round")),
            proposalRound: SumeragiV2ConsensusRound.decode(
                reader.field("qc proposal round")
            ),
            phase: SumeragiV2GlobalPhase.decode(reader.field("qc phase")),
            subject: SumeragiV2BlockSubject.decode(reader.field("qc subject")),
            executionCommitment: SumeragiV2ExecutionCommitment.decode(
                reader.field("qc execution commitment")
            ),
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

/// Archive-signed response carrying a certified body and a distinct
/// frozen-QC signer citation.
public struct SumeragiV2CertifiedBodyResponse: Equatable, Sendable {
    public let requestHash: SumeragiV2Hash
    public let manifest: SumeragiV2PayloadManifest
    public let body: Data
    public let citedResponder: UInt32
    public let signature: Data

    public init(
        requestHash: SumeragiV2Hash,
        manifest: SumeragiV2PayloadManifest,
        body: Data,
        citedResponder: UInt32,
        signature: Data
    ) {
        self.requestHash = requestHash
        self.manifest = manifest
        self.body = body
        self.citedResponder = citedResponder
        self.signature = signature
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            requestHash.bytes, manifest.encode(), sumeragiV2ByteVector(body),
            sumeragiV2U32(citedResponder), sumeragiV2ByteVector(signature)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            requestHash: SumeragiV2Hash(reader.field("body response request hash")),
            manifest: SumeragiV2PayloadManifest.decode(reader.field("body response manifest")),
            body: sumeragiV2DecodeByteVector(reader.field("body response body")),
            citedResponder: sumeragiV2DecodeU32(reader.field("body response cited responder")),
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
    public static let protocolVersion: UInt16 = 4

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

/// Consensus mode frozen in the status height context.
public enum SumeragiV2ConsensusMode: UInt32, Equatable, Sendable {
    case permissioned = 0
    case npos = 1

    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }

    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown consensus mode \(tag)")
        }
        return value
    }
}

/// Canonical count-and-power quorum frozen in a status context.
public struct SumeragiV2DualQuorum: Equatable, Sendable {
    public let minSigners: UInt32
    public let totalPower: UInt64

    public init(minSigners: UInt32, totalPower: UInt64) {
        self.minSigners = minSigners
        self.totalPower = totalPower
    }

    fileprivate func encode() -> Data {
        sumeragiV2Struct(sumeragiV2U32(minSigners), sumeragiV2U64(totalPower))
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            minSigners: sumeragiV2DecodeU32(reader.field("status quorum min signers")),
            totalPower: sumeragiV2DecodeU64(reader.field("status quorum total power"))
        )
        try reader.finish("status dual quorum")
        return value
    }
}

/// Frozen election context accompanying authoritative v2 status.
public struct SumeragiV2HeightContextStatus: Equatable, Sendable {
    public let epoch: UInt64
    public let epochEndHeight: UInt64
    public let mode: SumeragiV2ConsensusMode
    public let epochSeed: SumeragiV2Bytes32
    public let validatorCount: UInt32
    public let quorum: SumeragiV2DualQuorum

    public init(
        epoch: UInt64,
        epochEndHeight: UInt64,
        mode: SumeragiV2ConsensusMode,
        epochSeed: SumeragiV2Bytes32,
        validatorCount: UInt32,
        quorum: SumeragiV2DualQuorum
    ) {
        self.epoch = epoch
        self.epochEndHeight = epochEndHeight
        self.mode = mode
        self.epochSeed = epochSeed
        self.validatorCount = validatorCount
        self.quorum = quorum
    }

    fileprivate func encode() -> Data {
        sumeragiV2Struct(
            sumeragiV2U64(epoch), sumeragiV2U64(epochEndHeight), mode.encode(),
            epochSeed.bytes, sumeragiV2U32(validatorCount), quorum.encode()
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            epoch: sumeragiV2DecodeU64(reader.field("status context epoch")),
            epochEndHeight: sumeragiV2DecodeU64(reader.field("status context epoch end")),
            mode: SumeragiV2ConsensusMode.decode(reader.field("status context mode")),
            epochSeed: SumeragiV2Bytes32(reader.field("status context epoch seed")),
            validatorCount: sumeragiV2DecodeU32(reader.field("status context validator count")),
            quorum: SumeragiV2DualQuorum.decode(reader.field("status context quorum"))
        )
        try reader.finish("status height context")
        return value
    }
}

/// Power-aware summary of the latest durable CommitQC.
public struct SumeragiV2CommitQCStatus: Equatable, Sendable {
    public let certificate: SumeragiV2QuorumCertificateRef
    public let validatorCount: UInt32
    public let signerCount: UInt32
    public let minSigners: UInt32
    public let signedPower: UInt64
    public let totalPower: UInt64

    public init(
        certificate: SumeragiV2QuorumCertificateRef,
        validatorCount: UInt32,
        signerCount: UInt32,
        minSigners: UInt32,
        signedPower: UInt64,
        totalPower: UInt64
    ) {
        self.certificate = certificate
        self.validatorCount = validatorCount
        self.signerCount = signerCount
        self.minSigners = minSigners
        self.signedPower = signedPower
        self.totalPower = totalPower
    }

    fileprivate func encode() -> Data {
        sumeragiV2Struct(
            certificate.encode(), sumeragiV2U32(validatorCount), sumeragiV2U32(signerCount),
            sumeragiV2U32(minSigners), sumeragiV2U64(signedPower), sumeragiV2U64(totalPower)
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            certificate: SumeragiV2QuorumCertificateRef.decode(reader.field("status commit certificate")),
            validatorCount: sumeragiV2DecodeU32(reader.field("status commit validator count")),
            signerCount: sumeragiV2DecodeU32(reader.field("status commit signer count")),
            minSigners: sumeragiV2DecodeU32(reader.field("status commit min signers")),
            signedPower: sumeragiV2DecodeU64(reader.field("status commit signed power")),
            totalPower: sumeragiV2DecodeU64(reader.field("status commit total power"))
        )
        try reader.finish("status commit QC")
        return value
    }
}

/// Partial dual-quorum state for one exact proposal round.
public struct SumeragiV2VoteQuorumStatus: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let proposalRound: SumeragiV2ConsensusRound
    public let subject: SumeragiV2BlockSubject
    public let executionCommitment: SumeragiV2ExecutionCommitment
    public let signerCount: UInt32
    public let signedPower: UInt64
    public let minSigners: UInt32
    public let totalPower: UInt64

    public init(round: SumeragiV2ConsensusRound, proposalRound: SumeragiV2ConsensusRound,
                subject: SumeragiV2BlockSubject,
                executionCommitment: SumeragiV2ExecutionCommitment, signerCount: UInt32,
                signedPower: UInt64, minSigners: UInt32, totalPower: UInt64) throws {
        guard proposalRound == round else {
            throw SumeragiV2WireError.invalid(
                "Prepare/Commit quorum status proposal round must match its round"
            )
        }
        self.round = round
        self.proposalRound = proposalRound
        self.subject = subject
        self.executionCommitment = executionCommitment
        self.signerCount = signerCount
        self.signedPower = signedPower
        self.minSigners = minSigners
        self.totalPower = totalPower
    }

    fileprivate func encode() -> Data {
        sumeragiV2Struct(round.encode(), proposalRound.encode(), subject.encode(),
                         executionCommitment.encode(), sumeragiV2U32(signerCount),
                         sumeragiV2U64(signedPower),
                         sumeragiV2U32(minSigners), sumeragiV2U64(totalPower))
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("liveness vote round")),
            proposalRound: SumeragiV2ConsensusRound.decode(
                reader.field("liveness vote proposal round")
            ),
            subject: SumeragiV2BlockSubject.decode(reader.field("liveness vote subject")),
            executionCommitment: SumeragiV2ExecutionCommitment.decode(reader.field("liveness vote execution")),
            signerCount: sumeragiV2DecodeU32(reader.field("liveness vote signer count")),
            signedPower: sumeragiV2DecodeU64(reader.field("liveness vote signed power")),
            minSigners: sumeragiV2DecodeU32(reader.field("liveness vote min signers")),
            totalPower: sumeragiV2DecodeU64(reader.field("liveness vote total power"))
        )
        try reader.finish("liveness vote quorum")
        return value
    }
}

/// Partial timeout quorum state for one exact round.
public struct SumeragiV2TimeoutQuorumStatus: Equatable, Sendable {
    public let round: SumeragiV2ConsensusRound
    public let signerCount: UInt32
    public let signedPower: UInt64
    public let minSigners: UInt32
    public let totalPower: UInt64
    public let certificateFormed: Bool

    public init(round: SumeragiV2ConsensusRound, signerCount: UInt32, signedPower: UInt64,
                minSigners: UInt32, totalPower: UInt64, certificateFormed: Bool) {
        self.round = round
        self.signerCount = signerCount
        self.signedPower = signedPower
        self.minSigners = minSigners
        self.totalPower = totalPower
        self.certificateFormed = certificateFormed
    }

    fileprivate func encode() -> Data {
        sumeragiV2Struct(round.encode(), sumeragiV2U32(signerCount), sumeragiV2U64(signedPower),
                         sumeragiV2U32(minSigners), sumeragiV2U64(totalPower),
                         sumeragiV2Bool(certificateFormed))
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            round: SumeragiV2ConsensusRound.decode(reader.field("liveness timeout round")),
            signerCount: sumeragiV2DecodeU32(reader.field("liveness timeout signer count")),
            signedPower: sumeragiV2DecodeU64(reader.field("liveness timeout signed power")),
            minSigners: sumeragiV2DecodeU32(reader.field("liveness timeout min signers")),
            totalPower: sumeragiV2DecodeU64(reader.field("liveness timeout total power")),
            certificateFormed: sumeragiV2DecodeBool(reader.field("liveness timeout formed"))
        )
        try reader.finish("liveness timeout quorum")
        return value
    }
}

/// Durable outbound protocol role retained for fair service.
public enum SumeragiV2OutboundIntentKind: UInt32, Equatable, Sendable {
    case proposal = 0, prepareVote, commitVote, prepareQC, commitQC, timeoutVote, timeoutCertificate
    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }
    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown outbound intent kind \(tag)")
        }
        return value
    }
}

/// Current delivery stage of a durable outbound intent.
public enum SumeragiV2OutboundIntentStage: UInt32, Equatable, Sendable {
    case pendingPersistence = 0, pendingSignature, queued, sent
    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }
    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown outbound intent stage \(tag)")
        }
        return value
    }
}

/// Exact durable outbound intent visible to liveness diagnostics.
public struct SumeragiV2OutboundIntentStatus: Equatable, Sendable {
    public let kind: SumeragiV2OutboundIntentKind
    public let round: SumeragiV2ConsensusRound
    public let proposalRound: SumeragiV2ConsensusRound?
    public let subject: SumeragiV2BlockSubject?
    public let executionCommitment: SumeragiV2ExecutionCommitment?
    public let stage: SumeragiV2OutboundIntentStage

    public init(kind: SumeragiV2OutboundIntentKind, round: SumeragiV2ConsensusRound,
                proposalRound: SumeragiV2ConsensusRound?,
                subject: SumeragiV2BlockSubject?,
                executionCommitment: SumeragiV2ExecutionCommitment?,
                stage: SumeragiV2OutboundIntentStage) throws {
        let shapeIsValid: Bool
        switch kind {
        case .proposal:
            shapeIsValid = proposalRound != nil && subject != nil && executionCommitment == nil
        case .timeoutVote, .timeoutCertificate:
            shapeIsValid = proposalRound == nil && subject == nil && executionCommitment == nil
        case .prepareVote, .commitVote, .prepareQC, .commitQC:
            shapeIsValid = proposalRound != nil && subject != nil && executionCommitment != nil
        }
        guard shapeIsValid else {
            throw SumeragiV2WireError.invalid("invalid outbound intent shape for \(kind)")
        }
        if let proposalRound {
            guard proposalRound.contextID == round.contextID,
                  proposalRound.height == round.height,
                  proposalRound.view <= round.view else {
                throw SumeragiV2WireError.invalid("invalid outbound intent proposal round")
            }
            guard proposalRound == round else {
                throw SumeragiV2WireError.invalid(
                    "Proposal/Prepare/Commit outbound intent origin must match its round"
                )
            }
        }
        self.kind = kind
        self.round = round
        self.proposalRound = proposalRound
        self.subject = subject
        self.executionCommitment = executionCommitment
        self.stage = stage
    }

    fileprivate func encode() -> Data {
        sumeragiV2Struct(kind.encode(), round.encode(),
                         sumeragiV2Option(proposalRound?.encode()),
                         sumeragiV2Option(subject?.encode()),
                         sumeragiV2Option(executionCommitment?.encode()), stage.encode())
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            kind: SumeragiV2OutboundIntentKind.decode(reader.field("liveness outbound kind")),
            round: SumeragiV2ConsensusRound.decode(reader.field("liveness outbound round")),
            proposalRound: sumeragiV2DecodeOption(
                reader.field("liveness outbound proposal round"),
                decode: SumeragiV2ConsensusRound.decode
            ),
            subject: sumeragiV2DecodeOption(reader.field("liveness outbound subject"), decode: SumeragiV2BlockSubject.decode),
            executionCommitment: sumeragiV2DecodeOption(reader.field("liveness outbound execution"), decode: SumeragiV2ExecutionCommitment.decode),
            stage: SumeragiV2OutboundIntentStage.decode(reader.field("liveness outbound stage"))
        )
        try reader.finish("liveness outbound intent")
        return value
    }
}

/// State of one terminating local-work stage.
public enum SumeragiV2LocalWorkStage: UInt32, Equatable, Sendable {
    case idle = 0, queued, running, complete
    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }
    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown local work stage \(tag)")
        }
        return value
    }
}

/// Local body, validation, application, and handoff pipeline.
public struct SumeragiV2WorkStatus: Equatable, Sendable {
    public let candidate: SumeragiV2LocalWorkStage
    public let bodyRecovery: SumeragiV2LocalWorkStage
    public let bodyStore: SumeragiV2LocalWorkStage
    public let validation: SumeragiV2LocalWorkStage
    public let application: SumeragiV2LocalWorkStage
    public let successorHeight: SumeragiV2LocalWorkStage

    public static let idle = Self(candidate: .idle, bodyRecovery: .idle, bodyStore: .idle,
                                  validation: .idle, application: .idle, successorHeight: .idle)

    public init(candidate: SumeragiV2LocalWorkStage, bodyRecovery: SumeragiV2LocalWorkStage,
                bodyStore: SumeragiV2LocalWorkStage, validation: SumeragiV2LocalWorkStage,
                application: SumeragiV2LocalWorkStage,
                successorHeight: SumeragiV2LocalWorkStage) {
        self.candidate = candidate
        self.bodyRecovery = bodyRecovery
        self.bodyStore = bodyStore
        self.validation = validation
        self.application = application
        self.successorHeight = successorHeight
    }

    fileprivate func encode() -> Data {
        sumeragiV2Struct(candidate.encode(), bodyRecovery.encode(), bodyStore.encode(),
                         validation.encode(), application.encode(), successorHeight.encode())
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            candidate: SumeragiV2LocalWorkStage.decode(reader.field("liveness candidate work")),
            bodyRecovery: SumeragiV2LocalWorkStage.decode(reader.field("liveness recovery work")),
            bodyStore: SumeragiV2LocalWorkStage.decode(reader.field("liveness store work")),
            validation: SumeragiV2LocalWorkStage.decode(reader.field("liveness validation work")),
            application: SumeragiV2LocalWorkStage.decode(reader.field("liveness application work")),
            successorHeight: SumeragiV2LocalWorkStage.decode(reader.field("liveness successor work"))
        )
        try reader.finish("liveness work")
        return value
    }
}

/// Identity of a bounded local progress queue.
public enum SumeragiV2QueueKind: UInt32, Equatable, Sendable {
    case ingress = 0, deferredNormal, deferredProgress, deferredCompletion
    case runtimeNormal, runtimeProgress, runtimeCompletion, effectCompletion, networkIngress
    case effectDispatch = 9
    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }
    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown liveness queue kind \(tag)")
        }
        return value
    }
}

/// Occupancy and accumulated oldest-item service debt for one bounded queue.
public struct SumeragiV2QueueStatus: Equatable, Sendable {
    public let queue: SumeragiV2QueueKind
    public let depth: UInt32
    public let capacity: UInt32
    public let oldestAgeMs: UInt64?
    public let serviceDebt: UInt64

    public init(queue: SumeragiV2QueueKind, depth: UInt32, capacity: UInt32,
                oldestAgeMs: UInt64?, serviceDebt: UInt64) {
        self.queue = queue
        self.depth = depth
        self.capacity = capacity
        self.oldestAgeMs = oldestAgeMs
        self.serviceDebt = serviceDebt
    }

    fileprivate func encode() -> Data {
        sumeragiV2Struct(queue.encode(), sumeragiV2U32(depth), sumeragiV2U32(capacity),
                         sumeragiV2Option(oldestAgeMs.map(sumeragiV2U64)), sumeragiV2U64(serviceDebt))
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            queue: SumeragiV2QueueKind.decode(reader.field("liveness queue kind")),
            depth: sumeragiV2DecodeU32(reader.field("liveness queue depth")),
            capacity: sumeragiV2DecodeU32(reader.field("liveness queue capacity")),
            oldestAgeMs: sumeragiV2DecodeOption(reader.field("liveness queue oldest age"), decode: sumeragiV2DecodeU64),
            serviceDebt: sumeragiV2DecodeU64(reader.field("liveness queue service debt"))
        )
        try reader.finish("liveness queue")
        return value
    }
}

/// Semantic reducer transition retained for diagnostics; timeout churn does not
/// reset the separate height-level no-progress clock.
public enum SumeragiV2ProgressTransition: UInt32, Equatable, Sendable {
    case proposalAdmitted = 0, bodyAvailable, bodyStored, bodyValidated
    case prepareVoteAdmitted, commitVoteAdmitted, timeoutVoteAdmitted, prepareQuorum
    case lockInstalled, commitQuorum, timeoutCertificateInstalled, decisionPersisted
    case applied, successorHeightActivated, recoveryReplayed
    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }
    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown progress transition \(tag)")
        }
        return value
    }
}

/// Last tracked reducer transition and its local age.
public struct SumeragiV2ProgressTransitionStatus: Equatable, Sendable {
    public let generation: UInt64
    public let round: SumeragiV2ConsensusRound
    public let transition: SumeragiV2ProgressTransition
    public let ageMs: UInt64

    public init(generation: UInt64, round: SumeragiV2ConsensusRound,
                transition: SumeragiV2ProgressTransition, ageMs: UInt64) {
        self.generation = generation
        self.round = round
        self.transition = transition
        self.ageMs = ageMs
    }

    fileprivate func encode() -> Data {
        sumeragiV2Struct(sumeragiV2U64(generation), round.encode(), transition.encode(),
                         sumeragiV2U64(ageMs))
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            generation: sumeragiV2DecodeU64(reader.field("liveness progress generation")),
            round: SumeragiV2ConsensusRound.decode(reader.field("liveness progress round")),
            transition: SumeragiV2ProgressTransition.decode(reader.field("liveness progress transition")),
            ageMs: sumeragiV2DecodeU64(reader.field("liveness progress age"))
        )
        try reader.finish("liveness progress")
        return value
    }
}

/// Classified cause of an active no-progress interval.
public enum SumeragiV2LivenessBlocker: UInt32, Equatable, Sendable {
    case missingProposal = 0, bodyUnavailable, prepareQuorumMissing, commitQuorumMissing
    case timeoutCertificateMissing, schedulerStarvation, applicationPending
    case localControlPending = 7
    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }
    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown liveness blocker \(tag)")
        }
        return value
    }
}

/// Closed reducer reason for safely ignoring an input.
public enum SumeragiV2IgnoreReason: UInt32, Equatable, Sendable {
    case wrongHeight = 0, wrongView, staleGeneration, busy, duplicate, noMatchingWork
    case observer, viewClosed, alreadyDecided, recoveryPending, irrelevantView
    case unsafeProposal = 11
    fileprivate func encode() -> Data { sumeragiV2U32(rawValue) }
    fileprivate static func decode(_ data: Data) throws -> Self {
        let tag = try sumeragiV2DecodeU32(data)
        guard let value = Self(rawValue: tag) else {
            throw SumeragiV2WireError.invalid("unknown liveness ignore reason \(tag)")
        }
        return value
    }
}

/// Per-height counter for one input-ignore reason.
public struct SumeragiV2IgnoreCount: Equatable, Sendable {
    public let reason: SumeragiV2IgnoreReason
    public let count: UInt64

    public init(reason: SumeragiV2IgnoreReason, count: UInt64) {
        self.reason = reason
        self.count = count
    }

    fileprivate func encode() -> Data { sumeragiV2Struct(reason.encode(), sumeragiV2U64(count)) }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            reason: SumeragiV2IgnoreReason.decode(reader.field("liveness ignore reason")),
            count: sumeragiV2DecodeU64(reader.field("liveness ignore count"))
        )
        try reader.finish("liveness ignore count")
        return value
    }
}

/// Authoritative progress diagnostics for the active height.
public struct SumeragiV2LivenessStatus: Equatable, Sendable {
    public let generation: UInt64
    public let prepareQuorums: [SumeragiV2VoteQuorumStatus]
    public let commitQuorums: [SumeragiV2VoteQuorumStatus]
    public let timeoutQuorums: [SumeragiV2TimeoutQuorumStatus]
    public let outboundIntents: [SumeragiV2OutboundIntentStatus]
    public let work: SumeragiV2WorkStatus
    public let queues: [SumeragiV2QueueStatus]
    public let lastProgress: SumeragiV2ProgressTransitionStatus?
    public let noProgressAgeMs: UInt64
    public let blocker: SumeragiV2LivenessBlocker?
    public let ignoreCounts: [SumeragiV2IgnoreCount]

    public static let empty = Self(generation: 0, prepareQuorums: [], commitQuorums: [],
                                   timeoutQuorums: [], outboundIntents: [], work: .idle,
                                   queues: [], lastProgress: nil, noProgressAgeMs: 0,
                                   blocker: nil, ignoreCounts: [])

    public init(generation: UInt64, prepareQuorums: [SumeragiV2VoteQuorumStatus],
                commitQuorums: [SumeragiV2VoteQuorumStatus],
                timeoutQuorums: [SumeragiV2TimeoutQuorumStatus],
                outboundIntents: [SumeragiV2OutboundIntentStatus], work: SumeragiV2WorkStatus,
                queues: [SumeragiV2QueueStatus], lastProgress: SumeragiV2ProgressTransitionStatus?,
                noProgressAgeMs: UInt64, blocker: SumeragiV2LivenessBlocker?,
                ignoreCounts: [SumeragiV2IgnoreCount]) {
        self.generation = generation
        self.prepareQuorums = prepareQuorums
        self.commitQuorums = commitQuorums
        self.timeoutQuorums = timeoutQuorums
        self.outboundIntents = outboundIntents
        self.work = work
        self.queues = queues
        self.lastProgress = lastProgress
        self.noProgressAgeMs = noProgressAgeMs
        self.blocker = blocker
        self.ignoreCounts = ignoreCounts
    }

    fileprivate func encode() -> Data {
        sumeragiV2Struct(
            sumeragiV2U64(generation), sumeragiV2Vector(prepareQuorums) { $0.encode() },
            sumeragiV2Vector(commitQuorums) { $0.encode() },
            sumeragiV2Vector(timeoutQuorums) { $0.encode() },
            sumeragiV2Vector(outboundIntents) { $0.encode() }, work.encode(),
            sumeragiV2Vector(queues) { $0.encode() }, sumeragiV2Option(lastProgress?.encode()),
            sumeragiV2U64(noProgressAgeMs), sumeragiV2Option(blocker?.encode()),
            sumeragiV2Vector(ignoreCounts) { $0.encode() }
        )
    }

    fileprivate static func decode(_ data: Data) throws -> Self {
        var reader = SumeragiV2Reader(data)
        let value = try Self(
            generation: sumeragiV2DecodeU64(reader.field("liveness generation")),
            prepareQuorums: sumeragiV2DecodeVector(reader.field("liveness prepare"), decode: SumeragiV2VoteQuorumStatus.decode),
            commitQuorums: sumeragiV2DecodeVector(reader.field("liveness commit"), decode: SumeragiV2VoteQuorumStatus.decode),
            timeoutQuorums: sumeragiV2DecodeVector(reader.field("liveness timeout"), decode: SumeragiV2TimeoutQuorumStatus.decode),
            outboundIntents: sumeragiV2DecodeVector(reader.field("liveness outbound"), decode: SumeragiV2OutboundIntentStatus.decode),
            work: SumeragiV2WorkStatus.decode(reader.field("liveness work")),
            queues: sumeragiV2DecodeVector(reader.field("liveness queues"), decode: SumeragiV2QueueStatus.decode),
            lastProgress: sumeragiV2DecodeOption(reader.field("liveness last progress"), decode: SumeragiV2ProgressTransitionStatus.decode),
            noProgressAgeMs: sumeragiV2DecodeU64(reader.field("liveness no progress age")),
            blocker: sumeragiV2DecodeOption(reader.field("liveness blocker"), decode: SumeragiV2LivenessBlocker.decode),
            ignoreCounts: sumeragiV2DecodeVector(reader.field("liveness ignore counts"), decode: SumeragiV2IgnoreCount.decode)
        )
        try reader.finish("Sumeragi v2 liveness status")
        return value
    }
}

/// Compact protocol-v2-only `/v1/sumeragi/status` payload.
public struct SumeragiV2Status: Equatable, Sendable {
    public let protocolVersion: UInt16
    public let nodeFingerprint: SumeragiV2Hash
    public let buildFingerprint: SumeragiV2Hash
    public let configFingerprint: SumeragiV2Hash
    public let restartRequired: Bool
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
    public let heightContext: SumeragiV2HeightContextStatus
    public let lastCommitQC: SumeragiV2CommitQCStatus?
    public let liveness: SumeragiV2LivenessStatus

    public init(
        protocolVersion: UInt16 = SumeragiV2ConsensusMessage.protocolVersion,
        nodeFingerprint: SumeragiV2Hash,
        buildFingerprint: SumeragiV2Hash,
        configFingerprint: SumeragiV2Hash,
        restartRequired: Bool,
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
        lastCommittedSubject: SumeragiV2BlockSubject?,
        heightContext: SumeragiV2HeightContextStatus,
        lastCommitQC: SumeragiV2CommitQCStatus?,
        liveness: SumeragiV2LivenessStatus = .empty
    ) throws {
        guard protocolVersion == SumeragiV2ConsensusMessage.protocolVersion else {
            throw SumeragiV2WireError.invalid("unsupported status protocol version \(protocolVersion)")
        }
        self.protocolVersion = protocolVersion
        self.nodeFingerprint = nodeFingerprint
        self.buildFingerprint = buildFingerprint
        self.configFingerprint = configFingerprint
        self.restartRequired = restartRequired
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
        self.heightContext = heightContext
        self.lastCommitQC = lastCommitQC
        self.liveness = liveness
    }

    public func encode() -> Data {
        sumeragiV2Struct(
            sumeragiV2U16(protocolVersion), nodeFingerprint.bytes, buildFingerprint.bytes,
            configFingerprint.bytes, sumeragiV2Bool(restartRequired), heightContextID.encode(),
            sumeragiV2U64(height),
            sumeragiV2U64(view), phase.encode(), sumeragiV2U32(leader),
            sumeragiV2Option(lockedPrepareQC?.encode()),
            sumeragiV2Option(highestPrepareQC?.encode()),
            sumeragiV2Option(lastTimeoutCertificate?.encode()), bodyState.encode(),
            sumeragiV2Option(pendingPersistenceID.map(sumeragiV2U64)),
            sumeragiV2U64(lastCommittedHeight),
            sumeragiV2Option(lastCommittedSubject?.encode()), heightContext.encode(),
            sumeragiV2Option(lastCommitQC?.encode()), liveness.encode()
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
            restartRequired: sumeragiV2DecodeBool(reader.field("status restart required")),
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
            lastCommittedSubject: sumeragiV2DecodeOption(reader.field("status committed subject"), decode: SumeragiV2BlockSubject.decode),
            heightContext: SumeragiV2HeightContextStatus.decode(reader.field("status height context")),
            lastCommitQC: sumeragiV2DecodeOption(reader.field("status last commit qc"), decode: SumeragiV2CommitQCStatus.decode),
            liveness: SumeragiV2LivenessStatus.decode(reader.field("status liveness"))
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
private func sumeragiV2Bool(_ value: Bool) -> Data { Data([value ? 1 : 0]) }

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

private func sumeragiV2DecodeBool(_ data: Data) throws -> Bool {
    guard data.count == 1, let byte = data.first, byte <= 1 else {
        throw SumeragiV2WireError.invalid("bool must contain one canonical boolean byte")
    }
    return byte == 1
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

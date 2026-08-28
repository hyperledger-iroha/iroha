import Foundation

/// The authenticated ABI-21 artifact generation selected by a V4 operation.
public struct KagemushaRecursiveSpendArtifactBindingV4: Equatable, Hashable, Sendable {
    public let version: UInt16
    public let generation: String
    public let manifestSHA256: Data

    public init(generation: String, manifestSHA256: Data) throws {
        try KagemushaRecursiveSpend.requirePortableArtifactIdentifier(
            generation,
            field: "artifactBindingV4.generation"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            manifestSHA256,
            field: "artifactBindingV4.manifestSHA256"
        )
        version = KagemushaRecursiveSpend.wireVersionV4
        self.generation = generation
        self.manifestSHA256 = Data(manifestSHA256)
    }

    public func noritoEncoded() -> Data {
        KagemushaRecursiveSpendCodecsV4.encodeArtifactBinding(self)
    }
}

/// Wallet-safe public projection of one validated ABI-21 recursive bundle.
///
/// The frozen summary wire intentionally omits the NetworkId. Callers must not
/// use this projection to authenticate a network independently of the native
/// bridge that validated the opaque bundle.
public struct KagemushaRecursiveSpendBundleSummaryV4: Equatable, Sendable {
    public let assetDefinitionID: String
    public let amount: KagemushaScaledAmount
    public let noteCommitment: Data
    public let spendNullifier: Data
    public let hopCount: UInt32
    public let proofStepCount: UInt32
    public let branchClaims: [KagemushaRecursiveSpendBranchClaim]
    public let artifactBinding: KagemushaRecursiveSpendArtifactBindingV4
    public let verifierKeyID: String
    public let bundleDigest: Data

    public func conflicts(with other: Self) -> Bool {
        branchClaims.contains { claim in
            other.branchClaims.contains { claim.conflicts(with: $0) }
        }
    }
}

/// Opaque ABI-21 recursive state. Its archive is decoded only as a V4 bundle.
public struct KagemushaRecursiveSpendBundleV4: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV4 else {
            throw KagemushaRecursiveSpendError.invalidArchive("bundleV4.size")
        }
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.bundleWireNameV4,
            field: "bundleV4"
        )
        self.noritoArchive = Data(noritoArchive)
    }

    /// Ask the ABI-23 bridge to validate the opaque proof carrier and return
    /// only its wallet-safe public projection.
    public func projectedSummary() throws -> KagemushaRecursiveSpendBundleSummaryV4 {
        guard let archive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendBundleSummaryV4(bundleArchive: noritoArchive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendCodecs.decodeBundleSummaryV4(archive)
    }
}

/// Public split fields needed for idempotency and receiver binding. The rest
/// of the transition remains an authenticated opaque Norito carrier.
public struct KagemushaRecursiveSpendSplitIntentV4: Equatable, Sendable {
    public let noritoArchive: Data
    public let recipientRequestDigest: Data
    public let operationID: Data

    init(noritoArchive: Data, recipientRequestDigest: Data, operationID: Data) {
        self.noritoArchive = Data(noritoArchive)
        self.recipientRequestDigest = Data(recipientRequestDigest)
        self.operationID = Data(operationID)
    }
}

/// Recipient-only ABI-21 peer envelope. Sender change is never projected.
public struct KagemushaRecursiveSpendPeerPaymentV4: Equatable, Sendable {
    public let recipientBundle: KagemushaRecursiveSpendBundleV4
    public let recipientMembershipWitness: KagemushaNoteMembershipWitness
    public let topUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4
    public let noritoArchive: Data

    /// Canonical recipient-only bytes used by the shared peer transports.
    public var archive: Data { noritoArchive }

    public func noritoEncoded() -> Data { noritoArchive }

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV4 else {
            throw KagemushaRecursiveSpendError.invalidArchive("peerPaymentV4.size")
        }
        guard let canonical = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendPeerPaymentValidateV4(paymentArchive: noritoArchive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        guard canonical == noritoArchive else {
            throw KagemushaRecursiveSpendError.invalidArchive("peerPaymentV4.canonical")
        }
        self = try KagemushaRecursiveSpendCodecs.decodePeerPaymentV4(canonical)
    }

    init(
        recipientBundle: KagemushaRecursiveSpendBundleV4,
        recipientMembershipWitness: KagemushaNoteMembershipWitness,
        topUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4,
        noritoArchive: Data
    ) {
        self.recipientBundle = recipientBundle
        self.recipientMembershipWitness = recipientMembershipWitness
        self.topUpProvenance = topUpProvenance
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Finalized top-up receipt whose public statement selects an ABI-21 release.
public struct KagemushaRecursiveSpendTopUpAnchorV4: Equatable, Sendable {
    public let version: UInt16
    public let networkID: NetworkId
    public let topUpOperationID: Data
    public let artifactBinding: KagemushaRecursiveSpendArtifactBindingV4
    public let finalizedHeight: UInt64
    public let finalizedTransactionHash: Data
    public let anchorDigest: Data
    public let noritoArchive: Data

    public init(
        noritoArchive: Data,
        chainDiscriminant: UInt16
    ) throws {
        self = try KagemushaRecursiveSpendCodecs.decodeTopUpAnchorV4(
            noritoArchive,
            chainDiscriminant: chainDiscriminant
        )
    }

    init(
        networkID: NetworkId,
        topUpOperationID: Data,
        artifactBinding: KagemushaRecursiveSpendArtifactBindingV4,
        finalizedHeight: UInt64,
        finalizedTransactionHash: Data,
        anchorDigest: Data,
        noritoArchive: Data
    ) {
        version = KagemushaRecursiveSpend.wireVersionV4
        self.networkID = networkID
        self.topUpOperationID = Data(topUpOperationID)
        self.artifactBinding = artifactBinding
        self.finalizedHeight = finalizedHeight
        self.finalizedTransactionHash = Data(finalizedTransactionHash)
        self.anchorDigest = Data(anchorDigest)
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Complete V4 origin evidence. The consensus proof remains the stable V2 leaf.
public struct KagemushaRecursiveSpendTopUpFinalityEvidenceV4: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        guard noritoArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityProofMaximumArchiveBytes
                    + KagemushaRecursiveSpend.topUpFinalityAnchorMaximumArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "topUpFinalityEvidenceV4.size"
            )
        }
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpFinalityEvidenceWireNameV4,
            field: "topUpFinalityEvidenceV4"
        )
        self.noritoArchive = Data(noritoArchive)
    }

    public init(
        topUpAnchor: KagemushaRecursiveSpendTopUpAnchorV4,
        topUpFinalityProof: KagemushaTopUpFinalityProofArchive
    ) throws {
        try self.init(noritoArchive: KagemushaRecursiveSpendCodecsV4
            .encodeTopUpFinalityEvidence(
                topUpAnchor: topUpAnchor,
                topUpFinalityProof: topUpFinalityProof
            ))
    }
}

/// Canonical, authenticated origin inventory carried by every ABI-21 branch.
/// Evidence ordering is consensus-visible and is never normalized by the SDK.
public struct KagemushaRecursiveSpendTopUpProvenanceV4: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.maximumTopUpProvenanceArchiveBytesV4 else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpProvenanceV4.size")
        }
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpProvenanceWireNameV4,
            field: "topUpProvenanceV4"
        )
        self.noritoArchive = Data(noritoArchive)
    }

    /// Build the single-origin provenance of a newly initialized branch. The
    /// bridge verifies the roster, anchor, proof, installed release, and block
    /// context before returning canonical bytes.
    public static func build(
        for bundle: KagemushaRecursiveSpendBundleV4,
        roster: KagemushaTopUpFinalityRosterArtifactArchive,
        anchor: KagemushaRecursiveSpendTopUpAnchorV4,
        finalityProof: KagemushaTopUpFinalityProofArchive,
        blockHeight: UInt64
    ) throws -> Self {
        guard blockHeight > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("topUpProvenanceV4.blockHeight")
        }
        guard let canonical = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendTopUpProvenanceBuildV4(
                bundleArchive: bundle.noritoArchive,
                rosterArchive: roster.noritoArchive,
                anchorArchive: anchor.noritoArchive,
                finalityProofArchive: finalityProof.noritoArchive,
                blockHeight: blockHeight
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try Self(noritoArchive: canonical)
    }

    /// Revalidate persisted or received provenance against its exact branch
    /// and the currently installed authenticated release.
    public func validated(
        for bundle: KagemushaRecursiveSpendBundleV4,
        blockHeight: UInt64
    ) throws -> Self {
        guard blockHeight > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("topUpProvenanceV4.blockHeight")
        }
        guard let canonical = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendTopUpProvenanceValidateV4(
                bundleArchive: bundle.noritoArchive,
                provenanceArchive: noritoArchive,
                blockHeight: blockHeight
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        guard canonical == noritoArchive else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpProvenanceV4.canonical")
        }
        return try Self(noritoArchive: canonical)
    }
}

/// Local-only ABI-21 shield proof request. It contains note secrets and must
/// never be persisted or sent to Torii.
public struct KagemushaTopUpShieldBuildRequestV4: Equatable, Sendable {
    public let version: UInt16
    public let networkID: NetworkId
    public let assetID: String
    public let amount: KagemushaScaledAmount
    public let payer: String
    public let operationID: Data
    public let opening: KagemushaNoteOpening
    public let leafIndex: UInt32
    public let zeroPath: PrivacyConfidentialMerklePathWitnessV2
    public let shieldVerifierID: String
    public let shieldVerifierCommitment: Data
    public let artifactBinding: KagemushaRecursiveSpendArtifactBindingV4

    public init(
        networkID: NetworkId,
        assetID: String,
        amount: KagemushaScaledAmount,
        payer: String,
        operationID: Data,
        opening: KagemushaNoteOpening,
        leafIndex: UInt32,
        zeroPath: PrivacyConfidentialMerklePathWitnessV2,
        shieldVerifierID: String,
        shieldVerifierCommitment: Data,
        artifactBinding: KagemushaRecursiveSpendArtifactBindingV4
    ) throws {
        let canonicalAssetID = try KagemushaRecursiveSpendCodecs.canonicalAssetID(assetID)
        let assetParts = canonicalAssetID.split(separator: "#", omittingEmptySubsequences: false)
        guard assetParts.count == 2 || assetParts.count == 3,
              String(assetParts[1]) == payer,
              leafIndex < UInt32(ToriiZkMerklePathResponse.confidentialTreeCapacityV2),
              zeroPath.root.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidField("topUpShieldBuildRequestV4")
        }
        try KagemushaRecursiveSpend.requirePortableText(payer, field: "payer")
        try KagemushaRecursiveSpend.requirePortableText(
            shieldVerifierID,
            field: "shieldVerifierID"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            operationID,
            field: "operationID"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            shieldVerifierCommitment,
            field: "shieldVerifierCommitment"
        )
        version = KagemushaRecursiveSpend.localWitnessVersionV4
        self.networkID = networkID
        self.assetID = canonicalAssetID
        self.amount = amount
        self.payer = payer
        self.operationID = Data(operationID)
        self.opening = opening
        self.leafIndex = leafIndex
        self.zeroPath = zeroPath
        self.shieldVerifierID = shieldVerifierID
        self.shieldVerifierCommitment = Data(shieldVerifierCommitment)
        self.artifactBinding = artifactBinding
    }

    public func buildUnsigned() throws -> KagemushaRecursiveSpendTopUpUnsignedV4 {
        let chainDiscriminant = try KagemushaRecursiveSpend.canonicalAccountAddress(
            payer,
            field: "topUpShieldBuildRequestV4.payer"
        ).chainDiscriminant
        var archive = try KagemushaRecursiveSpendCodecs
            .encodeTopUpShieldBuildRequestV4(self)
        defer { archive.resetBytes(in: 0..<archive.count) }
        do {
            guard let result = try NoritoNativeBridge.shared
                .kagemushaTopUpShieldBuildUnsignedV4(requestArchive: archive) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            return try KagemushaRecursiveSpendCodecs.decodeTopUpUnsignedV4(
                result,
                chainDiscriminant: chainDiscriminant
            )
        } catch NativeBridgeError.kagemushaBusy {
            throw KagemushaRecursiveSpendError.proofWorkerBusy
        }
    }
}

/// Canonical unsigned ABI-21 online-to-offline request fields.
public struct KagemushaRecursiveSpendTopUpUnsignedV4: Equatable, Sendable {
    public let version: UInt16
    public let assetID: String
    public let amount: KagemushaScaledAmount
    public let currentNote: KagemushaSpendableNoteDescriptor
    public let shieldEvidence: KagemushaTopUpShieldEvidence
    public let artifactBinding: KagemushaRecursiveSpendArtifactBindingV4
    public let operationID: Data
    public let noritoArchive: Data

    init(
        assetID: String,
        amount: KagemushaScaledAmount,
        currentNote: KagemushaSpendableNoteDescriptor,
        shieldEvidence: KagemushaTopUpShieldEvidence,
        artifactBinding: KagemushaRecursiveSpendArtifactBindingV4,
        operationID: Data,
        noritoArchive: Data
    ) {
        version = KagemushaRecursiveSpend.wireVersionV4
        self.assetID = assetID
        self.amount = amount
        self.currentNote = currentNote
        self.shieldEvidence = shieldEvidence
        self.artifactBinding = artifactBinding
        self.operationID = Data(operationID)
        self.noritoArchive = Data(noritoArchive)
    }

    public func authorizationPayloadDigest() throws -> Data {
        guard let digest = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendTopUpUnsignedPayloadDigestV4(
                unsignedArchive: noritoArchive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            digest,
            field: "topUpUnsignedV4.payloadDigest"
        )
        return digest
    }

    public func finalize(
        authorization: KagemushaRequestAuthorization
    ) throws -> KagemushaRecursiveSpendTopUpRequestV4 {
        guard authorization.fields.operationID == operationID,
              authorization.fields.payloadDigest == (try authorizationPayloadDigest()),
              let requestArchive = try NoritoNativeBridge.shared
                .kagemushaRecursiveSpendTopUpFinalizeRequestV4(
                    unsignedArchive: noritoArchive,
                    authorizationArchive: authorization.archive
                ) else {
            throw KagemushaRecursiveSpendError.invalidField("authorization")
        }
        return try KagemushaRecursiveSpendTopUpRequestV4(
            unsigned: self,
            authorization: authorization,
            noritoArchive: requestArchive
        )
    }
}

/// Authoritative ABI-21 Torii top-up request.
public struct KagemushaRecursiveSpendTopUpRequestV4: Equatable, Sendable {
    public let unsigned: KagemushaRecursiveSpendTopUpUnsignedV4
    public let authorization: KagemushaRequestAuthorization
    public let noritoArchive: Data

    init(
        unsigned: KagemushaRecursiveSpendTopUpUnsignedV4,
        authorization: KagemushaRequestAuthorization,
        noritoArchive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            field: "topUpRequestV4"
        )
        self.unsigned = unsigned
        self.authorization = authorization
        self.noritoArchive = Data(noritoArchive)
    }
}

/// One output insertion path owned exclusively by the ABI-21 local carrier.
public struct KagemushaOutputMembershipLeafPathsV4: Equatable, Sendable {
    public let leafIndex: UInt32
    public let updatePath: PrivacyConfidentialMerklePathWitnessV2
    public let membershipPath: PrivacyConfidentialMerklePathWitnessV2

    public init(
        leafIndex: UInt32,
        updatePath: PrivacyConfidentialMerklePathWitnessV2,
        membershipPath: PrivacyConfidentialMerklePathWitnessV2
    ) throws {
        guard leafIndex < UInt32(PrivacyConfidentialWitnessCodecs.confidentialTreeCapacityV2),
              updatePath.root.contains(where: { $0 != 0 }),
              membershipPath.root.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidField("outputMembershipV4.leaf")
        }
        for path in [updatePath, membershipPath] {
            for (level, direction) in path.directions.enumerated()
                where direction != UInt8((UInt64(leafIndex) >> UInt64(level)) & 1)
            {
                throw KagemushaRecursiveSpendError.invalidField(
                    "outputMembershipV4.leaf.directions"
                )
            }
        }
        self.leafIndex = leafIndex
        self.updatePath = updatePath
        self.membershipPath = membershipPath
    }
}

/// Exact output-update witness decoded only by the ABI-23 bridge.
public struct KagemushaOutputMembershipPathsV4: Equatable, Sendable {
    public let initialRoot: Data
    public let finalRoot: Data
    public let recipient: KagemushaOutputMembershipLeafPathsV4?
    public let change: KagemushaOutputMembershipLeafPathsV4?
    public let dummyLeafIndex: UInt32
    public let dummyPath: PrivacyConfidentialMerklePathWitnessV2

    public init(
        initialRoot: Data,
        finalRoot: Data,
        recipient: KagemushaOutputMembershipLeafPathsV4?,
        change: KagemushaOutputMembershipLeafPathsV4?,
        dummyLeafIndex: UInt32,
        dummyPath: PrivacyConfidentialMerklePathWitnessV2
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            initialRoot,
            field: "outputMembershipV4.initialRoot"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            finalRoot,
            field: "outputMembershipV4.finalRoot"
        )
        guard initialRoot != finalRoot,
              recipient != nil || change != nil,
              dummyLeafIndex
                < UInt32(PrivacyConfidentialWitnessCodecs.confidentialTreeCapacityV2),
              dummyPath.root == finalRoot,
              recipient.map({ $0.membershipPath.root == finalRoot }) ?? true,
              change.map({ $0.membershipPath.root == finalRoot }) ?? true,
              recipient.map({ $0.updatePath.root == initialRoot })
                ?? change.map({ $0.updatePath.root == initialRoot })
                ?? false else {
            throw KagemushaRecursiveSpendError.invalidField("outputMembershipV4")
        }
        for (level, direction) in dummyPath.directions.enumerated()
            where direction != UInt8((UInt64(dummyLeafIndex) >> UInt64(level)) & 1)
        {
            throw KagemushaRecursiveSpendError.invalidField(
                "outputMembershipV4.dummyPath.directions"
            )
        }
        let occupied = [recipient?.leafIndex, change?.leafIndex, dummyLeafIndex]
            .compactMap { $0 }
        guard Set(occupied).count == occupied.count else {
            throw KagemushaRecursiveSpendError.invalidField(
                "outputMembershipV4.leafIndex"
            )
        }
        if let recipient, let change {
            let next = recipient.leafIndex.addingReportingOverflow(1)
            guard !next.overflow, next.partialValue == change.leafIndex else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "outputMembershipV4.change"
                )
            }
        }
        guard let lastOutputIndex = change?.leafIndex ?? recipient?.leafIndex else {
            throw KagemushaRecursiveSpendError.invalidField("outputMembershipV4")
        }
        let nextZero = lastOutputIndex.addingReportingOverflow(1)
        guard !nextZero.overflow, nextZero.partialValue == dummyLeafIndex else {
            throw KagemushaRecursiveSpendError.invalidField(
                "outputMembershipV4.dummyLeafIndex"
            )
        }
        self.initialRoot = Data(initialRoot)
        self.finalRoot = Data(finalRoot)
        self.recipient = recipient
        self.change = change
        self.dummyLeafIndex = dummyLeafIndex
        self.dummyPath = dummyPath
    }

    public init(noritoArchive: Data) throws {
        self = try KagemushaRecursiveSpendCodecs.decodeOutputMembershipPathsV4(
            noritoArchive
        )
    }
}

/// Authenticated next-zero cursor. Wallets persist these canonical bytes with
/// every branch and compare them to the frontier returned by native restore
/// validation before allowing the branch to become spendable.
public struct KagemushaOutputMembershipFrontierV4: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.maximumOutputMembershipFrontierArchiveBytesV4 else {
            throw KagemushaRecursiveSpendError.invalidArchive("outputMembershipFrontierV4.size")
        }
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.outputMembershipFrontierWireNameV4,
            field: "outputMembershipFrontierV4"
        )
        self.noritoArchive = Data(noritoArchive)
    }

    public static func build(
        leafIndex: UInt32,
        zeroPath: PrivacyConfidentialMerklePathWitnessV2
    ) throws -> Self {
        guard let archive = try NoritoNativeBridge.shared
            .kagemushaOutputMembershipFrontierBuildV4(
                leafIndex: leafIndex,
                zeroPath: zeroPath
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try Self(noritoArchive: archive)
    }

    public func derivePaths(
        recipientCommitment: Data?,
        changeCommitment: Data?
    ) throws -> KagemushaOutputMembershipPathsV4 {
        guard let archive = try NoritoNativeBridge.shared
            .kagemushaOutputMembershipPathsDeriveV4(
                frontierArchive: noritoArchive,
                recipientCommitment: recipientCommitment,
                changeCommitment: changeCommitment
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaOutputMembershipPathsV4(noritoArchive: archive)
    }
}

/// Canonical ABI-21 initialization request before local secret witnesses are added.
public struct KagemushaRecursiveSpendInitRequestV4: Equatable, Sendable {
    public let topUpAnchor: KagemushaRecursiveSpendTopUpAnchorV4
    public let topUpFinalityProof: KagemushaTopUpFinalityProofArchive
    public let topUpFinalityRosterArtifact: KagemushaTopUpFinalityRosterArtifactArchive
    public let artifactBinding: KagemushaRecursiveSpendArtifactBindingV4

    public init(
        topUpAnchor: KagemushaRecursiveSpendTopUpAnchorV4,
        topUpFinalityProof: KagemushaTopUpFinalityProofArchive,
        topUpFinalityRosterArtifact: KagemushaTopUpFinalityRosterArtifactArchive,
        artifactBinding: KagemushaRecursiveSpendArtifactBindingV4
    ) {
        self.topUpAnchor = topUpAnchor
        self.topUpFinalityProof = topUpFinalityProof
        self.topUpFinalityRosterArtifact = topUpFinalityRosterArtifact
        self.artifactBinding = artifactBinding
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecsV4.encodeInitRequest(self)
    }
}

/// Secret-bearing local ABI-21 initialization input.
public struct KagemushaRecursiveSpendInitLocalRequestV4: Equatable, Sendable {
    public let request: KagemushaRecursiveSpendInitRequestV4
    public let opening: KagemushaNoteOpening
    public let outputMembershipPaths: KagemushaOutputMembershipPathsV4

    public var artifactBinding: KagemushaRecursiveSpendArtifactBindingV4 {
        request.artifactBinding
    }

    public init(
        request: KagemushaRecursiveSpendInitRequestV4,
        opening: KagemushaNoteOpening,
        outputMembershipPaths: KagemushaOutputMembershipPathsV4
    ) throws {
        guard outputMembershipPaths.recipient != nil,
              outputMembershipPaths.change == nil else {
            throw KagemushaRecursiveSpendError.invalidField("initOutputMembershipV4")
        }
        self.request = request
        self.opening = opening
        self.outputMembershipPaths = outputMembershipPaths
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecsV4.encodeInitLocalRequest(self)
    }
}

/// One genuine ABI-21 previous-proof package.
public struct KagemushaRecursiveSpendAppendInputV4: Equatable, Sendable {
    public let previousBundle: KagemushaRecursiveSpendBundleV4
    public let topUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4

    public init(
        previousBundle: KagemushaRecursiveSpendBundleV4,
        topUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4
    ) {
        self.previousBundle = previousBundle
        self.topUpProvenance = topUpProvenance
    }
}

/// Secret-bearing spendable local state used only by ABI-21 builders.
public struct KagemushaRecursiveSpendSpendableBranchV4: Equatable, Sendable {
    public let bundle: KagemushaRecursiveSpendBundleV4
    public let membershipWitness: KagemushaNoteMembershipWitness
    public let opening: KagemushaNoteOpening
    public let topUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4

    public init(
        bundle: KagemushaRecursiveSpendBundleV4,
        membershipWitness: KagemushaNoteMembershipWitness,
        opening: KagemushaNoteOpening,
        topUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4
    ) {
        self.bundle = bundle
        self.membershipWitness = membershipWitness
        self.opening = opening
        self.topUpProvenance = topUpProvenance
    }

    /// Reauthenticate every persisted component and recover the only frontier
    /// that may be used for the next output insertion.
    public func validatedFrontier(blockHeight: UInt64) throws
        -> KagemushaOutputMembershipFrontierV4
    {
        guard blockHeight > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("branchV4.blockHeight")
        }
        var openingArchive = try opening.noritoEncoded()
        defer { openingArchive.resetBytes(in: 0..<openingArchive.count) }
        let witnessArchive = try membershipWitness.noritoEncoded()
        do {
            guard let frontier = try NoritoNativeBridge.shared
                .kagemushaRecursiveSpendBranchValidateV4(
                    bundleArchive: bundle.noritoArchive,
                    provenanceArchive: topUpProvenance.noritoArchive,
                    witnessArchive: witnessArchive,
                    openingArchive: openingArchive,
                    blockHeight: blockHeight
                ) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            return try KagemushaOutputMembershipFrontierV4(noritoArchive: frontier)
        } catch NativeBridgeError.kagemushaBusy {
            throw KagemushaRecursiveSpendError.proofWorkerBusy
        }
    }
}

/// Exact local bridge request for native-derived partial-redemption change.
/// The archive contains the input opening and is wiped immediately after use.
struct KagemushaRecursiveSpendRedemptionChangePrepareRequestV4: Equatable, Sendable {
    let version: UInt16
    let bundle: KagemushaRecursiveSpendBundleV4
    let inputOpening: KagemushaNoteOpening
    let changeAmount: KagemushaScaledAmount
    let operationID: Data
    let entropy: Data

    init(
        bundle: KagemushaRecursiveSpendBundleV4,
        inputOpening: KagemushaNoteOpening,
        changeAmount: KagemushaScaledAmount,
        operationID: Data,
        entropy: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            operationID,
            field: "redemptionChangeV4.operationID"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            entropy,
            field: "redemptionChangeV4.entropy"
        )
        guard operationID != entropy else {
            throw KagemushaRecursiveSpendError.invalidField("redemptionChangeV4.entropy")
        }
        version = KagemushaRecursiveSpend.wireVersionV4
        self.bundle = bundle
        self.inputOpening = inputOpening
        self.changeAmount = changeAmount
        self.operationID = Data(operationID)
        self.entropy = Data(entropy)
    }
}

/// Native-derived secret opening and complete public descriptor for a
/// partial-redemption change note.
public struct KagemushaRecursiveSpendRedemptionChangePreparationV4:
    Equatable, Sendable
{
    public let opening: KagemushaNoteOpening
    /// Complete descriptor returned by the native bridge. Swift rechecks every
    /// binding exposed by `KagemushaRecursiveSpendBundleSummaryV4`; because the
    /// frozen summary omits network ID, `output.networkID` remains native-authenticated
    /// rather than independently authenticated by this Swift result decoder.
    public let output: KagemushaSpendableNoteDescriptor
    /// Exact public unshield amount: input amount minus the private change output.
    public let publicAmount: KagemushaScaledAmount

    init(
        opening: KagemushaNoteOpening,
        output: KagemushaSpendableNoteDescriptor,
        inputOpening: KagemushaNoteOpening,
        inputSummary: KagemushaRecursiveSpendBundleSummaryV4,
        changeAmount: KagemushaScaledAmount
    ) throws {
        guard opening.spendKey == inputOpening.spendKey,
              opening.rho != inputOpening.rho,
              opening.rho != opening.diversifier,
              opening.diversifier == (try ConfidentialOwnerTag.defaultDiversifier()),
              output.assetDefinitionID == inputSummary.assetDefinitionID,
              output.amount == changeAmount,
              output.noteCommitment != inputSummary.noteCommitment,
              output.spendNullifier != inputSummary.spendNullifier,
              let publicAtomicUnits = KagemushaScaledAmount.subtractAtomicUnits(
                  output.amount.atomicUnits,
                  from: inputSummary.amount.atomicUnits
              ) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "redemptionChangePrepareResultV4.binding"
            )
        }
        self.opening = opening
        self.output = output
        publicAmount = try KagemushaScaledAmount(
            atomicUnits: publicAtomicUnits,
            scale: inputSummary.amount.scale
        )
    }
}

/// Secret-bearing local request for ordinary peer-split change preparation.
struct KagemushaRecursiveSpendPeerSplitChangePrepareRequestV4: Equatable, Sendable {
    let version: UInt16
    let bundles: [KagemushaRecursiveSpendBundleV4]
    let inputOpenings: [KagemushaNoteOpening]
    let recipientRequest: KagemushaRecipientPaymentRequest
    let changeAmount: KagemushaScaledAmount
    let operationID: Data
    let entropy: Data

    init(
        inputs: [KagemushaRecursiveSpendSpendableBranchV4],
        recipientRequest: KagemushaRecipientPaymentRequest,
        changeAmount: KagemushaScaledAmount,
        operationID: Data,
        entropy: Data
    ) throws {
        guard (1...KagemushaRecursiveSpend.maximumInputsPerTransition).contains(inputs.count) else {
            throw KagemushaRecursiveSpendError.invalidField("peerSplitChangeV4.inputs")
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            operationID,
            field: "peerSplitChangeV4.operationID"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            entropy,
            field: "peerSplitChangeV4.entropy"
        )
        guard operationID != entropy else {
            throw KagemushaRecursiveSpendError.invalidField("peerSplitChangeV4.entropy")
        }
        version = KagemushaRecursiveSpend.wireVersionV4
        bundles = inputs.map(\.bundle)
        inputOpenings = inputs.map(\.opening)
        self.recipientRequest = recipientRequest
        self.changeAmount = changeAmount
        self.operationID = Data(operationID)
        self.entropy = Data(entropy)
    }
}

/// Owned native-derived sender change for an ordinary peer split.
public struct KagemushaRecursiveSpendPeerSplitChangePreparationV4: Equatable, Sendable {
    public let opening: KagemushaNoteOpening
    public let output: KagemushaSpendableNoteDescriptor
    public let amount: KagemushaScaledAmount

    init(
        opening: KagemushaNoteOpening,
        output: KagemushaSpendableNoteDescriptor,
        inputOpenings: [KagemushaNoteOpening],
        inputSummaries: [KagemushaRecursiveSpendBundleSummaryV4],
        recipientRequest: KagemushaRecipientPaymentRequest,
        changeAmount: KagemushaScaledAmount
    ) throws {
        let total = try KagemushaScaledAmount.sum(inputSummaries.map(\.amount))
        let conserved = try recipientRequest.payload.amount.adding(changeAmount)
        guard total == conserved,
              output.networkID == recipientRequest.payload.networkID,
              output.assetDefinitionID == recipientRequest.payload.assetDefinitionID,
              output.amount == changeAmount,
              inputSummaries.allSatisfy({ $0.assetDefinitionID == output.assetDefinitionID }),
              !inputSummaries.contains(where: {
                  $0.noteCommitment == output.noteCommitment
                      || $0.spendNullifier == output.spendNullifier
              }),
              output.noteCommitment != recipientRequest.payload.recipientOutput.noteCommitment,
              output.spendNullifier != recipientRequest.payload.recipientOutput.spendNullifier,
              !inputOpenings.contains(where: {
                  $0.spendKey == opening.spendKey || $0.rho == opening.rho
              }),
              opening.rho != opening.diversifier,
              opening.diversifier == (try ConfidentialOwnerTag.defaultDiversifier()) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "peerSplitChangePrepareResultV4.binding"
            )
        }
        self.opening = opening
        self.output = output
        amount = changeAmount
    }
}

/// Secret-bearing ABI-23 append input. It encodes the flat V4 bridge carrier,
/// not a version wrapper around the frozen request.
public struct KagemushaRecursiveSpendAppendLocalRequestV4: Equatable, Sendable {
    public let previousInputs: [KagemushaRecursiveSpendAppendInputV4]
    public let inputOpenings: [KagemushaNoteOpening]
    public let inputMembershipWitnesses: [KagemushaNoteMembershipWitness]
    public let changeOpening: KagemushaNoteOpening?
    public let outputMembershipPaths: KagemushaOutputMembershipPathsV4
    public let outputArtifactBinding: KagemushaRecursiveSpendArtifactBindingV4
    public let transferVerifier: KagemushaConfidentialVerifierBinding
    public let operationID: Data
    public let blockHeight: UInt64

    public init(
        inputs: [KagemushaRecursiveSpendSpendableBranchV4],
        changeOpening: KagemushaNoteOpening? = nil,
        outputMembershipPaths: KagemushaOutputMembershipPathsV4,
        outputArtifactBinding: KagemushaRecursiveSpendArtifactBindingV4,
        transferVerifier: KagemushaConfidentialVerifierBinding,
        operationID: Data,
        blockHeight: UInt64
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            operationID,
            field: "appendRequestV4.operationID"
        )
        guard (1...KagemushaRecursiveSpend.maximumInputsPerTransition)
                .contains(inputs.count),
              Set(inputs.map { $0.bundle.noritoArchive }).count == inputs.count,
              outputMembershipPaths.recipient != nil,
              (outputMembershipPaths.change != nil) == (changeOpening != nil),
              transferVerifier.role == .transfer,
              transferVerifier.blockHeight == blockHeight,
              blockHeight > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("appendRequestV4")
        }
        self.previousInputs = inputs.map {
            KagemushaRecursiveSpendAppendInputV4(
                previousBundle: $0.bundle,
                topUpProvenance: $0.topUpProvenance
            )
        }
        self.inputOpenings = inputs.map(\.opening)
        self.inputMembershipWitnesses = inputs.map(\.membershipWitness)
        self.changeOpening = changeOpening
        self.outputMembershipPaths = outputMembershipPaths
        self.outputArtifactBinding = outputArtifactBinding
        self.transferVerifier = transferVerifier
        self.operationID = Data(operationID)
        self.blockHeight = blockHeight
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecsV4.encodeAppendLocalRequest(self)
    }
}

/// Encrypted-at-rest, crash-safe form of an already constructed local append
/// carrier. The carrier remains opaque because it contains note openings; the
/// native ABI-21 decoder revalidates every field before proving. Callers bind
/// it to the authenticated artifact release that was durable with the wallet
/// reservation and must verify the returned operation/request digests.
public struct KagemushaRecursiveSpendPersistedAppendLocalRequestV4: Equatable, Sendable {
    public let noritoArchive: Data
    public let artifactBinding: KagemushaRecursiveSpendArtifactBindingV4

    public init(
        noritoArchive: Data,
        artifactBinding: KagemushaRecursiveSpendArtifactBindingV4
    ) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV4 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "persistedAppendLocalRequestV4.size"
            )
        }
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.appendLocalRequestWireNameV4,
            field: "persistedAppendLocalRequestV4"
        )
        self.noritoArchive = Data(noritoArchive)
        self.artifactBinding = artifactBinding
    }
}

/// Canonical ABI-21 receiver-verification request.
public struct KagemushaRecursiveSpendVerifyRequestV4: Equatable, Sendable {
    public let bundle: KagemushaRecursiveSpendBundleV4
    public let recipientRequest: KagemushaRecipientPaymentRequest
    public let topUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4
    public let maximumHops: UInt32
    public let artifactBinding: KagemushaRecursiveSpendArtifactBindingV4
    public let blockHeight: UInt64
    public let verifiedAtMilliseconds: UInt64

    public init(
        bundle: KagemushaRecursiveSpendBundleV4,
        recipientRequest: KagemushaRecipientPaymentRequest,
        topUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4,
        maximumHops: UInt32 = KagemushaRecursiveSpend.maximumPeerHops,
        artifactBinding: KagemushaRecursiveSpendArtifactBindingV4,
        blockHeight: UInt64,
        verifiedAtMilliseconds: UInt64
    ) throws {
        guard maximumHops == KagemushaRecursiveSpend.maximumPeerHops,
              blockHeight > 0,
              verifiedAtMilliseconds > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("verifyRequestV4")
        }
        self.bundle = bundle
        self.recipientRequest = recipientRequest
        self.topUpProvenance = topUpProvenance
        self.maximumHops = maximumHops
        self.artifactBinding = artifactBinding
        self.blockHeight = blockHeight
        self.verifiedAtMilliseconds = verifiedAtMilliseconds
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecsV4.encodeVerifyRequest(self)
    }
}

/// Explicit ABI-21 local verification carrier.
public struct KagemushaRecursiveSpendVerifyLocalRequestV4: Equatable, Sendable {
    public let request: KagemushaRecursiveSpendVerifyRequestV4

    public init(request: KagemushaRecursiveSpendVerifyRequestV4) {
        self.request = request
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecsV4.encodeVerifyLocalRequest(self)
    }
}

/// Secret-bearing ABI-21 redemption input. Native derives the V4 public
/// redemption transition and proof; neither is caller-supplied.
public struct KagemushaRecursiveSpendRedeemLocalRequestV4: Equatable, Sendable {
    public let input: KagemushaRecursiveSpendSpendableBranchV4
    public let recipient: String
    public let publicAmount: KagemushaScaledAmount
    public let changeOpening: KagemushaNoteOpening?
    public let changeOutputMembershipPaths: KagemushaOutputMembershipPathsV4?
    public let unshieldVerifier: KagemushaConfidentialVerifierBinding
    public let blockHeight: UInt64
    public let operationID: Data

    public init(
        input: KagemushaRecursiveSpendSpendableBranchV4,
        recipient: String,
        publicAmount: KagemushaScaledAmount,
        changeOpening: KagemushaNoteOpening? = nil,
        changeOutputMembershipPaths: KagemushaOutputMembershipPathsV4? = nil,
        unshieldVerifier: KagemushaConfidentialVerifierBinding,
        blockHeight: UInt64,
        operationID: Data
    ) throws {
        _ = try KagemushaRecursiveSpend.canonicalAccountAddress(
            recipient,
            field: "redeemRequestV4.recipient"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            operationID,
            field: "redeemRequestV4.operationID"
        )
        guard (changeOpening != nil) == (changeOutputMembershipPaths != nil),
              changeOutputMembershipPaths.map({
                  $0.recipient == nil && $0.change != nil
              }) ?? true,
              unshieldVerifier.role == .unshield,
              unshieldVerifier.blockHeight == blockHeight,
              blockHeight > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("redeemRequestV4")
        }
        self.input = input
        self.recipient = recipient
        self.publicAmount = publicAmount
        self.changeOpening = changeOpening
        self.changeOutputMembershipPaths = changeOutputMembershipPaths
        self.unshieldVerifier = unshieldVerifier
        self.blockHeight = blockHeight
        self.operationID = Data(operationID)
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecsV4.encodeRedeemLocalRequest(self)
    }
}

/// Typed, exact ABI-21 initialization output.
public struct KagemushaRecursiveSpendInitResultV4: Equatable, Sendable {
    public let bundle: KagemushaRecursiveSpendBundleV4
    public let membershipWitness: KagemushaNoteMembershipWitness
    public let topUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4
    public let publicStatementDigest: Data
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        self = try KagemushaRecursiveSpendCodecs.decodeInitResultV4(noritoArchive)
    }

    init(
        bundle: KagemushaRecursiveSpendBundleV4,
        membershipWitness: KagemushaNoteMembershipWitness,
        topUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4,
        publicStatementDigest: Data,
        noritoArchive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            publicStatementDigest,
            field: "initResultV4.publicStatementDigest"
        )
        self.bundle = bundle
        self.membershipWitness = membershipWitness
        self.topUpProvenance = topUpProvenance
        self.publicStatementDigest = Data(publicStatementDigest)
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Typed, exact ABI-21 split output with recipient-only peer projection.
public struct KagemushaRecursiveSpendSplitResultV4: Equatable, Sendable {
    public let split: KagemushaRecursiveSpendSplitIntentV4
    public let splitBindingDigest: Data
    public let recipientBundle: KagemushaRecursiveSpendBundleV4
    public let recipientMembershipWitness: KagemushaNoteMembershipWitness
    public let recipientTopUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4
    public let changeBundle: KagemushaRecursiveSpendBundleV4?
    public let changeMembershipWitness: KagemushaNoteMembershipWitness?
    public let changeTopUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4?
    public let peerPayment: KagemushaRecursiveSpendPeerPaymentV4
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        self = try KagemushaRecursiveSpendCodecs.decodeSplitResultV4(noritoArchive)
    }

    init(
        split: KagemushaRecursiveSpendSplitIntentV4,
        splitBindingDigest: Data,
        recipientBundle: KagemushaRecursiveSpendBundleV4,
        recipientMembershipWitness: KagemushaNoteMembershipWitness,
        recipientTopUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4,
        changeBundle: KagemushaRecursiveSpendBundleV4?,
        changeMembershipWitness: KagemushaNoteMembershipWitness?,
        changeTopUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4?,
        peerPayment: KagemushaRecursiveSpendPeerPaymentV4,
        noritoArchive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            splitBindingDigest,
            field: "splitResultV4.splitBindingDigest"
        )
        guard (changeBundle == nil) == (changeMembershipWitness == nil),
              (changeBundle == nil) == (changeTopUpProvenance == nil),
              peerPayment.recipientBundle == recipientBundle,
              peerPayment.recipientMembershipWitness == recipientMembershipWitness,
              peerPayment.topUpProvenance == recipientTopUpProvenance else {
            throw KagemushaRecursiveSpendError.invalidArchive("splitResultV4.change")
        }
        self.split = split
        self.splitBindingDigest = Data(splitBindingDigest)
        self.recipientBundle = recipientBundle
        self.recipientMembershipWitness = recipientMembershipWitness
        self.recipientTopUpProvenance = recipientTopUpProvenance
        self.changeBundle = changeBundle
        self.changeMembershipWitness = changeMembershipWitness
        self.changeTopUpProvenance = changeTopUpProvenance
        self.peerPayment = peerPayment
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Typed terminal decision and exact verified ABI-21 state.
public struct KagemushaRecursiveSpendVerifyResultV4: Equatable, Sendable {
    public let valid: Bool
    public let chainAdmissible: Bool
    public let lineageRedeemable: Bool
    public let witnesslessRedemptionSupported: Bool
    public let summary: KagemushaRecursiveSpendBundleSummaryV4
    public let recipientRequestDigest: Data
    public let requestOutputBindingDigest: Data
    public let verifierKeyID: String
    public let verifierCircuitID: String
    public let verifierActivationHeight: UInt64?
    public let verifierWithdrawHeight: UInt64?
    public let verifiedAtBlockHeight: UInt64
    public let verifiedAtMilliseconds: UInt64
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        self = try KagemushaRecursiveSpendCodecs.decodeVerifyResultV4(noritoArchive)
    }

    init(
        valid: Bool,
        chainAdmissible: Bool,
        lineageRedeemable: Bool,
        witnesslessRedemptionSupported: Bool,
        summary: KagemushaRecursiveSpendBundleSummaryV4,
        recipientRequestDigest: Data,
        requestOutputBindingDigest: Data,
        verifierKeyID: String,
        verifierCircuitID: String,
        verifierActivationHeight: UInt64?,
        verifierWithdrawHeight: UInt64?,
        verifiedAtBlockHeight: UInt64,
        verifiedAtMilliseconds: UInt64,
        noritoArchive: Data
    ) {
        self.valid = valid
        self.chainAdmissible = chainAdmissible
        self.lineageRedeemable = lineageRedeemable
        self.witnesslessRedemptionSupported = witnesslessRedemptionSupported
        self.summary = summary
        self.recipientRequestDigest = Data(recipientRequestDigest)
        self.requestOutputBindingDigest = Data(requestOutputBindingDigest)
        self.verifierKeyID = verifierKeyID
        self.verifierCircuitID = verifierCircuitID
        self.verifierActivationHeight = verifierActivationHeight
        self.verifierWithdrawHeight = verifierWithdrawHeight
        self.verifiedAtBlockHeight = verifiedAtBlockHeight
        self.verifiedAtMilliseconds = verifiedAtMilliseconds
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Canonical unsigned ABI-21 redemption request projection.
public struct KagemushaRecursiveSpendRedeemUnsignedV4: Equatable, Sendable {
    public let version: UInt16
    public let operationID: Data
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        self = try KagemushaRecursiveSpendCodecs.decodeRedeemUnsignedV4(noritoArchive)
    }

    public func authorizationPayloadDigest() throws -> Data {
        guard let digest = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendRedeemUnsignedPayloadDigestV4(
                unsignedArchive: noritoArchive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            digest,
            field: "redeemUnsignedV4.payloadDigest"
        )
        return digest
    }

    init(version: UInt16, operationID: Data, noritoArchive: Data) {
        self.version = version
        self.operationID = Data(operationID)
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Typed, exact ABI-21 redemption-build output.
public struct KagemushaRecursiveSpendRedeemBuildResultV4: Equatable, Sendable {
    public let unsigned: KagemushaRecursiveSpendRedeemUnsignedV4
    public let authorizationDigest: Data
    public let offlineChangeBundle: KagemushaRecursiveSpendBundleV4?
    public let offlineChangeMembershipWitness: KagemushaNoteMembershipWitness?
    public let offlineChangeTopUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4?
    public let operationID: Data
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        self = try KagemushaRecursiveSpendCodecs.decodeRedeemBuildResultV4(noritoArchive)
    }

    init(
        unsigned: KagemushaRecursiveSpendRedeemUnsignedV4,
        authorizationDigest: Data,
        offlineChangeBundle: KagemushaRecursiveSpendBundleV4?,
        offlineChangeMembershipWitness: KagemushaNoteMembershipWitness?,
        offlineChangeTopUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4?,
        operationID: Data,
        noritoArchive: Data
    ) {
        self.unsigned = unsigned
        self.authorizationDigest = Data(authorizationDigest)
        self.offlineChangeBundle = offlineChangeBundle
        self.offlineChangeMembershipWitness = offlineChangeMembershipWitness
        self.offlineChangeTopUpProvenance = offlineChangeTopUpProvenance
        self.operationID = Data(operationID)
        self.noritoArchive = Data(noritoArchive)
    }

    public func finalize(
        authorization: KagemushaRequestAuthorization
    ) throws -> KagemushaRecursiveSpendRedeemResultV4 {
        guard authorization.fields.operationID == operationID,
              authorization.fields.payloadDigest == authorizationDigest else {
            throw KagemushaRecursiveSpendError.invalidField("authorization")
        }
        guard let archive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendRedeemFinalizeRequestV4(
                buildResultArchive: noritoArchive,
                authorizationArchive: authorization.archive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendCodecs.decodeRedeemResultV4(archive)
    }
}

/// Final ABI-21 redemption request plus proof-bound recovery state.
public struct KagemushaRecursiveSpendRedeemResultV4: Equatable, Sendable {
    public let version: UInt16
    public let redeemRequestArchive: Data
    public let offlineChangeBundle: KagemushaRecursiveSpendBundleV4?
    public let offlineChangeMembershipWitness: KagemushaNoteMembershipWitness?
    public let offlineChangeTopUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4?
    public let operationID: Data
    public let noritoArchive: Data

    init(
        redeemRequestArchive: Data,
        offlineChangeBundle: KagemushaRecursiveSpendBundleV4?,
        offlineChangeMembershipWitness: KagemushaNoteMembershipWitness?,
        offlineChangeTopUpProvenance: KagemushaRecursiveSpendTopUpProvenanceV4?,
        operationID: Data,
        noritoArchive: Data
    ) {
        version = KagemushaRecursiveSpend.wireVersionV4
        self.redeemRequestArchive = Data(redeemRequestArchive)
        self.offlineChangeBundle = offlineChangeBundle
        self.offlineChangeMembershipWitness = offlineChangeMembershipWitness
        self.offlineChangeTopUpProvenance = offlineChangeTopUpProvenance
        self.operationID = Data(operationID)
        self.noritoArchive = Data(noritoArchive)
    }
}

public extension KagemushaRecursiveSpend {
    /// Derive partial-redemption change entirely inside the native bridge.
    ///
    /// The input bundle and opening are reauthenticated by native code. The
    /// caller supplies only the exact smaller change amount, operation id, and
    /// fresh entropy; rho and the protocol diversifier are never caller-chosen.
    static func prepareRedemptionChangeV4(
        input: KagemushaRecursiveSpendSpendableBranchV4,
        changeAmount: KagemushaScaledAmount,
        operationID: Data,
        entropy: Data
    ) throws -> KagemushaRecursiveSpendRedemptionChangePreparationV4 {
        let request = try KagemushaRecursiveSpendRedemptionChangePrepareRequestV4(
            bundle: input.bundle,
            inputOpening: input.opening,
            changeAmount: changeAmount,
            operationID: operationID,
            entropy: entropy
        )
        let inputSummary = try input.bundle.projectedSummary()
        try validateRedemptionChangeV4(
            inputSummary: inputSummary,
            changeAmount: changeAmount,
            operationID: operationID,
            entropy: entropy
        )
        var requestArchive = try KagemushaRecursiveSpendCodecsV4
            .encodeRedemptionChangePrepareRequest(request)
        defer { requestArchive.resetBytes(in: 0..<requestArchive.count) }
        guard var resultArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendRedemptionChangePrepareV4(
                requestArchive: requestArchive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        defer { resultArchive.resetBytes(in: 0..<resultArchive.count) }
        return try KagemushaRecursiveSpendCodecs.decodeRedemptionChangePrepareResultV4(
            resultArchive,
            inputOpening: input.opening,
            inputSummary: inputSummary,
            changeAmount: changeAmount
        )
    }

    /// Prepare owned sender change for an ordinary one- or two-input peer split.
    ///
    /// Native reauthenticates every ordered input/opening pair, the exact signed
    /// receiver request and value conservation, then uses a peer-split-only KDF
    /// domain. The returned opening is local secret material.
    static func preparePeerSplitChangeV4(
        inputs: [KagemushaRecursiveSpendSpendableBranchV4],
        recipientRequest: KagemushaVerifiedRecipientPaymentRequest,
        changeAmount: KagemushaScaledAmount,
        operationID: Data,
        entropy: Data
    ) throws -> KagemushaRecursiveSpendPeerSplitChangePreparationV4 {
        let local = try KagemushaRecursiveSpendPeerSplitChangePrepareRequestV4(
            inputs: inputs,
            recipientRequest: recipientRequest.request,
            changeAmount: changeAmount,
            operationID: operationID,
            entropy: entropy
        )
        let summaries = try inputs.map { try $0.bundle.projectedSummary() }
        let total = try KagemushaScaledAmount.sum(summaries.map(\.amount))
        let conserved = try recipientRequest.request.payload.amount.adding(changeAmount)
        guard total == conserved,
              summaries.allSatisfy({
                  $0.assetDefinitionID == recipientRequest.request.payload.assetDefinitionID
              }) else {
            throw KagemushaRecursiveSpendError.invalidField("peerSplitChangeV4.value")
        }
        var requestArchive = try KagemushaRecursiveSpendCodecsV4
            .encodePeerSplitChangePrepareRequest(local)
        defer { requestArchive.resetBytes(in: 0..<requestArchive.count) }
        guard var resultArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendPeerSplitChangePrepareV4(
                requestArchive: requestArchive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        defer { resultArchive.resetBytes(in: 0..<resultArchive.count) }
        return try KagemushaRecursiveSpendCodecs.decodePeerSplitChangePrepareResultV4(
            resultArchive,
            inputOpenings: inputs.map(\.opening),
            inputSummaries: summaries,
            recipientRequest: recipientRequest.request,
            changeAmount: changeAmount
        )
    }

    static func validateRedemptionChangeV4(
        inputSummary: KagemushaRecursiveSpendBundleSummaryV4,
        changeAmount: KagemushaScaledAmount,
        operationID: Data,
        entropy: Data
    ) throws {
        try requireNonzeroFixed32(
            operationID,
            field: "redemptionChangeV4.operationID"
        )
        try requireNonzeroFixed32(
            entropy,
            field: "redemptionChangeV4.entropy"
        )
        guard operationID != entropy,
              changeAmount.scale == inputSummary.amount.scale,
              KagemushaScaledAmount.compareAtomicUnits(
                  changeAmount.atomicUnits,
                  inputSummary.amount.atomicUnits
              ) == .orderedAscending else {
            throw KagemushaRecursiveSpendError.invalidField("redemptionChangeV4")
        }
    }

    static func ensureProofBackendAvailableV4() throws {
        guard hasRequiredNativeSymbols else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        guard try nativeCapabilitiesV4().proofBackendAvailable else {
            throw KagemushaRecursiveSpendError.proofBackendUnavailable
        }
    }
}

extension KagemushaRecursiveSpend {
    /// Initializes a recursive-spend branch with the exact installed V4
    /// artifact generation bound by the request.
    public static func initSpendV4(
        request: KagemushaRecursiveSpendInitLocalRequestV4,
        installedArtifacts: KagemushaRecursiveSpendInstalledArtifactSetV4
    ) throws -> KagemushaRecursiveSpendInitResultV4 {
        return try KagemushaRecursiveSpendWorkerPermit.withPermit {
            try installedArtifacts.requireInstalled()
            guard request.artifactBinding == installedArtifacts.binding else {
                throw KagemushaRecursiveSpendError.invalidField("artifactBindingV4")
            }
            var requestArchive = try request.noritoEncoded()
            defer { requestArchive.resetBytes(in: 0..<requestArchive.count) }
            try ensureProofBackendAvailableV4()
            do {
                guard let output = try NoritoNativeBridge.shared
                    .kagemushaRecursiveSpendInitV4(requestArchive: requestArchive) else {
                    throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
                }
                return try KagemushaRecursiveSpendInitResultV4(noritoArchive: output)
            } catch NativeBridgeError.kagemushaRecursiveSpendV4Unavailable {
                throw KagemushaRecursiveSpendError.proofBackendUnavailable
            } catch NativeBridgeError.kagemushaBusy {
                throw KagemushaRecursiveSpendError.proofWorkerBusy
            }
        }
    }

    /// Appends a receiver-authorized payment, and optional change, to the V4
    /// recursive-spend branch bound to the installed artifact generation.
    public static func appendSpendV4(
        request: KagemushaRecursiveSpendAppendLocalRequestV4,
        signedRecipientRequest: KagemushaVerifiedRecipientPaymentRequest,
        installedArtifacts: KagemushaRecursiveSpendInstalledArtifactSetV4
    ) throws -> KagemushaRecursiveSpendSplitResultV4 {
        return try KagemushaRecursiveSpendWorkerPermit.withPermit {
            try installedArtifacts.requireInstalled()
            guard request.outputArtifactBinding == installedArtifacts.binding else {
                throw KagemushaRecursiveSpendError.invalidField("artifactBindingV4")
            }
            var requestArchive = try request.noritoEncoded()
            defer { requestArchive.resetBytes(in: 0..<requestArchive.count) }
            try ensureProofBackendAvailableV4()
            do {
                guard let output = try NoritoNativeBridge.shared.kagemushaRecursiveSpendAppendV4(
                    requestArchive: requestArchive,
                    recipientRequestArchive: signedRecipientRequest.request.archive,
                    verifiedAtMilliseconds: signedRecipientRequest.verifiedAtMilliseconds
                ) else {
                    throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
                }
                return try KagemushaRecursiveSpendSplitResultV4(noritoArchive: output)
            } catch NativeBridgeError.kagemushaRecursiveSpendV4Unavailable {
                throw KagemushaRecursiveSpendError.proofBackendUnavailable
            } catch NativeBridgeError.kagemushaBusy {
                throw KagemushaRecursiveSpendError.proofWorkerBusy
            }
        }
    }

    /// Resumes an append whose secret-bearing local carrier was committed in
    /// encrypted wallet state before process death.
    public static func appendSpendV4(
        persistedRequest: KagemushaRecursiveSpendPersistedAppendLocalRequestV4,
        signedRecipientRequest: KagemushaVerifiedRecipientPaymentRequest,
        installedArtifacts: KagemushaRecursiveSpendInstalledArtifactSetV4
    ) throws -> KagemushaRecursiveSpendSplitResultV4 {
        try installedArtifacts.requireInstalled()
        guard persistedRequest.artifactBinding == installedArtifacts.binding else {
            throw KagemushaRecursiveSpendError.invalidField("artifactBindingV4")
        }
        var requestArchive = persistedRequest.noritoArchive
        defer { requestArchive.resetBytes(in: 0..<requestArchive.count) }
        try ensureProofBackendAvailableV4()
        do {
            guard let output = try NoritoNativeBridge.shared.kagemushaRecursiveSpendAppendV4(
                requestArchive: requestArchive,
                recipientRequestArchive: signedRecipientRequest.request.archive,
                verifiedAtMilliseconds: signedRecipientRequest.verifiedAtMilliseconds
            ) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            return try KagemushaRecursiveSpendSplitResultV4(noritoArchive: output)
        } catch let error as NativeBridgeError {
            switch error {
            case .kagemushaRecursiveSpendV4Unavailable:
                throw KagemushaRecursiveSpendError.proofBackendUnavailable
            case .kagemushaBusy:
                throw KagemushaRecursiveSpendError.proofWorkerBusy
            default:
                throw error
            }
        }
    }

    /// Verifies an incoming V4 recursive-spend payment against the exact
    /// installed artifact generation bound by the request.
    public static func verifySpendV4(
        request: KagemushaRecursiveSpendVerifyLocalRequestV4,
        installedArtifacts: KagemushaRecursiveSpendInstalledArtifactSetV4
    ) throws -> KagemushaRecursiveSpendVerifyResultV4 {
        return try KagemushaRecursiveSpendWorkerPermit.withPermit {
            try installedArtifacts.requireInstalled()
            guard request.request.artifactBinding == installedArtifacts.binding else {
                throw KagemushaRecursiveSpendError.invalidField("artifactBindingV4")
            }
            let requestArchive = try request.noritoEncoded()
            try ensureProofBackendAvailableV4()
            do {
                guard let output = try NoritoNativeBridge.shared
                    .kagemushaRecursiveSpendVerifyV4(requestArchive: requestArchive) else {
                    throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
                }
                return try KagemushaRecursiveSpendVerifyResultV4(noritoArchive: output)
            } catch NativeBridgeError.kagemushaRecursiveSpendV4Unavailable {
                throw KagemushaRecursiveSpendError.proofBackendUnavailable
            } catch NativeBridgeError.kagemushaBusy {
                throw KagemushaRecursiveSpendError.proofWorkerBusy
            }
        }
    }

    /// Builds the canonical V4 redemption result with the selected installed
    /// artifact generation.
    public static func buildRedeemV4(
        request: KagemushaRecursiveSpendRedeemLocalRequestV4,
        installedArtifacts: KagemushaRecursiveSpendInstalledArtifactSetV4
    ) throws -> KagemushaRecursiveSpendRedeemBuildResultV4 {
        return try KagemushaRecursiveSpendWorkerPermit.withPermit {
            try installedArtifacts.requireInstalled()
            var requestArchive = try request.noritoEncoded()
            defer { requestArchive.resetBytes(in: 0..<requestArchive.count) }
            try ensureProofBackendAvailableV4()
            do {
                guard let output = try NoritoNativeBridge.shared
                    .kagemushaRecursiveSpendRedeemV4(requestArchive: requestArchive) else {
                    throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
                }
                return try KagemushaRecursiveSpendRedeemBuildResultV4(noritoArchive: output)
            } catch NativeBridgeError.kagemushaRecursiveSpendV4Unavailable {
                throw KagemushaRecursiveSpendError.proofBackendUnavailable
            } catch NativeBridgeError.kagemushaBusy {
                throw KagemushaRecursiveSpendError.proofWorkerBusy
            }
        }
    }
}

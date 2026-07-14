import Foundation

/// The authenticated ABI-20 artifact generation selected by a V4 operation.
/// It is intentionally not interchangeable with the frozen ABI-19 binding.
public struct KagemushaRecursiveSpendArtifactBindingV4: Equatable, Hashable, Sendable {
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
        self.generation = generation
        self.manifestSHA256 = Data(manifestSHA256)
    }

    public func noritoEncoded() -> Data {
        KagemushaRecursiveSpendCodecsV4.encodeArtifactBinding(self)
    }
}

/// Opaque ABI-20 recursive state. Its archive is never decoded as a V2/V3 bundle.
public struct KagemushaRecursiveSpendBundleV4: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.bundleWireNameV4,
            field: "bundleV4"
        )
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Finalized top-up receipt whose public statement selects an ABI-20 release.
public struct KagemushaRecursiveSpendTopUpAnchorV4: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        guard noritoArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityAnchorMaximumArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpAnchorV4.size")
        }
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpAnchorWireNameV4,
            field: "topUpAnchorV4"
        )
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

/// One output insertion path owned exclusively by the ABI-20 local carrier.
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

/// Exact output-update witness decoded only by the ABI-20 bridge.
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
            guard !next.overflow, next.partialValue == change.leafIndex,
                  change.updatePath.root == initialRoot else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "outputMembershipV4.change"
                )
            }
        }
        self.initialRoot = Data(initialRoot)
        self.finalRoot = Data(finalRoot)
        self.recipient = recipient
        self.change = change
        self.dummyLeafIndex = dummyLeafIndex
        self.dummyPath = dummyPath
    }
}

/// Canonical ABI-20 initialization request before local secret witnesses are added.
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

/// Secret-bearing local ABI-20 initialization input.
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

/// One genuine ABI-20 previous-proof package.
public struct KagemushaRecursiveSpendAppendInputV4: Equatable, Sendable {
    public let previousBundle: KagemushaRecursiveSpendBundleV4

    public init(previousBundle: KagemushaRecursiveSpendBundleV4) {
        self.previousBundle = previousBundle
    }
}

/// Secret-bearing spendable local state used only by ABI-20 builders.
public struct KagemushaRecursiveSpendSpendableBranchV4: Equatable, Sendable {
    public let bundle: KagemushaRecursiveSpendBundleV4
    public let membershipWitness: KagemushaNoteMembershipWitness
    public let opening: KagemushaNoteOpening

    public init(
        bundle: KagemushaRecursiveSpendBundleV4,
        membershipWitness: KagemushaNoteMembershipWitness,
        opening: KagemushaNoteOpening
    ) {
        self.bundle = bundle
        self.membershipWitness = membershipWitness
        self.opening = opening
    }
}

/// Secret-bearing ABI-20 append input. It encodes the flat V4 bridge carrier,
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
            KagemushaRecursiveSpendAppendInputV4(previousBundle: $0.bundle)
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

/// Canonical ABI-20 receiver-verification request.
public struct KagemushaRecursiveSpendVerifyRequestV4: Equatable, Sendable {
    public let bundle: KagemushaRecursiveSpendBundleV4
    public let recipientRequest: KagemushaRecipientPaymentRequest
    public let topUpFinalityRosterArtifact: KagemushaTopUpFinalityRosterArtifactArchive
    public let topUpFinalityEvidence: [KagemushaRecursiveSpendTopUpFinalityEvidenceV4]
    public let maximumHops: UInt32
    public let artifactBinding: KagemushaRecursiveSpendArtifactBindingV4
    public let blockHeight: UInt64
    public let verifiedAtMilliseconds: UInt64

    public init(
        bundle: KagemushaRecursiveSpendBundleV4,
        recipientRequest: KagemushaRecipientPaymentRequest,
        topUpFinalityRosterArtifact: KagemushaTopUpFinalityRosterArtifactArchive,
        topUpFinalityEvidence: [KagemushaRecursiveSpendTopUpFinalityEvidenceV4],
        maximumHops: UInt32 = KagemushaRecursiveSpend.maximumPeerHops,
        artifactBinding: KagemushaRecursiveSpendArtifactBindingV4,
        blockHeight: UInt64,
        verifiedAtMilliseconds: UInt64
    ) throws {
        guard maximumHops == KagemushaRecursiveSpend.maximumPeerHops,
              (1...KagemushaRecursiveSpend.maximumInputsPerTransition)
                .contains(topUpFinalityEvidence.count),
              Set(topUpFinalityEvidence.map(\.noritoArchive)).count
                == topUpFinalityEvidence.count,
              blockHeight > 0,
              verifiedAtMilliseconds > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("verifyRequestV4")
        }
        self.bundle = bundle
        self.recipientRequest = recipientRequest
        self.topUpFinalityRosterArtifact = topUpFinalityRosterArtifact
        self.topUpFinalityEvidence = topUpFinalityEvidence
        self.maximumHops = maximumHops
        self.artifactBinding = artifactBinding
        self.blockHeight = blockHeight
        self.verifiedAtMilliseconds = verifiedAtMilliseconds
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecsV4.encodeVerifyRequest(self)
    }
}

/// Explicit ABI-20 local verification carrier.
public struct KagemushaRecursiveSpendVerifyLocalRequestV4: Equatable, Sendable {
    public let request: KagemushaRecursiveSpendVerifyRequestV4

    public init(request: KagemushaRecursiveSpendVerifyRequestV4) {
        self.request = request
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecsV4.encodeVerifyLocalRequest(self)
    }
}

/// Secret-bearing ABI-20 redemption input. Native derives the V4 public
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
        _ = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
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

/// Opaque, exact ABI-20 initialization output.
public struct KagemushaRecursiveSpendInitResultV4: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.initResultWireNameV4,
            field: "initResultV4"
        )
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Opaque, exact ABI-20 split output.
public struct KagemushaRecursiveSpendSplitResultV4: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.splitResultWireNameV4,
            field: "splitResultV4"
        )
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Opaque, exact ABI-20 verification output.
public struct KagemushaRecursiveSpendVerifyResultV4: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.verifyResultWireNameV4,
            field: "verifyResultV4"
        )
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Opaque, exact ABI-20 redemption-build output.
public struct KagemushaRecursiveSpendRedeemBuildResultV4: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.redeemBuildResultWireNameV4,
            field: "redeemBuildResultV4"
        )
        self.noritoArchive = Data(noritoArchive)
    }
}

public extension KagemushaRecursiveSpend {
    static func ensureProofBackendAvailableV4() throws {
        guard hasRequiredNativeSymbols else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        guard try nativeCapabilitiesV4().proofBackendAvailable else {
            throw KagemushaRecursiveSpendError.proofBackendUnavailable
        }
    }

    static func initSpendV4(
        request: KagemushaRecursiveSpendInitLocalRequestV4,
        installedArtifacts: KagemushaRecursiveSpendInstalledArtifactSetV4
    ) throws -> KagemushaRecursiveSpendInitResultV4 {
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
        }
    }

    static func appendSpendV4(
        request: KagemushaRecursiveSpendAppendLocalRequestV4,
        signedRecipientRequest: KagemushaVerifiedRecipientPaymentRequest,
        installedArtifacts: KagemushaRecursiveSpendInstalledArtifactSetV4
    ) throws -> KagemushaRecursiveSpendSplitResultV4 {
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
        }
    }

    static func verifySpendV4(
        request: KagemushaRecursiveSpendVerifyLocalRequestV4,
        installedArtifacts: KagemushaRecursiveSpendInstalledArtifactSetV4
    ) throws -> KagemushaRecursiveSpendVerifyResultV4 {
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
        }
    }

    static func buildRedeemV4(
        request: KagemushaRecursiveSpendRedeemLocalRequestV4,
        installedArtifacts: KagemushaRecursiveSpendInstalledArtifactSetV4
    ) throws -> KagemushaRecursiveSpendRedeemBuildResultV4 {
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
        }
    }
}

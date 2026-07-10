import CryptoKit
import Foundation

public enum KagemushaRecursiveSpendRequestCodecError: Error, Equatable, LocalizedError {
    case invalidField(String)
    case invalidArchive(String)

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid Kagemusha recursive spend field: \(field)."
        case let .invalidArchive(field):
            return "Invalid Kagemusha recursive spend Norito archive: \(field)."
        }
    }
}

public struct KagemushaRecursiveSpendableNoteDescriptor: Equatable, Sendable {
    public let noteCommitment: Data
    public let spendNullifier: Data
    public let amount: String

    public init(noteCommitment: Data, spendNullifier: Data, amount: String) throws {
        guard noteCommitment.count == 32 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("noteCommitment")
        }
        guard spendNullifier.count == 32 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("spendNullifier")
        }
        guard noteCommitment.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("noteCommitment")
        }
        guard spendNullifier.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("spendNullifier")
        }
        guard noteCommitment != spendNullifier else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("spendNullifier")
        }
        self.noteCommitment = noteCommitment
        self.spendNullifier = spendNullifier
        self.amount = try KagemushaRecursiveSpendRequestCodecs.canonicalU128Decimal(
            amount,
            field: "amount"
        )
    }
}

public struct KagemushaRecursiveSpendVerifierRecordRef: Equatable, Sendable {
    public let verifierKeyId: String
    public let recordBytes: Data

    public init(verifierKeyId: String, recordBytes: Data) throws {
        try KagemushaRecursiveSpendRequestCodecs.requirePortableId(verifierKeyId, field: "verifierKeyId")
        _ = try KagemushaRecursiveSpendRequestCodecs.payloadArchive(
            recordBytes,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            field: "recordBytes"
        )
        self.verifierKeyId = verifierKeyId
        self.recordBytes = recordBytes
    }
}

public struct KagemushaVerifiedFoldHopEvidence: Equatable, Sendable {
    public let proofOutputArchive: Data
    public let verifierRecord: KagemushaRecursiveSpendVerifierRecordRef
    public let chainId: String
    public let assetDefinitionId: String
    public let rootAfter: Data

    public init(
        proofOutputArchive: Data,
        verifierRecord: KagemushaRecursiveSpendVerifierRecordRef,
        chainId: String,
        assetDefinitionId: String,
        rootAfter: Data
    ) throws {
        guard !proofOutputArchive.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("proofOutputArchive")
        }
        try KagemushaRecursiveSpendRequestCodecs.requirePortableId(chainId, field: "chainId")
        guard AssetDefinitionAddress.decode(assetDefinitionId) != nil else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("assetDefinitionId")
        }
        try KagemushaRecursiveSpendRequestCodecs.requireFixed32(rootAfter, field: "rootAfter")
        self.proofOutputArchive = Data(proofOutputArchive)
        self.verifierRecord = verifierRecord
        self.chainId = chainId
        self.assetDefinitionId = assetDefinitionId
        self.rootAfter = Data(rootAfter)
    }
}

public struct KagemushaRecursiveSpendInitRequest: Equatable, Sendable {
    public let recordBundle: Data
    public let pallasOpenEnvelopes: Data
    public let currentNote: KagemushaRecursiveSpendableNoteDescriptor
    public let lineageVerifierKey: Data
    public let lineageProvingKeyArchive: Data
    public let blockHeight: UInt64?

    public init(
        recordBundle: Data,
        pallasOpenEnvelopes: Data,
        currentNote: KagemushaRecursiveSpendableNoteDescriptor,
        lineageVerifierKey: Data,
        lineageProvingKeyArchive: Data,
        blockHeight: UInt64? = nil
    ) throws {
        let recordBundlePayload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            recordBundle,
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
            field: "recordBundle"
        )
        let recordBundleHopCount = try KagemushaRecursiveSpendRequestCodecs.readVerifiedFoldRecordBundleHopCount(
            recordBundlePayload,
            field: "recordBundle"
        )
        try KagemushaRecursiveSpendRequestCodecs.requirePallasOpenEnvelopesArchive(
            pallasOpenEnvelopes,
            expectedEnvelopeCount: recordBundleHopCount,
            field: "pallasOpenEnvelopes",
            maxBytes: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes
        )
        guard !lineageVerifierKey.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierKey")
        }
        guard !lineageProvingKeyArchive.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageProvingKeyArchive")
        }
        try KagemushaRecursiveSpendRequestCodecs.validateLineageKeyArtifactsForInit(
            lineageVerifierKey: lineageVerifierKey,
            lineageProvingKeyArchive: lineageProvingKeyArchive
        )
        self.recordBundle = recordBundle
        self.pallasOpenEnvelopes = pallasOpenEnvelopes
        self.currentNote = currentNote
        self.lineageVerifierKey = lineageVerifierKey
        self.lineageProvingKeyArchive = lineageProvingKeyArchive
        self.blockHeight = blockHeight
    }
}

public struct KagemushaRecursiveSpendTopUpInitRequest: Equatable, Sendable {
    public let recordBundle: Data
    public let pallasOpenEnvelopes: Data
    public let currentNote: KagemushaRecursiveSpendableNoteDescriptor
    public let blockHeight: UInt64?

    public init(
        recordBundle: Data,
        pallasOpenEnvelopes: Data,
        currentNote: KagemushaRecursiveSpendableNoteDescriptor,
        blockHeight: UInt64? = nil
    ) throws {
        let recordBundlePayload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            recordBundle,
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
            field: "recordBundle"
        )
        let recordBundleHopCount = try KagemushaRecursiveSpendRequestCodecs.readVerifiedFoldRecordBundleHopCount(
            recordBundlePayload,
            field: "recordBundle"
        )
        guard recordBundleHopCount == 1 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("recordBundle")
        }
        try KagemushaRecursiveSpendRequestCodecs.requirePallasOpenEnvelopesArchive(
            pallasOpenEnvelopes,
            expectedEnvelopeCount: recordBundleHopCount,
            field: "pallasOpenEnvelopes",
            maxBytes: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes
        )
        self.recordBundle = recordBundle
        self.pallasOpenEnvelopes = pallasOpenEnvelopes
        self.currentNote = currentNote
        self.blockHeight = blockHeight
    }
}

public struct KagemushaRecursiveSpendTopUpInitRequestSummary: Equatable, Sendable {
    public let assetDefinitionId: String
    public let amount: String
}

public struct KagemushaRecursiveSpendTopUpRequest: Equatable, Sendable {
    public let assetId: String
    public let amount: String
    public let initRequestArchive: Data

    public init(
        assetId: String,
        amount: String,
        initRequestArchive: Data
    ) throws {
        let canonicalAssetId = try KagemushaRecursiveSpendRequestCodecs.canonicalAssetId(
            assetId,
            field: "assetId"
        )
        let canonicalAmount = try KagemushaRecursiveSpendRequestCodecs.canonicalU128Decimal(
            amount,
            field: "amount"
        )
        _ = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            initRequestArchive,
            schema: KagemushaRecursiveSpendRequestCodecs.initRequestWireName,
            field: "initRequestArchive"
        )
        try KagemushaRecursiveSpendRequestCodecs.validateTopUpRequestPublicBinding(
            assetId: canonicalAssetId,
            amount: canonicalAmount,
            initRequestArchive: initRequestArchive
        )
        self.assetId = canonicalAssetId
        self.amount = canonicalAmount
        self.initRequestArchive = initRequestArchive
    }

    public init(
        accountId: String,
        assetDefinitionId: String,
        amount: String,
        initRequestArchive: Data,
        dataspaceId: UInt64? = nil
    ) throws {
        try self.init(
            assetId: KagemushaRecursiveSpendRequestCodecs.canonicalAssetId(
                accountId: accountId,
                assetDefinitionId: assetDefinitionId,
                dataspaceId: dataspaceId
            ),
            amount: amount,
            initRequestArchive: initRequestArchive
        )
    }
}

public struct KagemushaRecursiveSpendAppendRequest: Equatable, Sendable {
    public let previousBundle: Data
    public let recordBundle: Data
    public let pallasOpenEnvelopes: Data
    public let currentNote: KagemushaRecursiveSpendableNoteDescriptor
    public let outputProofCircuitId: String
    public let previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef?
    public let previousProofOpenEnvelopes: Data?
    public let lineageVerifierKey: Data?
    public let lineageProvingKeyArchive: Data?
    public let blockHeight: UInt64?

    public init(
        previousBundle: Data,
        recordBundle: Data,
        pallasOpenEnvelopes: Data,
        currentNote: KagemushaRecursiveSpendableNoteDescriptor,
        outputProofCircuitId: String,
        previousLineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef? = nil,
        previousProofOpenEnvelopes: Data? = nil,
        lineageVerifierKey: Data? = nil,
        lineageProvingKeyArchive: Data? = nil,
        blockHeight: UInt64? = nil
    ) throws {
        let previousSummary = try KagemushaRecursiveSpendRequestCodecs.decodeBundle(previousBundle)
        let recordBundlePayload = try KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            recordBundle,
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
            field: "recordBundle"
        )
        let recordBundleHopCount = try KagemushaRecursiveSpendRequestCodecs.readVerifiedFoldRecordBundleHopCount(
            recordBundlePayload,
            field: "recordBundle"
        )
        try KagemushaRecursiveSpendRequestCodecs.requirePallasOpenEnvelopesArchive(
            pallasOpenEnvelopes,
            expectedEnvelopeCount: recordBundleHopCount,
            field: "pallasOpenEnvelopes",
            maxBytes: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes
        )
        let normalizedOutput = KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(
            outputProofCircuitId
        )
        let appendNeedsPreviousProofOpenEnvelopes =
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId: normalizedOutput,
                previousHopCount: UInt32(previousSummary.hopCount)
            )
        let appendNeedsPreviousLineageVerifierRecord =
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousProofCircuitId: previousSummary.proofCircuitId
            )
        let appendNeedsLineageKeyArtifacts =
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                outputCircuitId: normalizedOutput
            )
        guard KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
            previousProofCircuitId: previousSummary.proofCircuitId,
            outputCircuitId: normalizedOutput,
            previousHopCount: UInt32(previousSummary.hopCount)
        ) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("outputProofCircuitId")
        }
        let suppliedLineageKeyMaterial = lineageVerifierKey != nil || lineageProvingKeyArchive != nil
        guard !suppliedLineageKeyMaterial || appendNeedsLineageKeyArtifacts else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageKeyArtifacts")
        }
        if appendNeedsPreviousLineageVerifierRecord, previousLineageVerifierRecord == nil {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("previousLineageVerifierRecord")
        }
        if !appendNeedsPreviousLineageVerifierRecord, previousLineageVerifierRecord != nil {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("previousLineageVerifierRecord")
        }
        guard previousProofOpenEnvelopes == nil || appendNeedsPreviousProofOpenEnvelopes else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("previousProofOpenEnvelopes")
        }
        if let previousProofOpenEnvelopes {
            try KagemushaRecursiveSpendRequestCodecs.requirePallasOpenEnvelopesArchive(
                previousProofOpenEnvelopes,
                expectedEnvelopeCount: Int(KagemushaRecursiveSpendProver
                    .recursivePreviousProofOpenEnvelopesRequiredCountV1),
                field: "previousProofOpenEnvelopes",
                maxBytes: KagemushaRecursiveSpendProver
                    .recursivePreviousProofOpenEnvelopesMaxBytes
            )
        }
        if appendNeedsPreviousProofOpenEnvelopes, previousProofOpenEnvelopes == nil {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("previousProofOpenEnvelopes")
        }
        if appendNeedsLineageKeyArtifacts {
            guard let lineageVerifierKey, !lineageVerifierKey.isEmpty else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierKey")
            }
            guard let lineageProvingKeyArchive, !lineageProvingKeyArchive.isEmpty else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageProvingKeyArchive")
            }
            try KagemushaRecursiveSpendRequestCodecs.validateLineageKeyArtifactsForAppend(
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
        }
        self.previousBundle = previousBundle
        self.recordBundle = recordBundle
        self.pallasOpenEnvelopes = pallasOpenEnvelopes
        self.currentNote = currentNote
        self.outputProofCircuitId = outputProofCircuitId
        self.previousLineageVerifierRecord = previousLineageVerifierRecord
        self.previousProofOpenEnvelopes = previousProofOpenEnvelopes
        self.lineageVerifierKey = lineageVerifierKey
        self.lineageProvingKeyArchive = lineageProvingKeyArchive
        self.blockHeight = blockHeight
    }
}

public struct KagemushaRecursiveSpendVerifyRequest: Equatable, Sendable {
    public let bundle: Data
    public let lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef?
    public let blockHeight: UInt64?

    public init(
        bundle: Data,
        lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef? = nil,
        blockHeight: UInt64? = nil
    ) throws {
        let bundleSummary = try KagemushaRecursiveSpendRequestCodecs.decodeBundle(bundle)
        guard !KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId)
            || lineageVerifierRecord != nil
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierRecord")
        }
        guard KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId)
            || lineageVerifierRecord == nil
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierRecord")
        }
        self.bundle = bundle
        self.lineageVerifierRecord = lineageVerifierRecord
        self.blockHeight = blockHeight
    }
}

public struct KagemushaRecursiveSpendVerifyResult: Equatable, Sendable {
    public let valid: Bool
    public let hopCount: UInt32
    public let encodedBytes: UInt32
    public let reason: String
    public let chainAdmissible: Bool
    public let chainAdmissionReason: String
    public let witnesslessRedeemSupported: Bool
    public let lineageWitnessRequiredForRedeem: Bool
}

public struct KagemushaRecursiveSpendRedeemRequest: Equatable, Sendable {
    public let bundle: Data
    public let recipient: String
    public let publicAmount: String
    public let redeemProof: Data
    public let lineageWitness: Data?
    public let changeOutput: Data?
    public let lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef?
    public let blockHeight: UInt64?
    public let lineageVerifierRecords: [KagemushaRecursiveSpendVerifierRecordRef]

    public init(
        bundle: Data,
        recipient: String,
        publicAmount: String,
        redeemProof: Data,
        lineageWitness: Data? = nil,
        changeOutput: Data? = nil,
        lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef? = nil,
        blockHeight: UInt64? = nil,
        lineageVerifierRecords: [KagemushaRecursiveSpendVerifierRecordRef] = []
    ) throws {
        try KagemushaRecursiveSpendRequestCodecs.requireNonBlankUnpadded(recipient, field: "recipient")
        if let changeOutput {
            try KagemushaRecursiveSpendRequestCodecs.requireFixed32(changeOutput, field: "changeOutput")
        }
        let canonicalPublicAmount = try KagemushaRecursiveSpendRequestCodecs.canonicalU128Decimal(
            publicAmount,
            field: "publicAmount"
        )
        let bundleSummary = try KagemushaRecursiveSpendRequestCodecs.decodeBundle(bundle)
        try KagemushaRecursiveSpendRequestCodecs.requireRedeemChangeBinding(
            publicAmount: canonicalPublicAmount,
            currentAmount: bundleSummary.currentNote.amount,
            hasChangeOutput: changeOutput != nil
        )
        if let changeOutput {
            try KagemushaRecursiveSpendRequestCodecs.requireRedeemChangeOutputNotReserved(
                changeOutput,
                bundleSummary: bundleSummary
            )
        }
        let finalIsLineage = KagemushaRecursiveSpendProver.isLineageProofCircuitId(
            bundleSummary.proofCircuitId
        )
        let hasLineageVerifierRecord = lineageVerifierRecord != nil || !lineageVerifierRecords.isEmpty
        if finalIsLineage, !hasLineageVerifierRecord {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierRecord")
        }
        if let lineageWitness {
            try KagemushaRecursiveSpendRequestCodecs.requireNestedArchive(lineageWitness, field: "lineageWitness")
        }
        let witnessHasReservedPrevious: Bool
        if let lineageWitness {
            witnessHasReservedPrevious = try KagemushaRecursiveSpendRequestCodecs
                .lineageWitnessHasReservedPreviousProof(lineageWitness)
        } else {
            witnessHasReservedPrevious = false
        }
        if !finalIsLineage {
            if witnessHasReservedPrevious && !hasLineageVerifierRecord {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierRecord")
            }
            if !witnessHasReservedPrevious && hasLineageVerifierRecord {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierRecord")
            }
        }
        guard !KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
            circuitId: bundleSummary.proofCircuitId,
            hopCount: UInt32(bundleSummary.hopCount)
        ) || lineageWitness != nil else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageWitness")
        }
        guard !KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId)
            || hasLineageVerifierRecord
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierRecord")
        }
        self.bundle = bundle
        self.recipient = recipient
        self.publicAmount = canonicalPublicAmount
        self.redeemProof = redeemProof
        self.lineageWitness = lineageWitness
        self.changeOutput = changeOutput
        self.lineageVerifierRecord = lineageVerifierRecord
        self.blockHeight = blockHeight
        self.lineageVerifierRecords = lineageVerifierRecords
    }
}

public struct KagemushaRecursiveSpendBundleSummary: Equatable, Sendable {
    public let hopCount: Int
    public let proofCircuitId: String
    public let asset: String
    public let chainId: String
    public let initialRoot: Data
    public let finalRoot: Data
    public let topupAnchorNullifiers: [Data]
    public let currentNote: KagemushaRecursiveSpendableNoteDescriptor

    public init(
        hopCount: Int,
        proofCircuitId: String,
        asset: String,
        chainId: String,
        initialRoot: Data,
        finalRoot: Data,
        topupAnchorNullifiers: [Data],
        currentNote: KagemushaRecursiveSpendableNoteDescriptor
    ) throws {
        guard hopCount >= 1,
              hopCount <= Int(KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1)
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.hop_count")
        }
        guard KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(proofCircuitId) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.proof_circuit_id")
        }
        try Self.requireAccumulatorAsset(asset)
        try KagemushaRecursiveSpendRequestCodecs.requirePortableId(chainId, field: "chainId")
        try Self.requireFixed32(initialRoot, field: "initialRoot")
        try Self.requireFixed32(finalRoot, field: "finalRoot")
        try Self.requireAccumulatorRoots(initialRoot: initialRoot, finalRoot: finalRoot)
        try topupAnchorNullifiers.forEach {
            try Self.requireFixed32($0, field: "topupAnchorNullifier")
        }
        try Self.requireTopupAnchorNullifiers(
            topupAnchorNullifiers,
            currentNote: currentNote
        )

        self.hopCount = hopCount
        self.proofCircuitId = proofCircuitId
        self.asset = asset
        self.chainId = chainId
        self.initialRoot = initialRoot
        self.finalRoot = finalRoot
        self.topupAnchorNullifiers = topupAnchorNullifiers
        self.currentNote = currentNote
    }

    private static func requireAccumulatorAsset(_ asset: String) throws {
        guard AssetDefinitionAddress.decode(asset) != nil || isRawHexAssetDefinitionLiteral(asset) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.asset")
        }
    }

    private static func isRawHexAssetDefinitionLiteral(_ asset: String) -> Bool {
        guard asset.count == 36, asset.hasPrefix("hex:") else {
            return false
        }
        return asset.dropFirst(4).allSatisfy { character in
            character >= "0" && character <= "9" || character >= "a" && character <= "f"
        }
    }

    private static func requireFixed32(_ value: Data, field: String) throws {
        guard value.count == 32 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
    }

    private static func requireAccumulatorRoots(initialRoot: Data, finalRoot: Data) throws {
        guard initialRoot.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.initial_root")
        }
        guard finalRoot.contains(where: { $0 != 0 }),
              finalRoot != initialRoot
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.final_root")
        }
    }

    private static func requireTopupAnchorNullifiers(
        _ nullifiers: [Data],
        currentNote: KagemushaRecursiveSpendableNoteDescriptor
    ) throws {
        guard !nullifiers.isEmpty,
              nullifiers.count <= KagemushaRecursiveSpendRequestCodecs.foldStepMaxInputs
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "bundle.accumulator.topup_anchor_nullifiers count is out of range"
            )
        }
        for (index, nullifier) in nullifiers.enumerated() {
            guard nullifier.contains(where: { $0 != 0 }) else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                    "bundle.accumulator.topup_anchor_nullifiers must not contain zero values"
                )
            }
            if index > 0 {
                guard nullifiers[index - 1].lexicographicallyPrecedes(nullifier) else {
                    throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                        "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique"
                    )
                }
            }
        }
        guard !nullifiers.contains(currentNote.noteCommitment),
              !nullifiers.contains(currentNote.spendNullifier)
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material"
            )
        }
    }
}

public enum KagemushaRecursiveSpendRequestCodecs {
    public static let initRequestWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendInitRequestV1"
    public static let topUpRequestWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpRequestV1"
    public static let appendRequestWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendAppendRequestV1"
    public static let verifyRequestWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyRequestV1"
    public static let verifyResultWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyResultV1"
    public static let redeemRequestWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1"
    public static let bundleWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendBundleV1"
    public static let recordBundleWireName =
        "iroha_data_model::offline::model::KagemushaVerifiedFoldRecordBundle"
    public static let lineageWitnessWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendLineageWitnessV1"
    public static let recursiveAggregationProofPublicInputsWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveAggregationProofPublicInputs"
    public static let proofAttachmentWireName =
        "iroha_data_model::proof::ProofAttachment"
    public static let verifyingKeyRecordWireName =
        "iroha_data_model::proof::VerifyingKeyRecord"
    public static let openVerifyEnvelopeWireName =
        "iroha_data_model::zk::OpenVerifyEnvelope"
    public static let confidentialTransferV2CircuitId =
        "halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified"
    public static let confidentialUnshieldV3CircuitId =
        "halo2/pasta/ipa/anon-unshield-2in-1change-merkle16-poseidon-diversified"

    static let requestFlags = NoritoHeader.compactLen
    private static let privacyFfiVersionV1: UInt32 = 1
    private static let privacyFfiStatusOk: UInt32 = 0
    private static let privacySchemaBuildProofResult: UInt8 = 0x42
    private static let backendTagHalo2IpaPasta: UInt32 = VerifyingKeyBackendTag.halo2IpaPasta.rawValue
    private static let confidentialStatusActive: UInt32 = 1
    private static let confidentialV2MaxProofBytes = 192 * 1024
    private static let kagemushaVerifierNamespace = "offline_kagemusha"
    private static let confidentialRecordCurve = "pallas"
    private static let confidentialTransferAlgorithmId = "confidential-transfer-v2"
    private static let confidentialTransferEntrypoint = "buildConfidentialTransferProofV2"
    private static let zk1Magic = Data([0x5a, 0x4b, 0x31, 0x00])
    private static let zk1MaxTlvBytes = 8 * 1024 * 1024
    private static let zk1MaxInstanceColumns = 64
    private static let zk1MaxInstanceRows = 8192
    static let foldStepMaxInputs = 2
    static let pallasOpenEnvelopesSchemaHash: [UInt8] = [
        0xfe, 0x38, 0x26, 0x32,
        0x8f, 0x08, 0x17, 0x71,
        0x75, 0x0f, 0x24, 0xfe,
        0x11, 0x02, 0x60, 0xca
    ]
    private static let pallasCurveId: UInt32 = 1
    private static let pallasOpenEnvelopeMaxK = 24
    private static let pallasOpenEnvelopeMaxN = 1 << pallasOpenEnvelopeMaxK
    private static let pallasOpenEnvelopeMaxTranscriptLabelBytes = 128

    public static func encodeInitRequest(_ request: KagemushaRecursiveSpendInitRequest) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try compactPayloadForRequest(
            request.recordBundle,
            schema: recordBundleWireName,
            field: "recordBundle"
        ))
        writer.writeField(encodeBytesVec(request.pallasOpenEnvelopes))
        writer.writeField(try encodeSpendableNote(request.currentNote))
        writer.writeField(encodeOptionRaw(verifyingKeyBoxPayload(request.lineageVerifierKey)))
        writer.writeField(encodeOptionBytesVec(request.lineageProvingKeyArchive))
        writer.writeField(encodeOptionUInt64(request.blockHeight))
        return noritoEncode(typeName: initRequestWireName, payload: writer.data, flags: requestFlags)
    }

    public static func encodeTopUpInitRequest(_ request: KagemushaRecursiveSpendTopUpInitRequest) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try compactPayloadForRequest(
            request.recordBundle,
            schema: recordBundleWireName,
            field: "recordBundle"
        ))
        writer.writeField(encodeBytesVec(request.pallasOpenEnvelopes))
        writer.writeField(try encodeSpendableNote(request.currentNote))
        writer.writeField(encodeOptionRaw(nil))
        writer.writeField(encodeOptionBytesVec(nil))
        writer.writeField(encodeOptionUInt64(request.blockHeight))
        return noritoEncode(typeName: initRequestWireName, payload: writer.data, flags: requestFlags)
    }

    public static func encodeTopUpRequest(_ request: KagemushaRecursiveSpendTopUpRequest) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try assetIdPayload(request.assetId))
        writer.writeField(try encodeNumeric(request.amount))
        writer.writeField(try compactPayloadForRequest(
            request.initRequestArchive,
            schema: initRequestWireName,
            field: "initRequestArchive"
        ))
        return noritoEncode(typeName: topUpRequestWireName, payload: writer.data, flags: requestFlags)
    }

    public static func encodeTopUpRequestFromInitRequest(
        accountId: String,
        initRequestArchive: Data,
        dataspaceId: UInt64? = nil
    ) throws -> Data {
        let summary = try topUpInitRequestSummary(initRequestArchive)
        return try encodeTopUpRequest(KagemushaRecursiveSpendTopUpRequest(
            accountId: accountId,
            assetDefinitionId: summary.assetDefinitionId,
            amount: summary.amount,
            initRequestArchive: initRequestArchive,
            dataspaceId: dataspaceId
        ))
    }

    public static func topUpInitRequestSummary(
        _ initRequestArchive: Data
    ) throws -> KagemushaRecursiveSpendTopUpInitRequestSummary {
        try readTopUpInitRequestSummary(initRequestArchive)
    }

    public static func encodeAppendRequest(_ request: KagemushaRecursiveSpendAppendRequest) throws -> Data {
        let normalizedOutput = KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(
            request.outputProofCircuitId
        )
        var previousRecordPayload: Data?
        if let previousLineageVerifierRecord = request.previousLineageVerifierRecord {
            previousRecordPayload = try compactPayloadForRequest(
                previousLineageVerifierRecord.recordBytes,
                schema: verifyingKeyRecordWireName,
                field: "previousLineageVerifierRecord"
            )
        }

        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try compactPayloadForRequest(
            request.previousBundle,
            schema: bundleWireName,
            field: "previousBundle"
        ))
        writer.writeField(try compactPayloadForRequest(
            request.recordBundle,
            schema: recordBundleWireName,
            field: "recordBundle"
        ))
        writer.writeField(encodeBytesVec(request.pallasOpenEnvelopes))
        writer.writeField(try encodeSpendableNote(request.currentNote))
        writer.writeField(OfflineCompactNorito.encodeString(normalizedOutput))
        writer.writeField(encodeOptionRaw(previousRecordPayload))
        writer.writeField(encodeBytesVec(request.previousProofOpenEnvelopes ?? Data()))
        writer.writeField(encodeOptionRaw(request.lineageVerifierKey.map(verifyingKeyBoxPayload)))
        writer.writeField(encodeOptionBytesVec(request.lineageProvingKeyArchive))
        writer.writeField(encodeOptionUInt64(request.blockHeight))
        return noritoEncode(typeName: appendRequestWireName, payload: writer.data, flags: requestFlags)
    }

    public static func encodeVerifyRequest(_ request: KagemushaRecursiveSpendVerifyRequest) throws -> Data {
        let bundleSummary = try decodeBundle(request.bundle)
        guard !KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId)
            || request.lineageVerifierRecord != nil
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierRecord")
        }
        guard KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId)
            || request.lineageVerifierRecord == nil
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierRecord")
        }
        var lineageRecordPayload: Data?
        if let lineageVerifierRecord = request.lineageVerifierRecord {
            lineageRecordPayload = try compactPayloadForRequest(
                lineageVerifierRecord.recordBytes,
                schema: verifyingKeyRecordWireName,
                field: "lineageVerifierRecord"
            )
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try compactPayloadForRequest(
            request.bundle,
            schema: bundleWireName,
            field: "bundle"
        ))
        writer.writeField(encodeOptionRaw(lineageRecordPayload))
        writer.writeField(encodeOptionUInt64(request.blockHeight))
        return noritoEncode(typeName: verifyRequestWireName, payload: writer.data, flags: requestFlags)
    }

    public static func encodeRedeemRequest(_ request: KagemushaRecursiveSpendRedeemRequest) throws -> Data {
        var lineageWitnessPayload: Data?
        if let lineageWitness = request.lineageWitness {
            lineageWitnessPayload = try compactPayloadForRequest(
                lineageWitness,
                schema: lineageWitnessWireName,
                field: "lineageWitness"
            )
        }
        var lineageRecordPayload: Data?
        if let lineageVerifierRecord = request.lineageVerifierRecord {
            lineageRecordPayload = try compactPayloadForRequest(
                lineageVerifierRecord.recordBytes,
                schema: verifyingKeyRecordWireName,
                field: "lineageVerifierRecord"
            )
        }
        let lineageRecordPayloads = try request.lineageVerifierRecords.map {
            try compactPayloadForRequest(
                $0.recordBytes,
                schema: verifyingKeyRecordWireName,
                field: "lineageVerifierRecords"
            )
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try compactPayloadForRequest(
            request.bundle,
            schema: bundleWireName,
            field: "bundle"
        ))
        writer.writeField(try accountIdPayload(request.recipient))
        writer.writeField(try encodeU128(request.publicAmount))
        writer.writeField(try compactPayloadForRequest(
            request.redeemProof,
            schema: proofAttachmentWireName,
            field: "redeemProof"
        ))
        writer.writeField(encodeOptionRaw(lineageWitnessPayload))
        writer.writeField(try encodeOptionFixed32(request.changeOutput, field: "changeOutput"))
        writer.writeField(encodeOptionRaw(lineageRecordPayload))
        writer.writeField(encodeOptionUInt64(request.blockHeight))
        writer.writeField(encodeRawVec(lineageRecordPayloads))
        return noritoEncode(typeName: redeemRequestWireName, payload: writer.data, flags: requestFlags)
    }

    public static func decodeVerifyResult(_ archive: Data) throws -> KagemushaRecursiveSpendVerifyResult {
        let payload = try payloadArchive(archive, schema: verifyResultWireName, field: "verifyResult")
        guard payload.flags == requestFlags else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("verifyResult")
        }
        var reader = CompactReader(data: payload.payload)
        let valid = try readField(&reader, readBool)
        let hopCount = try readField(&reader) { try $0.readUInt32LE() }
        let encodedBytes = try readField(&reader) { try $0.readUInt32LE() }
        let reason = try readField(&reader, readString)
        let chainAdmissible = try readField(&reader, readBool)
        let chainAdmissionReason = try readField(&reader, readString)
        let witnesslessRedeemSupported = reader.remaining == 0 ? false : try readField(&reader, readBool)
        let lineageWitnessRequiredForRedeem = reader.remaining == 0 ? false : try readField(&reader, readBool)
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("verifyResult")
        }
        return KagemushaRecursiveSpendVerifyResult(
            valid: valid,
            hopCount: hopCount,
            encodedBytes: encodedBytes,
            reason: reason,
            chainAdmissible: chainAdmissible,
            chainAdmissionReason: chainAdmissionReason,
            witnesslessRedeemSupported: witnesslessRedeemSupported,
            lineageWitnessRequiredForRedeem: lineageWitnessRequiredForRedeem
        )
    }

    public static func decodeBundle(_ archive: Data) throws -> KagemushaRecursiveSpendBundleSummary {
        let payload = try payloadArchive(archive, schema: bundleWireName, field: "bundle")
        guard payload.flags == requestFlags else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle")
        }
        var reader = CompactReader(data: payload.payload)
        let accumulatorPayload = try reader.readField()
        let proofPayload = try reader.readField()
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle")
        }
        let accumulator = try readAccumulatorSummary(accumulatorPayload)
        let proofCircuitId = try readRecursiveProofCircuitId(proofPayload)
        guard KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(proofCircuitId)
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "bundle.proof_circuit_id"
            )
        }
        return try KagemushaRecursiveSpendBundleSummary(
            hopCount: accumulator.hopCount,
            proofCircuitId: proofCircuitId,
            asset: accumulator.asset,
            chainId: accumulator.chainId,
            initialRoot: accumulator.initialRoot,
            finalRoot: accumulator.finalRoot,
            topupAnchorNullifiers: accumulator.topupAnchorNullifiers,
            currentNote: accumulator.currentNote
        )
    }

    public static func buildPallasOpenEnvelopesArchive(
        hops: [KagemushaVerifiedFoldHopEvidence]
    ) throws -> Data {
        try buildPallasOpenEnvelopesArchiveForRecordBundle(
            buildVerifiedFoldRecordBundle(hops: hops)
        )
    }

    public static func buildPallasOpenEnvelopesArchiveForRecordBundle(
        _ recordBundle: Data
    ) throws -> Data {
        _ = try compactPayloadForRequest(
            recordBundle,
            schema: recordBundleWireName,
            field: "recordBundle"
        )
        return try KagemushaRecursiveSpendProver.buildPallasOpenEnvelopesArchive(
            recordBundleArchive: recordBundle
        )
    }

    public static func buildVerifiedFoldRecordBundle(
        hops: [KagemushaVerifiedFoldHopEvidence]
    ) throws -> Data {
        guard !hops.isEmpty,
              hops.count <= Int(KagemushaRecursiveSpendProver.compactTokenMaxHops) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("hops")
        }
        let prepared = try hops.enumerated().map { try prepareTransferHop(index: $0.offset, hop: $0.element) }
        let chainId = prepared[0].chainId
        let asset = prepared[0].assetDefinitionId
        var expectedRootBefore: Data?
        for (index, hop) in prepared.enumerated() {
            guard hop.chainId == chainId else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("hops[\(index)].chainId")
            }
            guard hop.assetDefinitionId == asset else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("hops[\(index)].assetDefinitionId")
            }
            if let previousRoot = expectedRootBefore {
                guard hop.publicInputs.rootBefore == previousRoot else {
                    throw KagemushaRecursiveSpendRequestCodecError.invalidField("hops[\(index)].rootBefore")
                }
            }
            guard hop.publicInputs.rootBefore != hop.rootAfter else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("hops[\(index)].rootAfter")
            }
            expectedRootBefore = hop.rootAfter
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try verifiedFoldBundlePayload(prepared))
        writer.writeField(try verifiedFoldVerifierRecordsPayload(prepared))
        return noritoEncode(typeName: recordBundleWireName, payload: writer.data, flags: requestFlags)
    }

    public static func encodeConfidentialTransferV2VerifierRecordArchive(
        verifierKey: Data,
        maxProofBytes: UInt32 = 196_608
    ) throws -> Data {
        guard !verifierKey.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("verifierKey")
        }
        guard maxProofBytes > 0,
              maxProofBytes <= UInt32(confidentialV2MaxProofBytes) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("maxProofBytes")
        }
        let commitment = try verifyingKeyCommitment(
            backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
            bytes: verifierKey
        )
        let schemaHash = IrohaHash.hash(
            PrivacyConfidentialWitnessCodecs.confidentialTransferPublicInputsSchema()
        )
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeUInt32(1))
        writer.writeField(OfflineCompactNorito.encodeString(confidentialTransferV2CircuitId))
        writer.writeField(encodeOptionString(nil))
        writer.writeField(OfflineCompactNorito.encodeString(kagemushaVerifierNamespace))
        writer.writeField(OfflineCompactNorito.encodeUInt32(backendTagHalo2IpaPasta))
        writer.writeField(OfflineCompactNorito.encodeString(confidentialRecordCurve))
        writer.writeField(schemaHash)
        writer.writeField(commitment)
        writer.writeField(OfflineCompactNorito.encodeUInt32(UInt32(verifierKey.count)))
        writer.writeField(OfflineCompactNorito.encodeUInt32(maxProofBytes))
        writer.writeField(encodeOptionString(nil))
        writer.writeField(encodeOptionString(nil))
        writer.writeField(encodeOptionString(nil))
        writer.writeField(encodeOptionUInt64(nil))
        writer.writeField(encodeOptionUInt64(nil))
        writer.writeField(encodeOptionRaw(verifyingKeyBoxPayload(
            backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
            bytes: verifierKey
        )))
        writer.writeField(OfflineCompactNorito.encodeUInt32(confidentialStatusActive))
        return noritoEncode(
            typeName: verifyingKeyRecordWireName,
            payload: writer.data,
            flags: requestFlags
        )
    }
}

public extension KagemushaRecursiveSpendProver {
    static func initSpend(request: KagemushaRecursiveSpendInitRequest) throws -> Data {
        try initSpend(requestArchive: KagemushaRecursiveSpendRequestCodecs.encodeInitRequest(request))
    }

    static func appendSpend(request: KagemushaRecursiveSpendAppendRequest) throws -> Data {
        try appendSpend(requestArchive: KagemushaRecursiveSpendRequestCodecs.encodeAppendRequest(request))
    }

    static func verifySpend(
        request: KagemushaRecursiveSpendVerifyRequest
    ) throws -> KagemushaRecursiveSpendVerifyResult {
        try KagemushaRecursiveSpendRequestCodecs.decodeVerifyResult(
            verifySpend(requestArchive: KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(request))
        )
    }

    static func redeemSpend(request: KagemushaRecursiveSpendRedeemRequest) throws -> Data {
        try redeemSpend(requestArchive: KagemushaRecursiveSpendRequestCodecs.encodeRedeemRequest(request))
    }

    static func topUpSpend(request: KagemushaRecursiveSpendTopUpRequest) throws -> Data {
        try topUpSpend(requestArchive: KagemushaRecursiveSpendRequestCodecs.encodeTopUpRequest(request))
    }
}

struct ArchivePayload {
    let payload: Data
    let flags: UInt8
}

private struct AccumulatorSummary {
    let chainId: String
    let asset: String
    let initialRoot: Data
    let finalRoot: Data
    let topupAnchorNullifiers: [Data]
    let hopCount: Int
    let currentNote: KagemushaRecursiveSpendableNoteDescriptor
}

private struct KagemushaPrivacyBuildResult {
    let proof: Data
}

private struct KagemushaOpenVerifyEnvelopeValue {
    let archive: Data
    let circuitId: String
    let vkHash: Data
    let publicInputs: Data
    let proofBytes: Data
}

private struct KagemushaTransferPublicInputs {
    let rootBefore: Data
    let inputNullifiers: [Data]
    let outputCommitments: [Data]
}

private struct KagemushaVerifierKeyIdValue: Equatable {
    let backend: String
    let name: String
}

private struct KagemushaVerifyingKeyBoxValue {
    let backend: String
    let bytes: Data
}

private struct KagemushaDecodedVerifierRecord {
    let circuitId: String
    let namespace: String
    let backendTag: UInt32
    let curve: String
    let publicInputsSchemaHash: Data
    let commitment: Data
    let vkLen: UInt32
    let maxProofBytes: UInt32
    let key: KagemushaVerifyingKeyBoxValue?
    let status: UInt32
}

private struct KagemushaVerifierRecordValue {
    let id: KagemushaVerifierKeyIdValue
    let recordPayload: Data
    let commitment: Data
    let key: KagemushaVerifyingKeyBoxValue
}

private struct KagemushaPreparedVerifiedFoldHop {
    let chainId: String
    let assetDefinitionId: String
    let rootAfter: Data
    let publicInputs: KagemushaTransferPublicInputs
    let envelope: KagemushaOpenVerifyEnvelopeValue
    let verifierRecord: KagemushaVerifierRecordValue
}

extension KagemushaRecursiveSpendRequestCodecs {
    private static let lineageKeyArtifactValidationOpeningLen: UInt32 = 2

    static func payloadArchive(_ archive: Data, schema: String, field: String) throws -> ArchivePayload {
        guard !archive.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        guard archive.count <= KagemushaRecursiveSpendProver.nativeArchiveMaxBytes else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        guard let frame = noritoDecodeFrame(archive),
              frame.header.schema == noritoSchemaHash(forTypeName: schema),
              frame.header.compression == .none,
              frame.header.length > 0
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return ArchivePayload(payload: frame.payload, flags: frame.header.flags)
    }

    static func compactPayloadForRequest(_ archive: Data, schema: String, field: String) throws -> Data {
        let payload = try payloadArchive(archive, schema: schema, field: field)
        guard payload.flags == requestFlags else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return payload.payload
    }

    private static func prepareTransferHop(
        index: Int,
        hop: KagemushaVerifiedFoldHopEvidence
    ) throws -> KagemushaPreparedVerifiedFoldHop {
        let proof = try parsePrivacyBuildResult(
            hop.proofOutputArchive,
            expectedAlgorithmId: confidentialTransferAlgorithmId,
            expectedEntrypoint: confidentialTransferEntrypoint,
            label: "hops[\(index)].proofOutputArchive"
        )
        let envelope = try decodeOpenVerifyEnvelope(
            proof.proof,
            label: "hops[\(index)].proof"
        )
        let publicInputs = try parseTransferPublicInputs(
            envelope.proofBytes,
            label: "hops[\(index)]"
        )
        guard publicInputs.rootBefore.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("hops[\(index)].rootBefore")
        }
        let verifierRecord = try decodeAndValidateVerifierRecord(
            hop.verifierRecord,
            envelope: envelope,
            expectedCircuitId: confidentialTransferV2CircuitId,
            expectedSchema: PrivacyConfidentialWitnessCodecs.confidentialTransferPublicInputsSchema(),
            proofArchiveSize: proof.proof.count,
            label: "hops[\(index)].verifierRecord"
        )
        return KagemushaPreparedVerifiedFoldHop(
            chainId: hop.chainId,
            assetDefinitionId: hop.assetDefinitionId,
            rootAfter: hop.rootAfter,
            publicInputs: publicInputs,
            envelope: envelope,
            verifierRecord: verifierRecord
        )
    }

    private static func parsePrivacyBuildResult(
        _ archive: Data,
        expectedAlgorithmId: String,
        expectedEntrypoint: String,
        label: String
    ) throws -> KagemushaPrivacyBuildResult {
        let payload = try requirePrivacyBuildResultPayload(archive, label: label)
        guard payload.flags == requestFlags else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(label)
        }
        var reader = CompactReader(data: payload.payload)
        let version = try readField(&reader, field: "\(label).version") { try $0.readUInt32LE() }
        let status = try readField(&reader, field: "\(label).status") { try $0.readUInt32LE() }
        let errorCode = try readField(&reader, field: "\(label).error_code") { try $0.readUInt32LE() }
        let message = try readField(&reader, readString)
        let algorithmId = try readField(&reader, readString)
        let entrypoint = try readField(&reader, readString)
        let vkRef = try readField(&reader, readString)
        let publicInputs = try readField(&reader) { try $0.readByteVec() }
        let proof = try readField(&reader) { try $0.readByteVec() }
        let verified = try readField(&reader, readBool)
        guard reader.remaining == 0,
              version == privacyFfiVersionV1,
              status == privacyFfiStatusOk,
              errorCode == 0,
              message.isEmpty,
              algorithmId == expectedAlgorithmId,
              entrypoint == expectedEntrypoint,
              publicInputs.isEmpty,
              !proof.isEmpty,
              !verified
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(label)
        }
        try requirePortableId(vkRef, field: "\(label).vk_ref")
        return KagemushaPrivacyBuildResult(proof: proof)
    }

    private static func requirePrivacyBuildResultPayload(
        _ archive: Data,
        label: String
    ) throws -> ArchivePayload {
        guard !archive.isEmpty,
              archive.count <= KagemushaRecursiveSpendProver.nativeArchiveMaxBytes,
              let frame = noritoDecodeFrame(archive),
              frame.header.schema.allSatisfy({ $0 == privacySchemaBuildProofResult }),
              frame.header.compression == .none,
              frame.header.length > 0
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(label)
        }
        return ArchivePayload(payload: frame.payload, flags: frame.header.flags)
    }

    private static func decodeOpenVerifyEnvelope(
        _ archive: Data,
        label: String
    ) throws -> KagemushaOpenVerifyEnvelopeValue {
        let payload = try payloadArchive(archive, schema: openVerifyEnvelopeWireName, field: label)
        guard payload.flags == requestFlags else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(label)
        }
        var reader = CompactReader(data: payload.payload)
        let backendTag = try readField(&reader) { try $0.readUInt32LE() }
        let circuitId = try readField(&reader, readString)
        let vkHash = try readField(&reader, field: "\(label).vk_hash") {
            try $0.readFixed32Flexible(field: "\(label).vk_hash")
        }
        let publicInputs = try readField(&reader) { try $0.readByteVec() }
        let proofBytes = try readField(&reader) { try $0.readByteVec() }
        let aux = try readField(&reader) { try $0.readByteVec() }
        guard reader.remaining == 0,
              backendTag == backendTagHalo2IpaPasta,
              vkHash.contains(where: { $0 != 0 }),
              !proofBytes.isEmpty,
              aux.isEmpty
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(label)
        }
        try requirePortableId(circuitId, field: "\(label).circuit_id")
        return KagemushaOpenVerifyEnvelopeValue(
            archive: archive,
            circuitId: circuitId,
            vkHash: vkHash,
            publicInputs: publicInputs,
            proofBytes: proofBytes
        )
    }

    private static func decodeAndValidateVerifierRecord(
        _ ref: KagemushaRecursiveSpendVerifierRecordRef,
        envelope: KagemushaOpenVerifyEnvelopeValue,
        expectedCircuitId: String,
        expectedSchema: Data,
        proofArchiveSize: Int,
        label: String
    ) throws -> KagemushaVerifierRecordValue {
        let recordPayload = try compactPayloadForRequest(
            ref.recordBytes,
            schema: verifyingKeyRecordWireName,
            field: label
        )
        let record = try decodeVerifierRecordPayload(recordPayload, label: label)
        let id = try parseVerifierKeyId(ref.verifierKeyId, field: "\(label).verifierKeyId")
        guard id.backend == KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
              record.status == confidentialStatusActive,
              record.namespace == kagemushaVerifierNamespace,
              record.backendTag == backendTagHalo2IpaPasta,
              record.curve == confidentialRecordCurve,
              record.circuitId == expectedCircuitId,
              envelope.circuitId == expectedCircuitId,
              envelope.publicInputs == expectedSchema,
              record.publicInputsSchemaHash == IrohaHash.hash(expectedSchema),
              record.commitment == envelope.vkHash,
              record.maxProofBytes > 0,
              record.maxProofBytes <= UInt32(confidentialV2MaxProofBytes),
              proofArchiveSize <= Int(record.maxProofBytes)
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(label)
        }
        guard let key = record.key,
              key.backend == KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
              !key.bytes.isEmpty,
              record.vkLen == UInt32(key.bytes.count),
              record.commitment == (try? verifyingKeyCommitment(backend: key.backend, bytes: key.bytes))
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).key")
        }
        return KagemushaVerifierRecordValue(
            id: id,
            recordPayload: recordPayload,
            commitment: record.commitment,
            key: key
        )
    }

    private static func decodeVerifierRecordPayload(
        _ payload: Data,
        label: String
    ) throws -> KagemushaDecodedVerifierRecord {
        var reader = CompactReader(data: payload)
        _ = try readField(&reader, field: "\(label).version") { try $0.readUInt32LE() }
        let circuitId = try readField(&reader, readString)
        _ = try readField(&reader) { try readOptionStringPayload(&$0) }
        let namespace = try readField(&reader, readString)
        let backendTag = try readField(&reader) { try $0.readUInt32LE() }
        let curve = try readField(&reader, readString)
        let schemaHash = try readField(&reader, field: "\(label).public_inputs_schema_hash") {
            try $0.readFixed32Flexible(field: "\(label).public_inputs_schema_hash")
        }
        let commitment = try readField(&reader, field: "\(label).commitment") {
            try $0.readFixed32Flexible(field: "\(label).commitment")
        }
        let vkLen = try readField(&reader, field: "\(label).vk_len") { try $0.readUInt32LE() }
        let maxProofBytes = try readField(&reader, field: "\(label).max_proof_bytes") {
            try $0.readUInt32LE()
        }
        _ = try readField(&reader) { try readOptionStringPayload(&$0) }
        _ = try readField(&reader) { try readOptionStringPayload(&$0) }
        _ = try readField(&reader) { try readOptionStringPayload(&$0) }
        _ = try readField(&reader) { try readOptionUInt64Payload(&$0) }
        _ = try readField(&reader) { try readOptionUInt64Payload(&$0) }
        let key = try readField(&reader) { child -> KagemushaVerifyingKeyBoxValue? in
            guard let keyPayload = try readOptionRawPayload(&child, field: "\(label).key") else {
                return nil
            }
            return try readVerifyingKeyBoxPayload(keyPayload, label: "\(label).key")
        }
        let status = try readField(&reader, field: "\(label).status") { try $0.readUInt32LE() }
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(label)
        }
        return KagemushaDecodedVerifierRecord(
            circuitId: circuitId,
            namespace: namespace,
            backendTag: backendTag,
            curve: curve,
            publicInputsSchemaHash: schemaHash,
            commitment: commitment,
            vkLen: vkLen,
            maxProofBytes: maxProofBytes,
            key: key,
            status: status
        )
    }

    private static func readVerifyingKeyBoxPayload(
        _ payload: Data,
        label: String
    ) throws -> KagemushaVerifyingKeyBoxValue {
        var reader = CompactReader(data: payload)
        let backend = try readField(&reader, readString)
        let bytes = try readField(&reader) { try $0.readByteVec() }
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(label)
        }
        return KagemushaVerifyingKeyBoxValue(backend: backend, bytes: bytes)
    }

    private static func parseTransferPublicInputs(
        _ proofBytes: Data,
        label: String
    ) throws -> KagemushaTransferPublicInputs {
        let columns = try parseZk1InstanceColumns(proofBytes, label: label)
        guard columns.count == 9,
              columns.allSatisfy({ $0.count == 1 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
        }
        return KagemushaTransferPublicInputs(
            rootBefore: columns[6][0],
            inputNullifiers: try nonZeroSorted([columns[2][0], columns[3][0]], field: "\(label).inputNullifiers"),
            outputCommitments: try nonZeroSorted([columns[4][0], columns[5][0]], field: "\(label).outputCommitments")
        )
    }

    private static func parseZk1InstanceColumns(
        _ proofBytes: Data,
        label: String
    ) throws -> [[Data]] {
        guard proofBytes.count >= zk1Magic.count,
              proofBytes.prefix(zk1Magic.count) == zk1Magic else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
        }
        var offset = zk1Magic.count
        var sawProof = false
        var columns: [[Data]]?
        while offset < proofBytes.count {
            guard offset + 8 <= proofBytes.count else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
            }
            let tag = Data(proofBytes[offset..<(offset + 4)])
            let length = try readUInt32LittleEndian(proofBytes, offset: offset + 4, field: "\(label).proof")
            guard length <= zk1MaxTlvBytes else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
            }
            let start = offset + 8
            let end = start + length
            guard end <= proofBytes.count else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
            }
            let payload = Data(proofBytes[start..<end])
            switch String(data: tag, encoding: .ascii) {
            case "PROF":
                guard !sawProof, !payload.isEmpty else {
                    throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
                }
                sawProof = true
            case "I10P":
                guard columns == nil else {
                    throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
                }
                columns = try readZk1InstanceColumnsPayload(payload, label: label)
            default:
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
            }
            offset = end
        }
        guard sawProof, let columns else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
        }
        return columns
    }

    private static func readZk1InstanceColumnsPayload(
        _ payload: Data,
        label: String
    ) throws -> [[Data]] {
        guard payload.count >= 8 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
        }
        let columnCount = try readUInt32LittleEndian(payload, offset: 0, field: "\(label).proof")
        let rowCount = try readUInt32LittleEndian(payload, offset: 4, field: "\(label).proof")
        guard columnCount > 0,
              rowCount > 0,
              columnCount <= zk1MaxInstanceColumns,
              rowCount <= zk1MaxInstanceRows else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
        }
        let expected = 8 + columnCount * rowCount * 32
        guard payload.count == expected else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(label).proof")
        }
        var columns = Array(repeating: [Data](), count: columnCount)
        var offset = 8
        for _ in 0..<rowCount {
            for column in 0..<columnCount {
                columns[column].append(Data(payload[offset..<(offset + 32)]))
                offset += 32
            }
        }
        return columns
    }

    private static func verifiedFoldBundlePayload(
        _ hops: [KagemushaPreparedVerifiedFoldHop]
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(chainIdPayload(hops[0].chainId))
        writer.writeField(try assetDefinitionIdPayload(hops[0].assetDefinitionId))
        writer.writeField(try verifiedFoldStepsPayload(hops))
        return writer.data
    }

    private static func verifiedFoldStepsPayload(
        _ hops: [KagemushaPreparedVerifiedFoldHop]
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(hops.count))
        for hop in hops {
            var step = OfflineCompactNoritoWriter()
            step.writeField(try encodePackedFixed32(hop.publicInputs.rootBefore, field: "rootBefore"))
            step.writeField(try encodeFixed32Vec(hop.publicInputs.inputNullifiers, field: "inputNullifiers"))
            step.writeField(try encodeFixed32Vec(hop.publicInputs.outputCommitments, field: "outputCommitments"))
            step.writeField(try encodePackedFixed32(hop.rootAfter, field: "rootAfter"))
            step.writeField(try proofAttachmentPayload(envelope: hop.envelope, verifierRecord: hop.verifierRecord))
            step.writeField(verifyingKeyBoxPayload(
                backend: hop.verifierRecord.key.backend,
                bytes: hop.verifierRecord.key.bytes
            ))
            writer.writeField(step.data)
        }
        return writer.data
    }

    private static func verifiedFoldVerifierRecordsPayload(
        _ hops: [KagemushaPreparedVerifiedFoldHop]
    ) throws -> Data {
        var unique: [KagemushaVerifierRecordValue] = []
        for hop in hops {
            if !unique.contains(where: { $0.id == hop.verifierRecord.id }) {
                unique.append(hop.verifierRecord)
            }
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(unique.count))
        for record in unique {
            var entry = OfflineCompactNoritoWriter()
            entry.writeField(verifierKeyIdPayload(record.id))
            entry.writeField(record.recordPayload)
            writer.writeField(entry.data)
        }
        return writer.data
    }

    private static func proofAttachmentPayload(
        envelope: KagemushaOpenVerifyEnvelopeValue,
        verifierRecord: KagemushaVerifierRecordValue
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(KagemushaRecursiveSpendProver.recursiveAggregationProofBackend))
        writer.writeField(proofBoxPayload(envelope.archive))
        writer.writeField(verifierKeyIdPayload(verifierRecord.id))
        writer.writeField(try encodeOptionFixed32(verifierRecord.commitment, field: "vkCommitment"))
        writer.writeField(try encodeOptionFixed32(IrohaHash.hash(envelope.archive), field: "envelopeHash"))
        // Rust canonical encoding omits the absent trailing default `lane_privacy` field.
        return writer.data
    }

    private static func proofBoxPayload(_ proofBytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(KagemushaRecursiveSpendProver.recursiveAggregationProofBackend))
        writer.writeField(encodeBytesVec(proofBytes))
        return writer.data
    }

    private static func verifierKeyIdPayload(_ id: KagemushaVerifierKeyIdValue) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(id.backend))
        writer.writeField(OfflineCompactNorito.encodeString(id.name))
        return writer.data
    }

    private static func chainIdPayload(_ chainId: String) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(chainId))
        return writer.data
    }

    private static func assetDefinitionIdPayload(_ assetDefinitionId: String) throws -> Data {
        guard let bytes = AssetDefinitionAddress.decode(assetDefinitionId) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("assetDefinitionId")
        }
        // AssetDefinitionId delegates to `[u8; 16]`, whose canonical Norito representation keeps
        // per-element framing (unlike protocol-special direct `[u8; 32]` fields).
        return encodeConstVec(bytes)
    }

    private static func encodeFixed32Vec(_ values: [Data], field: String) throws -> Data {
        guard !values.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        for value in values {
            try requireFixed32(value, field: field)
            writer.writeField(encodeConstVec(value))
        }
        return writer.data
    }

    private static func encodeOptionString(_ value: String?) -> Data {
        guard let value else {
            return encodeOptionRaw(nil)
        }
        return encodeOptionRaw(OfflineCompactNorito.encodeString(value))
    }

    private static func parseVerifierKeyId(
        _ value: String,
        field: String
    ) throws -> KagemushaVerifierKeyIdValue {
        try requirePortableId(value, field: field)
        guard let separator = value.firstIndex(of: ":"),
              separator != value.startIndex,
              separator != value.index(before: value.endIndex) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
        let backend = String(value[..<separator])
        let name = String(value[value.index(after: separator)...])
        try requirePortableId(backend, field: "\(field).backend")
        try requirePortableId(name, field: "\(field).name")
        return KagemushaVerifierKeyIdValue(backend: backend, name: name)
    }

    private static func readOptionStringPayload(_ reader: inout CompactReader) throws -> String? {
        guard let payload = try readOptionRawPayload(&reader, field: "optionString") else {
            return nil
        }
        var child = CompactReader(data: payload)
        let value = try readString(&child)
        guard child.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("optionString")
        }
        return value
    }

    private static func readOptionUInt64Payload(_ reader: inout CompactReader) throws -> UInt64? {
        guard let payload = try readOptionRawPayload(&reader, field: "optionU64") else {
            return nil
        }
        var child = CompactReader(data: payload)
        let value = try child.readUInt64LE()
        guard child.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("optionU64")
        }
        return value
    }

    private static func verifyingKeyCommitment(backend: String, bytes: Data) throws -> Data {
        try requireNonBlankUnpadded(backend, field: "verifierKeyBackend")
        guard !bytes.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("verifierKey")
        }
        var preimage = Data("iroha:zk:v1:vk".utf8)
        appendUInt64BE(UInt64(backend.utf8.count), to: &preimage)
        preimage.append(Data(backend.utf8))
        appendUInt64BE(UInt64(bytes.count), to: &preimage)
        preimage.append(bytes)
        return Data(SHA256.hash(data: preimage))
    }

    private static func appendUInt64BE(_ value: UInt64, to data: inout Data) {
        data.append(UInt8((value >> 56) & 0xff))
        data.append(UInt8((value >> 48) & 0xff))
        data.append(UInt8((value >> 40) & 0xff))
        data.append(UInt8((value >> 32) & 0xff))
        data.append(UInt8((value >> 24) & 0xff))
        data.append(UInt8((value >> 16) & 0xff))
        data.append(UInt8((value >> 8) & 0xff))
        data.append(UInt8(value & 0xff))
    }

    private static func readUInt32LittleEndian(
        _ bytes: Data,
        offset: Int,
        field: String
    ) throws -> Int {
        guard offset >= 0, offset + 4 <= bytes.count else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let value = UInt32(bytes[offset])
            | (UInt32(bytes[offset + 1]) << 8)
            | (UInt32(bytes[offset + 2]) << 16)
            | (UInt32(bytes[offset + 3]) << 24)
        guard UInt64(value) <= UInt64(Int.max) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return Int(value)
    }

    private static func nonZeroSorted(_ values: [Data], field: String) throws -> [Data] {
        let filtered = try values.map {
            try fixed32($0, field: field)
        }.filter { value in
            value.contains(where: { $0 != 0 })
        }
        guard !filtered.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let sorted = filtered.sorted { $0.lexicographicallyPrecedes($1) }
        for index in 1..<sorted.count where sorted[index - 1] == sorted[index] {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return sorted
    }

    private static func fixed32(_ value: Data, field: String) throws -> Data {
        guard value.count == 32 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return Data(value)
    }

    static func requireNestedArchive(_ archive: Data, field: String) throws {
        guard !archive.isEmpty,
              archive.count <= KagemushaRecursiveSpendProver.nativeArchiveMaxBytes,
              let frame = noritoDecodeFrame(archive),
              frame.header.length > 0
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
    }

    static func validateLineageKeyArtifactsForInit(
        lineageVerifierKey: Data,
        lineageProvingKeyArchive: Data
    ) throws {
        do {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                verifierOpeningLen: lineageKeyArtifactValidationOpeningLen,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
        } catch {
            throw requestCodecError(forLineageKeyArtifactError: error)
        }
    }

    static func validateLineageKeyArtifactsForAppend(
        lineageVerifierKey: Data,
        lineageProvingKeyArchive: Data
    ) throws {
        do {
            _ = try KagemushaRecursiveSpendProver.lineageKeyArtifactsForAppend(
                verifierOpeningLen: lineageKeyArtifactValidationOpeningLen,
                lineageVerifierKeyBackend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
                lineageVerifierKey: lineageVerifierKey,
                lineageProvingKeyArchive: lineageProvingKeyArchive
            )
        } catch {
            throw requestCodecError(forLineageKeyArtifactError: error)
        }
    }

    private static func requestCodecError(
        forLineageKeyArtifactError error: Error
    ) -> KagemushaRecursiveSpendRequestCodecError {
        if case let KagemushaRecursiveSpendProverError.invalidLineageKeyArtifact(field) = error {
            switch field {
            case "lineage_verifier_key":
                return .invalidField("lineageVerifierKey")
            case "lineage_proving_key_archive":
                return .invalidField("lineageProvingKeyArchive")
            default:
                return .invalidField("lineageKeyArtifacts")
            }
        }
        return .invalidField("lineageKeyArtifacts")
    }

    static func readVerifiedFoldRecordBundleHopCount(_ payload: Data, field: String) throws -> Int {
        var decoder = CompactReader(data: payload)
        let bundlePayload = try readField(&decoder) { reader in
            try reader.readBytes(reader.remaining)
        }
        _ = try readField(&decoder) { reader in
            try reader.readBytes(reader.remaining)
        }
        guard decoder.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }

        var bundle = CompactReader(data: bundlePayload)
        try skipFields(&bundle, count: 2)
        let hopCount = try readField(&bundle) { reader in
            try readVerifiedFoldStepCount(&reader, field: "\(field).steps")
        }
        guard bundle.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        guard hopCount > 0,
              hopCount <= Int(KagemushaRecursiveSpendProver.compactTokenMaxHops) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return hopCount
    }

    static func validateTopUpRequestPublicBinding(
        assetId: String,
        amount: String,
        initRequestArchive: Data
    ) throws {
        let summary = try readTopUpInitRequestSummary(initRequestArchive)
        let parsedAsset = try parseAssetId(assetId, field: "assetId")
        guard parsedAsset.assetDefinitionId == summary.assetDefinitionId else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("assetId")
        }
        guard amount == summary.amount else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("amount")
        }
    }

    private static func readTopUpInitRequestSummary(
        _ initRequestArchive: Data
    ) throws -> KagemushaRecursiveSpendTopUpInitRequestSummary {
        let payload = try compactPayloadForRequest(
            initRequestArchive,
            schema: initRequestWireName,
            field: "initRequestArchive"
        )
        var reader = CompactReader(data: payload)
        let recordBundlePayload = try reader.readField()
        let pallasOpenEnvelopes = try readField(&reader, field: "initRequestArchive.pallasOpenEnvelopes") {
            try $0.readByteVec()
        }
        let currentNote = try readField(&reader, field: "initRequestArchive.currentNote") {
            try readSpendableNote(&$0, field: "initRequestArchive.current_note")
        }
        let lineageVerifierKey = try readField(&reader, field: "initRequestArchive.lineageVerifierKey") {
            try readOptionRawPayload(&$0, field: "initRequestArchive.lineage_verifier_key")
        }
        let lineageProvingKeyArchive = try readField(&reader, field: "initRequestArchive.lineageProvingKeyArchive") {
            try readOptionRawPayload(&$0, field: "initRequestArchive.lineage_proving_key_archive")
        }
        _ = try readField(&reader, field: "initRequestArchive.blockHeight") {
            try readOptionUInt64Payload(&$0)
        }
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("initRequestArchive")
        }
        guard lineageVerifierKey == nil else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierKey")
        }
        guard lineageProvingKeyArchive == nil else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageProvingKeyArchive")
        }
        let recordBundle = try readTopUpInitRecordBundleSummary(recordBundlePayload)
        guard recordBundle.hopCount == 1 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("initRequestArchive.recordBundle")
        }
        try requirePallasOpenEnvelopesArchive(
            pallasOpenEnvelopes,
            expectedEnvelopeCount: recordBundle.hopCount,
            field: "initRequestArchive.pallas_open_envelopes",
            maxBytes: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes
        )
        return KagemushaRecursiveSpendTopUpInitRequestSummary(
            assetDefinitionId: recordBundle.assetDefinitionId,
            amount: currentNote.amount
        )
    }

    private struct TopUpInitRecordBundleSummary {
        let assetDefinitionId: String
        let hopCount: Int
    }

    private static func readTopUpInitRecordBundleSummary(
        _ recordBundlePayload: Data
    ) throws -> TopUpInitRecordBundleSummary {
        var decoder = CompactReader(data: recordBundlePayload)
        let bundlePayload = try decoder.readField()
        _ = try decoder.readField()
        guard decoder.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("initRequestArchive.record_bundle")
        }

        var bundle = CompactReader(data: bundlePayload)
        _ = try readField(&bundle, field: "initRequestArchive.record_bundle.chain_id") {
            try readChainIdPayload(&$0)
        }
        let assetBytes = try readField(&bundle, field: "initRequestArchive.record_bundle.asset") {
            try $0.readFixedBytesFlexible(
                expectedCount: 16,
                field: "initRequestArchive.record_bundle.asset"
            )
        }
        guard let assetDefinitionId = AssetDefinitionAddress.encode(uuidBytes: assetBytes) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "initRequestArchive.record_bundle.asset"
            )
        }
        let hopCount = try readField(&bundle, field: "initRequestArchive.record_bundle.steps") {
            try readVerifiedFoldStepCount(&$0, field: "initRequestArchive.record_bundle.steps")
        }
        guard bundle.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("initRequestArchive.record_bundle")
        }
        return TopUpInitRecordBundleSummary(
            assetDefinitionId: assetDefinitionId,
            hopCount: hopCount
        )
    }

    private static func readVerifiedFoldStepCount(
        _ decoder: inout CompactReader,
        field: String
    ) throws -> Int {
        let count64 = try decoder.readUInt64LE()
        guard count64 <= UInt64(Int.max) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        guard count64 <= UInt64(KagemushaRecursiveSpendProver.compactTokenMaxHops) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let count = Int(count64)
        for _ in 0..<count {
            let itemLength = try decoder.readLength()
            var item = CompactReader(data: try decoder.readBytes(itemLength))
            try skipFields(&item, count: 6)
            guard item.remaining == 0 else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
            }
        }
        guard decoder.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return count
    }

    static func requirePallasOpenEnvelopesArchive(
        _ archive: Data,
        expectedEnvelopeCount: Int,
        field: String,
        maxBytes: Int
    ) throws {
        guard !archive.isEmpty,
              archive.count <= maxBytes,
              let frame = noritoDecodeFrame(archive),
              frame.header.schema == pallasOpenEnvelopesSchemaHash,
              frame.header.compression == .none,
              frame.header.length > 0,
              frame.header.flags == requestFlags
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }

        var decoder = CompactReader(data: frame.payload)
        let count64 = try decoder.readUInt64LE()
        guard count64 <= UInt64(Int.max),
              Int(count64) == expectedEnvelopeCount else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        for _ in 0..<Int(count64) {
            let itemLength = try decoder.readLength()
            let itemPayload = try decoder.readBytes(itemLength)
            try validatePallasOpenEnvelopePayload(itemPayload, field: field)
        }
        guard decoder.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
    }

    private static func validatePallasOpenEnvelopePayload(_ payload: Data, field: String) throws {
        var reader = CompactReader(data: payload)
        let paramsN = try readField(&reader) { child in
            try readPallasIpaParams(&child, field: "\(field).params")
        }
        let publicN = try readField(&reader) { child in
            try readPallasPolyOpenPublic(&child, field: "\(field).public")
        }
        guard publicN == paramsN else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        try readField(&reader) { child in
            try readPallasIpaProof(&child, n: paramsN, field: "\(field).proof")
        }
        let transcriptLabel = try readField(&reader, readString)
        guard !transcriptLabel.isEmpty,
              transcriptLabel.utf8.count <= pallasOpenEnvelopeMaxTranscriptLabelBytes else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        try readField(&reader, field: "\(field).vk_commitment") { child in
            try readRequiredMetadataOption(&child, field: "\(field).vk_commitment")
        }
        try readField(&reader, field: "\(field).public_inputs_schema_hash") { child in
            try readRequiredMetadataOption(&child, field: "\(field).public_inputs_schema_hash")
        }
        try readField(&reader, field: "\(field).domain_tag") { child in
            try readRequiredMetadataOption(&child, field: "\(field).domain_tag")
        }
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
    }

    private static func readPallasIpaParams(
        _ reader: inout CompactReader,
        field: String
    ) throws -> Int {
        let version = try readField(&reader) { try $0.readUInt16LE() }
        guard version == 1 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let curveId = try readField(&reader) { UInt32(try $0.readUInt16LE()) }
        guard curveId == pallasCurveId else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let n = try readField(&reader) { try $0.readUInt32LE() }
        guard n >= 2,
              n.nonzeroBitCount == 1,
              n <= UInt32(pallasOpenEnvelopeMaxN) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let intN = Int(n)
        let gCount = try readField(&reader) { child in
            try readFixed32SequenceCount(&child, field: "\(field).g", expectedCount: intN, mismatchField: field)
        }
        guard gCount == intN else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let hCount = try readField(&reader) { child in
            try readFixed32SequenceCount(&child, field: "\(field).h", expectedCount: intN, mismatchField: field)
        }
        guard hCount == intN else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        _ = try readField(&reader) { child in
            try child.readFixedBytes(32)
        }
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return intN
    }

    private static func readPallasPolyOpenPublic(
        _ reader: inout CompactReader,
        field: String
    ) throws -> Int {
        let version = try readField(&reader) { try $0.readUInt16LE() }
        guard version == 1 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let curveId = try readField(&reader) { UInt32(try $0.readUInt16LE()) }
        guard curveId == pallasCurveId else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let n = try readField(&reader) { try $0.readUInt32LE() }
        _ = try readField(&reader) { try $0.readFixedBytes(32) }
        _ = try readField(&reader) { try $0.readFixedBytes(32) }
        _ = try readField(&reader) { try $0.readFixedBytes(32) }
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return Int(n)
    }

    private static func readPallasIpaProof(
        _ reader: inout CompactReader,
        n: Int,
        field: String
    ) throws {
        let version = try readField(&reader) { try $0.readUInt16LE() }
        guard version == 1 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let expectedRounds = n.trailingZeroBitCount
        let lCount = try readField(&reader) { child in
            try readFixed32SequenceCount(
                &child,
                field: "\(field).l",
                expectedCount: expectedRounds,
                mismatchField: field
            )
        }
        let rCount = try readField(&reader) { child in
            try readFixed32SequenceCount(
                &child,
                field: "\(field).r",
                expectedCount: expectedRounds,
                mismatchField: field
            )
        }
        guard lCount == rCount,
              lCount == expectedRounds else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        _ = try readField(&reader) { try $0.readFixedBytes(32) }
        _ = try readField(&reader) { try $0.readFixedBytes(32) }
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
    }

    private static func readFixed32SequenceCount(
        _ reader: inout CompactReader,
        field: String,
        expectedCount: Int? = nil,
        mismatchField: String? = nil
    ) throws -> Int {
        let count64 = try reader.readUInt64LE()
        guard count64 <= UInt64(Int.max) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let count = Int(count64)
        if let expectedCount = expectedCount {
            guard count == expectedCount else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(mismatchField ?? field)
            }
        }
        for _ in 0..<count {
            _ = try readField(&reader, field: field) { child in
                try child.readFixed32Flexible(field: field)
            }
        }
        return count
    }

    private static func readFixed32Sequence(
        _ reader: inout CompactReader,
        field: String,
        maxCount: Int? = nil
    ) throws -> [Data] {
        let count64 = try reader.readUInt64LE()
        guard count64 <= UInt64(Int.max) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let count = Int(count64)
        if let maxCount = maxCount {
            guard count > 0, count <= maxCount else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("\(field) count is out of range")
            }
        }
        var values: [Data] = []
        values.reserveCapacity(count)
        for _ in 0..<count {
            values.append(try readField(&reader, field: field) { child in
                try child.readFixed32Flexible(field: field)
            })
        }
        return values
    }

    private static func readRequiredMetadataOption(
        _ reader: inout CompactReader,
        field: String
    ) throws {
        guard let payload = try readOptionRawPayload(&reader, field: field) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        var child = CompactReader(data: payload)
        let value = try child.readFixed32Flexible(field: field)
        guard child.remaining == 0,
              value.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
    }

    private static func readOptionRawPayload(
        _ reader: inout CompactReader,
        field: String
    ) throws -> Data? {
        switch try reader.readUInt8() {
        case 0:
            return nil
        case 1:
            let length = try reader.readLength()
            guard reader.remaining >= length else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
            }
            return try reader.readBytes(length)
        default:
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
    }

    static func canonicalU128Decimal(_ value: String, field: String) throws -> String {
        guard !value.isEmpty, value.allSatisfy({ $0 >= "0" && $0 <= "9" }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
        guard value.count == 1 || value.first != "0" else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
        guard value != "0" else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
        let max = "340282366920938463463374607431768211455"
        guard value.count < max.count || (value.count == max.count && value <= max) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
        return value
    }

    static func requirePortableId(_ value: String, field: String) throws {
        try requireNonBlankUnpadded(value, field: field)
        guard value.count <= 256,
              value.unicodeScalars.allSatisfy({ scalar in
                  (scalar.value >= 0x30 && scalar.value <= 0x39)
                      || (scalar.value >= 0x41 && scalar.value <= 0x5a)
                      || (scalar.value >= 0x61 && scalar.value <= 0x7a)
                      || ". _-/::@+=".replacingOccurrences(of: " ", with: "").unicodeScalars.contains(scalar)
              })
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
    }

    static func requireNonBlankUnpadded(_ value: String, field: String) throws {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed == value else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
    }

    static func requireFixed32(_ value: Data, field: String) throws {
        guard value.count == 32, value.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
    }

    static func requireRedeemChangeBinding(
        publicAmount: String,
        currentAmount: String,
        hasChangeOutput: Bool
    ) throws {
        let comparison = compareCanonicalDecimal(publicAmount, currentAmount)
        if hasChangeOutput {
            guard comparison < 0 else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("publicAmount")
            }
        } else {
            if comparison < 0 {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("changeOutput")
            }
            if comparison > 0 {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("publicAmount")
            }
        }
    }

    static func requireRedeemChangeOutputNotReserved(
        _ changeOutput: Data,
        bundleSummary: KagemushaRecursiveSpendBundleSummary
    ) throws {
        let reserved = [bundleSummary.currentNote.noteCommitment, bundleSummary.currentNote.spendNullifier]
            + bundleSummary.topupAnchorNullifiers
        guard !reserved.contains(changeOutput) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("changeOutput")
        }
    }

    private static func requireTopupAnchorNullifiers(
        _ nullifiers: [Data],
        currentNote: KagemushaRecursiveSpendableNoteDescriptor
    ) throws {
        let countMessage = "bundle.accumulator.topup_anchor_nullifiers count is out of range"
        let zeroMessage = "bundle.accumulator.topup_anchor_nullifiers must not contain zero values"
        let orderMessage = "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique"
        let currentNoteReuseMessage = "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material"
        guard !nullifiers.isEmpty, nullifiers.count <= foldStepMaxInputs else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(countMessage)
        }
        for (index, nullifier) in nullifiers.enumerated() {
            guard !nullifier.allSatisfy({ $0 == 0 }) else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(zeroMessage)
            }
            if index > 0 {
                guard nullifiers[index - 1].lexicographicallyPrecedes(nullifier) else {
                    throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(orderMessage)
                }
            }
        }
        guard !nullifiers.contains(currentNote.noteCommitment),
              !nullifiers.contains(currentNote.spendNullifier)
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(currentNoteReuseMessage)
        }
    }

    private static func requireAccumulatorRoots(initialRoot: Data, finalRoot: Data) throws {
        guard initialRoot.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.initial_root")
        }
        guard finalRoot.contains(where: { $0 != 0 }),
              finalRoot != initialRoot
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.final_root")
        }
    }

    private static func requireAccumulatorCorridor(
        _ reader: inout CompactReader,
        hopCount: Int
    ) throws {
        func readFixed32(_ field: String) throws -> Data {
            let qualifiedField = "bundle.accumulator.\(field)"
            return try readField(&reader, field: qualifiedField) {
                try $0.readFixed32Flexible(field: qualifiedField)
            }
        }

        func requireNonzero(_ field: String) throws -> Data {
            let value = try readFixed32(field)
            guard value.contains(where: { $0 != 0 }) else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.\(field)")
            }
            return value
        }

        let lineageDigest = try requireNonzero("lineage_digest")
        let aggregationTranscriptDigest = try readFixed32("aggregation_transcript_digest")
        guard aggregationTranscriptDigest.contains(where: { $0 != 0 }),
              aggregationTranscriptDigest == lineageDigest
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.aggregation_transcript_digest")
        }
        for field in [
            "nullifier_digest",
            "output_commitment_digest",
            "fold_digest",
            "recursive_proof_chain_digest",
            "transition_profile_binding_digest",
        ] {
            _ = try requireNonzero(field)
        }
        let appendOpeningPreflightDigest = try readFixed32("append_opening_preflight_digest")
        if appendOpeningPreflightDigest.contains(where: { $0 != 0 }), hopCount <= 1 {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.append_opening_preflight_digest")
        }
        let appendBoundaryDigest = try readFixed32("append_boundary_digest")
        if appendBoundaryDigest.contains(where: { $0 != 0 }),
           !appendOpeningPreflightDigest.contains(where: { $0 != 0 }) || hopCount <= 1
        {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.append_boundary_digest")
        }
        for field in [
            "verifier_params_fingerprint",
            "fixed_window_table_schedule_digest",
            "fixed_window_shared_table_manifest_digest",
            "fixed_window_table_base_digest",
            "verifier_witness_batch_digest",
        ] {
            _ = try requireNonzero(field)
        }
        let verifierOpeningLen: Int
        do {
            verifierOpeningLen = try Int(readField(
                &reader,
                field: "bundle.accumulator.verifier_opening_len"
            ) { try $0.readUInt32LE() })
        } catch {
            if let codecError = error as? KagemushaRecursiveSpendRequestCodecError,
               codecError == .invalidArchive("truncated") {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.verifier_opening_len")
            }
            throw error
        }
        guard [2, 4, 8, 16, 32, 64, 128].contains(verifierOpeningLen) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.verifier_opening_len")
        }
    }

    static func compareCanonicalDecimal(_ lhs: String, _ rhs: String) -> Int {
        if lhs.count != rhs.count {
            return lhs.count < rhs.count ? -1 : 1
        }
        if lhs == rhs {
            return 0
        }
        return lhs < rhs ? -1 : 1
    }

    private static func verifyingKeyBoxPayload(_ bytes: Data) -> Data {
        verifyingKeyBoxPayload(
            backend: KagemushaRecursiveSpendProver.recursiveAggregationProofBackend,
            bytes: bytes
        )
    }

    private static func verifyingKeyBoxPayload(backend: String, bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(backend))
        writer.writeField(encodeBytesVec(bytes))
        return writer.data
    }

    private static func encodeSpendableNote(_ note: KagemushaRecursiveSpendableNoteDescriptor) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try encodePackedFixed32(note.noteCommitment, field: "noteCommitment"))
        writer.writeField(try encodePackedFixed32(note.spendNullifier, field: "spendNullifier"))
        writer.writeField(try encodeNumeric(note.amount))
        return writer.data
    }

    // A direct `[u8; 32]` struct field is packed by Norito derive. Fixed arrays reached through a
    // generic container (`Vec` element or `Option` inner) retain ConstVec's per-byte framing.
    private static func encodePackedFixed32(_ bytes: Data, field: String) throws -> Data {
        try requireFixed32(bytes, field: field)
        return bytes
    }

    private static func encodeConstVec(_ bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }

    private static func encodeBytesVec(_ bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    private static func encodeRawVec(_ payloads: [Data]) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(payloads.count))
        for payload in payloads {
            writer.writeField(payload)
        }
        return writer.data
    }

    private static func encodeOptionRaw(_ payload: Data?) -> Data {
        var writer = OfflineCompactNoritoWriter()
        guard let payload else {
            writer.writeUInt8(0)
            return writer.data
        }
        writer.writeUInt8(1)
        writer.writeField(payload)
        return writer.data
    }

    private static func encodeOptionBytesVec(_ bytes: Data?) -> Data {
        guard let bytes else {
            return encodeOptionRaw(nil)
        }
        return encodeOptionRaw(encodeBytesVec(bytes))
    }

    private static func encodeOptionFixed32(_ bytes: Data?, field: String) throws -> Data {
        guard let bytes else {
            return encodeOptionRaw(nil)
        }
        try requireFixed32(bytes, field: field)
        return encodeOptionRaw(encodeConstVec(bytes))
    }

    private static func encodeOptionUInt64(_ value: UInt64?) -> Data {
        guard let value else {
            return encodeOptionRaw(nil)
        }
        var payload = Data()
        var littleEndian = value.littleEndian
        payload.append(contentsOf: withUnsafeBytes(of: &littleEndian, Array.init))
        return encodeOptionRaw(payload)
    }

    private static func encodeNumeric(_ value: String) throws -> Data {
        let mantissaBytes = try decimalLittleEndianBytes(value, fixedByteCount: nil, signed: true)
        var mantissa = OfflineCompactNoritoWriter()
        mantissa.writeUInt32LE(UInt32(mantissaBytes.count))
        mantissa.writeBytes(mantissaBytes)

        var writer = OfflineCompactNoritoWriter()
        writer.writeField(mantissa.data)
        writer.writeField(OfflineCompactNorito.encodeUInt32(0))
        return writer.data
    }

    private static func encodeU128(_ value: String) throws -> Data {
        try decimalLittleEndianBytes(value, fixedByteCount: 16, signed: false)
    }

    private static func accountIdPayload(_ recipient: String) throws -> Data {
        do {
            let address = try AccountAddress.parseEncoded(recipient, expectedPrefix: 0x02F1)
            return try address.compactNoritoAccountControllerPayload()
        } catch {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("recipient")
        }
    }

    static func canonicalAssetId(_ value: String, field: String) throws -> String {
        let parsed = try parseAssetId(value, field: field)
        let definition = AssetDefinitionAddress.encode(uuidBytes: parsed.definitionBytes)
            ?? parsed.assetDefinitionId
        let base = "\(definition)#\(parsed.accountId)"
        guard let dataspaceId = parsed.dataspaceId else {
            return base
        }
        return "\(base)#dataspace:\(dataspaceId)"
    }

    static func canonicalAssetId(
        accountId: String,
        assetDefinitionId: String,
        dataspaceId: UInt64? = nil
    ) throws -> String {
        let canonicalAccount = try canonicalAccountId(accountId, field: "accountId")
        guard AssetDefinitionAddress.decode(assetDefinitionId) != nil else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("assetDefinitionId")
        }
        let base = "\(assetDefinitionId)#\(canonicalAccount)"
        guard let dataspaceId else {
            return base
        }
        return "\(base)#dataspace:\(dataspaceId)"
    }

    private struct ParsedAssetId {
        let accountId: String
        let assetDefinitionId: String
        let definitionBytes: Data
        let dataspaceId: UInt64?
    }

    private static func assetIdPayload(_ assetId: String) throws -> Data {
        let parsed = try parseAssetId(assetId, field: "assetId")
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try accountIdPayload(parsed.accountId, field: "assetId.account"))
        writer.writeField(encodeConstVec(parsed.definitionBytes))
        writer.writeField(assetBalanceScopePayload(dataspaceId: parsed.dataspaceId))
        return writer.data
    }

    private static func parseAssetId(_ value: String, field: String) throws -> ParsedAssetId {
        let parts = value.split(separator: "#", omittingEmptySubsequences: false)
        guard parts.count == 2 || parts.count == 3,
              !parts[0].isEmpty,
              !parts[1].isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
        let definition = String(parts[0])
        guard let definitionBytes = AssetDefinitionAddress.decode(definition) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("\(field).definition")
        }
        let account = try canonicalAccountId(String(parts[1]), field: "\(field).account")
        var dataspaceId: UInt64?
        if parts.count == 3 {
            let scope = String(parts[2])
            let prefix = "dataspace:"
            guard scope.hasPrefix(prefix) else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("\(field).scope")
            }
            let raw = String(scope.dropFirst(prefix.count))
            guard !raw.isEmpty,
                  (raw == "0" || !raw.hasPrefix("0")),
                  raw.allSatisfy({ $0 >= "0" && $0 <= "9" }),
                  let parsed = UInt64(raw) else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("\(field).scope")
            }
            dataspaceId = parsed
        }
        return ParsedAssetId(
            accountId: account,
            assetDefinitionId: definition,
            definitionBytes: definitionBytes,
            dataspaceId: dataspaceId
        )
    }

    private static func canonicalAccountId(_ value: String, field: String) throws -> String {
        do {
            let address = try AccountAddress.parseEncoded(value, expectedPrefix: 0x02F1)
            return try address.toI105(networkPrefix: 0x02F1)
        } catch {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
    }

    private static func accountIdPayload(_ accountId: String, field: String) throws -> Data {
        do {
            let address = try AccountAddress.parseEncoded(accountId, expectedPrefix: 0x02F1)
            return try address.compactNoritoAccountControllerPayload()
        } catch {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
    }

    private static func assetBalanceScopePayload(dataspaceId: UInt64?) -> Data {
        var writer = OfflineCompactNoritoWriter()
        guard let dataspaceId else {
            writer.writeUInt32LE(0)
            return writer.data
        }
        writer.writeUInt32LE(1)
        var payload = OfflineCompactNoritoWriter()
        payload.writeUInt64LE(dataspaceId)
        writer.writeField(payload.data)
        return writer.data
    }

    private static func decimalLittleEndianBytes(
        _ value: String,
        fixedByteCount: Int?,
        signed: Bool
    ) throws -> Data {
        var digits = value.compactMap(\.wholeNumberValue)
        var output = Data()
        while !(digits.count == 1 && digits[0] == 0) {
            var quotient: [Int] = []
            var remainder = 0
            for digit in digits {
                let current = remainder * 10 + digit
                let q = current / 256
                remainder = current % 256
                if !quotient.isEmpty || q != 0 {
                    quotient.append(q)
                }
            }
            output.append(UInt8(remainder))
            digits = quotient.isEmpty ? [0] : quotient
        }
        if let fixedByteCount {
            guard output.count <= fixedByteCount else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("u128")
            }
            while output.count < fixedByteCount {
                output.append(0)
            }
            return output
        }
        while output.count > 1 && output.last == 0 {
            output.removeLast()
        }
        if signed, let last = output.last, (last & 0x80) != 0 {
            output.append(0)
        }
        return output
    }

    private static func readAccumulatorSummary(_ payload: Data) throws -> AccumulatorSummary {
        var reader = CompactReader(data: payload)
        let domain = try readField(&reader, readString)
        guard domain == KagemushaRecursiveSpendProver.recursiveSpendAccumulatorDomain else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.domain")
        }
        let chainId = try readField(&reader, readChainIdPayload)
        let assetBytes = try readField(&reader, field: "bundle.accumulator.asset") {
            try $0.readFixedBytesFlexible(expectedCount: 16, field: "bundle.accumulator.asset")
        }
        let asset = AssetDefinitionAddress.encode(uuidBytes: assetBytes)
            ?? "hex:\(assetBytes.map { String(format: "%02x", $0) }.joined())"
        let initialRoot = try readField(&reader, field: "bundle.accumulator.initial_root") {
            try $0.readFixed32Flexible(field: "bundle.accumulator.initial_root")
        }
        let finalRoot = try readField(&reader, field: "bundle.accumulator.final_root") {
            try $0.readFixed32Flexible(field: "bundle.accumulator.final_root")
        }
        try requireAccumulatorRoots(initialRoot: initialRoot, finalRoot: finalRoot)
        let topupAnchorNullifiers = try readField(&reader) { child in
            try readFixed32Sequence(
                &child,
                field: "bundle.accumulator.topup_anchor_nullifiers",
                maxCount: foldStepMaxInputs
            )
        }
        let hopCount = try Int(readField(
            &reader,
            field: "bundle.accumulator.hop_count"
        ) { try $0.readUInt32LE() })
        guard hopCount >= 1,
              hopCount <= Int(KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1)
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.hop_count")
        }
        try requireAccumulatorCorridor(&reader, hopCount: hopCount)
        let currentNote = try readField(&reader, field: "bundle.accumulator.current_note") {
            try readSpendableNote(&$0, field: "bundle.accumulator.current_note")
        }
        try requireTopupAnchorNullifiers(topupAnchorNullifiers, currentNote: currentNote)
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle")
        }
        return AccumulatorSummary(
            chainId: chainId,
            asset: asset,
            initialRoot: initialRoot,
            finalRoot: finalRoot,
            topupAnchorNullifiers: topupAnchorNullifiers,
            hopCount: hopCount,
            currentNote: currentNote
        )
    }

    private static func readChainIdPayload(_ reader: inout CompactReader) throws -> String {
        let value: String
        do {
            value = try readField(&reader, readString)
        } catch {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.chain_id")
        }
        try requirePortableId(value, field: "bundle.accumulator.chain_id")
        return value
    }

    private struct RecursiveProofDecodeContext {
        let trailingField: String
        let verifierTrailingField: String
        let verifierBackendField: String
        let verifierNameField: String
        let proofPublicInputsField: String
        let proofPublicInputsHashField: String
        let proofBackendField: String
        let proofBytesField: String

        static let bundle = RecursiveProofDecodeContext(
            trailingField: "bundle",
            verifierTrailingField: "bundle",
            verifierBackendField: "verifierKeyId.backend",
            verifierNameField: "verifierKeyId",
            proofPublicInputsField: "bundle.proof_public_inputs",
            proofPublicInputsHashField: "bundle.proof_public_inputs_hash",
            proofBackendField: "bundle.proof_backend",
            proofBytesField: "bundle.proof_bytes"
        )

        static let lineagePreviousProof = RecursiveProofDecodeContext(
            trailingField: "lineageWitness.previousRecursiveProofs",
            verifierTrailingField: "lineageWitness.previousRecursiveProofs.verifierKeyId",
            verifierBackendField: "lineageWitness.previousRecursiveProofs.verifierKeyId.backend",
            verifierNameField: "lineageWitness.previousRecursiveProofs.verifierKeyId.name",
            proofPublicInputsField: "lineageWitness.previousRecursiveProofs.proof_public_inputs",
            proofPublicInputsHashField: "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash",
            proofBackendField: "lineageWitness.previousRecursiveProofs.proof_backend",
            proofBytesField: "lineageWitness.previousRecursiveProofs.proof_bytes"
        )
    }

    private static func readRecursiveProofCircuitId(_ payload: Data) throws -> String {
        try readRecursiveProofCircuitId(payload, context: .bundle)
    }

    private static func readRecursiveProofCircuitId(
        _ payload: Data,
        context: RecursiveProofDecodeContext
    ) throws -> String {
        var reader = CompactReader(data: payload)
        let verifierKeyIdPayload = try reader.readField()
        let publicInputsPayload = try reader.readField()
        guard !publicInputsPayload.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                context.proofPublicInputsField
            )
        }
        let publicInputsHash = try readField(
            &reader,
            field: context.proofPublicInputsHashField
        ) {
            try $0.readFixed32Flexible(field: context.proofPublicInputsHashField)
        }
        guard publicInputsHash.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                context.proofPublicInputsHashField
            )
        }
        let publicInputsArchive = noritoEncode(
            typeName: recursiveAggregationProofPublicInputsWireName,
            payload: publicInputsPayload,
            flags: requestFlags
        )
        guard publicInputsHash == IrohaHash.hash(publicInputsArchive) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                context.proofPublicInputsHashField
            )
        }
        let proofPayload = try reader.readField()
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(context.trailingField)
        }
        var verifierKeyId = CompactReader(data: verifierKeyIdPayload)
        let backend = try readField(&verifierKeyId, readString)
        let name = try readField(&verifierKeyId, readString)
        guard verifierKeyId.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                context.verifierTrailingField
            )
        }
        try requirePortableId(backend, field: context.verifierBackendField)
        guard backend == KagemushaRecursiveSpendProver.recursiveAggregationProofBackend else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                context.proofBackendField
            )
        }
        let proofBackend = try readProofBoxBackend(proofPayload, context: context)
        guard proofBackend == backend else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                context.proofBackendField
            )
        }
        try requirePortableId(name, field: context.verifierNameField)
        return name
    }

    static func lineageWitnessHasReservedPreviousProof(_ archive: Data) throws -> Bool {
        let payload = try payloadArchive(
            archive,
            schema: lineageWitnessWireName,
            field: "lineageWitness"
        )
        guard payload.flags == requestFlags else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("lineageWitness")
        }
        var reader = CompactReader(data: payload.payload)
        try skipFields(&reader, count: 3)
        let previousProofsPayload = try reader.readField()
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("lineageWitness")
        }
        var previousProofs = CompactReader(data: previousProofsPayload)
        let count = try previousProofs.readUInt64LE()
        guard count <= UInt64(Int.max) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "lineageWitness.previousRecursiveProofs"
            )
        }
        guard count <= UInt64(KagemushaRecursiveSpendProver.compactTokenMaxHops) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "lineageWitness.previousRecursiveProofs"
            )
        }
        var hasReserved = false
        for _ in 0..<Int(count) {
            let proofPayload = try previousProofs.readField()
            let circuitId = try readPreviousRecursiveProofCircuitId(proofPayload)
            hasReserved = hasReserved || KagemushaRecursiveSpendProver.isLineageProofCircuitId(circuitId)
        }
        guard previousProofs.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "lineageWitness.previousRecursiveProofs"
            )
        }
        return hasReserved
    }

    private static func readPreviousRecursiveProofCircuitId(_ payload: Data) throws -> String {
        let name = try readRecursiveProofCircuitId(payload, context: .lineagePreviousProof)
        guard KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(name) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "lineageWitness.previousRecursiveProofs.verifierKeyId.name"
            )
        }
        return name
    }

    private static func readProofBoxBackend(
        _ payload: Data,
        context: RecursiveProofDecodeContext
    ) throws -> String {
        var reader = CompactReader(data: payload)
        let backend = try readField(&reader, readString)
        let proofBytes = try readField(&reader) { try $0.readByteVec() }
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(context.trailingField)
        }
        try requirePortableId(backend, field: "proof.backend")
        guard backend == KagemushaRecursiveSpendProver.recursiveAggregationProofBackend else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                context.proofBackendField
            )
        }
        guard !proofBytes.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                context.proofBytesField
            )
        }
        return backend
    }

    private static func readSpendableNote(
        _ reader: inout CompactReader,
        field: String = "current_note"
    ) throws
        -> KagemushaRecursiveSpendableNoteDescriptor
    {
        try KagemushaRecursiveSpendableNoteDescriptor(
            noteCommitment: readField(&reader, field: "\(field).note_commitment") {
                try $0.readFixed32Flexible(field: "\(field).note_commitment")
            },
            spendNullifier: readField(&reader, field: "\(field).spend_nullifier") {
                try $0.readFixed32Flexible(field: "\(field).spend_nullifier")
            },
            amount: readField(&reader, field: "\(field).amount") { child in
                try readNumeric(&child, field: "\(field).amount")
            }
        )
    }

    private static func readNumeric(
        _ reader: inout CompactReader,
        field: String = "amount"
    ) throws -> String {
        let mantissa = try readField(&reader, field: "\(field).mantissa") { child -> Data in
            let count = try Int(child.readUInt32LE())
            return try child.readBytes(count)
        }
        let scale = try readField(&reader, field: "\(field).scale") { try $0.readUInt32LE() }
        guard scale == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField(field)
        }
        let value = decimalString(fromLittleEndianTwosComplement: mantissa)
        return try canonicalU128Decimal(value, field: field)
    }

    private static func decimalString(fromLittleEndianTwosComplement bytes: Data) -> String {
        guard !bytes.isEmpty else { return "0" }
        let negative = (bytes.last ?? 0) & 0x80 != 0
        let magnitudeBytes: Data
        if negative {
            var magnitude = bytes.map { ~$0 }
            var carry = UInt16(1)
            for index in magnitude.indices {
                let sum = UInt16(magnitude[index]) + carry
                magnitude[index] = UInt8(sum & 0xff)
                carry = sum >> 8
                if carry == 0 {
                    break
                }
            }
            magnitudeBytes = Data(magnitude)
        } else {
            magnitudeBytes = bytes
        }
        var digits = "0"
        for byte in magnitudeBytes.reversed() {
            digits = multiplyDecimalString(digits, by: 256)
            digits = addDecimalString(digits, UInt16(byte))
        }
        if negative, digits != "0" {
            return "-\(digits)"
        }
        return digits
    }

    private static func multiplyDecimalString(_ value: String, by multiplier: UInt16) -> String {
        var carry: UInt32 = 0
        var out = ""
        for char in value.reversed() {
            let digit = UInt32(char.wholeNumberValue ?? 0)
            let product = digit * UInt32(multiplier) + carry
            out.insert(Character(String(product % 10)), at: out.startIndex)
            carry = product / 10
        }
        while carry > 0 {
            out.insert(Character(String(carry % 10)), at: out.startIndex)
            carry /= 10
        }
        return trimLeadingZeros(out)
    }

    private static func addDecimalString(_ value: String, _ addend: UInt16) -> String {
        var carry = UInt32(addend)
        var out = ""
        for char in value.reversed() {
            let digit = UInt32(char.wholeNumberValue ?? 0)
            let sum = digit + carry
            out.insert(Character(String(sum % 10)), at: out.startIndex)
            carry = sum / 10
        }
        while carry > 0 {
            out.insert(Character(String(carry % 10)), at: out.startIndex)
            carry /= 10
        }
        return trimLeadingZeros(out)
    }

    private static func trimLeadingZeros(_ value: String) -> String {
        let trimmed = value.drop { $0 == "0" }
        return trimmed.isEmpty ? "0" : String(trimmed)
    }

    private static func readField<T>(
        _ reader: inout CompactReader,
        field: String = "field",
        _ decode: (inout CompactReader) throws -> T
    ) throws -> T {
        var child = CompactReader(data: try reader.readField())
        let value = try decode(&child)
        guard child.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return value
    }

    private static func readString(_ reader: inout CompactReader) throws -> String {
        let length = try reader.readLength()
        let bytes = try reader.readBytes(length)
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("string")
        }
        return value
    }

    private static func readBool(_ reader: inout CompactReader) throws -> Bool {
        switch try reader.readUInt8() {
        case 0: return false
        case 1: return true
        default: throw KagemushaRecursiveSpendRequestCodecError.invalidField("bool")
        }
    }

    private static func skipFields(_ reader: inout CompactReader, count: Int) throws {
        for _ in 0..<count {
            _ = try reader.readField()
        }
    }
}

private struct CompactReader {
    private let data: Data
    private(set) var offset: Int = 0

    init(data: Data) {
        self.data = data
    }

    var remaining: Int {
        data.count - offset
    }

    mutating func readUInt8() throws -> UInt8 {
        guard offset < data.count else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("truncated")
        }
        let value = data[data.startIndex + offset]
        offset += 1
        return value
    }

    mutating func readUInt16LE() throws -> UInt16 {
        let bytes = try readBytes(2)
        var value: UInt16 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 2)
        }
        return UInt16(littleEndian: value)
    }

    mutating func readUInt32LE() throws -> UInt32 {
        let bytes = try readBytes(4)
        var value: UInt32 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 4)
        }
        return UInt32(littleEndian: value)
    }

    mutating func readUInt64LE() throws -> UInt64 {
        let bytes = try readBytes(8)
        var value: UInt64 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 8)
        }
        return UInt64(littleEndian: value)
    }

    mutating func readByteVec() throws -> Data {
        let length = try readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bytes")
        }
        return try readBytes(Int(length))
    }

    mutating func readFixedBytes(_ count: Int) throws -> Data {
        try readBytes(count)
    }

    mutating func readFixed32Flexible(field: String = "fixedArray") throws -> Data {
        try readFixedBytesFlexible(expectedCount: 32, field: field)
    }

    mutating func readFixedBytesFlexible(expectedCount: Int, field: String = "fixedArray") throws -> Data {
        if remaining == expectedCount {
            return try readFixedBytes(expectedCount)
        }
        return try readFixedArrayBytes(expectedCount: expectedCount, field: field)
    }

    mutating func readFixedArrayBytes(expectedCount: Int, field: String = "fixedArray") throws -> Data {
        var out = Data()
        out.reserveCapacity(expectedCount)
        while remaining > 0 {
            let length = try readLength()
            guard length == 1 else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
            }
            out.append(try readUInt8())
        }
        guard out.count == expectedCount else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        return out
    }

    mutating func readBytes(_ count: Int) throws -> Data {
        guard count >= 0, offset + count <= data.count else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("truncated")
        }
        let start = data.startIndex + offset
        let result = Data(data[start..<(start + count)])
        offset += count
        return result
    }

    mutating func readField() throws -> Data {
        let length = try readLength()
        return try readBytes(length)
    }

    mutating func readLength() throws -> Int {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        let start = offset
        for _ in 0..<10 {
            let byte = try readUInt8()
            let chunk = UInt64(byte & 0x7f)
            if shift >= 63 && chunk > 1 {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("length")
            }
            value |= chunk << shift
            if (byte & 0x80) == 0 {
                let encodedLength = offset - start
                if encodedLength > 1 && value < (UInt64(1) << UInt64(7 * (encodedLength - 1))) {
                    throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("length")
                }
                guard value <= UInt64(Int.max), value <= UInt64(data.count) else {
                    throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("length")
                }
                return Int(value)
            }
            shift += 7
        }
        throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("length")
    }
}

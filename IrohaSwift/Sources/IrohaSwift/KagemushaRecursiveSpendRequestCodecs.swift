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
        lineageVerifierKey: Data?,
        lineageProvingKeyArchive: Data?,
        blockHeight: UInt64? = nil
    ) throws {
        guard let lineageVerifierKey, !lineageVerifierKey.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierKey")
        }
        guard let lineageProvingKeyArchive else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageProvingKeyArchive")
        }
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

public struct KagemushaRecursiveSpendAppendRequest: Equatable, Sendable {
    public let previousBundle: Data
    public let recordBundle: Data
    public let pallasOpenEnvelopes: Data
    public let currentNote: KagemushaRecursiveSpendableNoteDescriptor
    public let outputProofCircuitId: String?
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
        outputProofCircuitId: String? = nil,
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
    public let lineageWitnessRequired: Bool
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

    public init(
        bundle: Data,
        recipient: String,
        publicAmount: String,
        redeemProof: Data,
        lineageWitness: Data? = nil,
        changeOutput: Data? = nil,
        lineageVerifierRecord: KagemushaRecursiveSpendVerifierRecordRef? = nil,
        blockHeight: UInt64? = nil
    ) throws {
        try KagemushaRecursiveSpendRequestCodecs.requireNonBlankUnpadded(recipient, field: "recipient")
        if let lineageWitness {
            try KagemushaRecursiveSpendRequestCodecs.requireNestedArchive(lineageWitness, field: "lineageWitness")
        }
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
        let finalIsLineage = KagemushaRecursiveSpendProver.isLineageProofCircuitId(
            bundleSummary.proofCircuitId
        )
        let witnessHasReservedPrevious: Bool
        if let lineageWitness {
            witnessHasReservedPrevious = try KagemushaRecursiveSpendRequestCodecs
                .lineageWitnessHasReservedPreviousProof(lineageWitness)
        } else {
            witnessHasReservedPrevious = false
        }
        if !finalIsLineage {
            if witnessHasReservedPrevious && lineageVerifierRecord == nil {
                throw KagemushaRecursiveSpendRequestCodecError.invalidField("lineageVerifierRecord")
            }
            if !witnessHasReservedPrevious && lineageVerifierRecord != nil {
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
            || lineageVerifierRecord != nil
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
    }
}

public struct KagemushaRecursiveSpendBundleSummary: Equatable, Sendable {
    public let hopCount: Int
    public let proofCircuitId: String
    public let asset: String
    public let chainId: String
    public let initialRoot: Data
    public let finalRoot: Data
    public let currentNote: KagemushaRecursiveSpendableNoteDescriptor
}

public enum KagemushaRecursiveSpendRequestCodecs {
    public static let initRequestWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendInitRequestV1"
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

    static let requestFlags = NoritoHeader.compactLen
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

    public static func encodeAppendRequest(_ request: KagemushaRecursiveSpendAppendRequest) throws -> Data {
        let normalizedOutput = KagemushaRecursiveSpendProver.normalizedAppendOutputCircuitId(
            request.outputProofCircuitId
        )
        let outputWire = normalizedOutput == KagemushaRecursiveSpendProver.recursiveAggregationProofCircuitIdV1
            ? ""
            : normalizedOutput
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
        writer.writeField(OfflineCompactNorito.encodeString(outputWire))
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
        let lineageWitnessRequired = reader.remaining == 0 ? false : try readField(&reader, readBool)
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
            lineageWitnessRequired: lineageWitnessRequired
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
        return KagemushaRecursiveSpendBundleSummary(
            hopCount: accumulator.hopCount,
            proofCircuitId: proofCircuitId,
            asset: accumulator.asset,
            chainId: accumulator.chainId,
            initialRoot: accumulator.initialRoot,
            finalRoot: accumulator.finalRoot,
            currentNote: accumulator.currentNote
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
    let hopCount: Int
    let currentNote: KagemushaRecursiveSpendableNoteDescriptor
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

    private static func readVerifiedFoldStepCount(
        _ decoder: inout CompactReader,
        field: String
    ) throws -> Int {
        let count64 = try decoder.readUInt64LE()
        guard count64 <= UInt64(Int.max) else {
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
        try readField(&reader) { child in
            try readRequiredMetadataOption(&child, field: "\(field).vk_commitment")
        }
        try readField(&reader) { child in
            try readRequiredMetadataOption(&child, field: "\(field).public_inputs_schema_hash")
        }
        try readField(&reader) { child in
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
            try readFixed32SequenceCount(&child, field: "\(field).g")
        }
        guard gCount == intN else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let hCount = try readField(&reader) { child in
            try readFixed32SequenceCount(&child, field: "\(field).h")
        }
        guard hCount == intN else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        try readField(&reader) { child in
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
        try readField(&reader) { try $0.readFixedBytes(32) }
        try readField(&reader) { try $0.readFixedBytes(32) }
        try readField(&reader) { try $0.readFixedBytes(32) }
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
        let lCount = try readField(&reader) { child in
            try readFixed32SequenceCount(&child, field: "\(field).l")
        }
        let rCount = try readField(&reader) { child in
            try readFixed32SequenceCount(&child, field: "\(field).r")
        }
        guard lCount == rCount,
              lCount == n.trailingZeroBitCount else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        try readField(&reader) { try $0.readFixedBytes(32) }
        try readField(&reader) { try $0.readFixedBytes(32) }
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
    }

    private static func readFixed32SequenceCount(
        _ reader: inout CompactReader,
        field: String
    ) throws -> Int {
        let count64 = try reader.readUInt64LE()
        guard count64 <= UInt64(Int.max) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
        let count = Int(count64)
        for _ in 0..<count {
            try readField(&reader) { child in
                try child.readFixedBytes(32)
            }
        }
        return count
    }

    private static func readRequiredMetadataOption(
        _ reader: inout CompactReader,
        field: String
    ) throws {
        guard let payload = try readOptionRawPayload(&reader),
              payload.count == 32,
              payload.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(field)
        }
    }

    private static func readOptionRawPayload(_ reader: inout CompactReader) throws -> Data? {
        switch try reader.readUInt8() {
        case 0:
            return nil
        case 1:
            let length = try reader.readLength()
            return try reader.readBytes(length)
        default:
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("option")
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
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(
            KagemushaRecursiveSpendProver.recursiveAggregationProofBackend
        ))
        writer.writeField(encodeBytesVec(bytes))
        return writer.data
    }

    private static func encodeSpendableNote(_ note: KagemushaRecursiveSpendableNoteDescriptor) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(encodeFixedBytes(note.noteCommitment))
        writer.writeField(encodeFixedBytes(note.spendNullifier))
        writer.writeField(try encodeNumeric(note.amount))
        return writer.data
    }

    private static func encodeFixedBytes(_ bytes: Data) -> Data {
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
        return encodeOptionRaw(encodeFixedBytes(bytes))
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
        let assetBytes = try readField(&reader) { try $0.readFixedBytesFlexible(expectedCount: 16) }
        let asset = AssetDefinitionAddress.encode(uuidBytes: assetBytes)
            ?? "hex:\(assetBytes.map { String(format: "%02x", $0) }.joined())"
        let initialRoot = try readField(&reader) { try $0.readFixed32Flexible() }
        let finalRoot = try readField(&reader) { try $0.readFixed32Flexible() }
        try skipFields(&reader, count: 1)
        let hopCount = try Int(readField(&reader) { try $0.readUInt32LE() })
        guard hopCount >= 1,
              hopCount <= Int(KagemushaRecursiveSpendProver.recursiveSpendLineageWitnesslessMaxHopsV1)
        else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.hop_count")
        }
        try skipFields(&reader, count: 15)
        let currentNote = try readField(&reader, readSpendableNote)
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle")
        }
        return AccumulatorSummary(
            chainId: chainId,
            asset: asset,
            initialRoot: initialRoot,
            finalRoot: finalRoot,
            hopCount: hopCount,
            currentNote: currentNote
        )
    }

    private static func readChainIdPayload(_ reader: inout CompactReader) throws -> String {
        do {
            return try readField(&reader, readString)
        } catch {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle.accumulator.chain_id")
        }
    }

    private static func readRecursiveProofCircuitId(_ payload: Data) throws -> String {
        var reader = CompactReader(data: payload)
        let verifierKeyIdPayload = try reader.readField()
        let publicInputsPayload = try reader.readField()
        guard !publicInputsPayload.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "bundle.proof_public_inputs"
            )
        }
        let publicInputsHash = try readField(&reader) { try $0.readFixed32Flexible() }
        guard publicInputsHash.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "bundle.proof_public_inputs_hash"
            )
        }
        let publicInputsArchive = noritoEncode(
            typeName: recursiveAggregationProofPublicInputsWireName,
            payload: publicInputsPayload,
            flags: requestFlags
        )
        guard publicInputsHash == IrohaHash.hash(publicInputsArchive) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "bundle.proof_public_inputs_hash"
            )
        }
        let proofPayload = try reader.readField()
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle")
        }
        var verifierKeyId = CompactReader(data: verifierKeyIdPayload)
        let backend = try readField(&verifierKeyId, readString)
        let name = try readField(&verifierKeyId, readString)
        guard verifierKeyId.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle")
        }
        try requirePortableId(backend, field: "verifierKeyId.backend")
        guard backend == KagemushaRecursiveSpendProver.recursiveAggregationProofBackend else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "bundle.proof_backend"
            )
        }
        let proofBackend = try readProofBoxBackend(proofPayload)
        guard proofBackend == backend else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "bundle.proof_backend"
            )
        }
        try requirePortableId(name, field: "verifierKeyId")
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
        var reader = CompactReader(data: payload)
        let verifierKeyIdPayload = try reader.readField()
        try skipFields(&reader, count: 3)
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "lineageWitness.previousRecursiveProofs"
            )
        }
        var verifierKeyId = CompactReader(data: verifierKeyIdPayload)
        let backend = try readField(&verifierKeyId, readString)
        let name = try readField(&verifierKeyId, readString)
        guard verifierKeyId.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "lineageWitness.previousRecursiveProofs.verifierKeyId"
            )
        }
        try requirePortableId(
            backend,
            field: "lineageWitness.previousRecursiveProofs.verifierKeyId.backend"
        )
        guard backend == KagemushaRecursiveSpendProver.recursiveAggregationProofBackend else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "lineageWitness.previousRecursiveProofs.verifierKeyId.backend"
            )
        }
        try requirePortableId(
            name,
            field: "lineageWitness.previousRecursiveProofs.verifierKeyId.name"
        )
        guard KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(name) else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "lineageWitness.previousRecursiveProofs.verifierKeyId.name"
            )
        }
        return name
    }

    private static func readProofBoxBackend(_ payload: Data) throws -> String {
        var reader = CompactReader(data: payload)
        let backend = try readField(&reader, readString)
        let proofBytes = try readField(&reader) { try $0.readByteVec() }
        guard reader.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("bundle")
        }
        try requirePortableId(backend, field: "proof.backend")
        guard backend == KagemushaRecursiveSpendProver.recursiveAggregationProofBackend else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "bundle.proof_backend"
            )
        }
        guard !proofBytes.isEmpty else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive(
                "bundle.proof_bytes"
            )
        }
        return backend
    }

    private static func readSpendableNote(_ reader: inout CompactReader) throws
        -> KagemushaRecursiveSpendableNoteDescriptor
    {
        try KagemushaRecursiveSpendableNoteDescriptor(
            noteCommitment: readField(&reader) { try $0.readFixed32Flexible() },
            spendNullifier: readField(&reader) { try $0.readFixed32Flexible() },
            amount: readField(&reader, readNumeric)
        )
    }

    private static func readNumeric(_ reader: inout CompactReader) throws -> String {
        let mantissa = try readField(&reader) { child -> Data in
            let count = try Int(child.readUInt32LE())
            return try child.readBytes(count)
        }
        let scale = try readField(&reader) { try $0.readUInt32LE() }
        guard scale == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidField("numeric")
        }
        let value = decimalString(fromLittleEndianTwosComplement: mantissa)
        return try canonicalU128Decimal(value, field: "amount")
    }

    private static func decimalString(fromLittleEndianTwosComplement bytes: Data) -> String {
        guard !bytes.isEmpty else { return "0" }
        var digits = "0"
        for byte in bytes.reversed() {
            digits = multiplyDecimalString(digits, by: 256)
            digits = addDecimalString(digits, UInt16(byte))
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
        _ decode: (inout CompactReader) throws -> T
    ) throws -> T {
        var child = CompactReader(data: try reader.readField())
        let value = try decode(&child)
        guard child.remaining == 0 else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("field")
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

    mutating func readFixed32Flexible() throws -> Data {
        try readFixedBytesFlexible(expectedCount: 32)
    }

    mutating func readFixedBytesFlexible(expectedCount: Int) throws -> Data {
        if remaining == expectedCount {
            return try readFixedBytes(expectedCount)
        }
        return try readFixedArrayBytes(expectedCount: expectedCount)
    }

    mutating func readFixedArrayBytes(expectedCount: Int) throws -> Data {
        var out = Data()
        out.reserveCapacity(expectedCount)
        while remaining > 0 {
            let length = try readLength()
            guard length == 1 else {
                throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("fixedArray")
            }
            out.append(try readUInt8())
        }
        guard out.count == expectedCount else {
            throw KagemushaRecursiveSpendRequestCodecError.invalidArchive("fixedArray")
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

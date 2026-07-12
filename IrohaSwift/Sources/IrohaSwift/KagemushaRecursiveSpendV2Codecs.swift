import Foundation

struct KagemushaTransferEvidenceSummary: Equatable, Sendable {
    let chainID: String
    let assetDefinitionID: String
    let rootBefore: Data
    let rootAfter: Data
    let inputNullifiers: [Data]
    let outputCommitments: [Data]
    let verifierKeyID: String
    let verifierKeyCommitment: Data
}

public enum KagemushaRecursiveSpendCodecs {
    private static let flags = NoritoHeader.compactLen
    private static let pallasOpenEnvelopesSchemaHash: [UInt8] = [
        0xfe, 0x38, 0x26, 0x32, 0x8f, 0x08, 0x17, 0x71,
        0x75, 0x0f, 0x24, 0xfe, 0x11, 0x02, 0x60, 0xca,
    ]

    public static func decodeNativeCapabilities(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendNativeCapabilities {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.nativeCapabilitiesWireName,
            field: "nativeCapabilities"
        ))
        let value = try KagemushaRecursiveSpendNativeCapabilities(
            bridgeABIVersion: scalarUInt32(
                reader.field(),
                field: "nativeCapabilities.bridgeABIVersion"
            ),
            artifactManifestSchema: decodeString(
                reader.field(),
                field: "nativeCapabilities.artifactManifestSchema"
            ),
            mode: decodeString(reader.field(), field: "nativeCapabilities.mode"),
            proofBackend: decodeString(
                reader.field(),
                field: "nativeCapabilities.proofBackend"
            ),
            transcriptProfile: decodeString(
                reader.field(),
                field: "nativeCapabilities.transcriptProfile"
            ),
            proofEnvelopeVersion: scalarUInt16(
                reader.field(),
                field: "nativeCapabilities.proofEnvelopeVersion"
            ),
            stateBoundaryVersion: scalarUInt16(
                reader.field(),
                field: "nativeCapabilities.stateBoundaryVersion"
            ),
            transitionCircuitID: decodeString(
                reader.field(),
                field: "nativeCapabilities.transitionCircuitID"
            ),
            stateCircuitID: decodeString(
                reader.field(),
                field: "nativeCapabilities.stateCircuitID"
            ),
            maxProofBytes: scalarUInt32(
                reader.field(),
                field: "nativeCapabilities.maxProofBytes"
            ),
            proofBackendAvailable: decodeBool(
                reader.field(),
                field: "nativeCapabilities.proofBackendAvailable"
            ),
            missingGates: decodeStringVector(
                reader.field(),
                field: "nativeCapabilities.missingGates",
                maximumCount: 16
            )
        )
        try reader.finish("nativeCapabilities")
        return value
    }

    /// Strictly validates the one-hop confidential-transfer evidence that
    /// initializes a V2 top-up branch. Cryptographic verification remains in
    /// the ABI-18 bridge; this parser rejects malformed, ambiguous, or
    /// cross-record Swift inputs before crossing the FFI boundary.
    static func transferEvidenceSummary(
        recordBundle: Data,
        pallasOpenEnvelopes: Data
    ) throws -> KagemushaTransferEvidenceSummary {
        var recordBundleReader = KagemushaV2Reader(try payload(
            recordBundle,
            schema: KagemushaRecursiveSpend.verifiedFoldRecordBundleWireName,
            field: "transferEvidence.recordBundle"
        ))
        let bundlePayload = try recordBundleReader.field()
        let recordsPayload = try recordBundleReader.field()
        try recordBundleReader.finish("transferEvidence.recordBundle")

        var bundle = KagemushaV2Reader(bundlePayload)
        let chain = try decodeChainID(bundle.field())
        let asset = try decodeAssetDefinitionID(bundle.field())
        var steps = KagemushaV2Reader(try bundle.field())
        guard try steps.uint64() == 1 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "transferEvidence.steps"
            )
        }
        var step = KagemushaV2Reader(try steps.field())
        try steps.finish("transferEvidence.steps")
        let rootBefore = try packedFixed(
            step.field(), count: 32, field: "transferEvidence.rootBefore"
        )
        let inputNullifiers = try decodeFixed32Vector(
            step.field(), field: "transferEvidence.inputNullifiers"
        )
        let outputCommitments = try decodeFixed32Vector(
            step.field(), field: "transferEvidence.outputCommitments"
        )
        let rootAfter = try packedFixed(
            step.field(), count: 32, field: "transferEvidence.rootAfter"
        )
        var attachment = KagemushaV2Reader(try step.field())
        let backend = try decodeString(
            attachment.field(), field: "transferEvidence.attachment.backend"
        )
        let proofBox = try attachment.field()
        guard !proofBox.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "transferEvidence.attachment.proof"
            )
        }
        let verifierKeyID = try decodeVerifierKeyID(attachment.field())
        let parsedVerifierKey = verifierKeyID.split(
            separator: ":", maxSplits: 1, omittingEmptySubsequences: false
        )
        guard parsedVerifierKey.count == 2,
              String(parsedVerifierKey[0]) == backend,
              let commitmentPayload = try decodeOption(
                attachment.field(),
                field: "transferEvidence.attachment.verifierKeyCommitment"
              ) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "transferEvidence.attachment"
            )
        }
        let verifierKeyCommitment = try packedFixed(
            commitmentPayload,
            count: 32,
            field: "transferEvidence.attachment.verifierKeyCommitment"
        )
        _ = try attachment.field()
        _ = try attachment.field()
        try attachment.finish("transferEvidence.attachment")
        guard !(try step.field()).isEmpty else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "transferEvidence.verifierKey"
            )
        }
        try step.finish("transferEvidence.step")
        try bundle.finish("transferEvidence.bundle")

        var records = KagemushaV2Reader(recordsPayload)
        guard try records.uint64() == 1 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "transferEvidence.verifierRecords"
            )
        }
        var recordEntry = KagemushaV2Reader(try records.field())
        let recordID = try decodeVerifierKeyID(recordEntry.field())
        let recordPayload = try recordEntry.field()
        try recordEntry.finish("transferEvidence.verifierRecord")
        try records.finish("transferEvidence.verifierRecords")

        guard recordID == verifierKeyID,
              !recordPayload.isEmpty,
              rootBefore.contains(where: { $0 != 0 }),
              rootAfter.contains(where: { $0 != 0 }),
              rootBefore != rootAfter,
              (1...KagemushaRecursiveSpend.maximumInputNullifiers)
                .contains(inputNullifiers.count),
              outputCommitments.count == 1,
              inputNullifiers.allSatisfy({ $0.contains(where: { $0 != 0 }) }),
              outputCommitments[0].contains(where: { $0 != 0 }),
              Set(inputNullifiers).count == inputNullifiers.count,
              !inputNullifiers.contains(outputCommitments[0]),
              verifierKeyCommitment.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "transferEvidence.recordBundle"
            )
        }

        guard !pallasOpenEnvelopes.isEmpty,
              pallasOpenEnvelopes.count
                <= KagemushaRecursiveSpend.artifactMaximumFileBytes,
              let frame = noritoDecodeFrame(pallasOpenEnvelopes),
              frame.header.schema == pallasOpenEnvelopesSchemaHash,
              frame.header.compression == .none,
              frame.header.flags == flags,
              frame.paddingLength == 0,
              !frame.payload.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "transferEvidence.pallasOpenEnvelopes"
            )
        }
        var envelopes = KagemushaV2Reader(frame.payload)
        guard try envelopes.uint64() == 1,
              !(try envelopes.field()).isEmpty else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "transferEvidence.pallasOpenEnvelopes"
            )
        }
        try envelopes.finish("transferEvidence.pallasOpenEnvelopes")

        return KagemushaTransferEvidenceSummary(
            chainID: chain,
            assetDefinitionID: asset,
            rootBefore: rootBefore,
            rootAfter: rootAfter,
            inputNullifiers: inputNullifiers,
            outputCommitments: outputCommitments,
            verifierKeyID: verifierKeyID,
            verifierKeyCommitment: verifierKeyCommitment
        )
    }

    static func canonicalAssetID(_ value: String) throws -> String {
        let canonical = try decodeAssetID(assetID(value))
        guard canonical == value else {
            throw KagemushaRecursiveSpendError.invalidField("assetID")
        }
        return canonical
    }

    public static func encodeRecipientRequestPayload(
        _ payload: KagemushaRecipientPaymentRequestSigningPayload
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(chainID(payload.chainID))
        writer.writeField(try assetDefinitionID(payload.assetDefinitionID))
        writer.writeField(try scaledAmount(payload.amount))
        writer.writeField(try accountID(payload.recipient))
        writer.writeField(payload.recipientKeyReference)
        writer.writeField(string(payload.receiverDeviceID))
        writer.writeField(publicKey(payload.receiverPublicKey))
        writer.writeField(payload.requestID)
        writer.writeField(uint64(payload.issuedAtMilliseconds))
        writer.writeField(uint64(payload.expiresAtMilliseconds))
        writer.writeField(try note(payload.recipientOutput))
        writer.writeField(bytes(payload.recipientOutputProverMaterial))
        return frame(
            KagemushaRecursiveSpend.recipientRequestPayloadWireName,
            payload: writer.data
        )
    }

    public static func encodeRecipientOutputDerivationRequest(
        _ request: KagemushaRecipientOutputDerivationRequest
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(chainID(request.chainID))
        writer.writeField(try assetDefinitionID(request.assetDefinitionID))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(request.requestID)
        return frame(
            KagemushaRecursiveSpend.recipientOutputDerivationRequestWireName,
            payload: writer.data
        )
    }

    public static func decodeRecipientOutputDerivationResult(
        _ archive: Data,
        request: KagemushaRecipientOutputDerivationRequest
    ) throws -> KagemushaRecipientOutputDerivationResult {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.recipientOutputDerivationResultWireName,
            field: "recipientOutputDerivationResult"
        ))
        let output = try decodeNote(reader.field())
        let proverMaterial = try decodeBytes(
            reader.field(),
            field: "recipientOutputProverMaterial"
        )
        try reader.finish("recipientOutputDerivationResult")
        return try KagemushaRecipientOutputDerivationResult(
            recipientOutput: output,
            recipientOutputProverMaterial: proverMaterial,
            request: request
        )
    }

    public static func decodeRecipientRequest(
        _ archive: Data
    ) throws -> KagemushaRecipientPaymentRequest {
        let payloadData = try payload(
            archive,
            schema: KagemushaRecursiveSpend.recipientRequestWireName,
            field: "recipientRequest"
        )
        var reader = KagemushaV2Reader(payloadData)
        let signedPayload = try decodeRecipientRequestPayloadFields(&reader)
        let signature = try decodeConstVec(try reader.field(), field: "recipientRequest.signature")
        try reader.finish("recipientRequest")
        let canonical = try encodeRecipientRequest(signedPayload, signature: signature)
        guard canonical == archive,
              archive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("recipientRequest.canonical")
        }
        return try KagemushaRecipientPaymentRequest(
            payload: signedPayload,
            signature: signature,
            archive: archive
        )
    }

    public static func decodeAndVerifyRecipientRequest(
        _ archive: Data,
        atMilliseconds: UInt64
    ) throws -> KagemushaVerifiedRecipientPaymentRequest {
        try decodeRecipientRequest(archive).verified(atMilliseconds: atMilliseconds)
    }

    public static func encodeAuthorizationTemplate(
        _ fields: KagemushaRequestAuthorizationFields
    ) throws -> Data {
        try encodeAuthorization(fields, signature: Data([1]))
    }

    public static func encodeArtifactReference(
        _ reference: KagemushaRecursiveSpendArtifactReference
    ) throws -> Data {
        frame(
            KagemushaRecursiveSpend.artifactReferenceWireName,
            payload: artifactReference(reference)
        )
    }

    public static func encodeInitRequest(
        _ request: KagemushaRecursiveSpendInitRequest
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.topUpAnchor.archive,
            schema: KagemushaRecursiveSpend.topUpAnchorWireName,
            field: "topUpAnchor"
        ))
        writer.writeField(try nestedPayload(
            request.topUpFinalityProof.noritoArchive,
            schema: KagemushaRecursiveSpend.topUpFinalityProofWireName,
            field: "topUpFinalityProof"
        ))
        writer.writeField(uint32(request.lineageMode.rawValue))
        writer.writeField(option(request.lineageArtifact.map(artifactReference)))
        return frame(KagemushaRecursiveSpend.initRequestWireName, payload: writer.data)
    }

    public static func decodeInitRequest(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendInitRequest {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.initRequestWireName,
            field: "initRequest"
        ))
        let anchorArchive = frame(
            KagemushaRecursiveSpend.topUpAnchorWireName,
            payload: try reader.field()
        )
        let finalityProof = frame(
            KagemushaRecursiveSpend.topUpFinalityProofWireName,
            payload: try reader.field()
        )
        let mode = try decodeLineageMode(reader.field())
        let artifact = try decodeOption(reader.field(), field: "lineageArtifact")
            .map(decodeArtifactReference)
        try reader.finish("initRequest")
        let request = try KagemushaRecursiveSpendInitRequest(
            topUpAnchor: decodeTopUpAnchor(anchorArchive),
            topUpFinalityProof: KagemushaTopUpFinalityProofArchive(
                noritoArchive: finalityProof
            ),
            lineageMode: mode,
            lineageArtifact: artifact
        )
        guard try encodeInitRequest(request) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("initRequest.canonical")
        }
        return request
    }

    public static func encodeTopUpUnsigned(
        _ request: KagemushaRecursiveSpendTopUpUnsigned
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try assetID(request.assetID))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try note(request.currentNote))
        writer.writeField(try topUpShieldEvidence(request.shieldEvidence))
        writer.writeField(string(request.artifactGeneration))
        writer.writeField(request.operationID)
        return frame(KagemushaRecursiveSpend.topUpUnsignedWireName, payload: writer.data)
    }

    public static func decodeTopUpUnsigned(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendTopUpUnsigned {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.topUpUnsignedWireName,
            field: "topUpUnsigned"
        ))
        let assetID = try decodeAssetID(reader.field())
        let amount = try decodeScaledAmount(reader.field())
        let currentNote = try decodeNote(reader.field())
        let shieldEvidence = try decodeTopUpShieldEvidence(reader.field())
        let artifactGeneration = try decodeString(
            reader.field(),
            field: "artifactGeneration"
        )
        let operationID = try packedFixed(
            reader.field(),
            count: 32,
            field: "operationID"
        )
        try reader.finish("topUpUnsigned")
        let value = try KagemushaRecursiveSpendTopUpUnsigned(
            assetID: assetID,
            amount: amount,
            currentNote: currentNote,
            shieldEvidence: shieldEvidence,
            artifactGeneration: artifactGeneration,
            operationID: operationID
        )
        guard try encodeTopUpUnsigned(value) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpUnsigned.canonical")
        }
        return value
    }

    public static func encodeTopUpShieldBuildRequest(
        _ request: KagemushaTopUpShieldBuildRequest
    ) throws -> Data {
        var zeroPath = OfflineCompactNoritoWriter()
        zeroPath.writeField(try sequence(request.zeroPath.siblings))
        zeroPath.writeField(bytes(request.zeroPath.directions))
        zeroPath.writeField(request.zeroPath.root)

        var writer = OfflineCompactNoritoWriter()
        writer.writeField(chainID(request.chainID))
        writer.writeField(try assetID(request.assetID))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try accountID(request.payer))
        writer.writeField(request.operationID)
        writer.writeField(request.spendKey)
        writer.writeField(request.rho)
        writer.writeField(request.diversifier)
        writer.writeField(uint32(request.leafIndex))
        writer.writeField(zeroPath.data)
        writer.writeField(try verifierKeyID(request.shieldVerifierID))
        writer.writeField(request.shieldVerifierCommitment)
        writer.writeField(string(request.artifactGeneration))
        return frame(
            KagemushaRecursiveSpend.topUpShieldBuildRequestWireName,
            payload: writer.data
        )
    }

    public static func encodeTopUpRequest(
        _ request: KagemushaRecursiveSpendTopUpRequest
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try assetID(request.assetID))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try note(request.currentNote))
        writer.writeField(try topUpShieldEvidence(request.shieldEvidence))
        writer.writeField(string(request.artifactGeneration))
        writer.writeField(request.operationID)
        writer.writeField(try nestedPayload(
            request.authorization.archive,
            schema: KagemushaRecursiveSpend.authorizationWireName,
            field: "authorization"
        ))
        return frame(KagemushaRecursiveSpend.topUpRequestWireName, payload: writer.data)
    }

    public static func encodeTopUpAnchor(
        _ anchor: KagemushaRecursiveSpendTopUpAnchor
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(uint16(anchor.version))
        writer.writeField(chainID(anchor.chainID))
        writer.writeField(try accountID(anchor.payer))
        writer.writeField(try assetID(anchor.assetID))
        writer.writeField(uint32(anchor.assetScale))
        writer.writeField(try scaledAmount(anchor.amount))
        writer.writeField(anchor.initialRoot)
        writer.writeField(anchor.finalizedRoot)
        writer.writeField(uint32(anchor.shieldLeafIndex))
        writer.writeField(try note(anchor.currentNote))
        writer.writeField(anchor.topUpOperationID)
        writer.writeField(try verifierKeyID(anchor.shieldVerifierID))
        writer.writeField(anchor.shieldVerifierCommitment)
        writer.writeField(string(anchor.artifactGeneration))
        writer.writeField(uint64(anchor.finalizedHeight))
        writer.writeField(anchor.finalizedTransactionHash)
        writer.writeField(anchor.anchorDigest)
        return frame(KagemushaRecursiveSpend.topUpAnchorWireName, payload: writer.data)
    }

    public static func decodeTopUpAnchor(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendTopUpAnchor {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.topUpAnchorWireName,
            field: "topUpAnchor"
        ))
        let version = try scalarUInt16(reader.field(), field: "version")
        let chain = try decodeChainID(reader.field())
        let payer = try decodeAccountID(reader.field())
        let asset = try decodeAssetID(reader.field())
        let scale = try scalarUInt32(reader.field(), field: "assetScale")
        let amount = try decodeScaledAmount(reader.field())
        let initialRoot = try packedFixed(reader.field(), count: 32, field: "initialRoot")
        let finalizedRoot = try packedFixed(reader.field(), count: 32, field: "finalizedRoot")
        let shieldLeafIndex = try scalarUInt32(reader.field(), field: "shieldLeafIndex")
        let currentNote = try decodeNote(reader.field())
        let operationID = try packedFixed(reader.field(), count: 32, field: "topUpOperationID")
        let verifierID = try decodeVerifierKeyID(reader.field())
        let verifierCommitment = try packedFixed(
            reader.field(), count: 32, field: "shieldVerifierCommitment"
        )
        let generation = try decodeString(reader.field(), field: "artifactGeneration")
        let height = try scalarUInt64(reader.field(), field: "finalizedHeight")
        let transactionHash = try packedFixed(
            reader.field(), count: 32, field: "finalizedTransactionHash"
        )
        let digest = try packedFixed(reader.field(), count: 32, field: "anchorDigest")
        try reader.finish("topUpAnchor")
        let anchor = try KagemushaRecursiveSpendTopUpAnchor(
            version: version,
            chainID: chain,
            payer: payer,
            assetID: asset,
            assetScale: scale,
            amount: amount,
            initialRoot: initialRoot,
            finalizedRoot: finalizedRoot,
            shieldLeafIndex: shieldLeafIndex,
            currentNote: currentNote,
            topUpOperationID: operationID,
            shieldVerifierID: verifierID,
            shieldVerifierCommitment: verifierCommitment,
            artifactGeneration: generation,
            finalizedHeight: height,
            finalizedTransactionHash: transactionHash,
            anchorDigest: digest,
            archive: archive
        )
        guard try encodeTopUpAnchor(anchor) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpAnchor.canonical")
        }
        return anchor
    }

    public static func encodeSplitIntentBuildRequest(
        _ request: KagemushaRecursiveSpendSplitIntentBuildRequest
    ) throws -> Data {
        let bundles = try request.previousBundles.map {
            try nestedPayload(
                $0.archive,
                schema: KagemushaRecursiveSpend.bundleWireName,
                field: "previousBundles"
            )
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try sequence(bundles))
        writer.writeField(string(request.outputArtifactGeneration))
        writer.writeField(try scaledAmount(request.transferAmount))
        writer.writeField(try note(request.recipientOutput))
        writer.writeField(option(try request.changeOutput.map(note)))
        writer.writeField(request.recipientRequestDigest)
        writer.writeField(request.operationID)
        return frame(
            KagemushaRecursiveSpend.splitIntentBuildRequestWireName,
            payload: writer.data
        )
    }

    public static func decodeSplitIntent(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendSplitIntent {
        try decodeSplit(try payload(
            archive,
            schema: KagemushaRecursiveSpend.splitIntentWireName,
            field: "splitIntent"
        ))
    }

    public static func encodeRedemptionIntentBuildRequest(
        _ request: KagemushaRecursiveSpendRedemptionIntentBuildRequest
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.previousBundle.archive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "previousBundle"
        ))
        writer.writeField(try accountID(request.recipient))
        writer.writeField(try scaledAmount(request.publicAmount))
        writer.writeField(option(try request.changeOutput.map(note)))
        writer.writeField(option(request.changeArtifactGeneration.map(string)))
        writer.writeField(unshieldBinding(request.unshieldPublicInputs))
        writer.writeField(request.unshieldPublicInputsDigest)
        writer.writeField(request.operationID)
        return frame(
            KagemushaRecursiveSpend.redemptionIntentBuildRequestWireName,
            payload: writer.data
        )
    }

    public static func decodeRedemptionIntent(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendRedemptionIntent {
        try decodeRedemptionIntentPayload(try payload(
            archive,
            schema: KagemushaRecursiveSpend.redemptionIntentWireName,
            field: "redemptionIntent"
        ))
    }

    public static func encodeAppendRequest(
        _ request: KagemushaRecursiveSpendAppendRequest
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try sequence(request.previousInputs.map(appendInput)))
        writer.writeField(try nestedPayload(
            request.recordBundle,
            schema: KagemushaRecursiveSpend.verifiedFoldRecordBundleWireName,
            field: "recordBundle"
        ))
        writer.writeField(bytes(request.pallasOpenEnvelopesArchive))
        writer.writeField(try split(request.split))
        writer.writeField(option(request.lineageArtifact.map(artifactReference)))
        writer.writeField(string(request.outputProofCircuitID))
        writer.writeField(uint64(request.blockHeight))
        return frame(KagemushaRecursiveSpend.appendRequestWireName, payload: writer.data)
    }

    public static func encodeVerifyRequest(
        _ request: KagemushaRecursiveSpendVerifyRequest
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.bundle.archive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "bundle"
        ))
        writer.writeField(try nestedPayload(
            request.recipientRequest.archive,
            schema: KagemushaRecursiveSpend.recipientRequestWireName,
            field: "recipientRequest"
        ))
        writer.writeField(uint32(request.maximumHops))
        writer.writeField(string(request.artifactGeneration))
        writer.writeField(uint64(request.verifiedAtMilliseconds))
        return frame(KagemushaRecursiveSpend.verifyRequestWireName, payload: writer.data)
    }

    public static func encodeLineageWitness(
        _ witness: KagemushaRecursiveSpendLineageWitness
    ) throws -> Data {
        frame(
            KagemushaRecursiveSpend.lineageWitnessWireName,
            payload: lineageWitness(witness)
        )
    }

    public static func decodeLineageWitness(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendLineageWitness {
        let witness = try decodeLineageWitnessPayload(payload(
            archive,
            schema: KagemushaRecursiveSpend.lineageWitnessWireName,
            field: "lineageWitness"
        ))
        guard try encodeLineageWitness(witness) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("lineageWitness.canonical")
        }
        return witness
    }

    public static func encodeRedeemRequest(
        _ request: KagemushaRecursiveSpendRedeemRequest
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.bundle.archive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "bundle"
        ))
        writer.writeField(try accountID(request.recipient))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try nestedPayload(
            request.redeemProof,
            schema: KagemushaRecursiveSpend.proofAttachmentWireName,
            field: "redeemProof"
        ))
        writer.writeField(try redemptionIntent(request.redemption))
        writer.writeField(option(request.lineageWitness.map(lineageWitness)))
        writer.writeField(try nestedPayload(
            request.lineageVerifierRecord.recordBytes,
            schema: KagemushaRecursiveSpend.verifyingKeyRecordWireName,
            field: "lineageVerifierRecord"
        ))
        writer.writeField(option(try request.offlineChange.map(redeemChangeBranch)))
        writer.writeField(uint64(request.blockHeight))
        writer.writeField(request.operationID)
        writer.writeField(try nestedPayload(
            request.authorization.archive,
            schema: KagemushaRecursiveSpend.authorizationWireName,
            field: "authorization"
        ))
        return frame(KagemushaRecursiveSpend.redeemRequestWireName, payload: writer.data)
    }

    public static func encodeRedeemUnsigned(
        _ request: KagemushaRecursiveSpendRedeemUnsigned
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.bundle.archive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "bundle"
        ))
        writer.writeField(try accountID(request.recipient))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try nestedPayload(
            request.redeemProof,
            schema: KagemushaRecursiveSpend.proofAttachmentWireName,
            field: "redeemProof"
        ))
        writer.writeField(try redemptionIntent(request.redemption))
        writer.writeField(option(request.lineageWitness.map(lineageWitness)))
        writer.writeField(try nestedPayload(
            request.lineageVerifierRecord.recordBytes,
            schema: KagemushaRecursiveSpend.verifyingKeyRecordWireName,
            field: "lineageVerifierRecord"
        ))
        writer.writeField(option(try request.offlineChange.map(redeemChangeBranch)))
        writer.writeField(uint64(request.blockHeight))
        writer.writeField(request.operationID)
        return frame(KagemushaRecursiveSpend.redeemUnsignedWireName, payload: writer.data)
    }

    public static func encodeRedeemChangeBuildRequest(
        _ request: KagemushaRecursiveSpendRedeemChangeBuildRequest
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.previousBundle.archive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "previousBundle"
        ))
        writer.writeField(bytes(request.previousRecursiveProofOpenEnvelopesArchive))
        writer.writeField(try nestedPayload(
            request.unshieldRecordBundle,
            schema: KagemushaRecursiveSpend.verifiedFoldRecordBundleWireName,
            field: "unshieldRecordBundle"
        ))
        writer.writeField(bytes(request.pallasOpenEnvelopesArchive))
        writer.writeField(try redemptionIntent(request.redemption))
        writer.writeField(artifactReference(request.lineageArtifact))
        writer.writeField(try nestedPayload(
            request.previousLineageVerifierRecord.recordBytes,
            schema: KagemushaRecursiveSpend.verifyingKeyRecordWireName,
            field: "previousLineageVerifierRecord"
        ))
        writer.writeField(uint64(request.blockHeight))
        return frame(
            KagemushaRecursiveSpend.redeemChangeBuildRequestWireName,
            payload: writer.data
        )
    }

    public static func decodeRedeemChangeBuildResult(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendRedeemChangeBuildResult {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.redeemChangeBuildResultWireName,
            field: "redeemChangeBuildResult"
        ))
        let branch = try decodeRedeemChangeBranch(reader.field())
        let transitionDigest = try packedFixed(
            reader.field(), count: 32, field: "transitionBindingDigest"
        )
        let statementDigest = try packedFixed(
            reader.field(), count: 32, field: "publicStatementDigest"
        )
        try reader.finish("redeemChangeBuildResult")
        return KagemushaRecursiveSpendRedeemChangeBuildResult(
            changeBranch: branch,
            transitionBindingDigest: transitionDigest,
            publicStatementDigest: statementDigest
        )
    }

    public static func decodeBundleSummary(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendBundleSummary {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.bundleSummaryWireName,
            field: "bundleSummary"
        ))
        let asset = try decodeAssetDefinitionID(reader.field())
        let amount = try decodeScaledAmount(reader.field())
        let commitment = try packedFixed(reader.field(), count: 32, field: "noteCommitment")
        let nullifier = try packedFixed(reader.field(), count: 32, field: "spendNullifier")
        let hopCount = try scalarUInt32(reader.field(), field: "hopCount")
        let branchClaims = try decodeBranchClaims(reader.field(), field: "branchClaims")
        let generation = try decodeString(reader.field(), field: "artifactGeneration")
        let verifierKeyID = try decodeVerifierKeyID(reader.field())
        let lineageMode = try decodeLineageMode(reader.field())
        let digest = try packedFixed(reader.field(), count: 32, field: "bundleDigest")
        try reader.finish("bundleSummary")
        return KagemushaRecursiveSpendBundleSummary(
            assetDefinitionID: asset,
            amount: amount,
            noteCommitment: commitment,
            spendNullifier: nullifier,
            hopCount: hopCount,
            branchClaims: branchClaims,
            artifactGeneration: generation,
            verifierKeyID: verifierKeyID,
            lineageMode: lineageMode,
            bundleDigest: digest
        )
    }

    public static func decodeSplitResult(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendSplitResult {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.splitResultWireName,
            field: "splitResult"
        ))
        let split = try decodeSplit(reader.field())
        let binding = try packedFixed(reader.field(), count: 32, field: "splitBindingDigest")
        let recipientArchive = frame(
            KagemushaRecursiveSpend.bundleWireName,
            payload: try reader.field()
        )
        let changePayload = try decodeOption(reader.field(), field: "changeBundle")
        try reader.finish("splitResult")
        let recipient = try KagemushaRecursiveSpendBundle(noritoArchive: recipientArchive)
        let change = try changePayload.map {
            try KagemushaRecursiveSpendBundle(
                noritoArchive: frame(KagemushaRecursiveSpend.bundleWireName, payload: $0)
            )
        }
        return try KagemushaRecursiveSpendSplitResult(
            split: split,
            splitBindingDigest: binding,
            recipientBundle: recipient,
            changeBundle: change,
            archive: archive
        )
    }

    public static func decodePeerPayment(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendPeerPayment {
        guard archive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("peerPayment.size")
        }
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.peerPaymentWireName,
            field: "peerPayment"
        ))
        let bundleArchive = frame(
            KagemushaRecursiveSpend.bundleWireName,
            payload: try reader.field()
        )
        try reader.finish("peerPayment")
        return try KagemushaRecursiveSpendPeerPayment(
            recipientBundle: KagemushaRecursiveSpendBundle(noritoArchive: bundleArchive),
            archive: archive
        )
    }

    public static func encodePeerPayment(
        recipientBundle: KagemushaRecursiveSpendBundle
    ) throws -> Data {
        _ = try recipientPeerSplitIdentity(from: recipientBundle.archive)
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            recipientBundle.archive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "recipientBundle"
        ))
        let archive = frame(KagemushaRecursiveSpend.peerPaymentWireName, payload: writer.data)
        guard archive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("peerPayment.size")
        }
        return archive
    }

    /// Extract the canonical replay identity from a recipient bundle's
    /// proof-bound peer-split transition. The transport wire never duplicates
    /// either value beside the bundle.
    static func recipientPeerSplitIdentity(
        from bundleArchive: Data
    ) throws -> (operationID: Data, recipientRequestDigest: Data) {
        var bundleReader = KagemushaV2Reader(try payload(
            bundleArchive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "peerPayment.recipientBundle"
        ))
        let statementPayload = try bundleReader.field()
        _ = try bundleReader.field() // Recursive proof; validated by the native bridge.
        try bundleReader.finish("peerPayment.recipientBundle")

        var statementReader = KagemushaV2Reader(statementPayload)
        // chain, asset, scale, final root, anchor refs, proof steps, peer hops,
        // current note, and branch claims precede the producing transition.
        for _ in 0..<9 {
            _ = try statementReader.field()
        }
        guard let encodedTransition = try decodeOption(
            statementReader.field(),
            field: "peerPayment.transition"
        ) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "peerPayment.transition"
            )
        }
        // Artifact generation, lineage mode, and verifier id follow it.
        for _ in 0..<3 {
            _ = try statementReader.field()
        }
        try statementReader.finish("peerPayment.statement")

        var transitionReader = KagemushaV2Reader(encodedTransition)
        guard try transitionReader.uint32() == 0 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "peerPayment.transition"
            )
        }
        let peerSplitPayload = try transitionReader.field()
        try transitionReader.finish("peerPayment.transition")

        var peerSplitReader = KagemushaV2Reader(peerSplitPayload)
        _ = try packedFixed(
            peerSplitReader.field(),
            count: 32,
            field: "peerPayment.bindingDigest"
        )
        guard try scalarUInt32(
            peerSplitReader.field(),
            field: "peerPayment.branch"
        ) == KagemushaRecursiveSpendBranch.recipient.rawValue else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "peerPayment.branch"
            )
        }
        let requestDigest = try packedFixed(
            peerSplitReader.field(),
            count: 32,
            field: "peerPayment.recipientRequestDigest"
        )
        let operationID = try packedFixed(
            peerSplitReader.field(),
            count: 32,
            field: "peerPayment.operationID"
        )
        _ = try scalarUInt32(
            peerSplitReader.field(),
            field: "peerPayment.parentMaxProofStepCount"
        )
        _ = try scalarUInt32(
            peerSplitReader.field(),
            field: "peerPayment.parentMaxPeerHopCount"
        )
        try peerSplitReader.finish("peerPayment.peerSplit")
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            operationID,
            field: "peerPayment.operationID"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            requestDigest,
            field: "peerPayment.recipientRequestDigest"
        )
        return (operationID, requestDigest)
    }

    public static func decodeAcknowledgementPayload(
        _ archive: Data
    ) throws -> KagemushaReceiverAcknowledgementPayload {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.acknowledgementPayloadWireName,
            field: "acknowledgementPayload"
        ))
        let operationID = try packedFixed(reader.field(), count: 32, field: "operationID")
        let requestDigest = try packedFixed(
            reader.field(), count: 32, field: "recipientRequestDigest"
        )
        let bundleDigest = try packedFixed(
            reader.field(), count: 32, field: "paymentBundleDigest"
        )
        let commitment = try packedFixed(
            reader.field(), count: 32, field: "recipientCommitment"
        )
        let acceptedAt = try scalarUInt64(reader.field(), field: "acceptedAtMilliseconds")
        let deviceID = try decodeString(reader.field(), field: "receiverDeviceID")
        let keyReference = try packedFixed(
            reader.field(), count: 32, field: "receiverKeyReference"
        )
        let key = try decodePublicKey(reader.field())
        try reader.finish("acknowledgementPayload")
        let canonical = frame(
            KagemushaRecursiveSpend.acknowledgementPayloadWireName,
            payload: acknowledgementPayload(
                operationID: operationID,
                requestDigest: requestDigest,
                bundleDigest: bundleDigest,
                commitment: commitment,
                acceptedAt: acceptedAt,
                deviceID: deviceID,
                keyReference: keyReference,
                key: key
            )
        )
        guard canonical == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("acknowledgementPayload.canonical")
        }
        return try KagemushaReceiverAcknowledgementPayload(
            operationID: operationID,
            recipientRequestDigest: requestDigest,
            paymentBundleDigest: bundleDigest,
            recipientCommitment: commitment,
            acceptedAtMilliseconds: acceptedAt,
            receiverDeviceID: deviceID,
            receiverKeyReference: keyReference,
            receiverPublicKey: key,
            archive: archive
        )
    }

    public static func decodeAcknowledgement(
        _ archive: Data
    ) throws -> KagemushaReceiverAcknowledgement {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.acknowledgementWireName,
            field: "acknowledgement"
        ))
        let payloadArchive = frame(
            KagemushaRecursiveSpend.acknowledgementPayloadWireName,
            payload: try reader.field()
        )
        let signature = try decodeConstVec(try reader.field(), field: "acknowledgement.signature")
        try reader.finish("acknowledgement")
        guard archive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("acknowledgement.size")
        }
        return try KagemushaReceiverAcknowledgement(
            payload: decodeAcknowledgementPayload(payloadArchive),
            signature: signature,
            archive: archive
        )
    }

    public static func decodeAcknowledgementVerifyResult(
        _ archive: Data
    ) throws -> KagemushaReceiverAcknowledgementVerifyResult {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.acknowledgementVerifyResultWireName,
            field: "acknowledgementVerifyResult"
        ))
        let valid = try decodeBool(reader.field(), field: "valid")
        let operation = try packedFixed(reader.field(), count: 32, field: "operationID")
        let request = try packedFixed(reader.field(), count: 32, field: "requestDigest")
        let bundle = try packedFixed(reader.field(), count: 32, field: "bundleDigest")
        let acknowledgement = try packedFixed(
            reader.field(), count: 32, field: "acknowledgementDigest"
        )
        try reader.finish("acknowledgementVerifyResult")
        guard valid else {
            throw KagemushaRecursiveSpendError.invalidArchive("acknowledgementVerifyResult.valid")
        }
        return KagemushaReceiverAcknowledgementVerifyResult(
            valid: valid,
            operationID: operation,
            recipientRequestDigest: request,
            paymentBundleDigest: bundle,
            acknowledgementDigest: acknowledgement
        )
    }

    public static func decodeVerifyResult(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendVerifyResult {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.verifyResultWireName,
            field: "verifyResult"
        ))
        let valid = try decodeBool(reader.field(), field: "valid")
        let chainAdmissible = try decodeBool(reader.field(), field: "chainAdmissible")
        let lineageRedeemable = try decodeBool(reader.field(), field: "lineageRedeemable")
        let witnessless = try decodeBool(reader.field(), field: "witnesslessRedemptionSupported")
        let lineageMode = try decodeLineageMode(reader.field())
        let summaryArchive = frame(
            KagemushaRecursiveSpend.bundleSummaryWireName,
            payload: try reader.field()
        )
        let requestDigest = try packedFixed(
            reader.field(), count: 32, field: "recipientRequestDigest"
        )
        let bindingDigest = try packedFixed(
            reader.field(), count: 32, field: "requestOutputBindingDigest"
        )
        let verifierKeyID = try decodeVerifierKeyID(reader.field())
        let circuitID = try decodeString(reader.field(), field: "verifierCircuitID")
        let activation = try decodeOptionalUInt64(reader.field(), field: "activationHeight")
        let withdrawal = try decodeOptionalUInt64(reader.field(), field: "withdrawHeight")
        let blockHeight = try scalarUInt64(reader.field(), field: "verifiedAtBlockHeight")
        let verifiedAt = try scalarUInt64(reader.field(), field: "verifiedAtMilliseconds")
        let witness = try decodeOption(reader.field(), field: "verifiedLineageWitness")
            .map(decodeLineageWitnessPayload)
        try reader.finish("verifyResult")
        guard valid else {
            throw KagemushaRecursiveSpendError.invalidArchive("verifyResult.valid")
        }
        return KagemushaRecursiveSpendVerifyResult(
            valid: valid,
            chainAdmissible: chainAdmissible,
            lineageRedeemable: lineageRedeemable,
            witnesslessRedemptionSupported: witnessless,
            lineageMode: lineageMode,
            summary: try decodeBundleSummary(summaryArchive),
            recipientRequestDigest: requestDigest,
            requestOutputBindingDigest: bindingDigest,
            verifierKeyID: verifierKeyID,
            verifierCircuitID: circuitID,
            verifierActivationHeight: activation,
            verifierWithdrawHeight: withdrawal,
            verifiedAtBlockHeight: blockHeight,
            verifiedAtMilliseconds: verifiedAt,
            verifiedLineageWitness: witness
        )
    }

    public static func decodeRedeemResult(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendRedeemResult {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.redeemResultWireName,
            field: "redeemResult"
        ))
        let requestArchive = try decodeBytes(reader.field(), field: "redeemRequestArchive")
        let changePayload = try decodeOption(reader.field(), field: "offlineChangeBundle")
        let operationID = try packedFixed(reader.field(), count: 32, field: "operationID")
        try reader.finish("redeemResult")
        let change = try changePayload.map {
            try KagemushaRecursiveSpendBundle(
                noritoArchive: frame(KagemushaRecursiveSpend.bundleWireName, payload: $0)
            )
        }
        return KagemushaRecursiveSpendRedeemResult(
            redeemRequestArchive: requestArchive,
            offlineChangeBundle: change,
            operationID: operationID
        )
    }

    private static func encodeRecipientRequest(
        _ payload: KagemushaRecipientPaymentRequestSigningPayload,
        signature: Data
    ) throws -> Data {
        let payloadArchive = try encodeRecipientRequestPayload(payload)
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            payloadArchive,
            schema: KagemushaRecursiveSpend.recipientRequestPayloadWireName,
            field: "recipientRequestPayload"
        ))
        // The signed request flattens payload fields rather than nesting the
        // signing-payload type, so strip the field wrapper just constructed.
        var flattened = KagemushaV2Reader(writer.data)
        let payloadFields = try flattened.field()
        var result = OfflineCompactNoritoWriter()
        result.writeBytes(payloadFields)
        result.writeField(constVec(signature))
        return frame(KagemushaRecursiveSpend.recipientRequestWireName, payload: result.data)
    }

    private static func encodeAuthorization(
        _ fields: KagemushaRequestAuthorizationFields,
        signature: Data
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try accountID(fields.authority))
        writer.writeField(string(fields.deviceID))
        writer.writeField(fields.operationID)
        writer.writeField(uint64(fields.issuedAtMilliseconds))
        writer.writeField(uint64(fields.expiresAtMilliseconds))
        writer.writeField(fields.nonce)
        writer.writeField(fields.payloadDigest)
        writer.writeField(option(fields.appAttestEvidenceSHA256.map(constVec)))
        writer.writeField(option(fields.appAttestEvidence.map(bytes)))
        writer.writeField(constVec(signature))
        return frame(KagemushaRecursiveSpend.authorizationWireName, payload: writer.data)
    }

    private static func decodeRecipientRequestPayloadFields(
        _ reader: inout KagemushaV2Reader
    ) throws -> KagemushaRecipientPaymentRequestSigningPayload {
        let chain = try decodeChainID(reader.field())
        let asset = try decodeAssetDefinitionID(reader.field())
        let amount = try decodeScaledAmount(reader.field())
        let recipient = try decodeAccountID(reader.field())
        let keyReference = try packedFixed(
            reader.field(), count: 32, field: "recipientKeyReference"
        )
        let device = try decodeString(reader.field(), field: "receiverDeviceID")
        let key = try decodePublicKey(reader.field())
        let requestID = try packedFixed(reader.field(), count: 32, field: "requestID")
        let issued = try scalarUInt64(reader.field(), field: "issuedAtMilliseconds")
        let expires = try scalarUInt64(reader.field(), field: "expiresAtMilliseconds")
        let output = try decodeNote(reader.field())
        let material = try decodeBytes(reader.field(), field: "recipientOutputProverMaterial")
        return try KagemushaRecipientPaymentRequestSigningPayload(
            chainID: chain,
            assetDefinitionID: asset,
            amount: amount,
            recipient: recipient,
            recipientKeyReference: keyReference,
            receiverDeviceID: device,
            receiverPublicKey: key,
            requestID: requestID,
            issuedAtMilliseconds: issued,
            expiresAtMilliseconds: expires,
            recipientOutput: output,
            recipientOutputProverMaterial: material
        )
    }

    private static func topUpAnchorRef(
        _ value: KagemushaRecursiveSpendTopUpAnchorRef
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(value.topUpOperationID)
        writer.writeField(value.anchorDigest)
        return writer.data
    }

    private static func decodeTopUpAnchorRef(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendTopUpAnchorRef {
        var reader = KagemushaV2Reader(data)
        let operationID = try packedFixed(
            reader.field(), count: 32, field: "topUpAnchorRef.topUpOperationID"
        )
        let anchorDigest = try packedFixed(
            reader.field(), count: 32, field: "topUpAnchorRef.anchorDigest"
        )
        try reader.finish("topUpAnchorRef")
        return try KagemushaRecursiveSpendTopUpAnchorRef(
            topUpOperationID: operationID,
            anchorDigest: anchorDigest
        )
    }

    private static func split(_ value: KagemushaRecursiveSpendSplitIntent) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(chainID(value.chainID))
        writer.writeField(try assetDefinitionID(value.assetDefinitionID))
        writer.writeField(try sequence(value.inputs.map(inputBranch)))
        writer.writeField(try sequence(value.topUpAnchorRefs.map(topUpAnchorRef)))
        writer.writeField(uint32(value.assetScale))
        writer.writeField(uint32(value.lineageMode.rawValue))
        writer.writeField(string(value.outputArtifactGeneration))
        writer.writeField(try scaledAmount(value.transferAmount))
        writer.writeField(try note(value.recipientOutput))
        writer.writeField(option(try value.changeOutput.map(note)))
        writer.writeField(value.recipientRequestDigest)
        writer.writeField(value.operationID)
        return writer.data
    }

    private static func decodeSplit(_ data: Data) throws -> KagemushaRecursiveSpendSplitIntent {
        var reader = KagemushaV2Reader(data)
        let chain = try decodeChainID(reader.field())
        let asset = try decodeAssetDefinitionID(reader.field())
        var inputReader = KagemushaV2Reader(try reader.field())
        let inputCount = try inputReader.uint64()
        guard (1...2).contains(inputCount) else {
            throw KagemushaRecursiveSpendError.invalidArchive("split.inputs")
        }
        var inputs: [KagemushaRecursiveSpendInputBranch] = []
        inputs.reserveCapacity(Int(inputCount))
        for _ in 0..<inputCount {
            inputs.append(try decodeInputBranch(inputReader.field()))
        }
        try inputReader.finish("split.inputs")
        var anchorReader = KagemushaV2Reader(try reader.field())
        let anchorCount = try anchorReader.uint64()
        guard (1...2).contains(anchorCount) else {
            throw KagemushaRecursiveSpendError.invalidArchive("split.topUpAnchorRefs")
        }
        var anchorRefs: [KagemushaRecursiveSpendTopUpAnchorRef] = []
        anchorRefs.reserveCapacity(Int(anchorCount))
        for _ in 0..<anchorCount {
            anchorRefs.append(try decodeTopUpAnchorRef(anchorReader.field()))
        }
        try anchorReader.finish("split.topUpAnchorRefs")
        let scale = try scalarUInt32(reader.field(), field: "assetScale")
        let lineageMode = try decodeLineageMode(reader.field())
        let outputGeneration = try decodeString(
            reader.field(),
            field: "outputArtifactGeneration"
        )
        let amount = try decodeScaledAmount(reader.field())
        let recipient = try decodeNote(reader.field())
        let change = try decodeOption(reader.field(), field: "changeOutput").map(decodeNote)
        let requestDigest = try packedFixed(
            reader.field(), count: 32, field: "recipientRequestDigest"
        )
        let operation = try packedFixed(reader.field(), count: 32, field: "operationID")
        try reader.finish("split")
        return try KagemushaRecursiveSpendSplitIntent(
            chainID: chain,
            assetDefinitionID: asset,
            inputs: inputs,
            topUpAnchorRefs: anchorRefs,
            assetScale: scale,
            lineageMode: lineageMode,
            outputArtifactGeneration: outputGeneration,
            transferAmount: amount,
            recipientOutput: recipient,
            changeOutput: change,
            recipientRequestDigest: requestDigest,
            operationID: operation
        )
    }

    private static func inputBranch(
        _ value: KagemushaRecursiveSpendInputBranch
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(value.bundleDigest)
        writer.writeField(try note(value.inputNote))
        writer.writeField(try branchClaims(value.branchClaims))
        writer.writeField(value.inputRoot)
        writer.writeField(uint32(value.proofStepCount))
        writer.writeField(uint32(value.peerHopCount))
        return writer.data
    }

    private static func decodeInputBranch(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendInputBranch {
        var reader = KagemushaV2Reader(data)
        let digest = try packedFixed(reader.field(), count: 32, field: "input.bundleDigest")
        let note = try decodeNote(reader.field())
        let claims = try decodeBranchClaims(reader.field(), field: "input.branchClaims")
        let root = try packedFixed(reader.field(), count: 32, field: "input.inputRoot")
        let proofSteps = try scalarUInt32(reader.field(), field: "input.proofStepCount")
        let peerHops = try scalarUInt32(reader.field(), field: "input.peerHopCount")
        try reader.finish("split.input")
        return try KagemushaRecursiveSpendInputBranch(
            bundleDigest: digest,
            inputNote: note,
            branchClaims: claims,
            inputRoot: root,
            proofStepCount: proofSteps,
            peerHopCount: peerHops
        )
    }

    private static func appendInput(
        _ value: KagemushaRecursiveSpendAppendInput
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            value.previousBundle.archive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "previousInput.previousBundle"
        ))
        writer.writeField(try optionalVerifierRecord(value.previousLineageVerifierRecord))
        writer.writeField(bytes(value.previousRecursiveProofOpenEnvelopesArchive))
        return writer.data
    }

    private static func unshieldBinding(
        _ value: KagemushaUnshieldPublicInputsBinding
    ) -> Data {
        func pair(_ values: [Data]) -> Data {
            var writer = OfflineCompactNoritoWriter()
            for value in values { writer.writeField(constVec(value)) }
            return writer.data
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(pair(value.inputCommitments))
        writer.writeField(pair(value.nullifiers))
        writer.writeField(value.changeOutputCommitment)
        writer.writeField(value.root)
        writer.writeField(value.publicAmount)
        writer.writeField(value.assetTag)
        writer.writeField(value.chainTag)
        return writer.data
    }

    private static func decodeUnshieldBinding(
        _ data: Data
    ) throws -> KagemushaUnshieldPublicInputsBinding {
        func pair(_ data: Data, field: String) throws -> [Data] {
            var reader = KagemushaV2Reader(data)
            var values: [Data] = []
            for _ in 0..<2 {
                let value = try decodeConstVec(try reader.field(), field: field)
                guard value.count == 32 else {
                    throw KagemushaRecursiveSpendError.invalidArchive(field)
                }
                values.append(value)
            }
            try reader.finish(field)
            return values
        }
        var reader = KagemushaV2Reader(data)
        let commitments = try pair(try reader.field(), field: "inputCommitments")
        let nullifiers = try pair(try reader.field(), field: "nullifiers")
        let change = try packedFixed(
            reader.field(), count: 32, field: "changeOutputCommitment"
        )
        let root = try packedFixed(reader.field(), count: 32, field: "root")
        let amount = try packedFixed(reader.field(), count: 32, field: "publicAmount")
        let asset = try packedFixed(reader.field(), count: 32, field: "assetTag")
        let chain = try packedFixed(reader.field(), count: 32, field: "chainTag")
        try reader.finish("unshieldBinding")
        return try KagemushaUnshieldPublicInputsBinding(
            inputCommitments: commitments,
            nullifiers: nullifiers,
            changeOutputCommitment: change,
            root: root,
            publicAmount: amount,
            assetTag: asset,
            chainTag: chain
        )
    }

    private static func redemptionIntent(
        _ value: KagemushaRecursiveSpendRedemptionIntent
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(chainID(value.chainID))
        writer.writeField(try assetDefinitionID(value.assetDefinitionID))
        writer.writeField(try note(value.inputNote))
        writer.writeField(try branchClaims(value.parentBranchClaims))
        writer.writeField(try sequence(value.parentTopUpAnchorRefs.map(topUpAnchorRef)))
        writer.writeField(uint32(value.parentProofStepCount))
        writer.writeField(uint32(value.parentPeerHopCount))
        writer.writeField(value.parentBundleDigest)
        writer.writeField(value.inputRoot)
        writer.writeField(try accountID(value.recipient))
        writer.writeField(try scaledAmount(value.publicAmount))
        writer.writeField(option(try value.changeOutput.map(note)))
        writer.writeField(option(value.changeArtifactGeneration.map(string)))
        writer.writeField(unshieldBinding(value.unshieldPublicInputs))
        writer.writeField(value.unshieldPublicInputsDigest)
        writer.writeField(value.operationID)
        return writer.data
    }

    private static func decodeRedemptionIntentPayload(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendRedemptionIntent {
        var reader = KagemushaV2Reader(data)
        let chain = try decodeChainID(reader.field())
        let asset = try decodeAssetDefinitionID(reader.field())
        let input = try decodeNote(reader.field())
        let claims = try decodeBranchClaims(reader.field(), field: "parentBranchClaims")
        var anchorReader = KagemushaV2Reader(try reader.field())
        let anchorCount = try anchorReader.uint64()
        guard (1...2).contains(anchorCount) else {
            throw KagemushaRecursiveSpendError.invalidArchive("parentTopUpAnchorRefs")
        }
        var anchorRefs: [KagemushaRecursiveSpendTopUpAnchorRef] = []
        anchorRefs.reserveCapacity(Int(anchorCount))
        for _ in 0..<anchorCount {
            anchorRefs.append(try decodeTopUpAnchorRef(anchorReader.field()))
        }
        try anchorReader.finish("parentTopUpAnchorRefs")
        let proofSteps = try scalarUInt32(reader.field(), field: "parentProofStepCount")
        let peerHops = try scalarUInt32(reader.field(), field: "parentPeerHopCount")
        let parentDigest = try packedFixed(
            reader.field(), count: 32, field: "parentBundleDigest"
        )
        let root = try packedFixed(reader.field(), count: 32, field: "inputRoot")
        let recipient = try decodeAccountID(reader.field())
        let amount = try decodeScaledAmount(reader.field())
        let change = try decodeOption(reader.field(), field: "changeOutput").map(decodeNote)
        let changeGeneration = try decodeOption(
            reader.field(),
            field: "changeArtifactGeneration"
        ).map { try decodeString($0, field: "changeArtifactGeneration") }
        let inputs = try decodeUnshieldBinding(reader.field())
        let inputsDigest = try packedFixed(
            reader.field(), count: 32, field: "unshieldPublicInputsDigest"
        )
        let operationID = try packedFixed(reader.field(), count: 32, field: "operationID")
        try reader.finish("redemptionIntent")
        return try KagemushaRecursiveSpendRedemptionIntent(
            chainID: chain,
            assetDefinitionID: asset,
            inputNote: input,
            parentBranchClaims: claims,
            parentTopUpAnchorRefs: anchorRefs,
            parentProofStepCount: proofSteps,
            parentPeerHopCount: peerHops,
            parentBundleDigest: parentDigest,
            inputRoot: root,
            recipient: recipient,
            publicAmount: amount,
            changeOutput: change,
            changeArtifactGeneration: changeGeneration,
            unshieldPublicInputs: inputs,
            unshieldPublicInputsDigest: inputsDigest,
            operationID: operationID
        )
    }

    private static func redeemChangeBranch(
        _ value: KagemushaRecursiveSpendRedeemChangeBranch
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try note(value.output))
        writer.writeField(try branchClaims(value.branchClaims))
        writer.writeField(try nestedPayload(
            value.bundle.archive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "redeemChangeBundle"
        ))
        return writer.data
    }

    private static func decodeRedeemChangeBranch(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendRedeemChangeBranch {
        var reader = KagemushaV2Reader(data)
        let output = try decodeNote(reader.field())
        let claims = try decodeBranchClaims(reader.field(), field: "redeemChange.branchClaims")
        let bundleArchive = frame(
            KagemushaRecursiveSpend.bundleWireName,
            payload: try reader.field()
        )
        try reader.finish("redeemChangeBranch")
        return KagemushaRecursiveSpendRedeemChangeBranch(
            output: output,
            branchClaims: claims,
            bundle: try KagemushaRecursiveSpendBundle(noritoArchive: bundleArchive)
        )
    }

    private static func scaledAmount(_ value: KagemushaScaledAmount) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try u128(value.atomicUnits))
        writer.writeField(uint32(value.scale))
        return writer.data
    }

    private static func decodeScaledAmount(_ data: Data) throws -> KagemushaScaledAmount {
        var reader = KagemushaV2Reader(data)
        let atomic = try decodeU128(reader.field())
        let scale = try scalarUInt32(reader.field(), field: "amount.scale")
        try reader.finish("amount")
        return try KagemushaScaledAmount(atomicUnits: atomic, scale: scale)
    }

    private static func note(_ value: KagemushaSpendableNoteDescriptor) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(chainID(value.chainID))
        writer.writeField(try assetDefinitionID(value.assetDefinitionID))
        writer.writeField(value.noteCommitment)
        writer.writeField(value.spendNullifier)
        writer.writeField(try scaledAmount(value.amount))
        return writer.data
    }

    private static func decodeNote(_ data: Data) throws -> KagemushaSpendableNoteDescriptor {
        var reader = KagemushaV2Reader(data)
        let chain = try decodeChainID(reader.field())
        let asset = try decodeAssetDefinitionID(reader.field())
        let commitment = try packedFixed(reader.field(), count: 32, field: "noteCommitment")
        let nullifier = try packedFixed(reader.field(), count: 32, field: "spendNullifier")
        let amount = try decodeScaledAmount(reader.field())
        try reader.finish("note")
        return try KagemushaSpendableNoteDescriptor(
            chainID: chain,
            assetDefinitionID: asset,
            noteCommitment: commitment,
            spendNullifier: nullifier,
            amount: amount
        )
    }

    private static func topUpShieldEvidence(
        _ value: KagemushaTopUpShieldEvidence
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(value.initialRoot)
        writer.writeField(value.finalizedRoot)
        writer.writeField(uint32(value.leafIndex))
        writer.writeField(try nestedPayload(
            value.proofAttachment,
            schema: KagemushaRecursiveSpend.proofAttachmentWireName,
            field: "shieldEvidence.proofAttachment"
        ))
        return writer.data
    }

    private static func decodeTopUpShieldEvidence(
        _ data: Data
    ) throws -> KagemushaTopUpShieldEvidence {
        var reader = KagemushaV2Reader(data)
        let initialRoot = try packedFixed(reader.field(), count: 32, field: "initialRoot")
        let finalizedRoot = try packedFixed(reader.field(), count: 32, field: "finalizedRoot")
        let leafIndex = try scalarUInt32(reader.field(), field: "leafIndex")
        let proofAttachment = frame(
            KagemushaRecursiveSpend.proofAttachmentWireName,
            payload: try reader.field()
        )
        try reader.finish("shieldEvidence")
        return try KagemushaTopUpShieldEvidence(
            initialRoot: initialRoot,
            finalizedRoot: finalizedRoot,
            leafIndex: leafIndex,
            proofAttachment: proofAttachment
        )
    }

    private static func branchPath(_ value: KagemushaRecursiveSpendBranchPath) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(value.lineageRoot)
        writer.writeField(Data([value.depth]))
        writer.writeField(value.pathBits)
        return writer.data
    }

    private static func decodeBranchPath(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendBranchPath {
        var reader = KagemushaV2Reader(data)
        let root = try packedFixed(reader.field(), count: 32, field: "lineageRoot")
        let depthData = try reader.field()
        guard depthData.count == 1, let depth = depthData.first else {
            throw KagemushaRecursiveSpendError.invalidArchive("branchPath.depth")
        }
        let path = try packedFixed(reader.field(), count: 8, field: "pathBits")
        try reader.finish("branchPath")
        return try KagemushaRecursiveSpendBranchPath(
            lineageRoot: root,
            depth: depth,
            pathBits: path
        )
    }

    static func encodeBranchClaim(
        _ value: KagemushaRecursiveSpendBranchClaim
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(branchPath(value.path))
        writer.writeField(bytes(value.transitionTags.reduce(into: Data()) { result, tag in
            result.append(tag)
        }))
        return writer.data
    }

    static func decodeBranchClaim(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendBranchClaim {
        var reader = KagemushaV2Reader(data)
        let path = try decodeBranchPath(reader.field())
        let flattenedTags = try decodeBytes(
            reader.field(),
            field: "branchClaim.transitionTags"
        )
        let expectedLength = Int(path.depth) * KagemushaRecursiveSpend.transitionTagBytes
        guard flattenedTags.count == expectedLength else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "branchClaim.transitionTags"
            )
        }
        var tags: [Data] = []
        tags.reserveCapacity(Int(path.depth))
        for start in stride(
            from: 0,
            to: flattenedTags.count,
            by: KagemushaRecursiveSpend.transitionTagBytes
        ) {
            let end = start + KagemushaRecursiveSpend.transitionTagBytes
            tags.append(Data(flattenedTags[start..<end]))
        }
        try reader.finish("branchClaim")
        return try KagemushaRecursiveSpendBranchClaim(
            path: path,
            transitionTags: tags
        )
    }

    private static func branchClaims(
        _ values: [KagemushaRecursiveSpendBranchClaim]
    ) throws -> Data {
        try KagemushaRecursiveSpend.validateBranchClaims(values)
        return try sequence(values.map(encodeBranchClaim))
    }

    private static func decodeBranchClaims(
        _ data: Data,
        field: String
    ) throws -> [KagemushaRecursiveSpendBranchClaim] {
        var reader = KagemushaV2Reader(data)
        let count = try reader.uint64()
        guard count > 0,
              count <= UInt64(KagemushaRecursiveSpend.maximumBranchClaims) else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
        var claims: [KagemushaRecursiveSpendBranchClaim] = []
        claims.reserveCapacity(Int(count))
        for _ in 0..<count {
            claims.append(try decodeBranchClaim(reader.field()))
        }
        try reader.finish(field)
        try KagemushaRecursiveSpend.validateBranchClaims(claims)
        return claims
    }

    private static func artifactReference(
        _ value: KagemushaRecursiveSpendArtifactReference
    ) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(uint32(value.role.rawValue))
        writer.writeField(string(value.generation))
        writer.writeField(string(value.circuitID))
        writer.writeField(string(value.artifactType))
        writer.writeField(uint64(value.sizeBytes))
        writer.writeField(value.sha256)
        return writer.data
    }

    private static func decodeArtifactReference(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendArtifactReference {
        var reader = KagemushaV2Reader(data)
        guard let role = KagemushaRecursiveSpendArtifactRole(
            rawValue: try scalarUInt32(reader.field(), field: "lineageArtifact.role")
        ) else {
            throw KagemushaRecursiveSpendError.invalidArchive("lineageArtifact.role")
        }
        let generation = try decodeString(reader.field(), field: "lineageArtifact.generation")
        let circuitID = try decodeString(reader.field(), field: "lineageArtifact.circuitID")
        let artifactType = try decodeString(reader.field(), field: "lineageArtifact.artifactType")
        let sizeBytes = try scalarUInt64(reader.field(), field: "lineageArtifact.sizeBytes")
        let sha256 = try packedFixed(reader.field(), count: 32, field: "lineageArtifact.sha256")
        try reader.finish("lineageArtifact")
        return try KagemushaRecursiveSpendArtifactReference(
            role: role,
            generation: generation,
            circuitID: circuitID,
            artifactType: artifactType,
            sizeBytes: sizeBytes,
            sha256: sha256
        )
    }

    private static func lineageWitness(
        _ value: KagemushaRecursiveSpendLineageWitness
    ) -> Data {
        var nodes = OfflineCompactNoritoWriter()
        nodes.writeUInt64LE(UInt64(value.nodes.count))
        for node in value.nodes {
            nodes.writeField(lineageNode(node))
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(nodes.data)
        writer.writeField(value.finalBundleDigest)
        return writer.data
    }

    private static func lineageNode(
        _ value: KagemushaRecursiveSpendLineageNode
    ) -> Data {
        var parents = OfflineCompactNoritoWriter()
        parents.writeUInt64LE(UInt64(value.parentBundleDigests.count))
        for parent in value.parentBundleDigests {
            parents.writeField(constVec(parent))
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(value.resultBundleDigest)
        writer.writeField(parents.data)
        writer.writeField(uint32(value.proofStepCount))
        writer.writeField(uint64(value.verifiedAtBlockHeight))
        writer.writeField(bytes(value.transitionArchive))
        return writer.data
    }

    private static func decodeLineageWitnessPayload(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendLineageWitness {
        var reader = KagemushaV2Reader(data)
        var nodeReader = KagemushaV2Reader(try reader.field())
        let count = try nodeReader.uint64()
        guard count > 0,
              count <= UInt64(KagemushaRecursiveSpend.semanticLineageMaximumNodes) else {
            throw KagemushaRecursiveSpendError.invalidArchive("lineageWitness.nodes")
        }
        var nodes: [KagemushaRecursiveSpendLineageNode] = []
        nodes.reserveCapacity(Int(count))
        for _ in 0..<count {
            nodes.append(try decodeLineageNode(nodeReader.field()))
        }
        try nodeReader.finish("lineageWitness.nodes")
        let digest = try packedFixed(reader.field(), count: 32, field: "finalBundleDigest")
        try reader.finish("lineageWitness")
        return try KagemushaRecursiveSpendLineageWitness(
            nodes: nodes,
            finalBundleDigest: digest
        )
    }

    private static func decodeLineageNode(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendLineageNode {
        var reader = KagemushaV2Reader(data)
        let resultDigest = try packedFixed(
            reader.field(),
            count: 32,
            field: "lineageNode.resultBundleDigest"
        )
        var parentReader = KagemushaV2Reader(try reader.field())
        let parentCount = try parentReader.uint64()
        guard parentCount <= 2 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "lineageNode.parentBundleDigests"
            )
        }
        var parents: [Data] = []
        parents.reserveCapacity(Int(parentCount))
        for _ in 0..<parentCount {
            let parent = try decodeConstVec(
                try parentReader.field(),
                field: "lineageNode.parentBundleDigest"
            )
            guard parent.count == 32 else {
                throw KagemushaRecursiveSpendError.invalidArchive(
                    "lineageNode.parentBundleDigest"
                )
            }
            parents.append(parent)
        }
        try parentReader.finish("lineageNode.parentBundleDigests")
        let proofStepCount = try scalarUInt32(
            reader.field(),
            field: "lineageNode.proofStepCount"
        )
        let verifiedAtBlockHeight = try scalarUInt64(
            reader.field(),
            field: "lineageNode.verifiedAtBlockHeight"
        )
        let transitionArchive = try decodeBytes(
            reader.field(),
            field: "lineageNode.transitionArchive"
        )
        try reader.finish("lineageNode")
        return try KagemushaRecursiveSpendLineageNode(
            resultBundleDigest: resultDigest,
            parentBundleDigests: parents,
            proofStepCount: proofStepCount,
            verifiedAtBlockHeight: verifiedAtBlockHeight,
            transitionArchive: transitionArchive
        )
    }

    private static func acknowledgementPayload(
        operationID: Data,
        requestDigest: Data,
        bundleDigest: Data,
        commitment: Data,
        acceptedAt: UInt64,
        deviceID: String,
        keyReference: Data,
        key: KagemushaPublicKey
    ) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(operationID)
        writer.writeField(requestDigest)
        writer.writeField(bundleDigest)
        writer.writeField(commitment)
        writer.writeField(uint64(acceptedAt))
        writer.writeField(string(deviceID))
        writer.writeField(keyReference)
        writer.writeField(publicKey(key))
        return writer.data
    }

    private static func publicKey(_ value: KagemushaPublicKey) -> Data {
        var compact = Data([value.algorithm])
        compact.append(value.payload)
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(compact.count))
        for byte in compact {
            writer.writeField(Data([byte]))
        }
        return writer.data
    }

    private static func decodePublicKey(_ data: Data) throws -> KagemushaPublicKey {
        var reader = KagemushaV2Reader(data)
        let count = try reader.uint64()
        guard count > 0, count <= 8_193 else {
            throw KagemushaRecursiveSpendError.invalidArchive("publicKey")
        }
        var compact = Data()
        compact.reserveCapacity(Int(count))
        for _ in 0..<count {
            let element = try reader.field()
            guard element.count == 1, let byte = element.first else {
                throw KagemushaRecursiveSpendError.invalidArchive("publicKey")
            }
            compact.append(byte)
        }
        try reader.finish("publicKey")
        guard let algorithm = compact.first else {
            throw KagemushaRecursiveSpendError.invalidArchive("publicKey")
        }
        return try KagemushaPublicKey(
            algorithm: algorithm,
            payload: Data(compact.dropFirst())
        )
    }

    private static func chainID(_ value: String) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(string(value))
        return writer.data
    }

    private static func decodeChainID(_ data: Data) throws -> String {
        var reader = KagemushaV2Reader(data)
        let value = try decodeString(reader.field(), field: "chainID")
        try reader.finish("chainID")
        return value
    }

    private static func assetDefinitionID(_ value: String) throws -> Data {
        guard let decoded = AssetDefinitionAddress.decode(value) else {
            throw KagemushaRecursiveSpendError.invalidField("assetDefinitionID")
        }
        return constVec(decoded)
    }

    private static func decodeAssetDefinitionID(_ data: Data) throws -> String {
        let bytes = try decodeConstVec(data, field: "assetDefinitionID")
        guard bytes.count == 16,
              let value = AssetDefinitionAddress.encode(uuidBytes: bytes) else {
            throw KagemushaRecursiveSpendError.invalidArchive("assetDefinitionID")
        }
        return value
    }

    private static func accountID(_ value: String) throws -> Data {
        do {
            let address = try AccountAddress.parseEncoded(value, expectedPrefix: 0x02F1)
            return try address.compactNoritoAccountControllerPayload()
        } catch {
            throw KagemushaRecursiveSpendError.invalidField("accountID")
        }
    }

    private static func decodeAccountID(_ data: Data) throws -> String {
        var reader = KagemushaV2Reader(data)
        let tag = try reader.uint32()
        guard tag == 0 else {
            throw KagemushaRecursiveSpendError.invalidArchive("accountID.controller")
        }
        let key = try decodePublicKey(reader.field())
        try reader.finish("accountID")
        guard let algorithm = SigningAlgorithm(noritoDiscriminant: key.algorithm) else {
            throw KagemushaRecursiveSpendError.invalidArchive("accountID.algorithm")
        }
        let address = try AccountAddress.fromAccount(
            publicKey: key.payload,
            algorithm: algorithm.wireName
        )
        return try address.toI105(networkPrefix: 0x02F1)
    }

    private static func assetID(_ value: String) throws -> Data {
        let parts = value.split(separator: "#", omittingEmptySubsequences: false)
        guard parts.count == 2 || parts.count == 3,
              let definition = AssetDefinitionAddress.decode(String(parts[0])) else {
            throw KagemushaRecursiveSpendError.invalidField("assetID")
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try accountID(String(parts[1])))
        writer.writeField(constVec(definition))
        if parts.count == 2 {
            writer.writeField(uint32(0))
        } else {
            let prefix = "dataspace:"
            let scope = String(parts[2])
            guard scope.hasPrefix(prefix),
                  let value = UInt64(scope.dropFirst(prefix.count)) else {
                throw KagemushaRecursiveSpendError.invalidField("assetID.scope")
            }
            var tagged = OfflineCompactNoritoWriter()
            tagged.writeUInt32LE(1)
            tagged.writeField(uint64(value))
            writer.writeField(tagged.data)
        }
        return writer.data
    }

    private static func decodeAssetID(_ data: Data) throws -> String {
        var reader = KagemushaV2Reader(data)
        let account = try decodeAccountID(reader.field())
        let definition = try decodeAssetDefinitionID(reader.field())
        var scope = KagemushaV2Reader(try reader.field())
        let tag = try scope.uint32()
        let dataspace: UInt64?
        switch tag {
        case 0:
            dataspace = nil
        case 1:
            dataspace = try scalarUInt64(scope.field(), field: "assetID.dataspace")
        default:
            throw KagemushaRecursiveSpendError.invalidArchive("assetID.scope")
        }
        try scope.finish("assetID.scope")
        try reader.finish("assetID")
        let base = "\(definition)#\(account)"
        return dataspace.map { "\(base)#dataspace:\($0)" } ?? base
    }

    private static func decodeFixed32Vector(_ data: Data, field: String) throws -> [Data] {
        var reader = KagemushaV2Reader(data)
        let count = try reader.uint64()
        guard count > 0, count <= 128 else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
        var values: [Data] = []
        values.reserveCapacity(Int(count))
        for _ in 0..<count {
            let value = try decodeConstVec(try reader.field(), field: field)
            guard value.count == 32 else {
                throw KagemushaRecursiveSpendError.invalidArchive(field)
            }
            values.append(value)
        }
        try reader.finish(field)
        return values
    }

    private static func sequence(_ values: [Data]) throws -> Data {
        guard values.count <= Int(UInt32.max) else {
            throw KagemushaRecursiveSpendError.invalidField("sequence")
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        for value in values {
            writer.writeField(value)
        }
        return writer.data
    }

    private static func optionalVerifierRecord(
        _ value: KagemushaRecursiveSpendVerifierRecordRef?
    ) throws -> Data {
        guard let value else { return option(nil) }
        return option(try nestedPayload(
            value.recordBytes,
            schema: KagemushaRecursiveSpend.verifyingKeyRecordWireName,
            field: "verifierRecord"
        ))
    }

    private static func decodeVerifierKeyID(_ data: Data) throws -> String {
        var reader = KagemushaV2Reader(data)
        let backend = try decodeString(reader.field(), field: "verifierKeyID.backend")
        let name = try decodeString(reader.field(), field: "verifierKeyID.name")
        try reader.finish("verifierKeyID")
        return "\(backend):\(name)"
    }

    private static func verifierKeyID(_ value: String) throws -> Data {
        guard let separator = value.firstIndex(of: ":"),
              separator != value.startIndex,
              separator != value.index(before: value.endIndex) else {
            throw KagemushaRecursiveSpendError.invalidField("verifierKeyID")
        }
        let backend = String(value[..<separator])
        let name = String(value[value.index(after: separator)...])
        try KagemushaRecursiveSpend.requirePortableText(backend, field: "verifierKeyID.backend")
        try KagemushaRecursiveSpend.requirePortableText(name, field: "verifierKeyID.name")
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(string(backend))
        writer.writeField(string(name))
        return writer.data
    }

    private static func decodeLineageMode(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendLineageMode {
        guard let value = KagemushaRecursiveSpendLineageMode(
            rawValue: try scalarUInt32(data, field: "lineageMode")
        ) else {
            throw KagemushaRecursiveSpendError.invalidArchive("lineageMode")
        }
        return value
    }

    private static func frame(_ schema: String, payload: Data) -> Data {
        noritoEncode(typeName: schema, payload: payload, flags: flags)
    }

    private static func payload(
        _ archive: Data,
        schema: String,
        field: String
    ) throws -> Data {
        try KagemushaRecursiveSpend.requireArchive(archive, schema: schema, field: field)
        guard let decoded = noritoDecodeFrame(archive), decoded.paddingLength == 0 else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
        return decoded.payload
    }

    private static func nestedPayload(
        _ archive: Data,
        schema: String,
        field: String
    ) throws -> Data {
        try payload(archive, schema: schema, field: field)
    }

    private static func string(_ value: String) -> Data {
        OfflineCompactNorito.encodeString(value)
    }

    private static func decodeString(_ data: Data, field: String) throws -> String {
        var reader = KagemushaV2Reader(data)
        let count = try reader.compactLength()
        let bytes = try reader.bytes(count)
        try reader.finish(field)
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
        return value
    }

    private static func decodeStringVector(
        _ data: Data,
        field: String,
        maximumCount: UInt64
    ) throws -> [String] {
        var reader = KagemushaV2Reader(data)
        let count = try reader.uint64()
        guard count <= maximumCount else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
        var values: [String] = []
        values.reserveCapacity(Int(count))
        for index in 0..<count {
            values.append(try decodeString(
                reader.field(),
                field: "\(field)[\(index)]"
            ))
        }
        try reader.finish(field)
        return values
    }

    private static func bytes(_ value: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(value.count))
        writer.writeBytes(value)
        return writer.data
    }

    private static func decodeBytes(_ data: Data, field: String) throws -> Data {
        var reader = KagemushaV2Reader(data)
        let count = try reader.uint64()
        guard count <= UInt64(Int.max) else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
        let value = try reader.bytes(Int(count))
        try reader.finish(field)
        return value
    }

    private static func constVec(_ value: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        for byte in value {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }

    private static func decodeConstVec(_ data: Data, field: String) throws -> Data {
        var reader = KagemushaV2Reader(data)
        var value = Data()
        while !reader.isEmpty {
            let element = try reader.field()
            guard element.count == 1, let byte = element.first else {
                throw KagemushaRecursiveSpendError.invalidArchive(field)
            }
            value.append(byte)
        }
        return value
    }

    private static func option(_ value: Data?) -> Data {
        var writer = OfflineCompactNoritoWriter()
        guard let value else {
            writer.writeUInt8(0)
            return writer.data
        }
        writer.writeUInt8(1)
        writer.writeField(value)
        return writer.data
    }

    private static func decodeOption(_ data: Data, field: String) throws -> Data? {
        var reader = KagemushaV2Reader(data)
        switch try reader.uint8() {
        case 0:
            try reader.finish(field)
            return nil
        case 1:
            let value = try reader.field()
            try reader.finish(field)
            return value
        default:
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
    }

    private static func optionalUInt64(_ value: UInt64?) -> Data {
        option(value.map(uint64))
    }

    private static func decodeOptionalUInt64(_ data: Data, field: String) throws -> UInt64? {
        try decodeOption(data, field: field).map { try scalarUInt64($0, field: field) }
    }

    private static func uint16(_ value: UInt16) -> Data {
        var value = value.littleEndian
        return withUnsafeBytes(of: &value) { Data($0) }
    }

    private static func uint32(_ value: UInt32) -> Data {
        var value = value.littleEndian
        return withUnsafeBytes(of: &value) { Data($0) }
    }

    private static func uint64(_ value: UInt64) -> Data {
        var value = value.littleEndian
        return withUnsafeBytes(of: &value) { Data($0) }
    }

    private static func scalarUInt32(_ data: Data, field: String) throws -> UInt32 {
        var reader = KagemushaV2Reader(data)
        let value = try reader.uint32()
        try reader.finish(field)
        return value
    }

    private static func scalarUInt16(_ data: Data, field: String) throws -> UInt16 {
        var reader = KagemushaV2Reader(data)
        let value = try reader.uint16()
        try reader.finish(field)
        return value
    }

    private static func scalarUInt64(_ data: Data, field: String) throws -> UInt64 {
        var reader = KagemushaV2Reader(data)
        let value = try reader.uint64()
        try reader.finish(field)
        return value
    }

    private static func decodeBool(_ data: Data, field: String) throws -> Bool {
        guard data.count == 1, let byte = data.first, byte <= 1 else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
        return byte == 1
    }

    private static func packedFixed(_ data: Data, count: Int, field: String) throws -> Data {
        guard data.count == count else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
        return data
    }

    private static func u128(_ value: String) throws -> Data {
        var digits = value.compactMap(\.wholeNumberValue)
        var output = Data()
        while !(digits.count == 1 && digits[0] == 0) {
            var quotient: [Int] = []
            var remainder = 0
            for digit in digits {
                let current = remainder * 10 + digit
                let next = current / 256
                remainder = current % 256
                if !quotient.isEmpty || next != 0 { quotient.append(next) }
            }
            output.append(UInt8(remainder))
            digits = quotient.isEmpty ? [0] : quotient
        }
        guard output.count <= 16 else {
            throw KagemushaRecursiveSpendError.invalidField("u128")
        }
        output.append(contentsOf: repeatElement(UInt8(0), count: 16 - output.count))
        return output
    }

    private static func decodeU128(_ data: Data) throws -> String {
        guard data.count == 16 else {
            throw KagemushaRecursiveSpendError.invalidArchive("u128")
        }
        var decimal = [0]
        for byte in data.reversed() {
            var carry = Int(byte)
            for index in decimal.indices.reversed() {
                let value = decimal[index] * 256 + carry
                decimal[index] = value % 10
                carry = value / 10
            }
            while carry > 0 {
                decimal.insert(carry % 10, at: 0)
                carry /= 10
            }
        }
        return decimal.map(String.init).joined()
    }
}

private struct KagemushaV2Reader {
    private let data: Data
    private(set) var offset = 0

    init(_ data: Data) {
        self.data = data
    }

    var isEmpty: Bool { offset == data.count }

    mutating func finish(_ field: String) throws {
        guard isEmpty else {
            throw KagemushaRecursiveSpendError.invalidArchive("\(field).trailing")
        }
    }

    mutating func uint8() throws -> UInt8 {
        guard offset < data.count else {
            throw KagemushaRecursiveSpendError.invalidArchive("truncated")
        }
        defer { offset += 1 }
        return data[data.startIndex + offset]
    }

    mutating func uint32() throws -> UInt32 {
        let bytes = try bytes(4)
        var value: UInt32 = 0
        bytes.withUnsafeBytes { buffer in
            if let base = buffer.baseAddress { memcpy(&value, base, 4) }
        }
        return UInt32(littleEndian: value)
    }

    mutating func uint16() throws -> UInt16 {
        let bytes = try bytes(2)
        var value: UInt16 = 0
        bytes.withUnsafeBytes { buffer in
            if let base = buffer.baseAddress { memcpy(&value, base, 2) }
        }
        return UInt16(littleEndian: value)
    }

    mutating func uint64() throws -> UInt64 {
        let bytes = try bytes(8)
        var value: UInt64 = 0
        bytes.withUnsafeBytes { buffer in
            if let base = buffer.baseAddress { memcpy(&value, base, 8) }
        }
        return UInt64(littleEndian: value)
    }

    mutating func bytes(_ count: Int) throws -> Data {
        guard count >= 0, offset + count <= data.count else {
            throw KagemushaRecursiveSpendError.invalidArchive("truncated")
        }
        let start = data.startIndex + offset
        offset += count
        return Data(data[start..<(start + count)])
    }

    mutating func field() throws -> Data {
        try bytes(length())
    }

    mutating func compactLength() throws -> Int {
        try length()
    }

    private mutating func length() throws -> Int {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        let start = offset
        for _ in 0..<10 {
            let byte = try uint8()
            let chunk = UInt64(byte & 0x7f)
            guard shift < 64, !(shift == 63 && chunk > 1) else {
                throw KagemushaRecursiveSpendError.invalidArchive("length")
            }
            value |= chunk << shift
            if byte & 0x80 == 0 {
                let width = offset - start
                if width > 1, value < UInt64(1) << UInt64(7 * (width - 1)) {
                    throw KagemushaRecursiveSpendError.invalidArchive("length.nonCanonical")
                }
                guard value <= UInt64(Int.max), value <= UInt64(data.count - offset) else {
                    throw KagemushaRecursiveSpendError.invalidArchive("length")
                }
                return Int(value)
            }
            shift += 7
        }
        throw KagemushaRecursiveSpendError.invalidArchive("length")
    }
}

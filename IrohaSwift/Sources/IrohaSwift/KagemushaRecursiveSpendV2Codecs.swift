import Foundation

public enum KagemushaRecursiveSpendCodecs {
    private static let flags = NoritoHeader.compactLen

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
            stepEqCircuitID: decodeString(
                reader.field(),
                field: "nativeCapabilities.stepEqCircuitID"
            ),
            stepEpCircuitID: decodeString(
                reader.field(),
                field: "nativeCapabilities.stepEpCircuitID"
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
        var writer = CompactNoritoWriter()
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
        writer.writeField(bytes(payload.senderOutputProverMaterial))
        return frame(
            KagemushaRecursiveSpend.recipientRequestPayloadWireName,
            payload: writer.data
        )
    }

    public static func encodeNoteOpening(_ opening: KagemushaNoteOpening) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(opening.spendKey)
        writer.writeField(opening.rho)
        writer.writeField(opening.diversifier)
        return frame(KagemushaRecursiveSpend.noteOpeningWireName, payload: writer.data)
    }

    /// Decodes the encrypted-at-rest, local-only opening for a spendable note.
    ///
    /// The decoder accepts exactly the canonical Norito representation emitted
    /// by `encodeNoteOpening`; truncated, extended, zero-valued, or otherwise
    /// non-canonical archives fail closed. Callers must never place this archive
    /// in a peer or Torii payload.
    public static func decodeNoteOpening(_ archive: Data) throws -> KagemushaNoteOpening {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.noteOpeningWireName,
            field: "noteOpening"
        ))
        let opening = try KagemushaNoteOpening(
            spendKey: packedFixed(
                reader.field(), count: 32, field: "noteOpening.spendKey"
            ),
            rho: packedFixed(reader.field(), count: 32, field: "noteOpening.rho"),
            diversifier: packedFixed(
                reader.field(), count: 32, field: "noteOpening.diversifier"
            )
        )
        try reader.finish("noteOpening")
        guard try encodeNoteOpening(opening) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("noteOpening.canonical")
        }
        return opening
    }

    /// Encodes local encrypted Merkle membership data for one owned note.
    /// This archive is native-prover input only and must never enter peer or
    /// Torii payloads.
    public static func encodeMembershipWitness(
        _ witness: KagemushaNoteMembershipWitness
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(uint32(witness.leafIndex))
        writer.writeField(try membershipPath(witness.inputPath))
        writer.writeField(try membershipPath(witness.dummyInputPath))
        return frame(
            KagemushaRecursiveSpend.membershipWitnessWireName,
            payload: writer.data
        )
    }

    /// Strictly decodes and canonically re-encodes a local membership witness.
    public static func decodeMembershipWitness(
        _ archive: Data
    ) throws -> KagemushaNoteMembershipWitness {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.membershipWitnessWireName,
            field: "membershipWitness"
        ))
        let witness = try KagemushaNoteMembershipWitness(
            leafIndex: scalarUInt32(
                reader.field(),
                field: "membershipWitness.leafIndex"
            ),
            inputPath: decodeMembershipPath(
                reader.field(),
                field: "membershipWitness.inputPath"
            ),
            dummyInputPath: decodeMembershipPath(
                reader.field(),
                field: "membershipWitness.dummyInputPath"
            )
        )
        try reader.finish("membershipWitness")
        guard try encodeMembershipWitness(witness) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "membershipWitness.canonical"
            )
        }
        return witness
    }

    public static func encodeRecipientOutputDerivationRequest(
        _ request: KagemushaRecipientOutputDerivationRequest
    ) throws -> Data {
        var writer = CompactNoritoWriter()
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
        request: KagemushaRecipientOutputDerivationRequest,
        opening: KagemushaNoteOpening
    ) throws -> KagemushaRecipientOutputDerivationResult {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.recipientOutputDerivationResultWireName,
            field: "recipientOutputDerivationResult"
        ))
        let output = try decodeNote(reader.field())
        let proverMaterial = try decodeBytes(
            reader.field(),
            field: "senderOutputProverMaterial"
        )
        try reader.finish("recipientOutputDerivationResult")
        return try KagemushaRecipientOutputDerivationResult(
            recipientOutput: output,
            senderOutputProverMaterial: proverMaterial,
            request: request,
            opening: opening
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

    public static func encodeArtifactBinding(
        _ binding: KagemushaRecursiveSpendArtifactBinding
    ) throws -> Data {
        frame(
            KagemushaRecursiveSpend.artifactBindingWireName,
            payload: artifactBinding(binding)
        )
    }

    public static func encodeInitRequest(
        _ request: KagemushaRecursiveSpendInitRequest
    ) throws -> Data {
        var writer = CompactNoritoWriter()
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
        writer.writeField(try nestedPayload(
            request.topUpFinalityRosterArtifact.noritoArchive,
            schema: KagemushaRecursiveSpend.topUpFinalityRosterArtifactWireName,
            field: "topUpFinalityRosterArtifact"
        ))
        writer.writeField(artifactBinding(request.artifactBinding))
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
        let finalityRosterArtifact = frame(
            KagemushaRecursiveSpend.topUpFinalityRosterArtifactWireName,
            payload: try reader.field()
        )
        let binding = try decodeArtifactBinding(reader.field())
        try reader.finish("initRequest")
        let anchor = try decodeTopUpAnchor(anchorArchive)
        guard anchor.artifactBinding == binding else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "initRequest.artifactBinding"
            )
        }
        let request = try KagemushaRecursiveSpendInitRequest(
            topUpAnchor: anchor,
            topUpFinalityProof: KagemushaTopUpFinalityProofArchive(
                noritoArchive: finalityProof
            ),
            topUpFinalityRosterArtifact: KagemushaTopUpFinalityRosterArtifactArchive(
                noritoArchive: finalityRosterArtifact
            )
        )
        guard try encodeInitRequest(request) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("initRequest.canonical")
        }
        return request
    }

    public static func decodeInitResult(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendInitResult {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.initResultWireName,
            field: "initResult"
        ))
        let bundleArchive = frame(
            KagemushaRecursiveSpend.bundleWireName,
            payload: try reader.field()
        )
        let statementDigest = try packedFixed(
            reader.field(),
            count: 32,
            field: "publicStatementDigest"
        )
        try reader.finish("initResult")
        return try KagemushaRecursiveSpendInitResult(
            bundle: KagemushaRecursiveSpendBundle(noritoArchive: bundleArchive),
            publicStatementDigest: statementDigest,
            archive: archive
        )
    }

    public static func encodeTopUpUnsigned(
        _ request: KagemushaRecursiveSpendTopUpUnsigned
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try assetID(request.assetID))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try note(request.currentNote))
        writer.writeField(try topUpShieldEvidence(request.shieldEvidence))
        writer.writeField(artifactBinding(request.artifactBinding))
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
        let artifactBinding = try decodeArtifactBinding(reader.field())
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
            artifactBinding: artifactBinding,
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
        var zeroPath = CompactNoritoWriter()
        zeroPath.writeField(try sequence(request.zeroPath.siblings))
        zeroPath.writeField(bytes(request.zeroPath.directions))
        zeroPath.writeField(request.zeroPath.root)

        var writer = CompactNoritoWriter()
        writer.writeField(chainID(request.chainID))
        writer.writeField(try assetID(request.assetID))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try accountID(request.payer))
        writer.writeField(request.operationID)
        writer.writeField(try nestedPayload(
            request.opening.noritoEncoded(),
            schema: KagemushaRecursiveSpend.noteOpeningWireName,
            field: "opening"
        ))
        writer.writeField(uint32(request.leafIndex))
        writer.writeField(zeroPath.data)
        writer.writeField(try verifierKeyID(request.shieldVerifierID))
        writer.writeField(request.shieldVerifierCommitment)
        writer.writeField(artifactBinding(request.artifactBinding))
        return frame(
            KagemushaRecursiveSpend.topUpShieldBuildRequestWireName,
            payload: writer.data
        )
    }

    public static func encodeTopUpRequest(
        _ request: KagemushaRecursiveSpendTopUpRequest
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try assetID(request.assetID))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try note(request.currentNote))
        writer.writeField(try topUpShieldEvidence(request.shieldEvidence))
        writer.writeField(artifactBinding(request.artifactBinding))
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
        var writer = CompactNoritoWriter()
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
        writer.writeField(artifactBinding(anchor.artifactBinding))
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
        let artifactBinding = try decodeArtifactBinding(reader.field())
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
            artifactBinding: artifactBinding,
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
        var writer = CompactNoritoWriter()
        writer.writeField(try sequence(bundles))
        writer.writeField(artifactBinding(request.outputArtifactBinding))
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

    public static func decodeRedemptionIntent(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendRedemptionIntent {
        try decodeRedemptionIntentPayload(try payload(
            archive,
            schema: KagemushaRecursiveSpend.redemptionIntentWireName,
            field: "redemptionIntent"
        ))
    }

    public static func encodeAppendLocalRequest(
        _ request: KagemushaRecursiveSpendAppendRequest
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try sequence(request.previousInputs.map(appendInput)))
        writer.writeField(try sequence(request.inputOpenings.map {
            try nestedPayload(
                encodeNoteOpening($0),
                schema: KagemushaRecursiveSpend.noteOpeningWireName,
                field: "inputOpening"
            )
        }))
        writer.writeField(try sequence(request.inputMembershipWitnesses.map {
            try nestedPayload(
                encodeMembershipWitness($0),
                schema: KagemushaRecursiveSpend.membershipWitnessWireName,
                field: "inputMembershipWitness"
            )
        }))
        writer.writeField(option(try request.changeOpening.map {
            try nestedPayload(
                encodeNoteOpening($0),
                schema: KagemushaRecursiveSpend.noteOpeningWireName,
                field: "changeOpening"
            )
        }))
        writer.writeField(artifactBinding(request.outputArtifactBinding))
        writer.writeField(try verifierKeyID(request.transferVerifier.identifier))
        writer.writeField(request.transferVerifier.commitment)
        writer.writeField(request.operationID)
        writer.writeField(uint64(request.blockHeight))
        return frame(
            KagemushaRecursiveSpend.appendLocalRequestWireName,
            payload: writer.data
        )
    }

    public static func encodeVerifyRequest(
        _ request: KagemushaRecursiveSpendVerifyRequest
    ) throws -> Data {
        var writer = CompactNoritoWriter()
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
        writer.writeField(artifactBinding(request.artifactBinding))
        writer.writeField(uint64(request.blockHeight))
        writer.writeField(uint64(request.verifiedAtMilliseconds))
        return frame(KagemushaRecursiveSpend.verifyRequestWireName, payload: writer.data)
    }

    public static func encodeRedeemRequest(
        _ request: KagemushaRecursiveSpendRedeemRequest
    ) throws -> Data {
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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
        writer.writeField(option(try request.offlineChange.map(redeemChangeBranch)))
        writer.writeField(uint64(request.blockHeight))
        writer.writeField(request.operationID)
        return frame(KagemushaRecursiveSpend.redeemUnsignedWireName, payload: writer.data)
    }

    public static func decodeRedeemUnsigned(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendRedeemUnsigned {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.redeemUnsignedWireName,
            field: "redeemUnsigned"
        ))
        let bundleArchive = frame(
            KagemushaRecursiveSpend.bundleWireName,
            payload: try reader.field()
        )
        let recipient = try decodeAccountID(reader.field())
        let amount = try decodeScaledAmount(reader.field())
        let redeemProof = frame(
            KagemushaRecursiveSpend.proofAttachmentWireName,
            payload: try reader.field()
        )
        let redemption = try decodeRedemptionIntentPayload(reader.field())
        let change = try decodeOption(
            reader.field(),
            field: "offlineChange"
        ).map(decodeRedeemChangeBranch)
        let blockHeight = try scalarUInt64(reader.field(), field: "blockHeight")
        let operationID = try packedFixed(reader.field(), count: 32, field: "operationID")
        try reader.finish("redeemUnsigned")
        let value = try KagemushaRecursiveSpendRedeemUnsigned(
            bundle: KagemushaRecursiveSpendBundle(noritoArchive: bundleArchive),
            recipient: recipient,
            amount: amount,
            redeemProof: redeemProof,
            redemption: redemption,
            offlineChange: change,
            blockHeight: blockHeight,
            operationID: operationID
        )
        guard try encodeRedeemUnsigned(value) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("redeemUnsigned.canonical")
        }
        return value
    }

    public static func encodeRedeemLocalRequest(
        _ request: KagemushaRecursiveSpendRedeemBuildRequest
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.bundle.archive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "bundle"
        ))
        writer.writeField(try nestedPayload(
            encodeNoteOpening(request.inputOpening),
            schema: KagemushaRecursiveSpend.noteOpeningWireName,
            field: "inputOpening"
        ))
        writer.writeField(try nestedPayload(
            encodeMembershipWitness(request.inputMembershipWitness),
            schema: KagemushaRecursiveSpend.membershipWitnessWireName,
            field: "inputMembershipWitness"
        ))
        writer.writeField(try accountID(request.recipient))
        writer.writeField(try scaledAmount(request.publicAmount))
        writer.writeField(option(try request.changeOpening.map {
            try nestedPayload(
                encodeNoteOpening($0),
                schema: KagemushaRecursiveSpend.noteOpeningWireName,
                field: "changeOpening"
            )
        }))
        writer.writeField(try verifierKeyID(request.unshieldVerifier.identifier))
        writer.writeField(request.unshieldVerifier.commitment)
        writer.writeField(uint64(request.blockHeight))
        writer.writeField(request.operationID)
        return frame(
            KagemushaRecursiveSpend.redeemLocalRequestWireName,
            payload: writer.data
        )
    }

    public static func decodeRedeemBuildResult(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendRedeemBuildResult {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.redeemBuildResultWireName,
            field: "redeemBuildResult"
        ))
        let unsignedArchive = frame(
            KagemushaRecursiveSpend.redeemUnsignedWireName,
            payload: try reader.field()
        )
        let authorizationDigest = try packedFixed(
            reader.field(), count: 32, field: "authorizationDigest"
        )
        let changePayload = try decodeOption(
            reader.field(),
            field: "offlineChangeBundle"
        )
        let operationID = try packedFixed(reader.field(), count: 32, field: "operationID")
        try reader.finish("redeemBuildResult")
        let unsigned = try decodeRedeemUnsigned(unsignedArchive)
        let change = try changePayload.map {
            try KagemushaRecursiveSpendBundle(
                noritoArchive: frame(KagemushaRecursiveSpend.bundleWireName, payload: $0)
            )
        }
        return try KagemushaRecursiveSpendRedeemBuildResult(
            unsigned: unsigned,
            authorizationDigest: authorizationDigest,
            offlineChangeBundle: change,
            operationID: operationID
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
        let artifactBinding = try decodeArtifactBinding(reader.field())
        let verifierKeyID = try decodeVerifierKeyID(reader.field())
        let digest = try packedFixed(reader.field(), count: 32, field: "bundleDigest")
        try reader.finish("bundleSummary")
        return KagemushaRecursiveSpendBundleSummary(
            assetDefinitionID: asset,
            amount: amount,
            noteCommitment: commitment,
            spendNullifier: nullifier,
            hopCount: hopCount,
            branchClaims: branchClaims,
            artifactBinding: artifactBinding,
            verifierKeyID: verifierKeyID,
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
        var writer = CompactNoritoWriter()
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
        // The authenticated artifact binding and native-selected verifier id
        // follow the producing transition.
        for _ in 0..<2 {
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
        let stateRedeemable = try decodeBool(reader.field(), field: "stateRedeemable")
        let witnessless = try decodeBool(reader.field(), field: "witnesslessRedemptionSupported")
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
        try reader.finish("verifyResult")
        let summary = try decodeBundleSummary(summaryArchive)
        guard valid else {
            throw KagemushaRecursiveSpendError.invalidArchive("verifyResult.valid")
        }
        // Mirror Rust `KagemushaRecursiveSpendVerifyResultV2::validate_public_binding`
        // before wallet code can persist a native result.
        guard chainAdmissible,
              stateRedeemable,
              witnessless,
              requestDigest.contains(where: { $0 != 0 }),
              bindingDigest.contains(where: { $0 != 0 }),
              circuitID == KagemushaRecursiveSpend.stateEpCircuitID,
              blockHeight > 0,
              verifiedAt > 0,
              summary.verifierKeyID == verifierKeyID else {
            throw KagemushaRecursiveSpendError.invalidArchive("verifyResult.binding")
        }
        return KagemushaRecursiveSpendVerifyResult(
            valid: valid,
            chainAdmissible: chainAdmissible,
            stateRedeemable: stateRedeemable,
            witnesslessRedemptionSupported: witnessless,
            summary: summary,
            recipientRequestDigest: requestDigest,
            requestOutputBindingDigest: bindingDigest,
            verifierKeyID: verifierKeyID,
            verifierCircuitID: circuitID,
            verifierActivationHeight: activation,
            verifierWithdrawHeight: withdrawal,
            verifiedAtBlockHeight: blockHeight,
            verifiedAtMilliseconds: verifiedAt
        )
    }

    public static func decodeRedeemResult(
        _ archive: Data,
        unsigned: KagemushaRecursiveSpendRedeemUnsigned,
        authorization: KagemushaRequestAuthorization
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
        let request = try KagemushaRecursiveSpendRedeemRequest(
            unsigned: unsigned,
            authorization: authorization,
            archive: requestArchive
        )
        return try KagemushaRecursiveSpendRedeemResult(
            request: request,
            offlineChangeBundle: change,
            operationID: operationID
        )
    }

    private static func encodeRecipientRequest(
        _ payload: KagemushaRecipientPaymentRequestSigningPayload,
        signature: Data
    ) throws -> Data {
        let payloadArchive = try encodeRecipientRequestPayload(payload)
        var writer = CompactNoritoWriter()
        writer.writeField(try nestedPayload(
            payloadArchive,
            schema: KagemushaRecursiveSpend.recipientRequestPayloadWireName,
            field: "recipientRequestPayload"
        ))
        // The signed request flattens payload fields rather than nesting the
        // signing-payload type, so strip the field wrapper just constructed.
        var flattened = KagemushaV2Reader(writer.data)
        let payloadFields = try flattened.field()
        var result = CompactNoritoWriter()
        result.writeBytes(payloadFields)
        result.writeField(constVec(signature))
        return frame(KagemushaRecursiveSpend.recipientRequestWireName, payload: result.data)
    }

    private static func encodeAuthorization(
        _ fields: KagemushaRequestAuthorizationFields,
        signature: Data
    ) throws -> Data {
        var writer = CompactNoritoWriter()
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
        let material = try decodeBytes(reader.field(), field: "senderOutputProverMaterial")
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
            senderOutputProverMaterial: material
        )
    }

    private static func topUpAnchorRef(
        _ value: KagemushaRecursiveSpendTopUpAnchorRef
    ) throws -> Data {
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
        writer.writeField(chainID(value.chainID))
        writer.writeField(try assetDefinitionID(value.assetDefinitionID))
        writer.writeField(try sequence(value.inputs.map(inputBranch)))
        writer.writeField(try sequence(value.topUpAnchorRefs.map(topUpAnchorRef)))
        writer.writeField(uint32(value.assetScale))
        writer.writeField(artifactBinding(value.outputArtifactBinding))
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
        let outputArtifactBinding = try decodeArtifactBinding(reader.field())
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
            outputArtifactBinding: outputArtifactBinding,
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
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
        writer.writeField(try nestedPayload(
            value.previousBundle.archive,
            schema: KagemushaRecursiveSpend.bundleWireName,
            field: "previousInput.previousBundle"
        ))
        return writer.data
    }

    private static func unshieldBinding(
        _ value: KagemushaUnshieldPublicInputsBinding
    ) -> Data {
        func pair(_ values: [Data]) -> Data {
            var writer = CompactNoritoWriter()
            for value in values { writer.writeField(constVec(value)) }
            return writer.data
        }
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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
        writer.writeField(option(value.changeArtifactBinding.map(artifactBinding)))
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
        let changeArtifactBinding = try decodeOption(
            reader.field(),
            field: "changeArtifactBinding"
        ).map(decodeArtifactBinding)
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
            changeArtifactBinding: changeArtifactBinding,
            unshieldPublicInputs: inputs,
            unshieldPublicInputsDigest: inputsDigest,
            operationID: operationID
        )
    }

    private static func redeemChangeBranch(
        _ value: KagemushaRecursiveSpendRedeemChangeBranch
    ) throws -> Data {
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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

    private static func artifactBinding(
        _ value: KagemushaRecursiveSpendArtifactBinding
    ) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(string(value.generation))
        writer.writeField(value.manifestSHA256)
        return writer.data
    }

    private static func decodeArtifactBinding(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendArtifactBinding {
        var reader = KagemushaV2Reader(data)
        let generation = try decodeString(
            reader.field(),
            field: "artifactBinding.generation"
        )
        let manifestSHA256 = try packedFixed(
            reader.field(),
            count: 32,
            field: "artifactBinding.manifestSHA256"
        )
        try reader.finish("artifactBinding")
        return try KagemushaRecursiveSpendArtifactBinding(
            generation: generation,
            manifestSHA256: manifestSHA256
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
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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
            var tagged = CompactNoritoWriter()
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
            let value = try reader.field()
            guard value.count == 32 else {
                throw KagemushaRecursiveSpendError.invalidArchive(field)
            }
            values.append(value)
        }
        try reader.finish(field)
        return values
    }

    private static func membershipPath(
        _ path: PrivacyConfidentialMerklePathWitnessV2
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try sequence(path.siblings))
        writer.writeField(bytes(path.directions))
        writer.writeField(path.root)
        return writer.data
    }

    private static func decodeMembershipPath(
        _ data: Data,
        field: String
    ) throws -> PrivacyConfidentialMerklePathWitnessV2 {
        var reader = KagemushaV2Reader(data)
        let siblings = try decodeFixed32Vector(
            reader.field(),
            field: "\(field).siblings"
        )
        let directions = try decodeBytes(
            reader.field(),
            field: "\(field).directions"
        )
        let root = try packedFixed(
            reader.field(),
            count: 32,
            field: "\(field).root"
        )
        try reader.finish(field)
        do {
            return try PrivacyConfidentialMerklePathWitnessV2(
                siblings: siblings,
                directions: directions,
                root: root
            )
        } catch {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
    }

    private static func sequence(_ values: [Data]) throws -> Data {
        guard values.count <= Int(UInt32.max) else {
            throw KagemushaRecursiveSpendError.invalidField("sequence")
        }
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        for value in values {
            writer.writeField(value)
        }
        return writer.data
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
        var writer = CompactNoritoWriter()
        writer.writeField(string(backend))
        writer.writeField(string(name))
        return writer.data
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
        CompactNorito.encodeString(value)
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
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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
        var writer = CompactNoritoWriter()
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

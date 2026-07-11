import Foundation

public enum KagemushaRecursiveSpendV2Codecs {
    private static let flags = NoritoHeader.compactLen

    public static func encodeRecipientRequestPayload(
        _ payload: KagemushaRecipientPaymentRequestSigningPayloadV2
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
            KagemushaRecursiveSpendV2.recipientRequestPayloadWireName,
            payload: writer.data
        )
    }

    public static func decodeRecipientRequest(
        _ archive: Data
    ) throws -> KagemushaRecipientPaymentRequestV2 {
        let payloadData = try payload(
            archive,
            schema: KagemushaRecursiveSpendV2.recipientRequestWireName,
            field: "recipientRequest"
        )
        var reader = KagemushaV2Reader(payloadData)
        let signedPayload = try decodeRecipientRequestPayloadFields(&reader)
        let signature = try decodeConstVec(try reader.field(), field: "recipientRequest.signature")
        try reader.finish("recipientRequest")
        let canonical = try encodeRecipientRequest(signedPayload, signature: signature)
        guard canonical == archive,
              archive.count <= KagemushaRecursiveSpendV2.maximumPeerArchiveBytes else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("recipientRequest.canonical")
        }
        return try KagemushaRecipientPaymentRequestV2(
            payload: signedPayload,
            signature: signature,
            archive: archive
        )
    }

    public static func decodeAndVerifyRecipientRequest(
        _ archive: Data,
        atMilliseconds: UInt64
    ) throws -> KagemushaVerifiedRecipientPaymentRequestV2 {
        try decodeRecipientRequest(archive).verified(atMilliseconds: atMilliseconds)
    }

    public static func encodeAuthorizationTemplate(
        _ fields: KagemushaRequestAuthorizationFieldsV2
    ) throws -> Data {
        try encodeAuthorization(fields, signature: Data([1]))
    }

    public static func encodeArtifactReference(
        _ reference: KagemushaRecursiveSpendArtifactReferenceV2
    ) throws -> Data {
        frame(
            KagemushaRecursiveSpendV2.artifactReferenceWireName,
            payload: artifactReference(reference)
        )
    }

    public static func encodeInitRequest(
        _ request: KagemushaRecursiveSpendInitRequestV2
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            KagemushaRecursiveSpendRequestCodecs.encodeInitRequest(request.initRequest),
            schema: KagemushaRecursiveSpendRequestCodecs.initRequestWireName,
            field: "initRequest"
        ))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try note(request.currentNote))
        writer.writeField(artifactReference(request.lineageArtifact))
        writer.writeField(request.operationID)
        return frame(KagemushaRecursiveSpendV2.initRequestWireName, payload: writer.data)
    }

    public static func encodeTopUpRequest(
        _ request: KagemushaRecursiveSpendTopUpRequestV2
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try assetID(request.assetID))
        writer.writeField(try nestedPayload(
            encodeInitRequest(request.initRequest),
            schema: KagemushaRecursiveSpendV2.initRequestWireName,
            field: "initRequest"
        ))
        writer.writeField(try nestedPayload(
            request.authorization.archive,
            schema: KagemushaRecursiveSpendV2.authorizationWireName,
            field: "authorization"
        ))
        return frame(KagemushaRecursiveSpendV2.topUpRequestWireName, payload: writer.data)
    }

    public static func decodeTopUpAnchor(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendTopUpAnchorV2 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpendV2.topUpAnchorWireName,
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
        let anchors = try decodeFixed32Vector(reader.field(), field: "topUpAnchorNullifiers")
        let currentNote = try decodeNote(reader.field())
        let operationID = try packedFixed(reader.field(), count: 32, field: "topUpOperationID")
        let verifierID = try decodeVerifierKeyID(reader.field())
        let verifierCommitment = try packedFixed(
            reader.field(), count: 32, field: "transferVerifierCommitment"
        )
        let generation = try decodeString(reader.field(), field: "artifactGeneration")
        let height = try scalarUInt64(reader.field(), field: "finalizedHeight")
        let transactionHash = try packedFixed(
            reader.field(), count: 32, field: "finalizedTransactionHash"
        )
        let digest = try packedFixed(reader.field(), count: 32, field: "anchorDigest")
        try reader.finish("topUpAnchor")
        return try KagemushaRecursiveSpendTopUpAnchorV2(
            version: version,
            chainID: chain,
            payer: payer,
            assetID: asset,
            assetScale: scale,
            amount: amount,
            initialRoot: initialRoot,
            finalizedRoot: finalizedRoot,
            topUpAnchorNullifiers: anchors,
            currentNote: currentNote,
            topUpOperationID: operationID,
            transferVerifierID: verifierID,
            transferVerifierCommitment: verifierCommitment,
            artifactGeneration: generation,
            finalizedHeight: height,
            finalizedTransactionHash: transactionHash,
            anchorDigest: digest,
            archive: archive
        )
    }

    public static func encodeAppendRequest(
        _ request: KagemushaRecursiveSpendAppendRequestV2
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.previousBundle.archive,
            schema: KagemushaRecursiveSpendV2.bundleWireName,
            field: "previousBundle"
        ))
        writer.writeField(try nestedPayload(
            request.recordBundle,
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
            field: "recordBundle"
        ))
        writer.writeField(bytes(request.pallasOpenEnvelopesArchive))
        writer.writeField(try split(request.split))
        writer.writeField(artifactReference(request.lineageArtifact))
        writer.writeField(string(request.outputProofCircuitID))
        writer.writeField(try optionalVerifierRecord(request.previousLineageVerifierRecord))
        writer.writeField(bytes(request.previousProofOpenEnvelopesArchive))
        // Streamed V2 artifacts replace both legacy in-request key blobs.
        writer.writeField(option(nil))
        writer.writeField(option(nil))
        writer.writeField(optionalUInt64(request.blockHeight))
        return frame(KagemushaRecursiveSpendV2.appendRequestWireName, payload: writer.data)
    }

    public static func encodeVerifyRequest(
        _ request: KagemushaRecursiveSpendVerifyRequestV2
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.bundle.archive,
            schema: KagemushaRecursiveSpendV2.bundleWireName,
            field: "bundle"
        ))
        writer.writeField(try nestedPayload(
            request.recipientRequest.archive,
            schema: KagemushaRecursiveSpendV2.recipientRequestWireName,
            field: "recipientRequest"
        ))
        writer.writeField(uint32(request.maximumHops))
        writer.writeField(string(request.artifactGeneration))
        writer.writeField(uint64(request.verifiedAtMilliseconds))
        writer.writeField(try optionalVerifierRecord(request.lineageVerifierRecord))
        writer.writeField(optionalUInt64(request.blockHeight))
        return frame(KagemushaRecursiveSpendV2.verifyRequestWireName, payload: writer.data)
    }

    public static func encodeLineageWitness(
        _ witness: KagemushaRecursiveSpendLineageWitnessV2
    ) throws -> Data {
        frame(
            KagemushaRecursiveSpendV2.lineageWitnessWireName,
            payload: lineageWitness(witness)
        )
    }

    public static func encodeRedeemRequest(
        _ request: KagemushaRecursiveSpendRedeemRequestV2
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.bundle.archive,
            schema: KagemushaRecursiveSpendV2.bundleWireName,
            field: "bundle"
        ))
        writer.writeField(try accountID(request.recipient))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try nestedPayload(
            request.redeemProof,
            schema: KagemushaRecursiveSpendRequestCodecs.proofAttachmentWireName,
            field: "redeemProof"
        ))
        writer.writeField(try redemptionIntent(request.redemption))
        writer.writeField(option(request.lineageWitness.map(lineageWitness)))
        writer.writeField(try nestedPayload(
            request.lineageVerifierRecord.recordBytes,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            field: "lineageVerifierRecord"
        ))
        writer.writeField(option(try request.offlineChange.map(redeemChangeBranch)))
        writer.writeField(uint64(request.blockHeight))
        writer.writeField(request.operationID)
        writer.writeField(try nestedPayload(
            request.authorization.archive,
            schema: KagemushaRecursiveSpendV2.authorizationWireName,
            field: "authorization"
        ))
        return frame(KagemushaRecursiveSpendV2.redeemRequestWireName, payload: writer.data)
    }

    public static func encodeRedeemChangeBuildRequest(
        _ request: KagemushaRecursiveSpendRedeemChangeBuildRequestV2
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.previousBundle.archive,
            schema: KagemushaRecursiveSpendV2.bundleWireName,
            field: "previousBundle"
        ))
        writer.writeField(bytes(request.previousRecursiveProofOpenEnvelopesArchive))
        writer.writeField(try nestedPayload(
            request.unshieldRecordBundle,
            schema: KagemushaRecursiveSpendRequestCodecs.recordBundleWireName,
            field: "unshieldRecordBundle"
        ))
        writer.writeField(bytes(request.pallasOpenEnvelopesArchive))
        writer.writeField(try redemptionIntent(request.redemption))
        writer.writeField(artifactReference(request.lineageArtifact))
        writer.writeField(try nestedPayload(
            request.previousLineageVerifierRecord.recordBytes,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
            field: "previousLineageVerifierRecord"
        ))
        writer.writeField(uint64(request.blockHeight))
        return frame(
            KagemushaRecursiveSpendV2.redeemChangeBuildRequestWireName,
            payload: writer.data
        )
    }

    public static func decodeRedeemChangeBuildResult(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendRedeemChangeBuildResultV2 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpendV2.redeemChangeBuildResultWireName,
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
        return KagemushaRecursiveSpendRedeemChangeBuildResultV2(
            changeBranch: branch,
            transitionBindingDigest: transitionDigest,
            publicStatementDigest: statementDigest
        )
    }

    public static func decodeBundleSummary(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendBundleSummaryV2 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpendV2.bundleSummaryWireName,
            field: "bundleSummary"
        ))
        let asset = try decodeAssetDefinitionID(reader.field())
        let amount = try decodeScaledAmount(reader.field())
        let commitment = try packedFixed(reader.field(), count: 32, field: "noteCommitment")
        let nullifier = try packedFixed(reader.field(), count: 32, field: "spendNullifier")
        let hopCount = try scalarUInt32(reader.field(), field: "hopCount")
        let branchPath = try decodeBranchPath(reader.field())
        let generation = try decodeString(reader.field(), field: "artifactGeneration")
        let verifierKeyID = try decodeVerifierKeyID(reader.field())
        let lineageMode = try decodeLineageMode(reader.field())
        let digest = try packedFixed(reader.field(), count: 32, field: "bundleDigest")
        try reader.finish("bundleSummary")
        return KagemushaRecursiveSpendBundleSummaryV2(
            assetDefinitionID: asset,
            amount: amount,
            noteCommitment: commitment,
            spendNullifier: nullifier,
            hopCount: hopCount,
            branchPath: branchPath,
            artifactGeneration: generation,
            verifierKeyID: verifierKeyID,
            lineageMode: lineageMode,
            bundleDigest: digest
        )
    }

    public static func decodeSplitResult(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendSplitResultV2 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpendV2.splitResultWireName,
            field: "splitResult"
        ))
        let split = try decodeSplit(reader.field())
        let binding = try packedFixed(reader.field(), count: 32, field: "splitBindingDigest")
        let recipientArchive = frame(
            KagemushaRecursiveSpendV2.bundleWireName,
            payload: try reader.field()
        )
        let changePayload = try decodeOption(reader.field(), field: "changeBundle")
        try reader.finish("splitResult")
        let recipient = try KagemushaRecursiveSpendBundleV2(noritoArchive: recipientArchive)
        let change = try changePayload.map {
            try KagemushaRecursiveSpendBundleV2(
                noritoArchive: frame(KagemushaRecursiveSpendV2.bundleWireName, payload: $0)
            )
        }
        return try KagemushaRecursiveSpendSplitResultV2(
            split: split,
            splitBindingDigest: binding,
            recipientBundle: recipient,
            changeBundle: change
        )
    }

    public static func decodeAcknowledgementPayload(
        _ archive: Data
    ) throws -> KagemushaReceiverAcknowledgementPayloadV2 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpendV2.acknowledgementPayloadWireName,
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
            KagemushaRecursiveSpendV2.acknowledgementPayloadWireName,
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive("acknowledgementPayload.canonical")
        }
        return try KagemushaReceiverAcknowledgementPayloadV2(
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
    ) throws -> KagemushaReceiverAcknowledgementV2 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpendV2.acknowledgementWireName,
            field: "acknowledgement"
        ))
        let payloadArchive = frame(
            KagemushaRecursiveSpendV2.acknowledgementPayloadWireName,
            payload: try reader.field()
        )
        let signature = try decodeConstVec(try reader.field(), field: "acknowledgement.signature")
        try reader.finish("acknowledgement")
        guard archive.count <= KagemushaRecursiveSpendV2.maximumPeerArchiveBytes else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("acknowledgement.size")
        }
        return try KagemushaReceiverAcknowledgementV2(
            payload: decodeAcknowledgementPayload(payloadArchive),
            signature: signature,
            archive: archive
        )
    }

    public static func decodeAcknowledgementVerifyResult(
        _ archive: Data
    ) throws -> KagemushaReceiverAcknowledgementVerifyResultV2 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpendV2.acknowledgementVerifyResultWireName,
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive("acknowledgementVerifyResult.valid")
        }
        return KagemushaReceiverAcknowledgementVerifyResultV2(
            valid: valid,
            operationID: operation,
            recipientRequestDigest: request,
            paymentBundleDigest: bundle,
            acknowledgementDigest: acknowledgement
        )
    }

    public static func decodeVerifyResult(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendVerifyResultV2 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpendV2.verifyResultWireName,
            field: "verifyResult"
        ))
        let valid = try decodeBool(reader.field(), field: "valid")
        let chainAdmissible = try decodeBool(reader.field(), field: "chainAdmissible")
        let lineageRedeemable = try decodeBool(reader.field(), field: "lineageRedeemable")
        let witnessless = try decodeBool(reader.field(), field: "witnesslessRedemptionSupported")
        let lineageMode = try decodeLineageMode(reader.field())
        let summaryArchive = frame(
            KagemushaRecursiveSpendV2.bundleSummaryWireName,
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive("verifyResult.valid")
        }
        return KagemushaRecursiveSpendVerifyResultV2(
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
    ) throws -> KagemushaRecursiveSpendRedeemResultV2 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpendV2.redeemResultWireName,
            field: "redeemResult"
        ))
        let requestArchive = try decodeBytes(reader.field(), field: "redeemRequestArchive")
        let changePayload = try decodeOption(reader.field(), field: "offlineChangeBundle")
        let operationID = try packedFixed(reader.field(), count: 32, field: "operationID")
        try reader.finish("redeemResult")
        let change = try changePayload.map {
            try KagemushaRecursiveSpendBundleV2(
                noritoArchive: frame(KagemushaRecursiveSpendV2.bundleWireName, payload: $0)
            )
        }
        return KagemushaRecursiveSpendRedeemResultV2(
            redeemRequestArchive: requestArchive,
            offlineChangeBundle: change,
            operationID: operationID
        )
    }

    private static func encodeRecipientRequest(
        _ payload: KagemushaRecipientPaymentRequestSigningPayloadV2,
        signature: Data
    ) throws -> Data {
        let payloadArchive = try encodeRecipientRequestPayload(payload)
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try nestedPayload(
            payloadArchive,
            schema: KagemushaRecursiveSpendV2.recipientRequestPayloadWireName,
            field: "recipientRequestPayload"
        ))
        // The signed request flattens payload fields rather than nesting the
        // signing-payload type, so strip the field wrapper just constructed.
        var flattened = KagemushaV2Reader(writer.data)
        let payloadFields = try flattened.field()
        var result = OfflineCompactNoritoWriter()
        result.writeBytes(payloadFields)
        result.writeField(constVec(signature))
        return frame(KagemushaRecursiveSpendV2.recipientRequestWireName, payload: result.data)
    }

    private static func encodeAuthorization(
        _ fields: KagemushaRequestAuthorizationFieldsV2,
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
        return frame(KagemushaRecursiveSpendV2.authorizationWireName, payload: writer.data)
    }

    private static func decodeRecipientRequestPayloadFields(
        _ reader: inout KagemushaV2Reader
    ) throws -> KagemushaRecipientPaymentRequestSigningPayloadV2 {
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
        return try KagemushaRecipientPaymentRequestSigningPayloadV2(
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

    private static func split(_ value: KagemushaRecursiveSpendSplitIntentV2) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(chainID(value.chainID))
        writer.writeField(try assetDefinitionID(value.assetDefinitionID))
        writer.writeField(try note(value.inputNote))
        writer.writeField(branchPath(value.parentBranchPath))
        writer.writeField(uint32(value.assetScale))
        writer.writeField(try scaledAmount(value.transferAmount))
        writer.writeField(try note(value.recipientOutput))
        writer.writeField(option(try value.changeOutput.map(note)))
        writer.writeField(value.recipientRequestDigest)
        writer.writeField(value.parentLineageDigest)
        writer.writeField(value.operationID)
        return writer.data
    }

    private static func decodeSplit(_ data: Data) throws -> KagemushaRecursiveSpendSplitIntentV2 {
        var reader = KagemushaV2Reader(data)
        let chain = try decodeChainID(reader.field())
        let asset = try decodeAssetDefinitionID(reader.field())
        let input = try decodeNote(reader.field())
        let path = try decodeBranchPath(reader.field())
        let scale = try scalarUInt32(reader.field(), field: "assetScale")
        let amount = try decodeScaledAmount(reader.field())
        let recipient = try decodeNote(reader.field())
        let change = try decodeOption(reader.field(), field: "changeOutput").map(decodeNote)
        let requestDigest = try packedFixed(
            reader.field(), count: 32, field: "recipientRequestDigest"
        )
        let parentDigest = try packedFixed(
            reader.field(), count: 32, field: "parentLineageDigest"
        )
        let operation = try packedFixed(reader.field(), count: 32, field: "operationID")
        try reader.finish("split")
        return try KagemushaRecursiveSpendSplitIntentV2(
            chainID: chain,
            assetDefinitionID: asset,
            inputNote: input,
            parentBranchPath: path,
            assetScale: scale,
            transferAmount: amount,
            recipientOutput: recipient,
            changeOutput: change,
            recipientRequestDigest: requestDigest,
            parentLineageDigest: parentDigest,
            operationID: operation
        )
    }

    private static func unshieldBinding(
        _ value: KagemushaUnshieldPublicInputsBindingV2
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
    ) throws -> KagemushaUnshieldPublicInputsBindingV2 {
        func pair(_ data: Data, field: String) throws -> [Data] {
            var reader = KagemushaV2Reader(data)
            var values: [Data] = []
            for _ in 0..<2 {
                let value = try decodeConstVec(try reader.field(), field: field)
                guard value.count == 32 else {
                    throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
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
        return try KagemushaUnshieldPublicInputsBindingV2(
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
        _ value: KagemushaRecursiveSpendRedemptionIntentV2
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(chainID(value.chainID))
        writer.writeField(try assetDefinitionID(value.assetDefinitionID))
        writer.writeField(try note(value.inputNote))
        writer.writeField(branchPath(value.parentBranchPath))
        writer.writeField(value.parentBundleDigest)
        writer.writeField(value.inputRoot)
        writer.writeField(try accountID(value.recipient))
        writer.writeField(try scaledAmount(value.publicAmount))
        writer.writeField(option(try value.changeOutput.map(note)))
        writer.writeField(unshieldBinding(value.unshieldPublicInputs))
        writer.writeField(value.unshieldPublicInputsDigest)
        writer.writeField(value.operationID)
        return writer.data
    }

    private static func decodeRedemptionIntent(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendRedemptionIntentV2 {
        var reader = KagemushaV2Reader(data)
        let chain = try decodeChainID(reader.field())
        let asset = try decodeAssetDefinitionID(reader.field())
        let input = try decodeNote(reader.field())
        let path = try decodeBranchPath(reader.field())
        let parentDigest = try packedFixed(
            reader.field(), count: 32, field: "parentBundleDigest"
        )
        let root = try packedFixed(reader.field(), count: 32, field: "inputRoot")
        let recipient = try decodeAccountID(reader.field())
        let amount = try decodeScaledAmount(reader.field())
        let change = try decodeOption(reader.field(), field: "changeOutput").map(decodeNote)
        let inputs = try decodeUnshieldBinding(reader.field())
        let inputsDigest = try packedFixed(
            reader.field(), count: 32, field: "unshieldPublicInputsDigest"
        )
        let operationID = try packedFixed(reader.field(), count: 32, field: "operationID")
        try reader.finish("redemptionIntent")
        return KagemushaRecursiveSpendRedemptionIntentV2(
            chainID: chain,
            assetDefinitionID: asset,
            inputNote: input,
            parentBranchPath: path,
            parentBundleDigest: parentDigest,
            inputRoot: root,
            recipient: recipient,
            publicAmount: amount,
            changeOutput: change,
            unshieldPublicInputs: inputs,
            unshieldPublicInputsDigest: inputsDigest,
            operationID: operationID
        )
    }

    private static func redeemChangeBranch(
        _ value: KagemushaRecursiveSpendRedeemChangeBranchV2
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try note(value.output))
        writer.writeField(branchPath(value.branchPath))
        writer.writeField(try nestedPayload(
            value.bundle.archive,
            schema: KagemushaRecursiveSpendV2.bundleWireName,
            field: "redeemChangeBundle"
        ))
        return writer.data
    }

    private static func decodeRedeemChangeBranch(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendRedeemChangeBranchV2 {
        var reader = KagemushaV2Reader(data)
        let output = try decodeNote(reader.field())
        let path = try decodeBranchPath(reader.field())
        let bundleArchive = frame(
            KagemushaRecursiveSpendV2.bundleWireName,
            payload: try reader.field()
        )
        try reader.finish("redeemChangeBranch")
        return KagemushaRecursiveSpendRedeemChangeBranchV2(
            output: output,
            branchPath: path,
            bundle: try KagemushaRecursiveSpendBundleV2(noritoArchive: bundleArchive)
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

    private static func note(_ value: KagemushaSpendableNoteDescriptorV2) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(chainID(value.chainID))
        writer.writeField(try assetDefinitionID(value.assetDefinitionID))
        writer.writeField(value.noteCommitment)
        writer.writeField(value.spendNullifier)
        writer.writeField(try scaledAmount(value.amount))
        return writer.data
    }

    private static func decodeNote(_ data: Data) throws -> KagemushaSpendableNoteDescriptorV2 {
        var reader = KagemushaV2Reader(data)
        let chain = try decodeChainID(reader.field())
        let asset = try decodeAssetDefinitionID(reader.field())
        let commitment = try packedFixed(reader.field(), count: 32, field: "noteCommitment")
        let nullifier = try packedFixed(reader.field(), count: 32, field: "spendNullifier")
        let amount = try decodeScaledAmount(reader.field())
        try reader.finish("note")
        return try KagemushaSpendableNoteDescriptorV2(
            chainID: chain,
            assetDefinitionID: asset,
            noteCommitment: commitment,
            spendNullifier: nullifier,
            amount: amount
        )
    }

    private static func branchPath(_ value: KagemushaRecursiveSpendBranchPathV2) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(value.lineageRoot)
        writer.writeField(Data([value.depth]))
        writer.writeField(value.pathBits)
        return writer.data
    }

    private static func decodeBranchPath(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendBranchPathV2 {
        var reader = KagemushaV2Reader(data)
        let root = try packedFixed(reader.field(), count: 32, field: "lineageRoot")
        let depthData = try reader.field()
        guard depthData.count == 1, let depth = depthData.first else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("branchPath.depth")
        }
        let path = try packedFixed(reader.field(), count: 8, field: "pathBits")
        try reader.finish("branchPath")
        return try KagemushaRecursiveSpendBranchPathV2(
            lineageRoot: root,
            depth: depth,
            pathBits: path
        )
    }

    private static func artifactReference(
        _ value: KagemushaRecursiveSpendArtifactReferenceV2
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

    private static func lineageWitness(
        _ value: KagemushaRecursiveSpendLineageWitnessV2
    ) -> Data {
        var transitions = OfflineCompactNoritoWriter()
        transitions.writeUInt64LE(UInt64(value.transitionArchives.count))
        for transition in value.transitionArchives {
            transitions.writeField(bytes(transition))
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(transitions.data)
        writer.writeField(value.finalBundleDigest)
        return writer.data
    }

    private static func decodeLineageWitnessPayload(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendLineageWitnessV2 {
        var reader = KagemushaV2Reader(data)
        var transitions = KagemushaV2Reader(try reader.field())
        let count = try transitions.uint64()
        guard count > 0, count <= 128 else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("lineageWitness.transitions")
        }
        var archives: [Data] = []
        archives.reserveCapacity(Int(count))
        for _ in 0..<count {
            archives.append(try decodeBytes(try transitions.field(), field: "transitionArchive"))
        }
        try transitions.finish("lineageWitness.transitions")
        let digest = try packedFixed(reader.field(), count: 32, field: "finalBundleDigest")
        try reader.finish("lineageWitness")
        return try KagemushaRecursiveSpendLineageWitnessV2(
            transitionArchives: archives,
            finalBundleDigest: digest
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
        key: KagemushaPublicKeyV2
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

    private static func publicKey(_ value: KagemushaPublicKeyV2) -> Data {
        var compact = Data([value.algorithm])
        compact.append(value.payload)
        return constVec(compact)
    }

    private static func decodePublicKey(_ data: Data) throws -> KagemushaPublicKeyV2 {
        let compact = try decodeConstVec(data, field: "publicKey")
        guard let algorithm = compact.first else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("publicKey")
        }
        return try KagemushaPublicKeyV2(
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
            throw KagemushaRecursiveSpendV2Error.invalidField("assetDefinitionID")
        }
        return constVec(decoded)
    }

    private static func decodeAssetDefinitionID(_ data: Data) throws -> String {
        let bytes = try decodeConstVec(data, field: "assetDefinitionID")
        guard bytes.count == 16,
              let value = AssetDefinitionAddress.encode(uuidBytes: bytes) else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("assetDefinitionID")
        }
        return value
    }

    private static func accountID(_ value: String) throws -> Data {
        do {
            let address = try AccountAddress.parseEncoded(value, expectedPrefix: 0x02F1)
            return try address.compactNoritoAccountControllerPayload()
        } catch {
            throw KagemushaRecursiveSpendV2Error.invalidField("accountID")
        }
    }

    private static func decodeAccountID(_ data: Data) throws -> String {
        var reader = KagemushaV2Reader(data)
        let tag = try reader.uint32()
        guard tag == 0 else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("accountID.controller")
        }
        let key = try decodePublicKey(reader.field())
        try reader.finish("accountID")
        guard let algorithm = SigningAlgorithm(noritoDiscriminant: key.algorithm) else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("accountID.algorithm")
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
            throw KagemushaRecursiveSpendV2Error.invalidField("assetID")
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
                throw KagemushaRecursiveSpendV2Error.invalidField("assetID.scope")
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive("assetID.scope")
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
        }
        var values: [Data] = []
        values.reserveCapacity(Int(count))
        for _ in 0..<count {
            let value = try decodeConstVec(try reader.field(), field: field)
            guard value.count == 32 else {
                throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
            }
            values.append(value)
        }
        try reader.finish(field)
        return values
    }

    private static func optionalVerifierRecord(
        _ value: KagemushaRecursiveSpendVerifierRecordRef?
    ) throws -> Data {
        guard let value else { return option(nil) }
        return option(try nestedPayload(
            value.recordBytes,
            schema: KagemushaRecursiveSpendRequestCodecs.verifyingKeyRecordWireName,
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

    private static func decodeLineageMode(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendLineageModeV2 {
        guard let value = KagemushaRecursiveSpendLineageModeV2(
            rawValue: try scalarUInt32(data, field: "lineageMode")
        ) else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("lineageMode")
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
        try KagemushaRecursiveSpendV2.requireArchive(archive, schema: schema, field: field)
        guard let decoded = noritoDecodeFrame(archive), decoded.paddingLength == 0 else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
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
        let count = try reader.uint64()
        guard count <= UInt64(Int.max) else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
        }
        let bytes = try reader.bytes(Int(count))
        try reader.finish(field)
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
        }
        return value
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
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
                throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
        }
    }

    private static func optionalUInt64(_ value: UInt64?) -> Data {
        option(value.map(uint64))
    }

    private static func decodeOptionalUInt64(_ data: Data, field: String) throws -> UInt64? {
        try decodeOption(data, field: field).map { try scalarUInt64($0, field: field) }
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
        }
        return byte == 1
    }

    private static func packedFixed(_ data: Data, count: Int, field: String) throws -> Data {
        guard data.count == count else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive(field)
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
            throw KagemushaRecursiveSpendV2Error.invalidField("u128")
        }
        output.append(contentsOf: repeatElement(UInt8(0), count: 16 - output.count))
        return output
    }

    private static func decodeU128(_ data: Data) throws -> String {
        guard data.count == 16 else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("u128")
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive("\(field).trailing")
        }
    }

    mutating func uint8() throws -> UInt8 {
        guard offset < data.count else {
            throw KagemushaRecursiveSpendV2Error.invalidArchive("truncated")
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
            throw KagemushaRecursiveSpendV2Error.invalidArchive("truncated")
        }
        let start = data.startIndex + offset
        offset += count
        return Data(data[start..<(start + count)])
    }

    mutating func field() throws -> Data {
        try bytes(length())
    }

    private mutating func length() throws -> Int {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        let start = offset
        for _ in 0..<10 {
            let byte = try uint8()
            let chunk = UInt64(byte & 0x7f)
            guard shift < 64, !(shift == 63 && chunk > 1) else {
                throw KagemushaRecursiveSpendV2Error.invalidArchive("length")
            }
            value |= chunk << shift
            if byte & 0x80 == 0 {
                let width = offset - start
                if width > 1, value < UInt64(1) << UInt64(7 * (width - 1)) {
                    throw KagemushaRecursiveSpendV2Error.invalidArchive("length.nonCanonical")
                }
                guard value <= UInt64(Int.max), value <= UInt64(data.count - offset) else {
                    throw KagemushaRecursiveSpendV2Error.invalidArchive("length")
                }
                return Int(value)
            }
            shift += 7
        }
        throw KagemushaRecursiveSpendV2Error.invalidArchive("length")
    }
}

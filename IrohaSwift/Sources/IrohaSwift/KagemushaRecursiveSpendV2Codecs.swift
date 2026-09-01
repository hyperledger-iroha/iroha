import Foundation

public enum KagemushaRecursiveSpendCodecs {
    public static func decodeNativeCapabilitiesV4(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendNativeCapabilitiesV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.nativeCapabilitiesWireNameV4,
            field: "nativeCapabilitiesV4"
        ))
        let value = try KagemushaRecursiveSpendNativeCapabilitiesV4(
            bridgeABIVersion: scalarUInt32(
                reader.field(),
                field: "nativeCapabilitiesV4.bridgeABIVersion"
            ),
            artifactManifestSchema: decodeString(
                reader.field(),
                field: "nativeCapabilitiesV4.artifactManifestSchema"
            ),
            proofBackend: decodeString(
                reader.field(),
                field: "nativeCapabilitiesV4.proofBackend"
            ),
            transcriptProfile: decodeString(
                reader.field(),
                field: "nativeCapabilitiesV4.transcriptProfile"
            ),
            proofEnvelopeVersion: scalarUInt16(
                reader.field(),
                field: "nativeCapabilitiesV4.proofEnvelopeVersion"
            ),
            stepEqCircuitID: decodeString(
                reader.field(),
                field: "nativeCapabilitiesV4.stepEqCircuitID"
            ),
            stepEpCircuitID: decodeString(
                reader.field(),
                field: "nativeCapabilitiesV4.stepEpCircuitID"
            ),
            artifactRoles: decodeStringVector(
                reader.field(),
                field: "nativeCapabilitiesV4.artifactRoles",
                maximumCount: UInt64(KagemushaRecursiveSpend.artifactRolesV4.count)
            ),
            maxProofBytes: scalarUInt32(
                reader.field(),
                field: "nativeCapabilitiesV4.maxProofBytes"
            ),
            proofBackendAvailable: decodeBool(
                reader.field(),
                field: "nativeCapabilitiesV4.proofBackendAvailable"
            ),
            missingGates: decodeStringVector(
                reader.field(),
                field: "nativeCapabilitiesV4.missingGates",
                maximumCount: 64
            )
        )
        try reader.finish("nativeCapabilitiesV4")
        return value
    }

    /// Decode the authenticated manifest prefix needed to bind the public
    /// generation label before any potentially large artifact stream begins.
    /// Native remains authoritative for validating the complete manifest.
    static func decodeArtifactManifestGeneration(_ archive: Data) throws -> String {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.artifactManifestWireName,
            field: "artifactManifest"
        ))
        let schema = try decodeString(
            reader.field(),
            field: "artifactManifest.schema"
        )
        let version = try scalarUInt16(
            reader.field(),
            field: "artifactManifest.version"
        )
        let bridgeABI = try scalarUInt32(
            reader.field(),
            field: "artifactManifest.bridgeABIVersion"
        )
        let proofBackend = try decodeString(
            reader.field(),
            field: "artifactManifest.proofBackend"
        )
        let transcript = try decodeString(
            reader.field(),
            field: "artifactManifest.transcriptProfile"
        )
        let generation = try decodeString(
            reader.field(),
            field: "artifactManifest.generation"
        )
        guard schema == KagemushaRecursiveSpend.artifactManifestSchemaV4,
              version == KagemushaRecursiveSpend.artifactManifestVersionV4,
              bridgeABI == KagemushaRecursiveSpend.requiredNativeBridgeAbiVersion,
              proofBackend == KagemushaRecursiveSpend.pastaCycleBackendV4,
              transcript == KagemushaRecursiveSpend.pastaCycleTranscriptV4 else {
            throw KagemushaRecursiveSpendError.invalidField("artifactManifest.contract")
        }
        try KagemushaRecursiveSpend.requirePortableArtifactIdentifier(
            generation,
            field: "artifactManifest.generation"
        )
        return generation
    }

    static func canonicalAssetID(_ value: String) throws -> String {
        let parts = value.split(separator: "#", omittingEmptySubsequences: false)
        guard parts.count == 2 || parts.count == 3 else {
            throw KagemushaRecursiveSpendError.invalidField("assetID")
        }
        let chainDiscriminant = try KagemushaRecursiveSpend.canonicalAccountAddress(
            String(parts[1]),
            field: "assetID.accountID"
        ).chainDiscriminant
        let canonical = try decodeAssetID(
            assetID(value),
            chainDiscriminant: chainDiscriminant
        )
        guard canonical == value else {
            throw KagemushaRecursiveSpendError.invalidField("assetID")
        }
        return canonical
    }

    public static func encodeRecipientRequestPayload(
        _ payload: KagemushaRecipientPaymentRequestSigningPayload
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(payload.networkID.bytes)
        writer.writeField(try assetDefinitionID(payload.assetDefinitionID))
        writer.writeField(try scaledAmount(payload.amount))
        writer.writeField(try accountID(payload.recipient))
        writer.writeField(payload.recipientKeyReference)
        writer.writeField(string(payload.receiverDeviceID))
        writer.writeField(devicePublicKey(payload.receiverPublicKey))
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
        defer { writer.wipe() }
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
        defer { reader.wipe() }
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
        var canonical = try encodeNoteOpening(opening)
        defer { canonical.resetBytes(in: 0..<canonical.count) }
        guard canonical == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("noteOpening.canonical")
        }
        return opening
    }

    static func decodeRedemptionChangePrepareResultV4(
        _ archive: Data,
        inputOpening: KagemushaNoteOpening,
        inputSummary: KagemushaRecursiveSpendBundleSummaryV4,
        changeAmount: KagemushaScaledAmount
    ) throws -> KagemushaRecursiveSpendRedemptionChangePreparationV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.redemptionChangePrepareResultWireNameV4,
            field: "redemptionChangePrepareResultV4"
        ))
        defer { reader.wipe() }
        let version = try scalarUInt16(
            reader.field(),
            field: "redemptionChangePrepareResultV4.version"
        )
        var openingPayload = try reader.field()
        defer { openingPayload.resetBytes(in: 0..<openingPayload.count) }
        var openingArchive = frame(
            KagemushaRecursiveSpend.noteOpeningWireName,
            payload: openingPayload
        )
        defer { openingArchive.resetBytes(in: 0..<openingArchive.count) }
        let opening = try decodeNoteOpening(openingArchive)
        let output = try decodeNote(reader.field())
        try reader.finish("redemptionChangePrepareResultV4")
        guard version == KagemushaRecursiveSpend.wireVersionV4 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "redemptionChangePrepareResultV4.version"
            )
        }
        let preparation = try KagemushaRecursiveSpendRedemptionChangePreparationV4(
            opening: opening,
            output: output,
            inputOpening: inputOpening,
            inputSummary: inputSummary,
            changeAmount: changeAmount
        )
        var canonical = try encodeRedemptionChangePrepareResultV4(preparation)
        defer { canonical.resetBytes(in: 0..<canonical.count) }
        guard canonical == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "redemptionChangePrepareResultV4.canonical"
            )
        }
        return preparation
    }

    static func decodePeerSplitChangePrepareResultV4(
        _ archive: Data,
        inputOpenings: [KagemushaNoteOpening],
        inputSummaries: [KagemushaRecursiveSpendBundleSummaryV4],
        recipientRequest: KagemushaRecipientPaymentRequest,
        changeAmount: KagemushaScaledAmount
    ) throws -> KagemushaRecursiveSpendPeerSplitChangePreparationV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.peerSplitChangePrepareResultWireNameV4,
            field: "peerSplitChangePrepareResultV4"
        ))
        defer { reader.wipe() }
        let version = try scalarUInt16(
            reader.field(),
            field: "peerSplitChangePrepareResultV4.version"
        )
        var openingPayload = try reader.field()
        defer { openingPayload.resetBytes(in: 0..<openingPayload.count) }
        var openingArchive = frame(
            KagemushaRecursiveSpend.noteOpeningWireName,
            payload: openingPayload
        )
        defer { openingArchive.resetBytes(in: 0..<openingArchive.count) }
        let opening = try decodeNoteOpening(openingArchive)
        let output = try decodeNote(reader.field())
        try reader.finish("peerSplitChangePrepareResultV4")
        guard version == KagemushaRecursiveSpend.wireVersionV4 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "peerSplitChangePrepareResultV4.version"
            )
        }
        let preparation = try KagemushaRecursiveSpendPeerSplitChangePreparationV4(
            opening: opening,
            output: output,
            inputOpenings: inputOpenings,
            inputSummaries: inputSummaries,
            recipientRequest: recipientRequest,
            changeAmount: changeAmount
        )
        var canonical = try encodePeerSplitChangePrepareResultV4(preparation)
        defer { canonical.resetBytes(in: 0..<canonical.count) }
        guard canonical == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "peerSplitChangePrepareResultV4.canonical"
            )
        }
        return preparation
    }

    /// Encodes local encrypted Merkle membership data for one owned note.
    /// This archive is native-prover input only and must never enter peer or
    /// Torii payloads.
    public static func encodeMembershipWitness(
        _ witness: KagemushaNoteMembershipWitness
    ) throws -> Data {
        try encodeMembershipWitness(
            witness,
            schema: KagemushaRecursiveSpend.membershipWitnessWireName
        )
    }

    private static func encodeMembershipWitness(
        _ witness: KagemushaNoteMembershipWitness,
        schema: String
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(uint32(witness.leafIndex))
        writer.writeField(try membershipPath(witness.inputPath))
        writer.writeField(try membershipPath(witness.dummyInputPath))
        return frame(schema, payload: writer.data)
    }

    /// Strictly decodes and canonically re-encodes a local membership witness.
    public static func decodeMembershipWitness(
        _ archive: Data
    ) throws -> KagemushaNoteMembershipWitness {
        try decodeMembershipWitness(
            archive,
            schema: KagemushaRecursiveSpend.membershipWitnessWireName,
            field: "membershipWitness"
        )
    }

    private static func decodeMembershipWitness(
        _ archive: Data,
        schema: String,
        field: String
    ) throws -> KagemushaNoteMembershipWitness {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: schema,
            field: field
        ))
        let witness = try KagemushaNoteMembershipWitness(
            leafIndex: scalarUInt32(
                reader.field(),
                field: "\(field).leafIndex"
            ),
            inputPath: decodeMembershipPath(
                reader.field(),
                field: "\(field).inputPath"
            ),
            dummyInputPath: decodeMembershipPath(
                reader.field(),
                field: "\(field).dummyInputPath"
            )
        )
        try reader.finish(field)
        guard try encodeMembershipWitness(witness, schema: schema) == archive else {
            throw KagemushaRecursiveSpendError.invalidArchive("\(field).canonical")
        }
        return witness
    }

    public static func encodeRecipientOutputDerivationRequest(
        _ request: KagemushaRecipientOutputDerivationRequest
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(request.networkID.bytes)
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
        _ archive: Data,
        chainDiscriminant: UInt16
    ) throws -> KagemushaRecipientPaymentRequest {
        let payloadData = try payload(
            archive,
            schema: KagemushaRecursiveSpend.recipientRequestWireName,
            field: "recipientRequest"
        )
        var reader = KagemushaV2Reader(payloadData)
        let signedPayload = try decodeRecipientRequestPayloadFields(
            &reader,
            chainDiscriminant: chainDiscriminant
        )
        let signature = try KagemushaDeviceSignatureV2(
            rawBytes: packedFixed(
                try reader.field(),
                count: KagemushaDeviceSignatureV2.rawByteCount,
                field: "recipientRequest.signature"
            )
        )
        try reader.finish("recipientRequest")
        let canonical = try encodeRecipientRequest(signedPayload, signature: signature)
        guard canonical == archive,
              archive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV2 else {
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
        chainDiscriminant: UInt16,
        atMilliseconds: UInt64
    ) throws -> KagemushaVerifiedRecipientPaymentRequest {
        try decodeRecipientRequest(
            archive,
            chainDiscriminant: chainDiscriminant
        ).verified(atMilliseconds: atMilliseconds)
    }

    public static func encodeAuthorizationPreparation(
        _ fields: KagemushaRequestAuthorizationFields
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(uint16(KagemushaRecursiveSpend.authorizationPreparationVersionV3))
        writer.writeField(try accountID(fields.authority))
        writer.writeField(string(fields.deviceID))
        writer.writeField(try assetDefinitionID(fields.assetDefinitionID))
        writer.writeField(uint64(fields.issuedAtMilliseconds))
        writer.writeField(uint64(fields.expiresAtMilliseconds))
        writer.writeField(fields.nonce)
        writer.writeField(fields.payloadDigest)
        writer.writeField(fields.registrationHash)
        writer.writeField(uint32(fields.platform.rawValue))
        return frame(
            KagemushaRecursiveSpend.authorizationPreparationWireNameV3,
            payload: writer.data
        )
    }

    public static func encodeTopUpShieldBuildRequestV5(
        _ request: KagemushaTopUpShieldBuildRequestV5
    ) throws -> Data {
        var zeroPath = CompactNoritoWriter()
        zeroPath.writeField(try sequence(request.zeroPath.siblings))
        zeroPath.writeField(bytes(request.zeroPath.directions))
        zeroPath.writeField(request.zeroPath.root)

        var writer = CompactNoritoWriter()
        writer.writeField(uint16(request.version))
        writer.writeField(request.networkID.bytes)
        writer.writeField(try assetID(request.assetID))
        writer.writeField(try scaledAmount(request.amount))
        writer.writeField(try accountID(request.payer))
        writer.writeField(request.nonce)
        writer.writeField(try nestedPayload(
            request.opening.noritoEncoded(),
            schema: KagemushaRecursiveSpend.noteOpeningWireName,
            field: "topUpShieldBuildRequestV5.opening"
        ))
        writer.writeField(uint32(request.leafIndex))
        writer.writeField(zeroPath.data)
        writer.writeField(try verifierKeyID(request.shieldVerifierID))
        writer.writeField(request.shieldVerifierCommitment)
        writer.writeField(artifactBindingV4(request.artifactBinding))
        return frame(
            KagemushaRecursiveSpend.topUpShieldBuildRequestWireNameV5,
            payload: writer.data
        )
    }

    public static func decodeTopUpUnsignedV4(
        _ archive: Data,
        chainDiscriminant: UInt16
    ) throws -> KagemushaRecursiveSpendTopUpUnsignedV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.topUpUnsignedWireNameV4,
            field: "topUpUnsignedV4"
        ))
        let version = try scalarUInt16(reader.field(), field: "topUpUnsignedV4.version")
        let asset = try decodeAssetID(
            reader.field(),
            chainDiscriminant: chainDiscriminant
        )
        let amount = try decodeScaledAmount(reader.field())
        let note = try decodeNote(reader.field())
        let evidence = try decodeTopUpShieldEvidence(reader.field())
        let binding = try decodeArtifactBindingV4(reader.field())
        let operationID = try packedFixed(
            reader.field(), count: 32, field: "topUpUnsignedV4.operationID"
        )
        try reader.finish("topUpUnsignedV4")
        let assetParts = asset.split(separator: "#", omittingEmptySubsequences: false)
        guard version == KagemushaRecursiveSpend.wireVersionV4,
              note.amount == amount,
              assetParts.first.map(String.init) == note.assetDefinitionID,
              operationID.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpUnsignedV4.binding")
        }
        return KagemushaRecursiveSpendTopUpUnsignedV4(
            assetID: asset,
            amount: amount,
            currentNote: note,
            shieldEvidence: evidence,
            artifactBinding: binding,
            operationID: operationID,
            noritoArchive: archive
        )
    }

    public static func decodeTopUpAnchorV4(
        _ archive: Data,
        chainDiscriminant: UInt16
    ) throws -> KagemushaRecursiveSpendTopUpAnchorV4 {
        guard archive.count
                <= KagemushaRecursiveSpend.topUpFinalityAnchorMaximumArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpAnchorV4.size")
        }
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.topUpAnchorWireNameV4,
            field: "topUpAnchorV4"
        ))
        let version = try scalarUInt16(reader.field(), field: "topUpAnchorV4.version")
        let networkID = try decodeNetworkID(reader.field())
        let payer = try decodeAccountID(
            reader.field(),
            chainDiscriminant: chainDiscriminant
        )
        let asset = try decodeAssetID(
            reader.field(),
            chainDiscriminant: chainDiscriminant
        )
        let scale = try scalarUInt32(reader.field(), field: "topUpAnchorV4.assetScale")
        let amount = try decodeScaledAmount(reader.field())
        let initialRoot = try packedFixed(
            reader.field(), count: 32, field: "topUpAnchorV4.initialRoot"
        )
        let finalizedRoot = try packedFixed(
            reader.field(), count: 32, field: "topUpAnchorV4.finalizedRoot"
        )
        let leafIndex = try scalarUInt32(reader.field(), field: "topUpAnchorV4.shieldLeafIndex")
        let note = try decodeNote(reader.field())
        let operationID = try packedFixed(
            reader.field(), count: 32, field: "topUpAnchorV4.topUpOperationID"
        )
        _ = try decodeVerifierKeyID(reader.field())
        let verifierCommitment = try packedFixed(
            reader.field(), count: 32, field: "topUpAnchorV4.shieldVerifierCommitment"
        )
        let binding = try decodeArtifactBindingV4(reader.field())
        let finalizedHeight = try scalarUInt64(
            reader.field(), field: "topUpAnchorV4.finalizedHeight"
        )
        let transactionHash = try packedFixed(
            reader.field(), count: 32, field: "topUpAnchorV4.finalizedTransactionHash"
        )
        let digest = try packedFixed(
            reader.field(), count: 32, field: "topUpAnchorV4.anchorDigest"
        )
        try reader.finish("topUpAnchorV4")
        let assetParts = asset.split(separator: "#", omittingEmptySubsequences: false)
        guard version == KagemushaRecursiveSpend.wireVersionV4,
              scale == amount.scale,
              note.amount == amount,
              note.networkID == networkID,
              assetParts.first.map(String.init) == note.assetDefinitionID,
              assetParts.count >= 2,
              String(assetParts[1]) == payer,
              initialRoot != finalizedRoot,
              initialRoot.contains(where: { $0 != 0 }),
              finalizedRoot.contains(where: { $0 != 0 }),
              leafIndex < KagemushaRecursiveSpend.topUpShieldInsertionCapacityV2,
              operationID.contains(where: { $0 != 0 }),
              verifierCommitment.contains(where: { $0 != 0 }),
              finalizedHeight > 0,
              transactionHash.contains(where: { $0 != 0 }),
              digest.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpAnchorV4.binding")
        }
        return KagemushaRecursiveSpendTopUpAnchorV4(
            networkID: networkID,
            topUpOperationID: operationID,
            artifactBinding: binding,
            finalizedHeight: finalizedHeight,
            finalizedTransactionHash: transactionHash,
            anchorDigest: digest,
            noritoArchive: archive
        )
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
        let key = try decodeDevicePublicKey(reader.field(), field: "receiverPublicKey")
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
        let signature = try KagemushaDeviceSignatureV2(
            rawBytes: packedFixed(
                try reader.field(),
                count: KagemushaDeviceSignatureV2.rawByteCount,
                field: "acknowledgement.signature"
            )
        )
        try reader.finish("acknowledgement")
        guard archive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV2 else {
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

    private static func encodeRecipientRequest(
        _ payload: KagemushaRecipientPaymentRequestSigningPayload,
        signature: KagemushaDeviceSignatureV2
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
        result.writeField(signature.rawBytes)
        return frame(KagemushaRecursiveSpend.recipientRequestWireName, payload: result.data)
    }

    private static func decodeRecipientRequestPayloadFields(
        _ reader: inout KagemushaV2Reader,
        chainDiscriminant: UInt16
    ) throws -> KagemushaRecipientPaymentRequestSigningPayload {
        let networkID = try decodeNetworkID(reader.field())
        let asset = try decodeAssetDefinitionID(reader.field())
        let amount = try decodeScaledAmount(reader.field())
        let recipient = try decodeAccountID(
            reader.field(),
            chainDiscriminant: chainDiscriminant
        )
        let keyReference = try packedFixed(
            reader.field(), count: 32, field: "recipientKeyReference"
        )
        let device = try decodeString(reader.field(), field: "receiverDeviceID")
        let key = try decodeDevicePublicKey(reader.field(), field: "receiverPublicKey")
        let requestID = try packedFixed(reader.field(), count: 32, field: "requestID")
        let issued = try scalarUInt64(reader.field(), field: "issuedAtMilliseconds")
        let expires = try scalarUInt64(reader.field(), field: "expiresAtMilliseconds")
        let output = try decodeNote(reader.field())
        let material = try decodeBytes(reader.field(), field: "senderOutputProverMaterial")
        return try KagemushaRecipientPaymentRequestSigningPayload(
            networkID: networkID,
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
        writer.writeField(value.networkID.bytes)
        writer.writeField(try assetDefinitionID(value.assetDefinitionID))
        writer.writeField(value.noteCommitment)
        writer.writeField(value.spendNullifier)
        writer.writeField(try scaledAmount(value.amount))
        return writer.data
    }

    static func encodeRedemptionChangePrepareResultV4(
        _ preparation: KagemushaRecursiveSpendRedemptionChangePreparationV4
    ) throws -> Data {
        var openingArchive = try encodeNoteOpening(preparation.opening)
        defer { openingArchive.resetBytes(in: 0..<openingArchive.count) }
        var writer = CompactNoritoWriter()
        defer { writer.wipe() }
        writer.writeField(uint16(KagemushaRecursiveSpend.wireVersionV4))
        var openingPayload = try payload(
            openingArchive,
            schema: KagemushaRecursiveSpend.noteOpeningWireName,
            field: "redemptionChangePrepareResultV4.opening"
        )
        defer { openingPayload.resetBytes(in: 0..<openingPayload.count) }
        writer.writeField(openingPayload)
        writer.writeField(try note(preparation.output))
        return frame(
            KagemushaRecursiveSpend.redemptionChangePrepareResultWireNameV4,
            payload: writer.data
        )
    }

    static func encodePeerSplitChangePrepareResultV4(
        _ preparation: KagemushaRecursiveSpendPeerSplitChangePreparationV4
    ) throws -> Data {
        var openingArchive = try encodeNoteOpening(preparation.opening)
        defer { openingArchive.resetBytes(in: 0..<openingArchive.count) }
        var writer = CompactNoritoWriter()
        defer { writer.wipe() }
        writer.writeField(uint16(KagemushaRecursiveSpend.wireVersionV4))
        var openingPayload = try payload(
            openingArchive,
            schema: KagemushaRecursiveSpend.noteOpeningWireName,
            field: "peerSplitChangePrepareResultV4.opening"
        )
        defer { openingPayload.resetBytes(in: 0..<openingPayload.count) }
        writer.writeField(openingPayload)
        writer.writeField(try note(preparation.output))
        return frame(
            KagemushaRecursiveSpend.peerSplitChangePrepareResultWireNameV4,
            payload: writer.data
        )
    }

    private static func decodeNote(_ data: Data) throws -> KagemushaSpendableNoteDescriptor {
        var reader = KagemushaV2Reader(data)
        let networkID = try decodeNetworkID(reader.field())
        let asset = try decodeAssetDefinitionID(reader.field())
        let commitment = try packedFixed(reader.field(), count: 32, field: "noteCommitment")
        let nullifier = try packedFixed(reader.field(), count: 32, field: "spendNullifier")
        let amount = try decodeScaledAmount(reader.field())
        try reader.finish("note")
        return try KagemushaSpendableNoteDescriptor(
            networkID: networkID,
            assetDefinitionID: asset,
            noteCommitment: commitment,
            spendNullifier: nullifier,
            amount: amount
        )
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

    private static func acknowledgementPayload(
        operationID: Data,
        requestDigest: Data,
        bundleDigest: Data,
        commitment: Data,
        acceptedAt: UInt64,
        deviceID: String,
        keyReference: Data,
        key: KagemushaDevicePublicKeyV2
    ) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(operationID)
        writer.writeField(requestDigest)
        writer.writeField(bundleDigest)
        writer.writeField(commitment)
        writer.writeField(uint64(acceptedAt))
        writer.writeField(string(deviceID))
        writer.writeField(keyReference)
        writer.writeField(devicePublicKey(key))
        return writer.data
    }

    private static func devicePublicKey(_ value: KagemushaDevicePublicKeyV2) -> Data {
        value.sec1Bytes
    }

    private static func decodeDevicePublicKey(
        _ data: Data,
        field: String
    ) throws -> KagemushaDevicePublicKeyV2 {
        try KagemushaDevicePublicKeyV2(
            sec1Bytes: packedFixed(
                data,
                count: KagemushaDevicePublicKeyV2.sec1ByteCount,
                field: field
            )
        )
    }

    private static func publicKey(_ value: KagemushaPublicKey) -> Data {
        var compact = Data([value.algorithm])
        compact.append(value.payload)
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(compact.count))
        writer.writeByteFields(compact)
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

    private static func decodeNetworkID(_ data: Data) throws -> NetworkId {
        guard data.count == 32 else {
            throw KagemushaRecursiveSpendError.invalidArchive("networkID")
        }
        do {
            return try NetworkId(bytes: data)
        } catch {
            throw KagemushaRecursiveSpendError.invalidArchive("networkID")
        }
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
            return try KagemushaRecursiveSpend.canonicalAccountAddress(
                value,
                field: "accountID"
            ).address.compactNoritoAccountControllerPayload()
        } catch {
            throw KagemushaRecursiveSpendError.invalidField("accountID")
        }
    }

    private static func decodeAccountID(
        _ data: Data,
        chainDiscriminant: UInt16
    ) throws -> String {
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
        return try address.toI105(networkPrefix: chainDiscriminant)
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

    private static func decodeAssetID(
        _ data: Data,
        chainDiscriminant: UInt16
    ) throws -> String {
        var reader = KagemushaV2Reader(data)
        let account = try decodeAccountID(
            reader.field(),
            chainDiscriminant: chainDiscriminant
        )
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
            let value = try decodeConstVec(reader.field(), field: field)
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
        writer.writeField(try sequence(path.siblings.map(constVec)))
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
        KagemushaRecursiveSpend.frameArchive(schema: schema, payload: payload)
    }

    private static func payload(
        _ archive: Data,
        schema: String,
        field: String
    ) throws -> Data {
        try KagemushaRecursiveSpend.requireArchive(archive, schema: schema, field: field)
        guard let requiredPaddingLength = KagemushaRecursiveSpend
            .requiredHeaderPaddingLength(forWireName: schema),
              let decoded = noritoDecodeFrame(archive),
              decoded.paddingLength == requiredPaddingLength else {
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
        writer.writeByteFields(value)
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

    /// Decode the wallet-safe projection produced by the ABI-21 bundle gate.
    public static func decodeBundleSummaryV4(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendBundleSummaryV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.bundleSummaryWireNameV4,
            field: "bundleSummaryV4"
        ))
        let asset = try decodeAssetDefinitionID(reader.field())
        let amount = try decodeScaledAmount(reader.field())
        let commitment = try packedFixed(
            reader.field(), count: 32, field: "bundleSummaryV4.noteCommitment"
        )
        let nullifier = try packedFixed(
            reader.field(), count: 32, field: "bundleSummaryV4.spendNullifier"
        )
        let hopCount = try scalarUInt32(reader.field(), field: "bundleSummaryV4.hopCount")
        let proofStepCount = try scalarUInt32(
            reader.field(), field: "bundleSummaryV4.proofStepCount"
        )
        let claims = try decodeBranchClaims(
            reader.field(), field: "bundleSummaryV4.branchClaims"
        )
        let binding = try decodeArtifactBindingV4(reader.field())
        let verifierKeyID = try decodeVerifierKeyID(reader.field())
        let digest = try packedFixed(
            reader.field(), count: 32, field: "bundleSummaryV4.bundleDigest"
        )
        try reader.finish("bundleSummaryV4")
        guard hopCount <= KagemushaRecursiveSpend.maximumPeerHops,
              proofStepCount > 0,
              proofStepCount <= KagemushaRecursiveSpend.maximumProofSteps,
              commitment.contains(where: { $0 != 0 }),
              nullifier.contains(where: { $0 != 0 }),
              digest.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidArchive("bundleSummaryV4.binding")
        }
        return KagemushaRecursiveSpendBundleSummaryV4(
            assetDefinitionID: asset,
            amount: amount,
            noteCommitment: commitment,
            spendNullifier: nullifier,
            hopCount: hopCount,
            proofStepCount: proofStepCount,
            branchClaims: claims,
            artifactBinding: binding,
            verifierKeyID: verifierKeyID,
            bundleDigest: digest
        )
    }

    public static func decodeInitResultV4(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendInitResultV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.initResultWireNameV4,
            field: "initResultV4"
        ))
        let bundle = try KagemushaRecursiveSpendBundleV4(noritoArchive: frame(
            KagemushaRecursiveSpend.bundleWireNameV4,
            payload: reader.field()
        ))
        let membershipWitness = try decodeMembershipWitness(
            frame(
                KagemushaRecursiveSpend.spendableMembershipWitnessWireName,
                payload: reader.field()
            ),
            schema: KagemushaRecursiveSpend.spendableMembershipWitnessWireName,
            field: "initResultV4.membershipWitness"
        )
        let provenance = try KagemushaRecursiveSpendTopUpProvenanceV4(
            noritoArchive: frame(
                KagemushaRecursiveSpend.topUpProvenanceWireNameV4,
                payload: reader.field()
            )
        )
        let digest = try packedFixed(
            reader.field(), count: 32, field: "initResultV4.publicStatementDigest"
        )
        try reader.finish("initResultV4")
        return try KagemushaRecursiveSpendInitResultV4(
            bundle: bundle,
            membershipWitness: membershipWitness,
            topUpProvenance: provenance,
            publicStatementDigest: digest,
            noritoArchive: archive
        )
    }

    public static func decodeOutputMembershipPathsV4(
        _ archive: Data
    ) throws -> KagemushaOutputMembershipPathsV4 {
        guard !archive.isEmpty,
              archive.count
                <= KagemushaRecursiveSpend.maximumOutputMembershipPathsArchiveBytesV4 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "outputMembershipPathsV4.size"
            )
        }
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.outputMembershipPathsWireNameV4,
            field: "outputMembershipPathsV4"
        ))
        let initialRoot = try packedFixed(
            reader.field(), count: 32, field: "outputMembershipPathsV4.initialRoot"
        )
        let finalRoot = try packedFixed(
            reader.field(), count: 32, field: "outputMembershipPathsV4.finalRoot"
        )
        let recipient = try decodeOption(
            reader.field(), field: "outputMembershipPathsV4.recipient"
        ).map {
            try decodeOutputMembershipLeafPathsV4(
                $0,
                field: "outputMembershipPathsV4.recipient"
            )
        }
        let change = try decodeOption(
            reader.field(), field: "outputMembershipPathsV4.change"
        ).map {
            try decodeOutputMembershipLeafPathsV4(
                $0,
                field: "outputMembershipPathsV4.change"
            )
        }
        let dummyLeafIndex = try scalarUInt32(
            reader.field(), field: "outputMembershipPathsV4.dummyLeafIndex"
        )
        let dummyPath = try decodeMembershipPath(
            reader.field(), field: "outputMembershipPathsV4.dummyPath"
        )
        try reader.finish("outputMembershipPathsV4")
        return try KagemushaOutputMembershipPathsV4(
            initialRoot: initialRoot,
            finalRoot: finalRoot,
            recipient: recipient,
            change: change,
            dummyLeafIndex: dummyLeafIndex,
            dummyPath: dummyPath
        )
    }

    private static func decodeOutputMembershipLeafPathsV4(
        _ data: Data,
        field: String
    ) throws -> KagemushaOutputMembershipLeafPathsV4 {
        var reader = KagemushaV2Reader(data)
        let leafIndex = try scalarUInt32(
            reader.field(), field: "\(field).leafIndex"
        )
        let updatePath = try decodeMembershipPath(
            reader.field(), field: "\(field).updatePath"
        )
        let membershipPath = try decodeMembershipPath(
            reader.field(), field: "\(field).membershipPath"
        )
        try reader.finish(field)
        return try KagemushaOutputMembershipLeafPathsV4(
            leafIndex: leafIndex,
            updatePath: updatePath,
            membershipPath: membershipPath
        )
    }

    public static func decodePeerPaymentV4(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendPeerPaymentV4 {
        guard !archive.isEmpty,
              archive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV4 else {
            throw KagemushaRecursiveSpendError.invalidArchive("peerPaymentV4.size")
        }
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.peerPaymentWireNameV4,
            field: "peerPaymentV4"
        ))
        let bundle = try KagemushaRecursiveSpendBundleV4(noritoArchive: frame(
            KagemushaRecursiveSpend.bundleWireNameV4,
            payload: reader.field()
        ))
        let witness = try decodeMembershipWitness(
            frame(
                KagemushaRecursiveSpend.spendableMembershipWitnessWireName,
                payload: reader.field()
            ),
            schema: KagemushaRecursiveSpend.spendableMembershipWitnessWireName,
            field: "peerPaymentV4.recipientMembershipWitness"
        )
        let provenance = try KagemushaRecursiveSpendTopUpProvenanceV4(
            noritoArchive: frame(
                KagemushaRecursiveSpend.topUpProvenanceWireNameV4,
                payload: reader.field()
            )
        )
        try reader.finish("peerPaymentV4")
        return KagemushaRecursiveSpendPeerPaymentV4(
            recipientBundle: bundle,
            recipientMembershipWitness: witness,
            topUpProvenance: provenance,
            noritoArchive: archive
        )
    }

    public static func decodeSplitResultV4(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendSplitResultV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.splitResultWireNameV4,
            field: "splitResultV4"
        ))
        let split = try decodeSplitIntentV4(frame(
            KagemushaRecursiveSpend.splitIntentWireNameV4,
            payload: reader.field()
        ))
        let bindingDigest = try packedFixed(
            reader.field(), count: 32, field: "splitResultV4.splitBindingDigest"
        )
        let recipient = try KagemushaRecursiveSpendBundleV4(noritoArchive: frame(
            KagemushaRecursiveSpend.bundleWireNameV4,
            payload: reader.field()
        ))
        let recipientWitness = try decodeMembershipWitness(
            frame(
                KagemushaRecursiveSpend.spendableMembershipWitnessWireName,
                payload: reader.field()
            ),
            schema: KagemushaRecursiveSpend.spendableMembershipWitnessWireName,
            field: "splitResultV4.recipientMembershipWitness"
        )
        let recipientProvenance = try KagemushaRecursiveSpendTopUpProvenanceV4(
            noritoArchive: frame(
                KagemushaRecursiveSpend.topUpProvenanceWireNameV4,
                payload: reader.field()
            )
        )
        let changePayload = try decodeOption(reader.field(), field: "splitResultV4.changeBundle")
        let changeWitnessPayload = try decodeOption(
            reader.field(), field: "splitResultV4.changeMembershipWitness"
        )
        let changeProvenancePayload = try decodeOption(
            reader.field(), field: "splitResultV4.changeTopUpProvenance"
        )
        try reader.finish("splitResultV4")
        guard (changePayload == nil) == (changeWitnessPayload == nil),
              (changePayload == nil) == (changeProvenancePayload == nil) else {
            throw KagemushaRecursiveSpendError.invalidArchive("splitResultV4.change")
        }
        let change = try changePayload.map {
            try KagemushaRecursiveSpendBundleV4(noritoArchive: frame(
                KagemushaRecursiveSpend.bundleWireNameV4,
                payload: $0
            ))
        }
        let changeWitness = try changeWitnessPayload.map {
            try decodeMembershipWitness(
                frame(KagemushaRecursiveSpend.spendableMembershipWitnessWireName, payload: $0),
                schema: KagemushaRecursiveSpend.spendableMembershipWitnessWireName,
                field: "splitResultV4.changeMembershipWitness"
            )
        }
        let changeProvenance = try changeProvenancePayload.map {
            try KagemushaRecursiveSpendTopUpProvenanceV4(noritoArchive: frame(
                KagemushaRecursiveSpend.topUpProvenanceWireNameV4,
                payload: $0
            ))
        }
        guard let paymentArchive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendPeerPaymentFromSplitV4(splitResultArchive: archive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        let payment = try decodePeerPaymentV4(paymentArchive)
        return try KagemushaRecursiveSpendSplitResultV4(
            split: split,
            splitBindingDigest: bindingDigest,
            recipientBundle: recipient,
            recipientMembershipWitness: recipientWitness,
            recipientTopUpProvenance: recipientProvenance,
            changeBundle: change,
            changeMembershipWitness: changeWitness,
            changeTopUpProvenance: changeProvenance,
            peerPayment: payment,
            noritoArchive: archive
        )
    }

    public static func decodeVerifyResultV4(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendVerifyResultV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.verifyResultWireNameV4,
            field: "verifyResultV4"
        ))
        let valid = try decodeBool(reader.field(), field: "verifyResultV4.valid")
        let chainAdmissible = try decodeBool(
            reader.field(), field: "verifyResultV4.chainAdmissible"
        )
        let lineageRedeemable = try decodeBool(
            reader.field(), field: "verifyResultV4.lineageRedeemable"
        )
        let witnessless = try decodeBool(
            reader.field(), field: "verifyResultV4.witnesslessRedemptionSupported"
        )
        let summary = try decodeBundleSummaryV4(frame(
            KagemushaRecursiveSpend.bundleSummaryWireNameV4,
            payload: reader.field()
        ))
        let requestDigest = try packedFixed(
            reader.field(), count: 32, field: "verifyResultV4.recipientRequestDigest"
        )
        let outputBindingDigest = try packedFixed(
            reader.field(), count: 32, field: "verifyResultV4.requestOutputBindingDigest"
        )
        let verifierKeyID = try decodeVerifierKeyID(reader.field())
        let verifierCircuitID = try decodeString(
            reader.field(), field: "verifyResultV4.verifierCircuitID"
        )
        let activation = try decodeOptionalUInt64(
            reader.field(), field: "verifyResultV4.verifierActivationHeight"
        )
        let withdrawal = try decodeOptionalUInt64(
            reader.field(), field: "verifyResultV4.verifierWithdrawHeight"
        )
        let blockHeight = try scalarUInt64(
            reader.field(), field: "verifyResultV4.verifiedAtBlockHeight"
        )
        let verifiedAt = try scalarUInt64(
            reader.field(), field: "verifyResultV4.verifiedAtMilliseconds"
        )
        try reader.finish("verifyResultV4")
        let expectedVerifierKeyID = try KagemushaRecursiveSpend
            .releaseQualifiedStepEqVerifierKeyIDV4(
                manifestSHA256: summary.artifactBinding.manifestSHA256
            )
        guard valid, chainAdmissible, lineageRedeemable, witnessless,
              requestDigest.contains(where: { $0 != 0 }),
              outputBindingDigest.contains(where: { $0 != 0 }),
              let activation,
              let withdrawal,
              activation > 0,
              activation < withdrawal,
              blockHeight >= activation,
              blockHeight < withdrawal,
              summary.verifierKeyID == verifierKeyID,
              verifierKeyID == expectedVerifierKeyID,
              verifierCircuitID == KagemushaRecursiveSpend.stepEqCircuitIDV4,
              blockHeight > 0, verifiedAt > 0 else {
            throw KagemushaRecursiveSpendError.invalidArchive("verifyResultV4.binding")
        }
        return KagemushaRecursiveSpendVerifyResultV4(
            valid: valid,
            chainAdmissible: chainAdmissible,
            lineageRedeemable: lineageRedeemable,
            witnesslessRedemptionSupported: witnessless,
            summary: summary,
            recipientRequestDigest: requestDigest,
            requestOutputBindingDigest: outputBindingDigest,
            verifierKeyID: verifierKeyID,
            verifierCircuitID: verifierCircuitID,
            verifierActivationHeight: activation,
            verifierWithdrawHeight: withdrawal,
            verifiedAtBlockHeight: blockHeight,
            verifiedAtMilliseconds: verifiedAt,
            noritoArchive: archive
        )
    }

    public static func decodeRedeemUnsignedV4(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendRedeemUnsignedV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.redeemUnsignedWireNameV4,
            field: "redeemUnsignedV4"
        ))
        let version = try scalarUInt16(reader.field(), field: "redeemUnsignedV4.version")
        for _ in 0..<7 { _ = try reader.field() }
        let operationID = try packedFixed(
            reader.field(), count: 32, field: "redeemUnsignedV4.operationID"
        )
        try reader.finish("redeemUnsignedV4")
        guard version == KagemushaRecursiveSpend.wireVersionV4,
              operationID.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidArchive("redeemUnsignedV4.binding")
        }
        return KagemushaRecursiveSpendRedeemUnsignedV4(
            version: version,
            operationID: operationID,
            noritoArchive: archive
        )
    }

    public static func decodeRedeemBuildResultV4(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendRedeemBuildResultV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.redeemBuildResultWireNameV4,
            field: "redeemBuildResultV4"
        ))
        let unsigned = try decodeRedeemUnsignedV4(frame(
            KagemushaRecursiveSpend.redeemUnsignedWireNameV4,
            payload: reader.field()
        ))
        let authorizationDigest = try packedFixed(
            reader.field(), count: 32, field: "redeemBuildResultV4.authorizationDigest"
        )
        let changePayload = try decodeOption(
            reader.field(), field: "redeemBuildResultV4.offlineChangeBundle"
        )
        let changeWitnessPayload = try decodeOption(
            reader.field(), field: "redeemBuildResultV4.offlineChangeMembershipWitness"
        )
        let changeProvenancePayload = try decodeOption(
            reader.field(), field: "redeemBuildResultV4.offlineChangeTopUpProvenance"
        )
        let operationID = try packedFixed(
            reader.field(), count: 32, field: "redeemBuildResultV4.operationID"
        )
        try reader.finish("redeemBuildResultV4")
        guard authorizationDigest.contains(where: { $0 != 0 }),
              operationID == unsigned.operationID,
              (changePayload == nil) == (changeWitnessPayload == nil),
              (changePayload == nil) == (changeProvenancePayload == nil) else {
            throw KagemushaRecursiveSpendError.invalidArchive("redeemBuildResultV4.binding")
        }
        let change = try changePayload.map {
            try KagemushaRecursiveSpendBundleV4(noritoArchive: frame(
                KagemushaRecursiveSpend.bundleWireNameV4,
                payload: $0
            ))
        }
        let changeWitness = try changeWitnessPayload.map {
            try decodeMembershipWitness(
                frame(KagemushaRecursiveSpend.spendableMembershipWitnessWireName, payload: $0),
                schema: KagemushaRecursiveSpend.spendableMembershipWitnessWireName,
                field: "redeemBuildResultV4.offlineChangeMembershipWitness"
            )
        }
        let changeProvenance = try changeProvenancePayload.map {
            try KagemushaRecursiveSpendTopUpProvenanceV4(noritoArchive: frame(
                KagemushaRecursiveSpend.topUpProvenanceWireNameV4,
                payload: $0
            ))
        }
        return KagemushaRecursiveSpendRedeemBuildResultV4(
            unsigned: unsigned,
            authorizationDigest: authorizationDigest,
            offlineChangeBundle: change,
            offlineChangeMembershipWitness: changeWitness,
            offlineChangeTopUpProvenance: changeProvenance,
            operationID: operationID,
            noritoArchive: archive
        )
    }

    public static func decodeRedeemResultV4(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendRedeemResultV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.redeemResultWireNameV4,
            field: "redeemResultV4"
        ))
        let version = try scalarUInt16(reader.field(), field: "redeemResultV4.version")
        let requestArchive = try decodeBytes(
            reader.field(), field: "redeemResultV4.redeemRequestArchive"
        )
        let changePayload = try decodeOption(
            reader.field(), field: "redeemResultV4.offlineChangeBundle"
        )
        let changeWitnessPayload = try decodeOption(
            reader.field(), field: "redeemResultV4.offlineChangeMembershipWitness"
        )
        let changeProvenancePayload = try decodeOption(
            reader.field(), field: "redeemResultV4.offlineChangeTopUpProvenance"
        )
        let operationID = try packedFixed(
            reader.field(), count: 32, field: "redeemResultV4.operationID"
        )
        try reader.finish("redeemResultV4")
        guard version == KagemushaRecursiveSpend.wireVersionV4,
              !requestArchive.isEmpty,
              operationID.contains(where: { $0 != 0 }),
              (changePayload == nil) == (changeWitnessPayload == nil),
              (changePayload == nil) == (changeProvenancePayload == nil) else {
            throw KagemushaRecursiveSpendError.invalidArchive("redeemResultV4.binding")
        }
        let change = try changePayload.map {
            try KagemushaRecursiveSpendBundleV4(noritoArchive: frame(
                KagemushaRecursiveSpend.bundleWireNameV4,
                payload: $0
            ))
        }
        let changeWitness = try changeWitnessPayload.map {
            try decodeMembershipWitness(
                frame(KagemushaRecursiveSpend.spendableMembershipWitnessWireName, payload: $0),
                schema: KagemushaRecursiveSpend.spendableMembershipWitnessWireName,
                field: "redeemResultV4.offlineChangeMembershipWitness"
            )
        }
        let changeProvenance = try changeProvenancePayload.map {
            try KagemushaRecursiveSpendTopUpProvenanceV4(noritoArchive: frame(
                KagemushaRecursiveSpend.topUpProvenanceWireNameV4,
                payload: $0
            ))
        }
        return KagemushaRecursiveSpendRedeemResultV4(
            redeemRequestArchive: requestArchive,
            offlineChangeBundle: change,
            offlineChangeMembershipWitness: changeWitness,
            offlineChangeTopUpProvenance: changeProvenance,
            operationID: operationID,
            noritoArchive: archive
        )
    }

    private static func decodeSplitIntentV4(
        _ archive: Data
    ) throws -> KagemushaRecursiveSpendSplitIntentV4 {
        var reader = KagemushaV2Reader(try payload(
            archive,
            schema: KagemushaRecursiveSpend.splitIntentWireNameV4,
            field: "splitIntentV4"
        ))
        for _ in 0..<9 { _ = try reader.field() }
        let requestDigest = try packedFixed(
            reader.field(), count: 32, field: "splitIntentV4.recipientRequestDigest"
        )
        let operationID = try packedFixed(
            reader.field(), count: 32, field: "splitIntentV4.operationID"
        )
        try reader.finish("splitIntentV4")
        guard requestDigest.contains(where: { $0 != 0 }),
              operationID.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidArchive("splitIntentV4.binding")
        }
        return KagemushaRecursiveSpendSplitIntentV4(
            noritoArchive: archive,
            recipientRequestDigest: requestDigest,
            operationID: operationID
        )
    }

    private static func decodeArtifactBindingV4(
        _ data: Data
    ) throws -> KagemushaRecursiveSpendArtifactBindingV4 {
        var reader = KagemushaV2Reader(data)
        let version = try scalarUInt16(reader.field(), field: "artifactBindingV4.version")
        let generation = try decodeString(
            reader.field(), field: "artifactBindingV4.generation"
        )
        let digest = try packedFixed(
            reader.field(), count: 32, field: "artifactBindingV4.manifestSHA256"
        )
        try reader.finish("artifactBindingV4")
        guard version == KagemushaRecursiveSpend.wireVersionV4 else {
            throw KagemushaRecursiveSpendError.invalidArchive("artifactBindingV4.version")
        }
        return try KagemushaRecursiveSpendArtifactBindingV4(
            generation: generation,
            manifestSHA256: digest
        )
    }

    private static func artifactBindingV4(
        _ value: KagemushaRecursiveSpendArtifactBindingV4
    ) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(uint16(value.version))
        writer.writeField(string(value.generation))
        writer.writeField(value.manifestSHA256)
        return writer.data
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
    private var data: Data
    private(set) var offset = 0

    init(_ data: Data) {
        self.data = data
    }

    var isEmpty: Bool { offset == data.count }

    mutating func wipe() {
        data.resetBytes(in: 0..<data.count)
        data.removeAll(keepingCapacity: false)
        offset = 0
    }

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

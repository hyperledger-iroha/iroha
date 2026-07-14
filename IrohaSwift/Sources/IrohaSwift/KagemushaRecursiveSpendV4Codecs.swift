import Foundation

/// Canonical Norito encoders for the distinct ABI-20 local lifecycle.
/// These functions construct the V4 bridge structs field-for-field; no older
/// request, bundle, result, or local carrier is nested or reinterpreted.
enum KagemushaRecursiveSpendCodecsV4 {
    static func encodeArtifactBinding(
        _ binding: KagemushaRecursiveSpendArtifactBindingV4
    ) -> Data {
        frame(
            KagemushaRecursiveSpend.artifactBindingWireNameV4,
            payload: artifactBinding(binding)
        )
    }

    static func encodeTopUpFinalityEvidence(
        topUpAnchor: KagemushaRecursiveSpendTopUpAnchorV4,
        topUpFinalityProof: KagemushaTopUpFinalityProofArchive
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try nestedPayload(
            topUpAnchor.noritoArchive,
            schema: KagemushaRecursiveSpend.topUpAnchorWireNameV4,
            field: "topUpFinalityEvidenceV4.topUpAnchor"
        ))
        writer.writeField(try nestedPayload(
            topUpFinalityProof.noritoArchive,
            schema: KagemushaRecursiveSpend.topUpFinalityProofWireName,
            field: "topUpFinalityEvidenceV4.topUpFinalityProof"
        ))
        return frame(
            KagemushaRecursiveSpend.topUpFinalityEvidenceWireNameV4,
            payload: writer.data
        )
    }

    static func encodeInitRequest(
        _ request: KagemushaRecursiveSpendInitRequestV4
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.topUpAnchor.noritoArchive,
            schema: KagemushaRecursiveSpend.topUpAnchorWireNameV4,
            field: "initRequestV4.topUpAnchor"
        ))
        writer.writeField(try nestedPayload(
            request.topUpFinalityProof.noritoArchive,
            schema: KagemushaRecursiveSpend.topUpFinalityProofWireName,
            field: "initRequestV4.topUpFinalityProof"
        ))
        writer.writeField(try nestedPayload(
            request.topUpFinalityRosterArtifact.noritoArchive,
            schema: KagemushaRecursiveSpend.topUpFinalityRosterArtifactWireName,
            field: "initRequestV4.topUpFinalityRosterArtifact"
        ))
        writer.writeField(artifactBinding(request.artifactBinding))
        return frame(KagemushaRecursiveSpend.initRequestWireNameV4, payload: writer.data)
    }

    static func encodeInitLocalRequest(
        _ request: KagemushaRecursiveSpendInitLocalRequestV4
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(uint16(KagemushaRecursiveSpend.localWitnessVersionV4))
        writer.writeField(try nestedPayload(
            encodeInitRequest(request.request),
            schema: KagemushaRecursiveSpend.initRequestWireNameV4,
            field: "initLocalRequestV4.request"
        ))
        writer.writeField(try noteOpening(request.opening, field: "initLocalRequestV4.opening"))
        writer.writeField(try outputMembership(request.outputMembershipPaths))
        return frame(
            KagemushaRecursiveSpend.initLocalRequestWireNameV4,
            payload: writer.data
        )
    }

    static func encodeAppendLocalRequest(
        _ request: KagemushaRecursiveSpendAppendLocalRequestV4
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(uint16(KagemushaRecursiveSpend.localWitnessVersionV4))
        writer.writeField(try sequence(request.previousInputs.map {
            try appendInput($0)
        }))
        writer.writeField(try sequence(request.inputOpenings.enumerated().map {
            try noteOpening($0.element, field: "appendLocalRequestV4.inputOpenings[\($0.offset)]")
        }))
        writer.writeField(try sequence(request.inputMembershipWitnesses.enumerated().map {
            try membershipWitness(
                $0.element,
                field: "appendLocalRequestV4.inputMembershipWitnesses[\($0.offset)]"
            )
        }))
        writer.writeField(option(try request.changeOpening.map {
            try noteOpening($0, field: "appendLocalRequestV4.changeOpening")
        }))
        writer.writeField(artifactBinding(request.outputArtifactBinding))
        writer.writeField(verifierKeyID(
            backend: request.transferVerifier.backend,
            name: request.transferVerifier.name
        ))
        writer.writeField(request.transferVerifier.commitment)
        writer.writeField(request.operationID)
        writer.writeField(uint64(request.blockHeight))
        writer.writeField(try outputMembership(request.outputMembershipPaths))
        return frame(
            KagemushaRecursiveSpend.appendLocalRequestWireNameV4,
            payload: writer.data
        )
    }

    static func encodeVerifyRequest(
        _ request: KagemushaRecursiveSpendVerifyRequestV4
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try nestedPayload(
            request.bundle.noritoArchive,
            schema: KagemushaRecursiveSpend.bundleWireNameV4,
            field: "verifyRequestV4.bundle"
        ))
        writer.writeField(try nestedPayload(
            request.recipientRequest.archive,
            schema: KagemushaRecursiveSpend.recipientRequestWireName,
            field: "verifyRequestV4.recipientRequest"
        ))
        writer.writeField(try nestedPayload(
            request.topUpProvenance.noritoArchive,
            schema: KagemushaRecursiveSpend.topUpProvenanceWireNameV4,
            field: "verifyRequestV4.topUpProvenance"
        ))
        writer.writeField(uint32(request.maximumHops))
        writer.writeField(artifactBinding(request.artifactBinding))
        writer.writeField(uint64(request.blockHeight))
        writer.writeField(uint64(request.verifiedAtMilliseconds))
        return frame(KagemushaRecursiveSpend.verifyRequestWireNameV4, payload: writer.data)
    }

    static func encodeVerifyLocalRequest(
        _ request: KagemushaRecursiveSpendVerifyLocalRequestV4
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(uint16(KagemushaRecursiveSpend.localWitnessVersionV4))
        writer.writeField(try nestedPayload(
            encodeVerifyRequest(request.request),
            schema: KagemushaRecursiveSpend.verifyRequestWireNameV4,
            field: "verifyLocalRequestV4.request"
        ))
        return frame(
            KagemushaRecursiveSpend.verifyLocalRequestWireNameV4,
            payload: writer.data
        )
    }

    static func encodeRedeemLocalRequest(
        _ request: KagemushaRecursiveSpendRedeemLocalRequestV4
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(uint16(KagemushaRecursiveSpend.localWitnessVersionV4))
        writer.writeField(try nestedPayload(
            request.input.bundle.noritoArchive,
            schema: KagemushaRecursiveSpend.bundleWireNameV4,
            field: "redeemLocalRequestV4.bundle"
        ))
        writer.writeField(try nestedPayload(
            request.input.topUpProvenance.noritoArchive,
            schema: KagemushaRecursiveSpend.topUpProvenanceWireNameV4,
            field: "redeemLocalRequestV4.topUpProvenance"
        ))
        writer.writeField(try noteOpening(
            request.input.opening,
            field: "redeemLocalRequestV4.inputOpening"
        ))
        writer.writeField(try membershipWitness(
            request.input.membershipWitness,
            field: "redeemLocalRequestV4.inputMembershipWitness"
        ))
        writer.writeField(try accountID(request.recipient))
        writer.writeField(try scaledAmount(request.publicAmount))
        writer.writeField(option(try request.changeOpening.map {
            try noteOpening($0, field: "redeemLocalRequestV4.changeOpening")
        }))
        writer.writeField(verifierKeyID(
            backend: request.unshieldVerifier.backend,
            name: request.unshieldVerifier.name
        ))
        writer.writeField(request.unshieldVerifier.commitment)
        writer.writeField(uint64(request.blockHeight))
        writer.writeField(request.operationID)
        writer.writeField(option(try request.changeOutputMembershipPaths.map {
            try outputMembership($0)
        }))
        return frame(
            KagemushaRecursiveSpend.redeemLocalRequestWireNameV4,
            payload: writer.data
        )
    }

    private static func appendInput(
        _ input: KagemushaRecursiveSpendAppendInputV4
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try nestedPayload(
            input.previousBundle.noritoArchive,
            schema: KagemushaRecursiveSpend.bundleWireNameV4,
            field: "appendInputV4.previousBundle"
        ))
        writer.writeField(try nestedPayload(
            input.topUpProvenance.noritoArchive,
            schema: KagemushaRecursiveSpend.topUpProvenanceWireNameV4,
            field: "appendInputV4.topUpProvenance"
        ))
        return writer.data
    }

    private static func artifactBinding(
        _ value: KagemushaRecursiveSpendArtifactBindingV4
    ) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(uint16(KagemushaRecursiveSpend.wireVersionV4))
        writer.writeField(CompactNorito.encodeString(value.generation))
        writer.writeField(value.manifestSHA256)
        return writer.data
    }

    private static func noteOpening(
        _ opening: KagemushaNoteOpening,
        field: String
    ) throws -> Data {
        try nestedPayload(
            KagemushaRecursiveSpendCodecs.encodeNoteOpening(opening),
            schema: KagemushaRecursiveSpend.noteOpeningWireName,
            field: field
        )
    }

    private static func membershipWitness(
        _ witness: KagemushaNoteMembershipWitness,
        field: String
    ) throws -> Data {
        try nestedPayload(
            KagemushaRecursiveSpendCodecs.encodeMembershipWitness(witness),
            schema: KagemushaRecursiveSpend.membershipWitnessWireName,
            field: field
        )
    }

    private static func outputMembership(
        _ paths: KagemushaOutputMembershipPathsV4
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(paths.initialRoot)
        writer.writeField(paths.finalRoot)
        writer.writeField(option(try paths.recipient.map {
            try outputMembershipLeaf($0)
        }))
        writer.writeField(option(try paths.change.map {
            try outputMembershipLeaf($0)
        }))
        writer.writeField(uint32(paths.dummyLeafIndex))
        writer.writeField(try membershipPath(paths.dummyPath))
        return writer.data
    }

    private static func outputMembershipLeaf(
        _ leaf: KagemushaOutputMembershipLeafPathsV4
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(uint32(leaf.leafIndex))
        writer.writeField(try membershipPath(leaf.updatePath))
        writer.writeField(try membershipPath(leaf.membershipPath))
        return writer.data
    }

    private static func membershipPath(
        _ path: PrivacyConfidentialMerklePathWitnessV2
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try sequence(path.siblings.map { constVector($0) }))
        writer.writeField(bytes(path.directions))
        writer.writeField(path.root)
        return writer.data
    }

    private static func verifierKeyID(backend: String, name: String) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeString(backend))
        writer.writeField(CompactNorito.encodeString(name))
        return writer.data
    }

    private static func accountID(_ value: String) throws -> Data {
        do {
            return try AccountAddress.parseEncoded(value, expectedPrefix: 0x02F1)
                .compactNoritoAccountControllerPayload()
        } catch {
            throw KagemushaRecursiveSpendError.invalidField("redeemLocalRequestV4.recipient")
        }
    }

    private static func scaledAmount(_ amount: KagemushaScaledAmount) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try unsigned128(amount.atomicUnits))
        writer.writeField(uint32(amount.scale))
        return writer.data
    }

    private static func unsigned128(_ value: String) throws -> Data {
        var digits = value.compactMap(\.wholeNumberValue)
        guard !digits.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidField("scaledAmountV4.atomicUnits")
        }
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
            throw KagemushaRecursiveSpendError.invalidField("scaledAmountV4.atomicUnits")
        }
        output.append(contentsOf: repeatElement(UInt8(0), count: 16 - output.count))
        return output
    }

    private static func sequence(_ values: [Data]) throws -> Data {
        guard values.count <= Int(UInt32.max) else {
            throw KagemushaRecursiveSpendError.invalidField("v4.sequence")
        }
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        for value in values { writer.writeField(value) }
        return writer.data
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

    private static func bytes(_ value: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(value.count))
        writer.writeBytes(value)
        return writer.data
    }

    private static func constVector(_ value: Data) -> Data {
        var writer = CompactNoritoWriter()
        for byte in value {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
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

    private static func nestedPayload(
        _ archive: Data,
        schema: String,
        field: String
    ) throws -> Data {
        try KagemushaRecursiveSpend.requireArchive(archive, schema: schema, field: field)
        guard let decoded = noritoDecodeFrame(archive) else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
        return decoded.payload
    }

    private static func frame(_ schema: String, payload: Data) -> Data {
        KagemushaRecursiveSpend.frameArchive(schema: schema, payload: payload)
    }
}

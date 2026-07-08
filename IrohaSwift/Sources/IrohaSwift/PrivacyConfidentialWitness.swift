import Foundation

public enum PrivacyConfidentialWitnessError: Error, Equatable, LocalizedError {
    case invalidField(String)
    case invalidArchive(String)

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid privacy confidential witness field: \(field)."
        case let .invalidArchive(field):
            return "Invalid privacy confidential witness archive: \(field)."
        }
    }
}

public struct PrivacyConfidentialNoteWitnessV1: Equatable, Sendable {
    public let amount: String
    public let rho: Data
    public let diversifier: Data
    public let leafIndex: UInt64

    public init(amount: String, rho: Data, diversifier: Data, leafIndex: UInt64) throws {
        self.amount = try PrivacyConfidentialWitnessCodecs.canonicalU128(amount, field: "amount")
        self.rho = try PrivacyConfidentialWitnessCodecs.fixed32(rho, field: "rho")
        self.diversifier = try PrivacyConfidentialWitnessCodecs.fixed32(
            diversifier,
            field: "diversifier"
        )
        self.leafIndex = leafIndex
    }
}

public struct PrivacyConfidentialTransferOutputWitnessV1: Equatable, Sendable {
    public let amount: String
    public let rho: Data
    public let ownerTag: Data

    public init(amount: String, rho: Data, ownerTag: Data) throws {
        self.amount = try PrivacyConfidentialWitnessCodecs.canonicalU128(amount, field: "amount")
        self.rho = try PrivacyConfidentialWitnessCodecs.fixed32(rho, field: "rho")
        self.ownerTag = try PrivacyConfidentialWitnessCodecs.fixed32(ownerTag, field: "ownerTag")
    }
}

public struct PrivacyConfidentialUnshieldChangeWitnessV1: Equatable, Sendable {
    public let amount: String
    public let rho: Data

    public init(amount: String, rho: Data) throws {
        self.amount = try PrivacyConfidentialWitnessCodecs.canonicalU128(amount, field: "amount")
        self.rho = try PrivacyConfidentialWitnessCodecs.fixed32(rho, field: "rho")
    }
}

public struct PrivacyConfidentialWitnessV1: Equatable, Sendable {
    public let chainId: String
    public let assetDefinitionId: String
    public let spendKey: Data
    public let treeCommitments: [Data]
    public let inputs: [PrivacyConfidentialNoteWitnessV1]
    public let transferOutputs: [PrivacyConfidentialTransferOutputWitnessV1]
    public let unshieldChange: [PrivacyConfidentialUnshieldChangeWitnessV1]
    public let publicAmount: String
    public let rootHint: Data

    public init(
        chainId: String,
        assetDefinitionId: String,
        spendKey: Data,
        treeCommitments: [Data],
        inputs: [PrivacyConfidentialNoteWitnessV1],
        transferOutputs: [PrivacyConfidentialTransferOutputWitnessV1],
        unshieldChange: [PrivacyConfidentialUnshieldChangeWitnessV1],
        publicAmount: String,
        rootHint: Data
    ) throws {
        self.chainId = try PrivacyConfidentialWitnessCodecs.canonicalText(
            chainId,
            field: "chainId"
        )
        self.assetDefinitionId = try PrivacyConfidentialWitnessCodecs.canonicalText(
            assetDefinitionId,
            field: "assetDefinitionId"
        )
        self.spendKey = try PrivacyConfidentialWitnessCodecs.fixed32(spendKey, field: "spendKey")
        self.treeCommitments = try treeCommitments.enumerated().map { index, value in
            try PrivacyConfidentialWitnessCodecs.fixed32(
                value,
                field: "treeCommitments[\(index)]"
            )
        }
        self.inputs = inputs
        self.transferOutputs = transferOutputs
        self.unshieldChange = unshieldChange
        self.publicAmount = try PrivacyConfidentialWitnessCodecs.canonicalU128(
            publicAmount,
            field: "publicAmount"
        )
        self.rootHint = try PrivacyConfidentialWitnessCodecs.fixed32(rootHint, field: "rootHint")

        guard !self.treeCommitments.isEmpty,
              self.treeCommitments.count <= PrivacyConfidentialWitnessCodecs.confidentialTreeCapacityV2
        else {
            throw PrivacyConfidentialWitnessError.invalidField("treeCommitments")
        }
        guard (1...PrivacyConfidentialWitnessCodecs.confidentialMaxInputsV2).contains(inputs.count) else {
            throw PrivacyConfidentialWitnessError.invalidField("inputs")
        }
        guard transferOutputs.count <= PrivacyConfidentialWitnessCodecs.confidentialMaxTransferOutputsV2 else {
            throw PrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard unshieldChange.count <= PrivacyConfidentialWitnessCodecs.confidentialMaxUnshieldChangeOutputsV3 else {
            throw PrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
        guard transferOutputs.isEmpty || unshieldChange.isEmpty else {
            throw PrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard transferOutputs.isEmpty || self.publicAmount == "0" else {
            throw PrivacyConfidentialWitnessError.invalidField("publicAmount")
        }
        var seenLeafIndexes = Set<UInt64>()
        var seenRhos = Set<Data>()
        for (index, input) in inputs.enumerated() {
            guard input.leafIndex < UInt64(self.treeCommitments.count) else {
                throw PrivacyConfidentialWitnessError.invalidField("inputs[\(index)].leafIndex")
            }
            guard seenLeafIndexes.insert(input.leafIndex).inserted else {
                throw PrivacyConfidentialWitnessError.invalidField("inputs[\(index)].leafIndex")
            }
            guard seenRhos.insert(input.rho).inserted else {
                throw PrivacyConfidentialWitnessError.invalidField("inputs[\(index)].rho")
            }
        }
    }
}

public enum PrivacyConfidentialWitnessCodecs {
    public static let privacyConfidentialWitnessV1WireName =
        "connect_norito_bridge::privacy_production::PrivacyConfidentialWitnessV1"
    public static let privacyProofRequestV1WireName =
        "connect_norito_bridge::PrivacyProofRequestV1"
    public static let confidentialTransferV2AlgorithmId = "confidential-transfer-v2"
    public static let confidentialTransferV2Entrypoint = "buildConfidentialTransferProofV2"
    public static let confidentialTransferV2VerifierRef =
        "halo2-ipa-pasta:confidential_transfer_v2"
    public static let confidentialTreeCapacityV2 = 1 << 16
    public static let confidentialMaxInputsV2 = 2
    public static let confidentialMaxTransferOutputsV2 = 2
    public static let confidentialMaxUnshieldChangeOutputsV3 = 1

    static let requestFlags = NoritoHeader.compactLen
    private static let proofMaxBytes = 32 * 1024 * 1024
    private static let witnessHeaderPaddingBytes = 8
    private static let privacyRequestSchemaByte: UInt8 = 0x52

    static let confidentialTransferPublicInputsSchemaV1 = Data(
        (
            "{\"schema\":\"confidential_transfer_v2\",\"public_inputs\":[\"input_commitment_0\"," +
                "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"output_commitment_0\"," +
                "\"output_commitment_1\",\"root\",\"asset_tag\",\"chain_tag\"]}"
        ).utf8
    )

    public static func confidentialTransferPublicInputsSchema() -> Data {
        confidentialTransferPublicInputsSchemaV1
    }

    public static func encodeWitness(_ witness: PrivacyConfidentialWitnessV1) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(witness.chainId))
        writer.writeField(OfflineCompactNorito.encodeString(witness.assetDefinitionId))
        writer.writeField(encodeBytesVec(witness.spendKey))
        writer.writeField(try encodeSequence(witness.treeCommitments, encodeBytesVec))
        writer.writeField(try encodeSequence(witness.inputs, encodeNoteWitness))
        writer.writeField(try encodeSequence(witness.transferOutputs, encodeTransferOutput))
        writer.writeField(try encodeSequence(witness.unshieldChange, encodeUnshieldChange))
        writer.writeField(try ConfidentialNoteCrypto.u128LittleEndianBytes(witness.publicAmount))
        writer.writeField(encodeBytesVec(witness.rootHint))
        let archive = noritoEncode(
            typeName: privacyConfidentialWitnessV1WireName,
            payload: writer.data,
            flags: requestFlags
        )
        return try addHeaderPadding(archive, bytes: witnessHeaderPaddingBytes)
    }

    public static func encodeTransferWitness(
        _ witness: PrivacyConfidentialWitnessV1
    ) throws -> Data {
        try validateTransferWitness(witness)
        return try encodeWitness(witness)
    }

    public static func buildConfidentialTransferProofRequestV1(
        witness: PrivacyConfidentialWitnessV1,
        vkRef: String = confidentialTransferV2VerifierRef
    ) throws -> Data {
        try validateVkRef(vkRef, expected: confidentialTransferV2VerifierRef)
        return try encodePrivacyProofRequest(
            algorithmId: confidentialTransferV2AlgorithmId,
            entrypoint: confidentialTransferV2Entrypoint,
            vkRef: vkRef,
            publicInputs: confidentialTransferPublicInputsSchemaV1,
            witness: encodeTransferWitness(witness),
            proof: Data()
        )
    }

    static func canonicalText(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed == value, !trimmed.contains("\0") else {
            throw PrivacyConfidentialWitnessError.invalidField(field)
        }
        return trimmed
    }

    static func canonicalU128(_ value: String, field: String) throws -> String {
        do {
            return try ConfidentialNoteCrypto.canonicalU128(value, field: field)
        } catch {
            throw PrivacyConfidentialWitnessError.invalidField(field)
        }
    }

    static func fixed32(_ value: Data, field: String) throws -> Data {
        guard value.count == 32 else {
            throw PrivacyConfidentialWitnessError.invalidField(field)
        }
        return Data(value)
    }

    private static func validateTransferWitness(_ witness: PrivacyConfidentialWitnessV1) throws {
        guard witness.publicAmount == "0" else {
            throw PrivacyConfidentialWitnessError.invalidField("publicAmount")
        }
        guard witness.unshieldChange.isEmpty else {
            throw PrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
        guard (1...confidentialMaxTransferOutputsV2).contains(witness.transferOutputs.count) else {
            throw PrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
    }

    private static func validateVkRef(_ value: String, expected: String) throws {
        let text = try privacyRequestText(value, field: "vkRef")
        guard text == expected else {
            throw PrivacyConfidentialWitnessError.invalidField("vkRef")
        }
    }

    private static func privacyRequestText(_ value: String, field: String) throws -> String {
        let text = try canonicalText(value, field: field)
        guard text.count <= 1024,
              text.unicodeScalars.allSatisfy({ scalar in
                  scalar.value >= 0x21 && scalar.value <= 0x7e
              }),
              text.allSatisfy({ character in
                  character.isLetter || character.isNumber
                      || character == "-"
                      || character == "_"
                      || character == "."
                      || character == ":"
              })
        else {
            throw PrivacyConfidentialWitnessError.invalidField(field)
        }
        return text
    }

    private static func encodePrivacyProofRequest(
        algorithmId: String,
        entrypoint: String,
        vkRef: String,
        publicInputs: Data,
        witness: Data,
        proof: Data
    ) throws -> Data {
        guard proof.count <= proofMaxBytes else {
            throw PrivacyConfidentialWitnessError.invalidField("proof")
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(try privacyRequestText(
            algorithmId,
            field: "algorithmId"
        )))
        writer.writeField(OfflineCompactNorito.encodeString(try privacyRequestText(
            entrypoint,
            field: "entrypoint"
        )))
        writer.writeField(OfflineCompactNorito.encodeString(try privacyRequestText(vkRef, field: "vkRef")))
        writer.writeField(encodeBytesVec(publicInputs))
        writer.writeField(encodeBytesVec(witness))
        writer.writeField(encodeBytesVec(proof))
        var archive = noritoEncode(
            typeName: privacyProofRequestV1WireName,
            payload: writer.data,
            flags: requestFlags
        )
        guard archive.count >= NoritoHeader.encodedLength else {
            throw PrivacyConfidentialWitnessError.invalidArchive("privacyProofRequest")
        }
        archive.replaceSubrange(6..<22, with: Data(repeating: privacyRequestSchemaByte, count: 16))
        return archive
    }

    private static func encodeNoteWitness(_ note: PrivacyConfidentialNoteWitnessV1) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try ConfidentialNoteCrypto.u128LittleEndianBytes(note.amount))
        writer.writeField(encodeBytesVec(note.rho))
        writer.writeField(encodeBytesVec(note.diversifier))
        writer.writeField(OfflineCompactNorito.encodeUInt64(note.leafIndex))
        return writer.data
    }

    private static func encodeTransferOutput(
        _ output: PrivacyConfidentialTransferOutputWitnessV1
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try ConfidentialNoteCrypto.u128LittleEndianBytes(output.amount))
        writer.writeField(encodeBytesVec(output.rho))
        writer.writeField(encodeBytesVec(output.ownerTag))
        return writer.data
    }

    private static func encodeUnshieldChange(
        _ change: PrivacyConfidentialUnshieldChangeWitnessV1
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try ConfidentialNoteCrypto.u128LittleEndianBytes(change.amount))
        writer.writeField(encodeBytesVec(change.rho))
        return writer.data
    }

    private static func encodeSequence<T>(
        _ values: [T],
        _ encode: (T) throws -> Data
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        for value in values {
            writer.writeField(try encode(value))
        }
        return writer.data
    }

    private static func encodeBytesVec(_ bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    private static func addHeaderPadding(_ archive: Data, bytes: Int) throws -> Data {
        guard archive.count >= NoritoHeader.encodedLength else {
            throw PrivacyConfidentialWitnessError.invalidArchive("witness")
        }
        var out = Data()
        out.reserveCapacity(archive.count + bytes)
        out.append(archive.prefix(NoritoHeader.encodedLength))
        out.append(Data(repeating: 0, count: bytes))
        out.append(archive.dropFirst(NoritoHeader.encodedLength))
        return out
    }
}

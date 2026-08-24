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
    public let networkId: NetworkId
    public let assetDefinitionId: String
    public let spendKey: Data
    public let treeCommitments: [Data]
    public let inputs: [PrivacyConfidentialNoteWitnessV1]
    public let transferOutputs: [PrivacyConfidentialTransferOutputWitnessV1]
    public let unshieldChange: [PrivacyConfidentialUnshieldChangeWitnessV1]
    public let publicAmount: String
    public let rootHint: Data

    public init(
        networkId: NetworkId,
        assetDefinitionId: String,
        spendKey: Data,
        treeCommitments: [Data],
        inputs: [PrivacyConfidentialNoteWitnessV1],
        transferOutputs: [PrivacyConfidentialTransferOutputWitnessV1],
        unshieldChange: [PrivacyConfidentialUnshieldChangeWitnessV1],
        publicAmount: String,
        rootHint: Data
    ) throws {
        self.networkId = networkId
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

/// Bounded confidential-v2 Merkle path supplied by Torii.
///
/// The path deliberately omits intermediate witness nodes because the native
/// prover recomputes and constrains them. This keeps an online preparation
/// proportional to the fixed tree depth instead of the complete frontier.
public struct PrivacyConfidentialMerklePathWitnessV2: Equatable, Sendable {
    public let siblings: [Data]
    public let directions: Data
    public let root: Data

    public init(siblings: [Data], directions: Data, root: Data) throws {
        guard siblings.count == PrivacyConfidentialWitnessCodecs.confidentialTreeDepthV2,
              directions.count == siblings.count else {
            throw PrivacyConfidentialWitnessError.invalidField("merklePath.depth")
        }
        self.siblings = try siblings.enumerated().map { index, value in
            try PrivacyConfidentialWitnessCodecs.fixed32(
                value,
                field: "merklePath.siblings[\(index)]"
            )
        }
        guard directions.allSatisfy({ $0 == 0 || $0 == 1 }) else {
            throw PrivacyConfidentialWitnessError.invalidField("merklePath.directions")
        }
        self.directions = Data(directions)
        self.root = try PrivacyConfidentialWitnessCodecs.fixed32(root, field: "merklePath.root")
    }

    public init(path: ZkAssetMerklePath) throws {
        try self.init(
            siblings: path.siblings,
            directions: path.directions,
            root: path.rootAtHeight
        )
    }
}

/// Path-based privacy witness used by first-release Kagemusha lifecycle calls.
///
/// Exactly two paths are carried because the transfer and unshield circuits
/// always expose two input slots. For a one-input proof the second path is the
/// authoritative `next_zero_path` returned by `POST /v1/zk/merkle-path`.
public struct PrivacyConfidentialWitnessV2: Equatable, Sendable {
    public let networkId: NetworkId
    public let assetDefinitionId: String
    public let spendKey: Data
    public let inputPaths: [PrivacyConfidentialMerklePathWitnessV2]
    public let inputs: [PrivacyConfidentialNoteWitnessV1]
    public let transferOutputs: [PrivacyConfidentialTransferOutputWitnessV1]
    public let unshieldChange: [PrivacyConfidentialUnshieldChangeWitnessV1]
    public let publicAmount: String
    public let rootHint: Data

    public init(
        networkId: NetworkId,
        assetDefinitionId: String,
        spendKey: Data,
        inputPaths: [PrivacyConfidentialMerklePathWitnessV2],
        inputs: [PrivacyConfidentialNoteWitnessV1],
        transferOutputs: [PrivacyConfidentialTransferOutputWitnessV1],
        unshieldChange: [PrivacyConfidentialUnshieldChangeWitnessV1],
        publicAmount: String,
        rootHint: Data
    ) throws {
        self.networkId = networkId
        self.assetDefinitionId = try PrivacyConfidentialWitnessCodecs.canonicalText(
            assetDefinitionId,
            field: "assetDefinitionId"
        )
        self.spendKey = try PrivacyConfidentialWitnessCodecs.fixed32(
            spendKey,
            field: "spendKey"
        )
        guard inputPaths.count == 2 else {
            throw PrivacyConfidentialWitnessError.invalidField("inputPaths")
        }
        self.inputPaths = inputPaths
        self.inputs = inputs
        self.transferOutputs = transferOutputs
        self.unshieldChange = unshieldChange
        self.publicAmount = try PrivacyConfidentialWitnessCodecs.canonicalU128(
            publicAmount,
            field: "publicAmount"
        )
        self.rootHint = try PrivacyConfidentialWitnessCodecs.fixed32(
            rootHint,
            field: "rootHint"
        )

        guard inputPaths.allSatisfy({ $0.root == self.rootHint }) else {
            throw PrivacyConfidentialWitnessError.invalidField("inputPaths.root")
        }
        guard (1...PrivacyConfidentialWitnessCodecs.confidentialMaxInputsV2)
            .contains(inputs.count) else {
            throw PrivacyConfidentialWitnessError.invalidField("inputs")
        }
        guard transferOutputs.count
            <= PrivacyConfidentialWitnessCodecs.confidentialMaxTransferOutputsV2 else {
            throw PrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard unshieldChange.count
            <= PrivacyConfidentialWitnessCodecs.confidentialMaxUnshieldChangeOutputsV3 else {
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
            guard input.leafIndex
                    < UInt64(PrivacyConfidentialWitnessCodecs.confidentialTreeCapacityV2),
                  seenLeafIndexes.insert(input.leafIndex).inserted,
                  seenRhos.insert(input.rho).inserted else {
                throw PrivacyConfidentialWitnessError.invalidField("inputs[\(index)]")
            }
        }
    }
}

public enum PrivacyConfidentialWitnessCodecs {
    public static let privacyConfidentialWitnessV1WireName =
        "connect_norito_bridge::privacy_production::PrivacyConfidentialWitnessV1"
    public static let privacyConfidentialWitnessV2WireName =
        "connect_norito_bridge::privacy_production::PrivacyConfidentialWitnessV2"
    public static let confidentialTreeCapacityV2 = 1 << 16
    public static let confidentialTreeDepthV2 = 16
    public static let confidentialMaxInputsV2 = 2
    public static let confidentialMaxTransferOutputsV2 = 2
    public static let confidentialMaxUnshieldChangeOutputsV3 = 1

    static let requestFlags = NoritoHeader.compactLen
    private static let witnessHeaderPaddingBytes = 8

    static let confidentialTransferPublicInputsSchemaV1 = Data(
        (
            "{\"schema\":\"confidential_transfer_v2\",\"public_inputs\":[\"input_commitment_0\"," +
                "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"output_commitment_0\"," +
                "\"output_commitment_1\",\"root\",\"asset_tag\",\"network_tag\"]}"
        ).utf8
    )
    static let confidentialUnshieldPublicInputsSchemaV1 = Data(
        (
            "{\"schema\":\"confidential_unshield_v3\",\"public_inputs\":[\"input_commitment_0\"," +
                "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"change_commitment_0\"," +
                "\"root\",\"public_amount\",\"asset_tag\",\"network_tag\"]}"
        ).utf8
    )

    public static func confidentialTransferPublicInputsSchema() -> Data {
        confidentialTransferPublicInputsSchemaV1
    }

    public static func confidentialUnshieldPublicInputsSchema() -> Data {
        confidentialUnshieldPublicInputsSchemaV1
    }

    public static func encodeWitness(_ witness: PrivacyConfidentialWitnessV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(witness.networkId.bytes)
        writer.writeField(CompactNorito.encodeString(witness.assetDefinitionId))
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

    public static func encodeWitnessV2(_ witness: PrivacyConfidentialWitnessV2) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(witness.networkId.bytes)
        writer.writeField(CompactNorito.encodeString(witness.assetDefinitionId))
        writer.writeField(encodeBytesVec(witness.spendKey))
        writer.writeField(try encodeSequence(witness.inputPaths, encodeMerklePathV2))
        writer.writeField(try encodeSequence(witness.inputs, encodeNoteWitness))
        writer.writeField(try encodeSequence(witness.transferOutputs, encodeTransferOutput))
        writer.writeField(try encodeSequence(witness.unshieldChange, encodeUnshieldChange))
        writer.writeField(try ConfidentialNoteCrypto.u128LittleEndianBytes(witness.publicAmount))
        writer.writeField(encodeBytesVec(witness.rootHint))
        let archive = noritoEncode(
            typeName: privacyConfidentialWitnessV2WireName,
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

    public static func encodeUnshieldWitness(
        _ witness: PrivacyConfidentialWitnessV1
    ) throws -> Data {
        try validateUnshieldWitness(witness)
        return try encodeWitness(witness)
    }

    public static func encodeTransferWitnessV2(
        _ witness: PrivacyConfidentialWitnessV2
    ) throws -> Data {
        try validateTransferWitnessV2(witness)
        return try encodeWitnessV2(witness)
    }

    public static func encodeUnshieldWitnessV2(
        _ witness: PrivacyConfidentialWitnessV2
    ) throws -> Data {
        try validateUnshieldWitnessV2(witness)
        return try encodeWitnessV2(witness)
    }

    static func canonicalText(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed == value, !trimmed.contains("\0") else {
            throw PrivacyConfidentialWitnessError.invalidField(field)
        }
        return trimmed
    }

    static func canonicalU128(_ value: String, field: String) throws -> String {
        // Privacy transfer witnesses use zero as the public-amount sentinel;
        // confidential note openings intentionally require positive amounts.
        let text = try canonicalText(value, field: field)
        guard text.allSatisfy({ $0 >= "0" && $0 <= "9" }) else {
            throw PrivacyConfidentialWitnessError.invalidField(field)
        }
        guard text == "0" || !text.hasPrefix("0") else {
            throw PrivacyConfidentialWitnessError.invalidField(field)
        }
        let max = "340282366920938463463374607431768211455"
        guard text.count < max.count || (text.count == max.count && text <= max) else {
            throw PrivacyConfidentialWitnessError.invalidField(field)
        }
        return text
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

    private static func validateUnshieldWitness(_ witness: PrivacyConfidentialWitnessV1) throws {
        guard witness.transferOutputs.isEmpty else {
            throw PrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard witness.unshieldChange.count <= confidentialMaxUnshieldChangeOutputsV3 else {
            throw PrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
    }

    private static func validateTransferWitnessV2(
        _ witness: PrivacyConfidentialWitnessV2
    ) throws {
        guard witness.publicAmount == "0" else {
            throw PrivacyConfidentialWitnessError.invalidField("publicAmount")
        }
        guard witness.unshieldChange.isEmpty else {
            throw PrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
        guard (1...confidentialMaxTransferOutputsV2).contains(witness.transferOutputs.count)
        else {
            throw PrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
    }

    private static func validateUnshieldWitnessV2(
        _ witness: PrivacyConfidentialWitnessV2
    ) throws {
        guard witness.transferOutputs.isEmpty else {
            throw PrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard witness.unshieldChange.count <= confidentialMaxUnshieldChangeOutputsV3 else {
            throw PrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
    }

    private static func encodeNoteWitness(_ note: PrivacyConfidentialNoteWitnessV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try ConfidentialNoteCrypto.u128LittleEndianBytes(note.amount))
        writer.writeField(encodeBytesVec(note.rho))
        writer.writeField(encodeBytesVec(note.diversifier))
        writer.writeField(CompactNorito.encodeUInt64(note.leafIndex))
        return writer.data
    }

    private static func encodeMerklePathV2(
        _ path: PrivacyConfidentialMerklePathWitnessV2
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try encodeSequence(path.siblings, encodeBytesVec))
        writer.writeField(encodeBytesVec(path.directions))
        // Native recomputes these nodes and rejects inconsistent supplied
        // nodes, so the compact wallet wire intentionally sends an empty list.
        writer.writeField(try encodeSequence([Data](), encodeBytesVec))
        writer.writeField(encodeBytesVec(path.root))
        return writer.data
    }

    private static func encodeTransferOutput(
        _ output: PrivacyConfidentialTransferOutputWitnessV1
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try ConfidentialNoteCrypto.u128LittleEndianBytes(output.amount))
        writer.writeField(encodeBytesVec(output.rho))
        writer.writeField(encodeBytesVec(output.ownerTag))
        return writer.data
    }

    private static func encodeUnshieldChange(
        _ change: PrivacyConfidentialUnshieldChangeWitnessV1
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try ConfidentialNoteCrypto.u128LittleEndianBytes(change.amount))
        writer.writeField(encodeBytesVec(change.rho))
        return writer.data
    }

    private static func encodeSequence<T>(
        _ values: [T],
        _ encode: (T) throws -> Data
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        for value in values {
            writer.writeField(try encode(value))
        }
        return writer.data
    }

    private static func encodeBytesVec(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
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

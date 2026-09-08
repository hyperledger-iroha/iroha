// Historical encode-only fixtures for rejection tests; never compile into an SDK artifact.
import Foundation
@testable import IrohaSwift

enum RetiredPrivacyConfidentialWitnessError: Error, Equatable, LocalizedError {
    case invalidField(String)
    case invalidArchive(String)

    var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid privacy confidential witness field: \(field)."
        case let .invalidArchive(field):
            return "Invalid privacy confidential witness archive: \(field)."
        }
    }
}

struct RetiredPrivacyConfidentialNoteWitnessV1: Equatable, Sendable {
    let amount: String
    let rho: Data
    let diversifier: Data
    let leafIndex: UInt64

    init(amount: String, rho: Data, diversifier: Data, leafIndex: UInt64) throws {
        self.amount = try RetiredPrivacyConfidentialWitnessCodecs.canonicalU128(amount, field: "amount")
        self.rho = try RetiredPrivacyConfidentialWitnessCodecs.fixed32(rho, field: "rho")
        self.diversifier = try RetiredPrivacyConfidentialWitnessCodecs.fixed32(
            diversifier,
            field: "diversifier"
        )
        self.leafIndex = leafIndex
    }
}

struct RetiredPrivacyConfidentialTransferOutputWitnessV1: Equatable, Sendable {
    let amount: String
    let rho: Data
    let ownerTag: Data

    init(amount: String, rho: Data, ownerTag: Data) throws {
        self.amount = try RetiredPrivacyConfidentialWitnessCodecs.canonicalU128(amount, field: "amount")
        self.rho = try RetiredPrivacyConfidentialWitnessCodecs.fixed32(rho, field: "rho")
        self.ownerTag = try RetiredPrivacyConfidentialWitnessCodecs.fixed32(ownerTag, field: "ownerTag")
    }
}

struct RetiredPrivacyConfidentialUnshieldChangeWitnessV1: Equatable, Sendable {
    let amount: String
    let rho: Data

    init(amount: String, rho: Data) throws {
        self.amount = try RetiredPrivacyConfidentialWitnessCodecs.canonicalU128(amount, field: "amount")
        self.rho = try RetiredPrivacyConfidentialWitnessCodecs.fixed32(rho, field: "rho")
    }
}

struct RetiredPrivacyConfidentialWitnessV1: Equatable, Sendable {
    let networkId: NetworkId
    let assetDefinitionId: String
    let spendKey: Data
    let treeCommitments: [Data]
    let inputs: [RetiredPrivacyConfidentialNoteWitnessV1]
    let transferOutputs: [RetiredPrivacyConfidentialTransferOutputWitnessV1]
    let unshieldChange: [RetiredPrivacyConfidentialUnshieldChangeWitnessV1]
    let publicAmount: String
    let rootHint: Data

    init(
        networkId: NetworkId,
        assetDefinitionId: String,
        spendKey: Data,
        treeCommitments: [Data],
        inputs: [RetiredPrivacyConfidentialNoteWitnessV1],
        transferOutputs: [RetiredPrivacyConfidentialTransferOutputWitnessV1],
        unshieldChange: [RetiredPrivacyConfidentialUnshieldChangeWitnessV1],
        publicAmount: String,
        rootHint: Data
    ) throws {
        self.networkId = networkId
        self.assetDefinitionId = try RetiredPrivacyConfidentialWitnessCodecs.canonicalText(
            assetDefinitionId,
            field: "assetDefinitionId"
        )
        self.spendKey = try RetiredPrivacyConfidentialWitnessCodecs.fixed32(spendKey, field: "spendKey")
        self.treeCommitments = try treeCommitments.enumerated().map { index, value in
            try RetiredPrivacyConfidentialWitnessCodecs.fixed32(
                value,
                field: "treeCommitments[\(index)]"
            )
        }
        self.inputs = inputs
        self.transferOutputs = transferOutputs
        self.unshieldChange = unshieldChange
        self.publicAmount = try RetiredPrivacyConfidentialWitnessCodecs.canonicalPublicAmount(
            publicAmount,
            field: "publicAmount"
        )
        self.rootHint = try RetiredPrivacyConfidentialWitnessCodecs.fixed32(rootHint, field: "rootHint")

        guard !self.treeCommitments.isEmpty,
              self.treeCommitments.count <= RetiredPrivacyConfidentialWitnessCodecs.confidentialTreeCapacityV2
        else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("treeCommitments")
        }
        guard (1...RetiredPrivacyConfidentialWitnessCodecs.confidentialMaxInputsV2).contains(inputs.count) else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("inputs")
        }
        guard transferOutputs.count <= RetiredPrivacyConfidentialWitnessCodecs.confidentialMaxTransferOutputsV2 else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard unshieldChange.count <= RetiredPrivacyConfidentialWitnessCodecs.confidentialMaxUnshieldChangeOutputsV3 else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
        guard transferOutputs.isEmpty || unshieldChange.isEmpty else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard transferOutputs.isEmpty || self.publicAmount == "0" else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("publicAmount")
        }
        var seenLeafIndexes = Set<UInt64>()
        var seenRhos = Set<Data>()
        for (index, input) in inputs.enumerated() {
            guard input.leafIndex < UInt64(self.treeCommitments.count) else {
                throw RetiredPrivacyConfidentialWitnessError.invalidField("inputs[\(index)].leafIndex")
            }
            guard seenLeafIndexes.insert(input.leafIndex).inserted else {
                throw RetiredPrivacyConfidentialWitnessError.invalidField("inputs[\(index)].leafIndex")
            }
            guard seenRhos.insert(input.rho).inserted else {
                throw RetiredPrivacyConfidentialWitnessError.invalidField("inputs[\(index)].rho")
            }
        }
    }
}

/// Bounded confidential-v2 Merkle path supplied by Torii.
///
/// The path deliberately omits intermediate witness nodes because the native
/// prover recomputes and constrains them. This keeps an online preparation
/// proportional to the fixed tree depth instead of the complete frontier.
struct RetiredPrivacyConfidentialMerklePathWitnessV2: Equatable, Sendable {
    let siblings: [Data]
    let directions: Data
    let root: Data

    init(siblings: [Data], directions: Data, root: Data) throws {
        guard siblings.count == RetiredPrivacyConfidentialWitnessCodecs.confidentialTreeDepthV2,
              directions.count == siblings.count else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("merklePath.depth")
        }
        self.siblings = try siblings.enumerated().map { index, value in
            try RetiredPrivacyConfidentialWitnessCodecs.fixed32(
                value,
                field: "merklePath.siblings[\(index)]"
            )
        }
        guard directions.allSatisfy({ $0 == 0 || $0 == 1 }) else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("merklePath.directions")
        }
        self.directions = Data(directions)
        self.root = try RetiredPrivacyConfidentialWitnessCodecs.fixed32(root, field: "merklePath.root")
    }

    init(path: ZkAssetMerklePath) throws {
        try self.init(
            siblings: path.siblings,
            directions: path.directions,
            root: path.rootAtHeight
        )
    }
}

/// Path-based privacy witness used by first-release confidential lifecycle calls.
///
/// Exactly two paths are carried because the transfer and unshield circuits
/// always expose two input slots. For a one-input proof the second path is the
/// authoritative `next_zero_path` returned by `POST /v1/zk/merkle-path`.
struct RetiredPrivacyConfidentialWitnessV2: Equatable, Sendable {
    let networkId: NetworkId
    let assetDefinitionId: String
    let spendKey: Data
    let inputPaths: [RetiredPrivacyConfidentialMerklePathWitnessV2]
    let inputs: [RetiredPrivacyConfidentialNoteWitnessV1]
    let transferOutputs: [RetiredPrivacyConfidentialTransferOutputWitnessV1]
    let unshieldChange: [RetiredPrivacyConfidentialUnshieldChangeWitnessV1]
    let publicAmount: String
    let rootHint: Data

    init(
        networkId: NetworkId,
        assetDefinitionId: String,
        spendKey: Data,
        inputPaths: [RetiredPrivacyConfidentialMerklePathWitnessV2],
        inputs: [RetiredPrivacyConfidentialNoteWitnessV1],
        transferOutputs: [RetiredPrivacyConfidentialTransferOutputWitnessV1],
        unshieldChange: [RetiredPrivacyConfidentialUnshieldChangeWitnessV1],
        publicAmount: String,
        rootHint: Data
    ) throws {
        self.networkId = networkId
        self.assetDefinitionId = try RetiredPrivacyConfidentialWitnessCodecs.canonicalText(
            assetDefinitionId,
            field: "assetDefinitionId"
        )
        self.spendKey = try RetiredPrivacyConfidentialWitnessCodecs.fixed32(
            spendKey,
            field: "spendKey"
        )
        guard inputPaths.count == 2 else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("inputPaths")
        }
        self.inputPaths = inputPaths
        self.inputs = inputs
        self.transferOutputs = transferOutputs
        self.unshieldChange = unshieldChange
        self.publicAmount = try RetiredPrivacyConfidentialWitnessCodecs.canonicalPublicAmount(
            publicAmount,
            field: "publicAmount"
        )
        self.rootHint = try RetiredPrivacyConfidentialWitnessCodecs.fixed32(
            rootHint,
            field: "rootHint"
        )

        guard inputPaths.allSatisfy({ $0.root == self.rootHint }) else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("inputPaths.root")
        }
        guard (1...RetiredPrivacyConfidentialWitnessCodecs.confidentialMaxInputsV2)
            .contains(inputs.count) else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("inputs")
        }
        guard transferOutputs.count
            <= RetiredPrivacyConfidentialWitnessCodecs.confidentialMaxTransferOutputsV2 else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard unshieldChange.count
            <= RetiredPrivacyConfidentialWitnessCodecs.confidentialMaxUnshieldChangeOutputsV3 else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
        guard transferOutputs.isEmpty || unshieldChange.isEmpty else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard transferOutputs.isEmpty || self.publicAmount == "0" else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("publicAmount")
        }
        var seenLeafIndexes = Set<UInt64>()
        var seenRhos = Set<Data>()
        for (index, input) in inputs.enumerated() {
            guard input.leafIndex
                    < UInt64(RetiredPrivacyConfidentialWitnessCodecs.confidentialTreeCapacityV2),
                  seenLeafIndexes.insert(input.leafIndex).inserted,
                  seenRhos.insert(input.rho).inserted else {
                throw RetiredPrivacyConfidentialWitnessError.invalidField("inputs[\(index)]")
            }
        }
    }
}

enum RetiredPrivacyConfidentialWitnessCodecs {
    static let privacyConfidentialWitnessV1WireName =
        "connect_norito_bridge::privacy_production::PrivacyConfidentialWitnessV1"
    static let privacyConfidentialWitnessV2WireName =
        "connect_norito_bridge::privacy_production::PrivacyConfidentialWitnessV2"
    static let confidentialTreeCapacityV2 = 1 << 16
    static let confidentialTreeDepthV2 = 16
    static let confidentialMaxInputsV2 = 2
    static let confidentialMaxTransferOutputsV2 = 2
    static let confidentialMaxUnshieldChangeOutputsV3 = 1

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

    static func confidentialTransferPublicInputsSchema() -> Data {
        confidentialTransferPublicInputsSchemaV1
    }

    static func confidentialUnshieldPublicInputsSchema() -> Data {
        confidentialUnshieldPublicInputsSchemaV1
    }

    static func encodeWitness(_ witness: RetiredPrivacyConfidentialWitnessV1) throws -> Data {
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

    static func encodeWitnessV2(_ witness: RetiredPrivacyConfidentialWitnessV2) throws -> Data {
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

    static func encodeTransferWitness(
        _ witness: RetiredPrivacyConfidentialWitnessV1
    ) throws -> Data {
        try validateTransferWitness(witness)
        return try encodeWitness(witness)
    }

    static func encodeUnshieldWitness(
        _ witness: RetiredPrivacyConfidentialWitnessV1
    ) throws -> Data {
        try validateUnshieldWitness(witness)
        return try encodeWitness(witness)
    }

    static func encodeTransferWitnessV2(
        _ witness: RetiredPrivacyConfidentialWitnessV2
    ) throws -> Data {
        try validateTransferWitnessV2(witness)
        return try encodeWitnessV2(witness)
    }

    static func encodeUnshieldWitnessV2(
        _ witness: RetiredPrivacyConfidentialWitnessV2
    ) throws -> Data {
        try validateUnshieldWitnessV2(witness)
        return try encodeWitnessV2(witness)
    }

    static func canonicalText(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed == value, !trimmed.contains("\0") else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField(field)
        }
        return trimmed
    }

    static func canonicalU128(_ value: String, field: String) throws -> String {
        do {
            return try ConfidentialNoteCrypto.canonicalU128(value, field: field)
        } catch {
            throw RetiredPrivacyConfidentialWitnessError.invalidField(field)
        }
    }

    static func canonicalPublicAmount(_ value: String, field: String) throws -> String {
        if value == "0" {
            return value
        }
        return try canonicalU128(value, field: field)
    }

    static func fixed32(_ value: Data, field: String) throws -> Data {
        guard value.count == 32 else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField(field)
        }
        return Data(value)
    }

    private static func validateTransferWitness(_ witness: RetiredPrivacyConfidentialWitnessV1) throws {
        guard witness.publicAmount == "0" else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("publicAmount")
        }
        guard witness.unshieldChange.isEmpty else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
        guard (1...confidentialMaxTransferOutputsV2).contains(witness.transferOutputs.count) else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
    }

    private static func validateUnshieldWitness(_ witness: RetiredPrivacyConfidentialWitnessV1) throws {
        guard witness.transferOutputs.isEmpty else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard witness.unshieldChange.count <= confidentialMaxUnshieldChangeOutputsV3 else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
    }

    private static func validateTransferWitnessV2(
        _ witness: RetiredPrivacyConfidentialWitnessV2
    ) throws {
        guard witness.publicAmount == "0" else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("publicAmount")
        }
        guard witness.unshieldChange.isEmpty else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
        guard (1...confidentialMaxTransferOutputsV2).contains(witness.transferOutputs.count)
        else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
    }

    private static func validateUnshieldWitnessV2(
        _ witness: RetiredPrivacyConfidentialWitnessV2
    ) throws {
        guard witness.transferOutputs.isEmpty else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("transferOutputs")
        }
        guard witness.unshieldChange.count <= confidentialMaxUnshieldChangeOutputsV3 else {
            throw RetiredPrivacyConfidentialWitnessError.invalidField("unshieldChange")
        }
    }

    private static func encodeNoteWitness(_ note: RetiredPrivacyConfidentialNoteWitnessV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try ConfidentialNoteCrypto.u128LittleEndianBytes(note.amount))
        writer.writeField(encodeBytesVec(note.rho))
        writer.writeField(encodeBytesVec(note.diversifier))
        writer.writeField(CompactNorito.encodeUInt64(note.leafIndex))
        return writer.data
    }

    private static func encodeMerklePathV2(
        _ path: RetiredPrivacyConfidentialMerklePathWitnessV2
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
        _ output: RetiredPrivacyConfidentialTransferOutputWitnessV1
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try ConfidentialNoteCrypto.u128LittleEndianBytes(output.amount))
        writer.writeField(encodeBytesVec(output.rho))
        writer.writeField(encodeBytesVec(output.ownerTag))
        return writer.data
    }

    private static func encodeUnshieldChange(
        _ change: RetiredPrivacyConfidentialUnshieldChangeWitnessV1
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
            throw RetiredPrivacyConfidentialWitnessError.invalidArchive("witness")
        }
        var out = Data()
        out.reserveCapacity(archive.count + bytes)
        out.append(archive.prefix(NoritoHeader.encodedLength))
        out.append(Data(repeating: 0, count: bytes))
        out.append(archive.dropFirst(NoritoHeader.encodedLength))
        return out
    }
}

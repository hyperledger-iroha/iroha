import Foundation

/// SCCP domain id for SORA/Nexus.
public let sccpDomainSora: UInt32 = 0

/// SCCP domain id for Solana.
public let sccpDomainSolana: UInt32 = 3

/// Proof backend id expected by the Solana SCCP recursive verifier.
public let sccpSolanaRecursiveProofBackendV1 = "sccp-solana-recursive-mainnet-v1"

/// Solana mainnet genesis hash used to bind SCCP witness requests.
public let sccpSolanaMainnetGenesisHash = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp"

/// Raw Solana SCCP witness data collected by UI code before local proof generation.
public struct SolanaSccpWitnessInput: Equatable {
    public let targetDomain: UInt32
    public let mainnetGenesisHash: String
    public let finalizedSlot: UInt64
    public let blockhash: String
    public let bankHash: String
    public let transactionStatusRoot: String
    public let messageProofHash: String
    public let transactionSignature: String
    public let emitterProgramId: String
    public let messageId: String
    public let payloadHash: String
    public let commitmentRoot: String
    public let sourceEventDigest: String

    public init(
        targetDomain: UInt32 = sccpDomainSora,
        mainnetGenesisHash: String = sccpSolanaMainnetGenesisHash,
        finalizedSlot: UInt64,
        blockhash: String,
        bankHash: String,
        transactionStatusRoot: String,
        messageProofHash: String,
        transactionSignature: String,
        emitterProgramId: String,
        messageId: String,
        payloadHash: String,
        commitmentRoot: String,
        sourceEventDigest: String
    ) {
        self.targetDomain = targetDomain
        self.mainnetGenesisHash = mainnetGenesisHash
        self.finalizedSlot = finalizedSlot
        self.blockhash = blockhash
        self.bankHash = bankHash
        self.transactionStatusRoot = transactionStatusRoot
        self.messageProofHash = messageProofHash
        self.transactionSignature = transactionSignature
        self.emitterProgramId = emitterProgramId
        self.messageId = messageId
        self.payloadHash = payloadHash
        self.commitmentRoot = commitmentRoot
        self.sourceEventDigest = sourceEventDigest
    }
}

/// Canonical Solana SCCP witness used as prover input.
public struct SolanaSccpWitness: Equatable {
    public let version: UInt8
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let mainnetGenesisHash: String
    public let finalizedSlot: UInt64
    public let blockhash: String
    public let bankHash: String
    public let transactionStatusRoot: String
    public let messageProofHash: String
    public let transactionSignature: String
    public let emitterProgramId: String
    public let messageId: String
    public let payloadHash: String
    public let commitmentRoot: String
    public let sourceEventDigest: String
}

/// Public inputs exposed by a Solana SCCP proof request.
public struct SolanaSccpPublicInputs: Equatable {
    public let messageId: String
    public let payloadHash: String
    public let commitmentRoot: String
    public let finalizedSlot: UInt64
    public let blockhash: String
    public let sourceEventDigest: String
}

/// Request object passed to a linked local Solana SCCP prover.
public struct SolanaSccpProofRequest: Equatable {
    public let version: UInt8
    public let backend: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let mainnetGenesisHash: String
    public let witnessHash: String
    public let publicInputs: SolanaSccpPublicInputs
    public let witness: SolanaSccpWitness
}

/// Proof envelope returned by a linked local Solana SCCP prover.
public struct SolanaSccpProofResult: Equatable {
    public let version: UInt8
    public let backend: String
    public let proofBytes: Data
    public let proofBase64: String
    public let publicInputs: SolanaSccpPublicInputs
    public let witnessHash: String
    public let envelopeHash: String
}

/// Error cases for Solana SCCP local proof request construction.
public enum SolanaSccpProverError: Error, Equatable {
    case invalidString(String)
    case invalidHex32(String)
    case localProverUnavailable
    case emptyProof
}

/// Optional async source for Solana RPC witness material.
public protocol SolanaSccpWitnessProvider {
    func resolveWitness(_ input: SolanaSccpWitnessInput) async throws -> SolanaSccpWitnessInput
}

/// Local-first Solana SCCP proof wrapper. It never fabricates proofs; callers must link a prover.
public final class SolanaSccpProver {
    public typealias ProveFunction = (SolanaSccpProofRequest) async throws -> Data

    private let witnessProvider: SolanaSccpWitnessProvider?
    private let proveFunction: ProveFunction?

    public init(
        witnessProvider: SolanaSccpWitnessProvider? = nil,
        proveFunction: ProveFunction? = nil
    ) {
        self.witnessProvider = witnessProvider
        self.proveFunction = proveFunction
    }

    public func buildRequest(_ input: SolanaSccpWitnessInput) async throws -> SolanaSccpProofRequest {
        let resolved = try await witnessProvider?.resolveWitness(input) ?? input
        return try buildSolanaSccpProofRequest(resolved)
    }

    public func prove(_ input: SolanaSccpWitnessInput) async throws -> SolanaSccpProofResult {
        let request = try await buildRequest(input)
        guard let proveFunction else {
            throw SolanaSccpProverError.localProverUnavailable
        }
        let proofBytes = try await proveFunction(request)
        return try normalizeSolanaSccpProofResult(proofBytes: proofBytes, request: request)
    }
}

/// Normalize raw Solana SCCP witness data.
public func normalizeSolanaSccpWitness(_ input: SolanaSccpWitnessInput) throws -> SolanaSccpWitness {
    SolanaSccpWitness(
        version: 1,
        sourceDomain: sccpDomainSolana,
        targetDomain: input.targetDomain,
        mainnetGenesisHash: try normalizeNonEmpty(input.mainnetGenesisHash, field: "mainnetGenesisHash"),
        finalizedSlot: input.finalizedSlot,
        blockhash: try normalizeNonEmpty(input.blockhash, field: "blockhash"),
        bankHash: try normalizeHex32(input.bankHash, field: "bankHash"),
        transactionStatusRoot: try normalizeHex32(input.transactionStatusRoot, field: "transactionStatusRoot"),
        messageProofHash: try normalizeHex32(input.messageProofHash, field: "messageProofHash"),
        transactionSignature: try normalizeNonEmpty(input.transactionSignature, field: "transactionSignature"),
        emitterProgramId: try normalizeNonEmpty(input.emitterProgramId, field: "emitterProgramId"),
        messageId: try normalizeHex32(input.messageId, field: "messageId"),
        payloadHash: try normalizeHex32(input.payloadHash, field: "payloadHash"),
        commitmentRoot: try normalizeHex32(input.commitmentRoot, field: "commitmentRoot"),
        sourceEventDigest: try normalizeHex32(input.sourceEventDigest, field: "sourceEventDigest")
    )
}

/// Canonical bytes hashed by the Solana SCCP proof request.
public func canonicalSolanaSccpWitnessBytes(_ input: SolanaSccpWitnessInput) throws -> Data {
    try canonicalSolanaSccpWitnessBytes(normalizeSolanaSccpWitness(input))
}

/// Build a Solana SCCP proof request for a linked local prover.
public func buildSolanaSccpProofRequest(_ input: SolanaSccpWitnessInput) throws -> SolanaSccpProofRequest {
    let witness = try normalizeSolanaSccpWitness(input)
    let witnessHash = hashHex(prefix: "sccp:solana:witness:v1", payload: try canonicalSolanaSccpWitnessBytes(witness))
    return SolanaSccpProofRequest(
        version: 1,
        backend: sccpSolanaRecursiveProofBackendV1,
        sourceDomain: sccpDomainSolana,
        targetDomain: witness.targetDomain,
        mainnetGenesisHash: witness.mainnetGenesisHash,
        witnessHash: witnessHash,
        publicInputs: SolanaSccpPublicInputs(
            messageId: witness.messageId,
            payloadHash: witness.payloadHash,
            commitmentRoot: witness.commitmentRoot,
            finalizedSlot: witness.finalizedSlot,
            blockhash: witness.blockhash,
            sourceEventDigest: witness.sourceEventDigest
        ),
        witness: witness
    )
}

/// Canonical bytes used for the Solana message inclusion proof hash.
public func canonicalSolanaSccpMessageProofBytes(
    sourceEventDigest: String,
    transactionStatusRoot: String,
    inclusionBranch: [Data]
) throws -> Data {
    var out = Data()
    out.append(1)
    try out.append(bytesFromHex32(sourceEventDigest, field: "sourceEventDigest"))
    try out.append(bytesFromHex32(transactionStatusRoot, field: "transactionStatusRoot"))
    appendU32Le(UInt32(inclusionBranch.count), to: &out)
    for (index, sibling) in inclusionBranch.enumerated() {
        guard sibling.count == 32 else {
            throw SolanaSccpProverError.invalidHex32("inclusionBranch[\(index)]")
        }
        out.append(sibling)
    }
    return out
}

/// Hash the Solana message inclusion proof in the same form expected by SCCP source adapters.
public func solanaSccpMessageProofHash(
    sourceEventDigest: String,
    transactionStatusRoot: String,
    inclusionBranch: [Data]
) throws -> String {
    hashHex(
        prefix: "sccp:solana:message-proof:v1",
        payload: try canonicalSolanaSccpMessageProofBytes(
            sourceEventDigest: sourceEventDigest,
            transactionStatusRoot: transactionStatusRoot,
            inclusionBranch: inclusionBranch
        )
    )
}

private func normalizeSolanaSccpProofResult(
    proofBytes: Data,
    request: SolanaSccpProofRequest
) throws -> SolanaSccpProofResult {
    guard !proofBytes.isEmpty else {
        throw SolanaSccpProverError.emptyProof
    }
    var envelopePayload = try bytesFromHex32(request.witnessHash, field: "witnessHash")
    envelopePayload.append(proofBytes)
    return SolanaSccpProofResult(
        version: 1,
        backend: request.backend,
        proofBytes: proofBytes,
        proofBase64: proofBytes.base64EncodedString(),
        publicInputs: request.publicInputs,
        witnessHash: request.witnessHash,
        envelopeHash: hashHex(prefix: "sccp:solana:proof-envelope:v1", payload: envelopePayload)
    )
}

private func canonicalSolanaSccpWitnessBytes(_ witness: SolanaSccpWitness) throws -> Data {
    var out = Data()
    out.append(witness.version)
    appendU32Le(witness.sourceDomain, to: &out)
    appendU32Le(witness.targetDomain, to: &out)
    try appendString(witness.mainnetGenesisHash, field: "mainnetGenesisHash", to: &out)
    appendU64Le(witness.finalizedSlot, to: &out)
    try appendString(witness.blockhash, field: "blockhash", to: &out)
    try appendString(witness.transactionSignature, field: "transactionSignature", to: &out)
    try appendString(witness.emitterProgramId, field: "emitterProgramId", to: &out)
    try out.append(bytesFromHex32(witness.bankHash, field: "bankHash"))
    try out.append(bytesFromHex32(witness.transactionStatusRoot, field: "transactionStatusRoot"))
    try out.append(bytesFromHex32(witness.messageProofHash, field: "messageProofHash"))
    try out.append(bytesFromHex32(witness.messageId, field: "messageId"))
    try out.append(bytesFromHex32(witness.payloadHash, field: "payloadHash"))
    try out.append(bytesFromHex32(witness.commitmentRoot, field: "commitmentRoot"))
    try out.append(bytesFromHex32(witness.sourceEventDigest, field: "sourceEventDigest"))
    return out
}

private func normalizeNonEmpty(_ value: String, field: String) throws -> String {
    let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else {
        throw SolanaSccpProverError.invalidString(field)
    }
    return trimmed
}

private func normalizeHex32(_ value: String, field: String) throws -> String {
    "0x" + (try bytesFromHex32(value, field: field)).hexEncodedString()
}

private func bytesFromHex32(_ value: String, field: String) throws -> Data {
    var hex = value.trimmingCharacters(in: .whitespacesAndNewlines)
    if hex.lowercased().hasPrefix("0x") {
        hex.removeFirst(2)
    }
    hex = hex.lowercased()
    guard hex.count == 64, let bytes = Data(hexString: hex), bytes.count == 32 else {
        throw SolanaSccpProverError.invalidHex32(field)
    }
    return bytes
}

private func appendString(_ value: String, field: String, to out: inout Data) throws {
    let bytes = Data(try normalizeNonEmpty(value, field: field).utf8)
    appendU32Le(UInt32(bytes.count), to: &out)
    out.append(bytes)
}

private func appendU32Le(_ value: UInt32, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 24) & 0xff))
}

private func appendU64Le(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 0, through: 56, by: 8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func hashHex(prefix: String, payload: Data) -> String {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return "0x" + Blake2b.hash256(preimage).hexEncodedString()
}

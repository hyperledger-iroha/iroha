import Foundation

/// Proof backend id expected by EVM-family SCCP Groth16 verifier contracts.
public let sccpEvmGroth16Bn254ProofBackendV1 = "evm-groth16-bn254-v1"
/// Canonical byte length of the static BN254 Groth16 ABI proof tuple.
public let sccpGroth16Bn254ProofAbiByteLengthV1 = 384
/// EVM-family contract-call envelope encoding used by SCCP verifier submissions.
public let sccpEvmContractCallAbiTupleV1 = "abi_tuple_v1"
/// Solidity ABI signature for SCCP Groth16 message proof submission.
public let sccpSubmitMessageProofAbiV1 = "submitSccpMessageProof(bytes,bytes32[6],bytes32)"
/// Keccak function selector for `submitSccpMessageProof(bytes,bytes32[6],bytes32)`.
public let sccpSubmitMessageProofSelectorV1 = "0xbd57826c"

private let sccpSubmitMessageProofSelectorBytesV1 = Data([0xbd, 0x57, 0x82, 0x6c])
private let sccpEvmSubmitMessageProofEntrypointV1 =
    "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)"

/// SCCP public inputs shared by EVM-family Groth16 proof requests.
public struct EvmSccpPublicInputsInput: Equatable {
    public let version: UInt8
    public let messageId: String
    public let payloadHash: String
    public let targetDomain: UInt32
    public let commitmentRoot: String
    public let finalityHeight: UInt64
    public let finalityBlockHash: String

    public init(version: UInt8 = 1,
                messageId: String,
                payloadHash: String,
                targetDomain: UInt32 = sccpDomainEthereum,
                commitmentRoot: String,
                finalityHeight: UInt64,
                finalityBlockHash: String) {
        self.version = version
        self.messageId = messageId
        self.payloadHash = payloadHash
        self.targetDomain = targetDomain
        self.commitmentRoot = commitmentRoot
        self.finalityHeight = finalityHeight
        self.finalityBlockHash = finalityBlockHash
    }
}

/// Inputs used to build a local EVM-family SCCP Groth16 proof request.
public struct EvmSccpProofRequestInput: Equatable {
    public let publicInputs: EvmSccpPublicInputsInput
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let backend: String
    public let sourceDomain: UInt32
    public let destinationBinding: EvmSccpDestinationBinding?

    public init(publicInputs: EvmSccpPublicInputsInput,
                bundleBytes: Data,
                sourceProofBytes: Data = Data(),
                statementHash: String,
                destinationBindingHash: String,
                backend: String = sccpEvmGroth16Bn254ProofBackendV1,
                sourceDomain: UInt32 = sccpDomainSora,
                destinationBinding: EvmSccpDestinationBinding? = nil) {
        self.publicInputs = publicInputs
        self.bundleBytes = bundleBytes
        self.sourceProofBytes = sourceProofBytes
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.backend = backend
        self.sourceDomain = sourceDomain
        self.destinationBinding = destinationBinding
    }

    public init(publicInputs: EvmSccpPublicInputsInput,
                bundleBytes: Data,
                sourceProofBytes: Data = Data(),
                statementHash: String,
                destinationBinding: EvmSccpDestinationBinding,
                backend: String = sccpEvmGroth16Bn254ProofBackendV1,
                sourceDomain: UInt32 = sccpDomainSora) throws {
        let destinationBindingHash = try requireEvmDestinationBindingForProofRequest(
            publicInputs: publicInputs,
            destinationBinding: destinationBinding,
            backend: backend,
            sourceDomain: sourceDomain
        )
        self.init(
            publicInputs: publicInputs,
            bundleBytes: bundleBytes,
            sourceProofBytes: sourceProofBytes,
            statementHash: statementHash,
            destinationBindingHash: destinationBindingHash,
            backend: backend,
            sourceDomain: sourceDomain,
            destinationBinding: destinationBinding
        )
    }
}

/// Statement and verifier deployment context proved by the local EVM-family SCCP prover.
public struct EvmSccpProofContext: Equatable {
    public let version: UInt8
    public let statementHash: String
    public let destinationBindingHash: String
}

/// Request passed to a linked local EVM-family Groth16 prover.
public struct EvmSccpProofRequest: Equatable {
    public let version: UInt8
    public let backend: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let publicInputs: EvmSccpPublicInputsInput
    public let publicInputsBytes: Data
    public let publicSignalWords: [String]
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let proofContext: EvmSccpProofContext
    public let statementHash: String
    public let destinationBindingHash: String
    public let requestHash: String
    public let destinationBinding: EvmSccpDestinationBinding?

    public init(version: UInt8,
                backend: String,
                sourceDomain: UInt32,
                targetDomain: UInt32,
                publicInputs: EvmSccpPublicInputsInput,
                publicInputsBytes: Data,
                publicSignalWords: [String],
                bundleBytes: Data,
                sourceProofBytes: Data,
                proofContext: EvmSccpProofContext,
                statementHash: String,
                destinationBindingHash: String,
                requestHash: String,
                destinationBinding: EvmSccpDestinationBinding? = nil) {
        self.version = version
        self.backend = backend
        self.sourceDomain = sourceDomain
        self.targetDomain = targetDomain
        self.publicInputs = publicInputs
        self.publicInputsBytes = publicInputsBytes
        self.publicSignalWords = publicSignalWords
        self.bundleBytes = bundleBytes
        self.sourceProofBytes = sourceProofBytes
        self.proofContext = proofContext
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.requestHash = requestHash
        self.destinationBinding = destinationBinding
    }
}

/// Proof bytes returned by a linked local EVM-family Groth16 prover.
public struct EvmSccpProofResult: Equatable {
    public let version: UInt8
    public let backend: String
    public let proofBytes: Data
    public let proofBase64: String
    public let publicInputs: EvmSccpPublicInputsInput
    public let publicSignalWords: [String]
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let proofContext: EvmSccpProofContext
    public let statementHash: String
    public let destinationBindingHash: String
    public let requestHash: String
    public let envelopeHash: String
    public let destinationBinding: EvmSccpDestinationBinding?

    public init(version: UInt8,
                backend: String,
                proofBytes: Data,
                proofBase64: String,
                publicInputs: EvmSccpPublicInputsInput,
                publicSignalWords: [String],
                bundleBytes: Data = Data(),
                sourceProofBytes: Data = Data(),
                proofContext: EvmSccpProofContext,
                statementHash: String,
                destinationBindingHash: String,
                requestHash: String,
                envelopeHash: String,
                destinationBinding: EvmSccpDestinationBinding? = nil) {
        self.version = version
        self.backend = backend
        self.proofBytes = proofBytes
        self.proofBase64 = proofBase64
        self.publicInputs = publicInputs
        self.publicSignalWords = publicSignalWords
        self.bundleBytes = bundleBytes
        self.sourceProofBytes = sourceProofBytes
        self.proofContext = proofContext
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.requestHash = requestHash
        self.envelopeHash = envelopeHash
        self.destinationBinding = destinationBinding
    }
}

/// Inputs used to package an EVM-family Groth16 proof for verifier-contract submission.
public struct EvmSccpSubmissionInput: Equatable {
    public let publicInputs: EvmSccpPublicInputsInput
    public let proofBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let sourceDomain: UInt32
    public let proofResult: EvmSccpProofResult?
    public let publicSignalWords: [String]?

    public init(publicInputs: EvmSccpPublicInputsInput,
                proofBytes: Data,
                statementHash: String,
                destinationBindingHash: String,
                sourceDomain: UInt32 = sccpDomainSora,
                proofResult: EvmSccpProofResult? = nil,
                publicSignalWords: [String]? = nil) {
        self.publicInputs = publicInputs
        self.proofBytes = proofBytes
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.sourceDomain = sourceDomain
        self.proofResult = proofResult
        self.publicSignalWords = publicSignalWords
    }

    public init(publicInputs: EvmSccpPublicInputsInput,
                proofBytes: Data,
                statementHash: String,
                destinationBinding: EvmSccpDestinationBinding,
                sourceDomain: UInt32 = sccpDomainSora,
                proofResult: EvmSccpProofResult? = nil,
                publicSignalWords: [String]? = nil) throws {
        let destinationBindingHash = try requireEvmDestinationBindingForProofRequest(
            publicInputs: publicInputs,
            destinationBinding: destinationBinding,
            backend: sccpEvmGroth16Bn254ProofBackendV1,
            sourceDomain: sourceDomain
        )
        self.init(
            publicInputs: publicInputs,
            proofBytes: proofBytes,
            statementHash: statementHash,
            destinationBindingHash: destinationBindingHash,
            sourceDomain: sourceDomain,
            proofResult: proofResult,
            publicSignalWords: publicSignalWords
        )
    }

    public init(proofResult: EvmSccpProofResult,
                sourceDomain: UInt32 = sccpDomainSora) {
        self.init(
            publicInputs: proofResult.publicInputs,
            proofBytes: proofResult.proofBytes,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            sourceDomain: sourceDomain,
            proofResult: proofResult,
            publicSignalWords: proofResult.publicSignalWords
        )
    }
}

/// One EVM-family SCCP submission ABI argument in verifier-contract order.
public struct EvmSccpSubmissionArgument: Equatable {
    public let key: String
    public let encoding: String
    public let bytesHex: String
}

/// EVM-family SCCP verifier-contract call data ready for wallet or relayer submission.
public struct EvmSccpSubmission: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let verifierBackend: String
    public let platformPayload: String
    public let envelopeEncoding: String
    public let submissionKind: String
    public let verifierEntrypoint: String
    public let contractMethod: String
    public let functionSelector: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let publicInputs: EvmSccpPublicInputsInput
    public let publicInputWords: [String]
    public let publicSignalWords: [String]
    public let statementHash: String
    public let destinationBindingHash: String
    public let arguments: [EvmSccpSubmissionArgument]
    public let proofBytes: Data
    public let callData: Data
    public let callDataHex: String
    public let envelopeBytes: Data
    public let envelopeHex: String
}

/// Error cases for EVM-family SCCP local proof request construction.
public enum EvmSccpProverError: Error, Equatable {
    case invalidHex32(String)
    case zeroField(String)
    case invalidPublicInputs(String)
    case localProverUnavailable
    case emptyProof
    case allZeroProof
    case invalidProofLength(Int)
}

private func requireEvmDestinationBindingForProofRequest(
    publicInputs: EvmSccpPublicInputsInput,
    destinationBinding: EvmSccpDestinationBinding,
    backend: String,
    sourceDomain: UInt32
) throws -> String {
    guard destinationBinding.version == 1 else {
        throw EvmSccpProverError.invalidPublicInputs("destinationBinding.version")
    }
    guard destinationBinding.sourceDomain == sourceDomain else {
        throw EvmSccpProverError.invalidPublicInputs("destinationBinding.sourceDomain")
    }
    guard destinationBinding.targetDomain == publicInputs.targetDomain else {
        throw EvmSccpProverError.invalidPublicInputs("destinationBinding.targetDomain")
    }
    guard destinationBinding.verifierBackend == backend else {
        throw EvmSccpProverError.invalidPublicInputs("destinationBinding.verifierBackend")
    }
    guard destinationBinding.proofFamily == sccpStarkFriProofFamilyV1 else {
        throw EvmSccpProverError.invalidPublicInputs("destinationBinding.proofFamily")
    }
    let expectedDestinationBinding: EvmSccpDestinationBinding
    do {
        expectedDestinationBinding = try sccpEvmDestinationBinding(
            sourceDomain: sourceDomain,
            targetDomain: publicInputs.targetDomain,
            networkId: destinationBinding.networkId,
            verifierAddress: destinationBinding.verifierAddress,
            bridgeAddress: destinationBinding.bridgeAddress,
            verifierCodeHash: destinationBinding.verifierCodeHash,
            verifierKeyHash: destinationBinding.verifierKeyHash,
            verifierBackend: backend,
            proofFamily: sccpStarkFriProofFamilyV1
        )
    } catch {
        throw EvmSccpProverError.invalidPublicInputs("destinationBinding")
    }
    guard destinationBinding == expectedDestinationBinding else {
        throw EvmSccpProverError.invalidPublicInputs("destinationBinding")
    }
    return expectedDestinationBinding.hash
}

/// Optional async witness resolver backed by app-controlled EVM RPC calls.
public protocol EvmSccpWitnessProvider {
    func resolveWitness(_ input: EvmSccpProofRequestInput) async throws -> EvmSccpProofRequestInput
}

/// Local-first EVM-family SCCP proof wrapper. It never fabricates proofs; callers must link a prover.
public final class EvmSccpProver {
    public typealias ProveFunction = (EvmSccpProofRequest) async throws -> Data

    private let witnessProvider: EvmSccpWitnessProvider?
    private let proveFunction: ProveFunction?

    public init(witnessProvider: EvmSccpWitnessProvider? = nil,
                proveFunction: ProveFunction? = nil) {
        self.witnessProvider = witnessProvider
        self.proveFunction = proveFunction
    }

    public func buildRequest(_ input: EvmSccpProofRequestInput) async throws -> EvmSccpProofRequest {
        let resolved = try await witnessProvider?.resolveWitness(evmSccpWitnessProviderInputSnapshot(input)) ?? input
        return try buildEvmSccpProofRequest(resolved)
    }

    public func prove(_ input: EvmSccpProofRequestInput) async throws -> EvmSccpProofResult {
        let request = try await buildRequest(input)
        guard let proveFunction else {
            throw EvmSccpProverError.localProverUnavailable
        }
        try requireProductionEvmSccpProofRequest(request)
        let proofBytes = try await proveFunction(evmSccpProofRequestCallbackSnapshot(request))
        return try wrapEvmSccpProofResult(proofBytes: proofBytes, request: request)
    }
}

private func evmSccpWitnessProviderInputSnapshot(_ input: EvmSccpProofRequestInput) -> EvmSccpProofRequestInput {
    EvmSccpProofRequestInput(
        publicInputs: input.publicInputs,
        bundleBytes: Data(input.bundleBytes),
        sourceProofBytes: Data(input.sourceProofBytes),
        statementHash: input.statementHash,
        destinationBindingHash: input.destinationBindingHash,
        backend: input.backend,
        sourceDomain: input.sourceDomain,
        destinationBinding: input.destinationBinding
    )
}

private func evmSccpProofRequestCallbackSnapshot(_ request: EvmSccpProofRequest) -> EvmSccpProofRequest {
    EvmSccpProofRequest(
        version: request.version,
        backend: request.backend,
        sourceDomain: request.sourceDomain,
        targetDomain: request.targetDomain,
        publicInputs: request.publicInputs,
        publicInputsBytes: Data(request.publicInputsBytes),
        publicSignalWords: Array(request.publicSignalWords),
        bundleBytes: Data(request.bundleBytes),
        sourceProofBytes: Data(request.sourceProofBytes),
        proofContext: request.proofContext,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
        requestHash: request.requestHash,
        destinationBinding: request.destinationBinding
    )
}

/// Canonical SCCP public-input bytes used by EVM-family proof requests.
public func canonicalEvmSccpPublicInputsBytes(_ input: EvmSccpPublicInputsInput) throws -> Data {
    guard input.version == 1 else {
        throw EvmSccpProverError.invalidPublicInputs("publicInputs.version")
    }
    guard input.targetDomain != 0 else {
        throw EvmSccpProverError.invalidPublicInputs("publicInputs.targetDomain")
    }
    guard input.targetDomain == sccpDomainEthereum || input.targetDomain == sccpDomainBsc else {
        throw EvmSccpProverError.invalidPublicInputs("publicInputs.targetDomain")
    }
    guard input.finalityHeight != 0 else {
        throw EvmSccpProverError.invalidPublicInputs("publicInputs.finalityHeight")
    }
    var out = Data()
    out.append(input.version)
    try out.append(evmNonZeroBytesFromHex32(input.messageId, field: "messageId"))
    try out.append(evmNonZeroBytesFromHex32(input.payloadHash, field: "payloadHash"))
    evmAppendU32Le(input.targetDomain, to: &out)
    try out.append(evmNonZeroBytesFromHex32(input.commitmentRoot, field: "commitmentRoot"))
    evmAppendU64Le(input.finalityHeight, to: &out)
    try out.append(evmNonZeroBytesFromHex32(input.finalityBlockHash, field: "finalityBlockHash"))
    return out
}

/// Derive the nine BN254 public signal words consumed by EVM/TRON Groth16 verifiers.
public func sccpGroth16Bn254PublicSignalWords(publicInputs: EvmSccpPublicInputsInput,
                                              sourceDomain: UInt32,
                                              statementHash: String,
                                              destinationBindingHash: String) throws -> [String] {
    guard publicInputs.version == 1 else {
        throw EvmSccpProverError.invalidPublicInputs("publicInputs.version")
    }
    guard publicInputs.targetDomain != 0 else {
        throw EvmSccpProverError.invalidPublicInputs("publicInputs.targetDomain")
    }
    guard publicInputs.targetDomain == sccpDomainEthereum || publicInputs.targetDomain == sccpDomainBsc else {
        throw EvmSccpProverError.invalidPublicInputs("publicInputs.targetDomain")
    }
    guard sourceDomain == sccpDomainSora else {
        throw EvmSccpProverError.invalidPublicInputs("sourceDomain")
    }
    guard sourceDomain != publicInputs.targetDomain else {
        throw EvmSccpProverError.invalidPublicInputs("sourceDomain")
    }
    guard publicInputs.finalityHeight != 0 else {
        throw EvmSccpProverError.invalidPublicInputs("publicInputs.finalityHeight")
    }
    let values = try [
        evmNonZeroBytesFromHex32(publicInputs.messageId, field: "messageId"),
        evmNonZeroBytesFromHex32(publicInputs.payloadHash, field: "payloadHash"),
        evmAbiWordU32(publicInputs.targetDomain),
        evmNonZeroBytesFromHex32(publicInputs.commitmentRoot, field: "commitmentRoot"),
        evmAbiWordU64(publicInputs.finalityHeight),
        evmNonZeroBytesFromHex32(publicInputs.finalityBlockHash, field: "finalityBlockHash"),
        evmAbiWordU32(sourceDomain),
        evmNonZeroBytesFromHex32(statementHash, field: "statementHash"),
        evmNonZeroBytesFromHex32(destinationBindingHash, field: "destinationBindingHash"),
    ]
    return try values.enumerated().map { index, value in
        try evmGroth16Bn254SignalWord(label: evmGroth16SignalLabels[index], value: value)
    }
}

/// Build an EVM-family SCCP Groth16 proof request for a linked local prover.
public func buildEvmSccpProofRequest(_ input: EvmSccpProofRequestInput) throws -> EvmSccpProofRequest {
    guard input.backend == sccpEvmGroth16Bn254ProofBackendV1 else {
        throw EvmSccpProverError.invalidPublicInputs("backend")
    }
    guard !input.bundleBytes.isEmpty else {
        throw EvmSccpProverError.invalidPublicInputs("bundleBytes")
    }
    guard UInt64(input.bundleBytes.count) <= UInt64(UInt32.max),
          UInt64(input.sourceProofBytes.count) <= UInt64(UInt32.max) else {
        throw EvmSccpProverError.invalidPublicInputs("proof byte length")
    }
    guard input.sourceProofBytes.isEmpty || input.sourceProofBytes.contains(where: { $0 != 0 }) else {
        throw EvmSccpProverError.invalidPublicInputs("sourceProofBytes")
    }
    let publicInputsBytes = try canonicalEvmSccpPublicInputsBytes(input.publicInputs)
    let proofContext = try normalizeEvmSccpProofContext(
        statementHash: input.statementHash,
        destinationBindingHash: input.destinationBindingHash
    )
    let publicSignalWords = try sccpGroth16Bn254PublicSignalWords(
        publicInputs: input.publicInputs,
        sourceDomain: input.sourceDomain,
        statementHash: proofContext.statementHash,
        destinationBindingHash: proofContext.destinationBindingHash
    )
    var preimage = Data()
    preimage.append(publicInputsBytes)
    evmAppendU32Le(UInt32(input.bundleBytes.count), to: &preimage)
    preimage.append(input.bundleBytes)
    evmAppendU32Le(UInt32(input.sourceProofBytes.count), to: &preimage)
    preimage.append(input.sourceProofBytes)
    try preimage.append(evmBytesFromHex32(proofContext.statementHash, field: "statementHash"))
    try preimage.append(evmBytesFromHex32(proofContext.destinationBindingHash, field: "destinationBindingHash"))
    for signal in publicSignalWords {
        try preimage.append(evmBytesFromHex32(signal, field: "publicSignalWords"))
    }
    return EvmSccpProofRequest(
        version: 1,
        backend: input.backend,
        sourceDomain: input.sourceDomain,
        targetDomain: input.publicInputs.targetDomain,
        publicInputs: input.publicInputs,
        publicInputsBytes: publicInputsBytes,
        publicSignalWords: publicSignalWords,
        bundleBytes: input.bundleBytes,
        sourceProofBytes: input.sourceProofBytes,
        proofContext: proofContext,
        statementHash: proofContext.statementHash,
        destinationBindingHash: proofContext.destinationBindingHash,
        requestHash: evmHashHex(prefix: "sccp:evm:groth16-proof-request:v1", payload: preimage),
        destinationBinding: input.destinationBinding
    )
}

private func normalizeEvmSccpProofContext(statementHash: String,
                                          destinationBindingHash: String) throws -> EvmSccpProofContext {
    EvmSccpProofContext(
        version: 1,
        statementHash: try evmNormalizeHex32(statementHash, field: "statementHash"),
        destinationBindingHash: try evmNormalizeHex32(destinationBindingHash, field: "destinationBindingHash")
    )
}

public func wrapEvmSccpProofResult(proofBytes: Data,
                                   request: EvmSccpProofRequest) throws -> EvmSccpProofResult {
    try requireProductionEvmSccpProofRequest(request)
    try requireEvmGroth16ProofBytes(
        proofBytes,
        publicInputs: request.publicInputs,
        sourceDomain: request.sourceDomain
    )
    var envelopePayload = try evmBytesFromHex32(request.requestHash, field: "requestHash")
    envelopePayload.append(proofBytes)
    return EvmSccpProofResult(
        version: 1,
        backend: request.backend,
        proofBytes: proofBytes,
        proofBase64: proofBytes.base64EncodedString(),
        publicInputs: request.publicInputs,
        publicSignalWords: request.publicSignalWords,
        bundleBytes: request.bundleBytes,
        sourceProofBytes: request.sourceProofBytes,
        proofContext: request.proofContext,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
        requestHash: request.requestHash,
        envelopeHash: evmHashHex(prefix: "sccp:evm:groth16-proof-envelope:v1", payload: envelopePayload),
        destinationBinding: request.destinationBinding
    )
}

private func requireWrappedEvmProofResultForSubmission(
    _ proofResult: EvmSccpProofResult
) throws -> EvmSccpProofResult {
    guard proofResult.backend == sccpEvmGroth16Bn254ProofBackendV1 else {
        throw EvmSccpProverError.invalidPublicInputs("proofResult.backend")
    }
    let expectedProofContext = try normalizeEvmSccpProofContext(
        statementHash: proofResult.statementHash,
        destinationBindingHash: proofResult.destinationBindingHash
    )
    guard proofResult.proofContext == expectedProofContext else {
        throw EvmSccpProverError.invalidPublicInputs("proofResult.proofContext")
    }
    try requireEvmGroth16ProofBytes(proofResult.proofBytes, publicInputs: proofResult.publicInputs)
    guard proofResult.proofBase64 == proofResult.proofBytes.base64EncodedString() else {
        throw EvmSccpProverError.invalidPublicInputs("proofResult.proofBase64")
    }
    let requestHash = try evmNormalizeHex32(proofResult.requestHash, field: "proofResult.requestHash")
    let envelopeHash = try evmNormalizeHex32(proofResult.envelopeHash, field: "proofResult.envelopeHash")
    var envelopePayload = try evmBytesFromHex32(requestHash, field: "proofResult.requestHash")
    envelopePayload.append(proofResult.proofBytes)
    guard envelopeHash == evmHashHex(prefix: "sccp:evm:groth16-proof-envelope:v1", payload: envelopePayload) else {
        throw EvmSccpProverError.invalidPublicInputs("proofResult.envelopeHash")
    }
    guard proofResult.sourceProofBytes.isEmpty || proofResult.sourceProofBytes.contains(where: { $0 != 0 }) else {
        throw EvmSccpProverError.invalidPublicInputs("proofResult.sourceProofBytes")
    }
    let expectedRequest = try buildEvmSccpProofRequest(EvmSccpProofRequestInput(
        publicInputs: proofResult.publicInputs,
        bundleBytes: proofResult.bundleBytes,
        sourceProofBytes: proofResult.sourceProofBytes,
        statementHash: proofResult.statementHash,
        destinationBindingHash: proofResult.destinationBindingHash,
        backend: proofResult.backend,
        sourceDomain: sccpDomainSora,
        destinationBinding: proofResult.destinationBinding
    ))
    guard expectedRequest.requestHash == requestHash else {
        throw EvmSccpProverError.invalidPublicInputs("proofResult.requestHash")
    }
    return proofResult
}

/// ABI words for the transparent public inputs passed to EVM-family verifier contracts.
public func evmSccpMessageTransparentPublicInputAbiWords(_ input: EvmSccpPublicInputsInput) throws -> [String] {
    try evmSccpMessageTransparentPublicInputAbiWordBytes(input).map { "0x" + $0.hexEncodedString() }
}

/// ABI call data for `submitSccpMessageProof(bytes,bytes32[6],bytes32)`.
public func evmSccpSubmitMessageProofCallData(proofBytes: Data,
                                              publicInputs: EvmSccpPublicInputsInput,
                                              statementHash: String,
                                              sourceDomain: UInt32 = sccpDomainSora) throws -> Data {
    guard sourceDomain == sccpDomainSora else {
        throw EvmSccpProverError.invalidPublicInputs("sourceDomain")
    }
    try requireEvmGroth16ProofBytes(proofBytes, publicInputs: publicInputs, sourceDomain: sourceDomain)
    let publicInputWords = try evmSccpMessageTransparentPublicInputAbiWordBytes(publicInputs)
    var out = Data(sccpSubmitMessageProofSelectorBytesV1)
    out.append(evmAbiWordU256(32 * 8))
    for word in publicInputWords {
        out.append(word)
    }
    try out.append(evmNonZeroBytesFromHex32(statementHash, field: "statementHash"))
    out.append(evmAbiWordU256(UInt64(proofBytes.count)))
    out.append(proofBytes)
    let padding = (32 - (proofBytes.count % 32)) % 32
    if padding > 0 {
        out.append(Data(repeating: 0, count: padding))
    }
    return out
}

/// Build EVM-family verifier-contract call data from UI-generated proof bytes.
public func buildEvmSccpSubmission(_ input: EvmSccpSubmissionInput) throws -> EvmSccpSubmission {
    guard input.sourceDomain == sccpDomainSora else {
        throw EvmSccpProverError.invalidPublicInputs("sourceDomain")
    }
    _ = try canonicalEvmSccpPublicInputsBytes(input.publicInputs)
    try requireEvmGroth16ProofBytes(
        input.proofBytes,
        publicInputs: input.publicInputs,
        sourceDomain: input.sourceDomain
    )
    let statementHash = try evmNormalizeHex32(input.statementHash, field: "statementHash")
    let destinationBindingHash = try evmNormalizeHex32(
        input.destinationBindingHash,
        field: "destinationBindingHash"
    )
    if let proofResult = input.proofResult {
        let proofResult = try requireWrappedEvmProofResultForSubmission(proofResult)
        guard proofResult.backend == sccpEvmGroth16Bn254ProofBackendV1 else {
            throw EvmSccpProverError.invalidPublicInputs("proofResult.backend")
        }
        guard proofResult.publicInputs == input.publicInputs else {
            throw EvmSccpProverError.invalidPublicInputs("proofResult.publicInputs")
        }
        guard proofResult.proofBytes == input.proofBytes else {
            throw EvmSccpProverError.invalidPublicInputs("proofBytes")
        }
        guard proofResult.statementHash == statementHash else {
            throw EvmSccpProverError.invalidPublicInputs("statementHash")
        }
        guard proofResult.destinationBindingHash == destinationBindingHash else {
            throw EvmSccpProverError.invalidPublicInputs("destinationBindingHash")
        }
    }
    let publicSignalWords = try sccpGroth16Bn254PublicSignalWords(
        publicInputs: input.publicInputs,
        sourceDomain: input.sourceDomain,
        statementHash: statementHash,
        destinationBindingHash: destinationBindingHash
    )
    if let suppliedSignals = input.publicSignalWords ?? input.proofResult?.publicSignalWords {
        guard suppliedSignals.count == 9 else {
            throw EvmSccpProverError.invalidPublicInputs("publicSignalWords")
        }
        let normalizedSignals = try suppliedSignals.enumerated().map { index, word in
            "0x" + (try evmBytesFromHex32(word, field: "publicSignalWords[\(index)]")).hexEncodedString()
        }
        guard normalizedSignals == publicSignalWords else {
            throw EvmSccpProverError.invalidPublicInputs("publicSignalWords")
        }
    }
    let publicInputWordsBytesArray = try evmSccpMessageTransparentPublicInputAbiWordBytes(input.publicInputs)
    let publicInputWordsBytes = publicInputWordsBytesArray.reduce(into: Data()) { out, word in
        out.append(word)
    }
    let publicInputWords = publicInputWordsBytesArray.map { "0x" + $0.hexEncodedString() }
    let callData = try evmSccpSubmitMessageProofCallData(
        proofBytes: input.proofBytes,
        publicInputs: input.publicInputs,
        statementHash: statementHash,
        sourceDomain: input.sourceDomain
    )
    let arguments = [
        EvmSccpSubmissionArgument(
            key: "proof_bytes",
            encoding: "raw_bytes",
            bytesHex: "0x" + input.proofBytes.hexEncodedString()
        ),
        EvmSccpSubmissionArgument(
            key: "public_inputs",
            encoding: "abi_bytes32x6",
            bytesHex: "0x" + publicInputWordsBytes.hexEncodedString()
        ),
        EvmSccpSubmissionArgument(
            key: "statement_hash",
            encoding: "abi_bytes32",
            bytesHex: statementHash
        ),
    ]
    return EvmSccpSubmission(
        version: 1,
        proofFamily: "stark-fri-v1",
        verifierBackend: sccpEvmGroth16Bn254ProofBackendV1,
        platformPayload: "evm_groth16_contract_call",
        envelopeEncoding: sccpEvmContractCallAbiTupleV1,
        submissionKind: "contract_call",
        verifierEntrypoint: sccpEvmSubmitMessageProofEntrypointV1,
        contractMethod: sccpSubmitMessageProofAbiV1,
        functionSelector: sccpSubmitMessageProofSelectorV1,
        sourceDomain: input.sourceDomain,
        targetDomain: input.publicInputs.targetDomain,
        publicInputs: input.publicInputs,
        publicInputWords: publicInputWords,
        publicSignalWords: publicSignalWords,
        statementHash: statementHash,
        destinationBindingHash: destinationBindingHash,
        arguments: arguments,
        proofBytes: input.proofBytes,
        callData: callData,
        callDataHex: "0x" + callData.hexEncodedString(),
        envelopeBytes: callData,
        envelopeHex: "0x" + callData.hexEncodedString()
    )
}

private func requireCanonicalEvmSccpProofRequest(_ request: EvmSccpProofRequest) throws {
    let expected = try buildEvmSccpProofRequest(EvmSccpProofRequestInput(
        publicInputs: request.publicInputs,
        bundleBytes: request.bundleBytes,
        sourceProofBytes: request.sourceProofBytes,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
        backend: request.backend,
        sourceDomain: request.sourceDomain,
        destinationBinding: request.destinationBinding
    ))
    guard expected == request else {
        throw EvmSccpProverError.invalidPublicInputs("request")
    }
}

private func requireProductionEvmSccpProofRequest(_ request: EvmSccpProofRequest) throws {
    try requireCanonicalEvmSccpProofRequest(request)
    guard request.version == 1 else {
        throw EvmSccpProverError.invalidPublicInputs("request.version")
    }
    guard request.backend == sccpEvmGroth16Bn254ProofBackendV1 else {
        throw EvmSccpProverError.invalidPublicInputs("request.backend")
    }
    guard request.sourceDomain == sccpDomainSora else {
        throw EvmSccpProverError.invalidPublicInputs("request.sourceDomain")
    }
    guard request.targetDomain == request.publicInputs.targetDomain,
          request.targetDomain == sccpDomainEthereum || request.targetDomain == sccpDomainBsc else {
        throw EvmSccpProverError.invalidPublicInputs("request.targetDomain")
    }
    guard !request.bundleBytes.isEmpty else {
        throw EvmSccpProverError.invalidPublicInputs("request.bundleBytes")
    }
    guard request.sourceProofBytes.isEmpty || request.sourceProofBytes.contains(where: { $0 != 0 }) else {
        throw EvmSccpProverError.invalidPublicInputs("request.sourceProofBytes")
    }
    try requireProductionEvmDestinationBinding(request)
}

private func requireProductionEvmDestinationBinding(_ request: EvmSccpProofRequest) throws {
    guard let destinationBinding = request.destinationBinding else {
        throw EvmSccpProverError.invalidPublicInputs("request.destinationBinding")
    }
    let destinationBindingHash = try requireEvmDestinationBindingForProofRequest(
        publicInputs: request.publicInputs,
        destinationBinding: destinationBinding,
        backend: request.backend,
        sourceDomain: request.sourceDomain
    )
    guard request.destinationBindingHash == destinationBindingHash else {
        throw EvmSccpProverError.invalidPublicInputs("destinationBindingHash")
    }
    guard request.proofContext.destinationBindingHash == destinationBindingHash else {
        throw EvmSccpProverError.invalidPublicInputs("proofContext.destinationBindingHash")
    }
}

private let evmGroth16SignalLabels = [
    "sccp:groth16-bn254:signal:message-id:v1",
    "sccp:groth16-bn254:signal:payload-hash:v1",
    "sccp:groth16-bn254:signal:target-domain:v1",
    "sccp:groth16-bn254:signal:commitment-root:v1",
    "sccp:groth16-bn254:signal:finality-height:v1",
    "sccp:groth16-bn254:signal:finality-block-hash:v1",
    "sccp:groth16-bn254:signal:source-domain:v1",
    "sccp:groth16-bn254:signal:statement-hash:v1",
    "sccp:groth16-bn254:signal:destination-binding-hash:v1",
]

private let evmBn254ScalarFieldModulus = Data(hexString:
    "30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001"
)!
private let evmBn254BaseFieldModulus = Data(hexString:
    "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47"
)!

private func evmGroth16Bn254SignalWord(label: String, value: Data) throws -> String {
    let labelHash = irohaKeccak256(Data(label.utf8))
    var payload = Data(labelHash)
    payload.append(value)
    return "0x" + evmReduceModBn254(irohaKeccak256(payload)).hexEncodedString()
}

private func evmReduceModBn254(_ value: Data) -> Data {
    var out = Data(value)
    while evmCompareBytes(out, evmBn254ScalarFieldModulus) != .orderedAscending {
        out = evmSubtractBytes(out, evmBn254ScalarFieldModulus)
    }
    return out
}

private func evmCompareBytes(_ lhs: Data, _ rhs: Data) -> ComparisonResult {
    for (left, right) in zip(lhs, rhs) where left != right {
        return left < right ? .orderedAscending : .orderedDescending
    }
    if lhs.count == rhs.count {
        return .orderedSame
    }
    return lhs.count < rhs.count ? .orderedAscending : .orderedDescending
}

private func evmSubtractBytes(_ lhs: Data, _ rhs: Data) -> Data {
    var out = [UInt8](lhs)
    let subtrahend = [UInt8](rhs)
    var borrow = 0
    for index in stride(from: out.count - 1, through: 0, by: -1) {
        let difference = Int(out[index]) - Int(subtrahend[index]) - borrow
        if difference < 0 {
            out[index] = UInt8(difference + 256)
            borrow = 1
        } else {
            out[index] = UInt8(difference)
            borrow = 0
        }
    }
    return Data(out)
}

private func evmAbiWordU32(_ value: UInt32) -> Data {
    var out = Data(repeating: 0, count: 32)
    out[28] = UInt8((value >> 24) & 0xff)
    out[29] = UInt8((value >> 16) & 0xff)
    out[30] = UInt8((value >> 8) & 0xff)
    out[31] = UInt8(value & 0xff)
    return out
}

private func evmAbiWordU64(_ value: UInt64) -> Data {
    var out = Data(repeating: 0, count: 32)
    for offset in 0..<8 {
        out[24 + offset] = UInt8((value >> UInt64((7 - offset) * 8)) & 0xff)
    }
    return out
}

private func evmAbiWordU256(_ value: UInt64) -> Data {
    var out = Data(repeating: 0, count: 32)
    for offset in 0..<8 {
        out[24 + offset] = UInt8((value >> UInt64((7 - offset) * 8)) & 0xff)
    }
    return out
}

private func evmSccpMessageTransparentPublicInputAbiWordBytes(_ input: EvmSccpPublicInputsInput) throws -> [Data] {
    _ = try canonicalEvmSccpPublicInputsBytes(input)
    return try [
        evmNonZeroBytesFromHex32(input.messageId, field: "messageId"),
        evmNonZeroBytesFromHex32(input.payloadHash, field: "payloadHash"),
        evmAbiWordU32(input.targetDomain),
        evmNonZeroBytesFromHex32(input.commitmentRoot, field: "commitmentRoot"),
        evmAbiWordU64(input.finalityHeight),
        evmNonZeroBytesFromHex32(input.finalityBlockHash, field: "finalityBlockHash"),
    ]
}

private func requireEvmGroth16ProofBytes(_ proofBytes: Data) throws {
    guard !proofBytes.isEmpty else {
        throw EvmSccpProverError.emptyProof
    }
    guard proofBytes.contains(where: { $0 != 0 }) else {
        throw EvmSccpProverError.allZeroProof
    }
    guard proofBytes.count == sccpGroth16Bn254ProofAbiByteLengthV1 else {
        throw EvmSccpProverError.invalidProofLength(proofBytes.count)
    }
    try requireEvmGroth16ProofTuple(proofBytes)
}

private func requireEvmGroth16ProofBytes(_ proofBytes: Data,
                                         publicInputs: EvmSccpPublicInputsInput,
                                         sourceDomain: UInt32? = nil) throws {
    try requireEvmGroth16ProofBytes(proofBytes)
    guard evmGroth16ProofWord(proofBytes, index: 1) == (try evmNonZeroBytesFromHex32(
        publicInputs.messageId,
        field: "publicInputs.messageId"
    )) else {
        throw EvmSccpProverError.invalidPublicInputs("proofBytes.messageId")
    }
    guard evmGroth16ProofWord(proofBytes, index: 3) == (try evmNonZeroBytesFromHex32(
        publicInputs.commitmentRoot,
        field: "publicInputs.commitmentRoot"
    )) else {
        throw EvmSccpProverError.invalidPublicInputs("proofBytes.commitmentRoot")
    }
    if let sourceDomain {
        guard evmGroth16ProofWord(proofBytes, index: 2) == evmAbiWordU32(sourceDomain) else {
            throw EvmSccpProverError.invalidPublicInputs("proofBytes.sourceDomain")
        }
    }
}

private func evmGroth16ProofWord(_ proofBytes: Data, index: Int) -> Data {
    let start = index * 32
    return Data(proofBytes[start..<(start + 32)])
}

private func evmGroth16ProofWordIsZero(_ proofBytes: Data, index: Int) -> Bool {
    !evmGroth16ProofWord(proofBytes, index: index).contains { $0 != 0 }
}

private func requireEvmGroth16BaseFieldWord(_ proofBytes: Data, index: Int, field: String) throws {
    let word = evmGroth16ProofWord(proofBytes, index: index)
    guard evmCompareBytes(word, evmBn254BaseFieldModulus) == .orderedAscending else {
        throw EvmSccpProverError.invalidPublicInputs(field)
    }
}

private func requireEvmGroth16NonZeroPoint(_ proofBytes: Data, indexes: [Int], field: String) throws {
    guard indexes.contains(where: { !evmGroth16ProofWordIsZero(proofBytes, index: $0) }) else {
        throw EvmSccpProverError.invalidPublicInputs(field)
    }
}

private func requireEvmGroth16ProofTuple(_ proofBytes: Data) throws {
    if let field = sccpGroth16Bn254ProofTupleInvalidField(proofBytes) {
        throw EvmSccpProverError.invalidPublicInputs(field)
    }
}

private func evmBytesFromHex32(_ value: String, field: String) throws -> Data {
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
        throw EvmSccpProverError.invalidHex32(field)
    }
    var hex = value
    if hex.lowercased().hasPrefix("0x") {
        hex.removeFirst(2)
    }
    guard hex.unicodeScalars.allSatisfy({ !CharacterSet.whitespacesAndNewlines.contains($0) }) else {
        throw EvmSccpProverError.invalidHex32(field)
    }
    hex = hex.lowercased()
    guard hex.count == 64, let bytes = Data(hexString: hex), bytes.count == 32 else {
        throw EvmSccpProverError.invalidHex32(field)
    }
    return bytes
}

private func evmNormalizeHex32(_ value: String, field: String) throws -> String {
    "0x" + (try evmNonZeroBytesFromHex32(value, field: field)).hexEncodedString()
}

private func evmNonZeroBytesFromHex32(_ value: String, field: String) throws -> Data {
    let bytes = try evmBytesFromHex32(value, field: field)
    guard bytes.contains(where: { $0 != 0 }) else {
        throw EvmSccpProverError.zeroField(field)
    }
    return bytes
}

private func evmAppendU32Le(_ value: UInt32, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 24) & 0xff))
}

private func evmAppendU64Le(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 0, through: 56, by: 8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func evmHashHex(prefix: String, payload: Data) -> String {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return "0x" + Blake2b.hash256(preimage).hexEncodedString()
}

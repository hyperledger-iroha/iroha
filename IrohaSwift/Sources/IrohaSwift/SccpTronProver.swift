import Foundation

/// Proof backend id expected by the TRON SCCP Groth16 verifier contract.
public let sccpTronGroth16Bn254ProofBackendV1 = "tron-groth16-bn254-v1"
/// TRON contract-call envelope encoding used by SCCP verifier submissions.
public let sccpTronContractCallAbiTupleV1 = "tron_abi_tuple_v1"

private let sccpTronSubmitMessageProofEntrypointV1 =
    "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)"
private let sccpTronSubmitMessageProofSelectorBytesV1 = Data([0xbd, 0x57, 0x82, 0x6c])

/// SCCP public inputs shared by TRON Groth16 proof requests.
public struct TronSccpPublicInputsInput: Equatable {
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
                targetDomain: UInt32 = sccpDomainTron,
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

/// Inputs used to build a local TRON SCCP Groth16 proof request.
public struct TronSccpProofRequestInput: Equatable {
    public let publicInputs: TronSccpPublicInputsInput
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let backend: String
    public let sourceDomain: UInt32
    public let destinationBinding: TronSccpDestinationBinding?

    public init(publicInputs: TronSccpPublicInputsInput,
                bundleBytes: Data,
                sourceProofBytes: Data = Data(),
                statementHash: String,
                destinationBindingHash: String,
                backend: String = sccpTronGroth16Bn254ProofBackendV1,
                sourceDomain: UInt32 = sccpDomainSora,
                destinationBinding: TronSccpDestinationBinding? = nil) {
        self.publicInputs = publicInputs
        self.bundleBytes = bundleBytes
        self.sourceProofBytes = sourceProofBytes
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.backend = backend
        self.sourceDomain = sourceDomain
        self.destinationBinding = destinationBinding
    }

    public init(publicInputs: TronSccpPublicInputsInput,
                bundleBytes: Data,
                sourceProofBytes: Data = Data(),
                statementHash: String,
                destinationBinding: TronSccpDestinationBinding,
                backend: String = sccpTronGroth16Bn254ProofBackendV1,
                sourceDomain: UInt32 = sccpDomainSora) throws {
        let destinationBindingHash = try requireTronDestinationBindingForProofRequest(
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

/// Statement and verifier deployment context proved by the local TRON SCCP prover.
public struct TronSccpProofContext: Equatable {
    public let version: UInt8
    public let statementHash: String
    public let destinationBindingHash: String
}

/// Request passed to a linked local TRON Groth16 prover.
public struct TronSccpProofRequest: Equatable {
    public let version: UInt8
    public let backend: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let publicInputs: TronSccpPublicInputsInput
    public let publicInputsBytes: Data
    public let publicSignalWords: [String]
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let proofContext: TronSccpProofContext
    public let statementHash: String
    public let destinationBindingHash: String
    public let requestHash: String
    public let destinationBinding: TronSccpDestinationBinding?

    public init(version: UInt8,
                backend: String,
                sourceDomain: UInt32,
                targetDomain: UInt32,
                publicInputs: TronSccpPublicInputsInput,
                publicInputsBytes: Data,
                publicSignalWords: [String],
                bundleBytes: Data,
                sourceProofBytes: Data,
                proofContext: TronSccpProofContext,
                statementHash: String,
                destinationBindingHash: String,
                requestHash: String,
                destinationBinding: TronSccpDestinationBinding? = nil) {
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

/// Proof bytes returned by a linked local TRON Groth16 prover.
public struct TronSccpProofResult: Equatable {
    public let version: UInt8
    public let backend: String
    public let proofBytes: Data
    public let proofBase64: String
    public let publicInputs: TronSccpPublicInputsInput
    public let publicSignalWords: [String]
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let proofContext: TronSccpProofContext
    public let statementHash: String
    public let destinationBindingHash: String
    public let requestHash: String
    public let envelopeHash: String
    public let destinationBinding: TronSccpDestinationBinding?

    public init(version: UInt8,
                backend: String,
                proofBytes: Data,
                proofBase64: String,
                publicInputs: TronSccpPublicInputsInput,
                publicSignalWords: [String],
                bundleBytes: Data = Data(),
                sourceProofBytes: Data = Data(),
                proofContext: TronSccpProofContext,
                statementHash: String,
                destinationBindingHash: String,
                requestHash: String,
                envelopeHash: String,
                destinationBinding: TronSccpDestinationBinding? = nil) {
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

/// Inputs used to package a TRON Groth16 proof for verifier-contract submission.
public struct TronSccpSubmissionInput: Equatable {
    public let publicInputs: TronSccpPublicInputsInput
    public let proofBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let sourceDomain: UInt32
    public let proofResult: TronSccpProofResult?
    public let publicSignalWords: [String]?

    public init(publicInputs: TronSccpPublicInputsInput,
                proofBytes: Data,
                statementHash: String,
                destinationBindingHash: String,
                sourceDomain: UInt32 = sccpDomainSora,
                proofResult: TronSccpProofResult? = nil,
                publicSignalWords: [String]? = nil) {
        self.publicInputs = publicInputs
        self.proofBytes = proofBytes
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.sourceDomain = sourceDomain
        self.proofResult = proofResult
        self.publicSignalWords = publicSignalWords
    }

    public init(publicInputs: TronSccpPublicInputsInput,
                proofBytes: Data,
                statementHash: String,
                destinationBinding: TronSccpDestinationBinding,
                sourceDomain: UInt32 = sccpDomainSora,
                proofResult: TronSccpProofResult? = nil,
                publicSignalWords: [String]? = nil) throws {
        let destinationBindingHash = try requireTronDestinationBindingForProofRequest(
            publicInputs: publicInputs,
            destinationBinding: destinationBinding,
            backend: sccpTronGroth16Bn254ProofBackendV1,
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

    public init(proofResult: TronSccpProofResult,
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

/// One TRON SCCP submission ABI argument in verifier-contract order.
public struct TronSccpSubmissionArgument: Equatable {
    public let key: String
    public let encoding: String
    public let bytesHex: String
}

/// TRON SCCP verifier-contract call data ready for wallet or relayer submission.
public struct TronSccpSubmission: Equatable {
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
    public let publicInputs: TronSccpPublicInputsInput
    public let publicInputWords: [String]
    public let publicSignalWords: [String]
    public let statementHash: String
    public let destinationBindingHash: String
    public let arguments: [TronSccpSubmissionArgument]
    public let proofBytes: Data
    public let callData: Data
    public let callDataHex: String
    public let envelopeBytes: Data
    public let envelopeHex: String
}

/// Error cases for TRON SCCP local proof request construction.
public enum TronSccpProverError: Error, Equatable {
    case invalidHex32(String)
    case zeroField(String)
    case invalidPublicInputs(String)
    case localProverUnavailable
    case emptyProof
    case allZeroProof
    case invalidProofLength(Int)
}

private func requireTronDestinationBindingForProofRequest(
    publicInputs: TronSccpPublicInputsInput,
    destinationBinding: TronSccpDestinationBinding,
    backend: String,
    sourceDomain: UInt32
) throws -> String {
    guard destinationBinding.version == 1 else {
        throw TronSccpProverError.invalidPublicInputs("destinationBinding.version")
    }
    guard destinationBinding.sourceDomain == sourceDomain else {
        throw TronSccpProverError.invalidPublicInputs("destinationBinding.sourceDomain")
    }
    guard destinationBinding.targetDomain == publicInputs.targetDomain else {
        throw TronSccpProverError.invalidPublicInputs("destinationBinding.targetDomain")
    }
    guard destinationBinding.verifierBackend == backend else {
        throw TronSccpProverError.invalidPublicInputs("destinationBinding.verifierBackend")
    }
    guard destinationBinding.proofFamily == sccpStarkFriProofFamilyV1 else {
        throw TronSccpProverError.invalidPublicInputs("destinationBinding.proofFamily")
    }
    let expectedDestinationBinding: TronSccpDestinationBinding
    do {
        expectedDestinationBinding = try sccpTronDestinationBinding(
            sourceDomain: sourceDomain,
            targetDomain: publicInputs.targetDomain,
            networkId: destinationBinding.networkId,
            verifierAddress: destinationBinding.verifierAddress,
            verifierCodeHash: destinationBinding.verifierCodeHash,
            verifierKeyHash: destinationBinding.verifierKeyHash,
            verifierBackend: backend,
            proofFamily: sccpStarkFriProofFamilyV1
        )
    } catch {
        throw TronSccpProverError.invalidPublicInputs("destinationBinding")
    }
    guard destinationBinding == expectedDestinationBinding else {
        throw TronSccpProverError.invalidPublicInputs("destinationBinding")
    }
    return expectedDestinationBinding.hash
}

/// Optional async witness resolver backed by app-controlled TRON RPC calls.
public protocol TronSccpWitnessProvider {
    func resolveWitness(_ input: TronSccpProofRequestInput) async throws -> TronSccpProofRequestInput
}

/// Local-first TRON SCCP proof wrapper. It never fabricates proofs; callers must link a prover.
public final class TronSccpProver {
    public typealias ProveFunction = (TronSccpProofRequest) async throws -> Data

    private let witnessProvider: TronSccpWitnessProvider?
    private let proveFunction: ProveFunction?

    public init(witnessProvider: TronSccpWitnessProvider? = nil,
                proveFunction: ProveFunction? = nil) {
        self.witnessProvider = witnessProvider
        self.proveFunction = proveFunction
    }

    public func buildRequest(_ input: TronSccpProofRequestInput) async throws -> TronSccpProofRequest {
        let resolved = try await witnessProvider?.resolveWitness(tronSccpWitnessProviderInputSnapshot(input)) ?? input
        return try buildTronSccpProofRequest(resolved)
    }

    public func prove(_ input: TronSccpProofRequestInput) async throws -> TronSccpProofResult {
        let request = try await buildRequest(input)
        guard let proveFunction else {
            throw TronSccpProverError.localProverUnavailable
        }
        try requireProductionTronSccpProofRequest(request)
        let proofBytes = try await proveFunction(tronSccpProofRequestCallbackSnapshot(request))
        return try wrapTronSccpProofResult(proofBytes: proofBytes, request: request)
    }
}

private func tronSccpWitnessProviderInputSnapshot(_ input: TronSccpProofRequestInput) -> TronSccpProofRequestInput {
    TronSccpProofRequestInput(
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

private func tronSccpProofRequestCallbackSnapshot(_ request: TronSccpProofRequest) -> TronSccpProofRequest {
    TronSccpProofRequest(
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

/// Canonical SCCP public-input bytes used by TRON proof requests.
public func canonicalTronSccpPublicInputsBytes(_ input: TronSccpPublicInputsInput) throws -> Data {
    guard input.version == 1 else {
        throw TronSccpProverError.invalidPublicInputs("publicInputs.version")
    }
    guard input.targetDomain != 0 else {
        throw TronSccpProverError.invalidPublicInputs("publicInputs.targetDomain")
    }
    guard input.targetDomain == sccpDomainTron else {
        throw TronSccpProverError.invalidPublicInputs("publicInputs.targetDomain")
    }
    guard input.finalityHeight != 0 else {
        throw TronSccpProverError.invalidPublicInputs("publicInputs.finalityHeight")
    }
    var out = Data()
    out.append(input.version)
    try out.append(tronNonZeroBytesFromHex32(input.messageId, field: "messageId"))
    try out.append(tronNonZeroBytesFromHex32(input.payloadHash, field: "payloadHash"))
    tronAppendU32Le(input.targetDomain, to: &out)
    try out.append(tronNonZeroBytesFromHex32(input.commitmentRoot, field: "commitmentRoot"))
    tronAppendU64Le(input.finalityHeight, to: &out)
    try out.append(tronNonZeroBytesFromHex32(input.finalityBlockHash, field: "finalityBlockHash"))
    return out
}

/// Derive the nine BN254 public signal words consumed by EVM/TRON Groth16 verifiers.
public func sccpGroth16Bn254PublicSignalWords(publicInputs: TronSccpPublicInputsInput,
                                              sourceDomain: UInt32,
                                              statementHash: String,
                                              destinationBindingHash: String) throws -> [String] {
    guard publicInputs.version == 1 else {
        throw TronSccpProverError.invalidPublicInputs("publicInputs.version")
    }
    guard publicInputs.targetDomain != 0 else {
        throw TronSccpProverError.invalidPublicInputs("publicInputs.targetDomain")
    }
    guard publicInputs.targetDomain == sccpDomainTron else {
        throw TronSccpProverError.invalidPublicInputs("publicInputs.targetDomain")
    }
    guard sourceDomain == sccpDomainSora else {
        throw TronSccpProverError.invalidPublicInputs("sourceDomain")
    }
    guard sourceDomain != publicInputs.targetDomain else {
        throw TronSccpProverError.invalidPublicInputs("sourceDomain")
    }
    guard publicInputs.finalityHeight != 0 else {
        throw TronSccpProverError.invalidPublicInputs("publicInputs.finalityHeight")
    }
    let values = try [
        tronNonZeroBytesFromHex32(publicInputs.messageId, field: "messageId"),
        tronNonZeroBytesFromHex32(publicInputs.payloadHash, field: "payloadHash"),
        tronAbiWordU32(publicInputs.targetDomain),
        tronNonZeroBytesFromHex32(publicInputs.commitmentRoot, field: "commitmentRoot"),
        tronAbiWordU64(publicInputs.finalityHeight),
        tronNonZeroBytesFromHex32(publicInputs.finalityBlockHash, field: "finalityBlockHash"),
        tronAbiWordU32(sourceDomain),
        tronNonZeroBytesFromHex32(statementHash, field: "statementHash"),
        tronNonZeroBytesFromHex32(destinationBindingHash, field: "destinationBindingHash"),
    ]
    return try values.enumerated().map { index, value in
        try tronGroth16Bn254SignalWord(label: tronGroth16SignalLabels[index], value: value)
    }
}

/// Build a TRON SCCP Groth16 proof request for a linked local prover.
public func buildTronSccpProofRequest(_ input: TronSccpProofRequestInput) throws -> TronSccpProofRequest {
    guard input.backend == sccpTronGroth16Bn254ProofBackendV1 else {
        throw TronSccpProverError.invalidPublicInputs("backend")
    }
    guard !input.bundleBytes.isEmpty else {
        throw TronSccpProverError.invalidPublicInputs("bundleBytes")
    }
    guard input.bundleBytes.count <= Int(UInt32.max) else {
        throw TronSccpProverError.invalidPublicInputs("bundleBytes")
    }
    guard input.sourceProofBytes.count <= Int(UInt32.max) else {
        throw TronSccpProverError.invalidPublicInputs("sourceProofBytes")
    }
    guard input.sourceProofBytes.isEmpty || input.sourceProofBytes.contains(where: { $0 != 0 }) else {
        throw TronSccpProverError.invalidPublicInputs("sourceProofBytes")
    }
    let publicInputsBytes = try canonicalTronSccpPublicInputsBytes(input.publicInputs)
    let proofContext = try normalizeTronSccpProofContext(
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
    tronAppendU32Le(UInt32(input.bundleBytes.count), to: &preimage)
    preimage.append(input.bundleBytes)
    tronAppendU32Le(UInt32(input.sourceProofBytes.count), to: &preimage)
    preimage.append(input.sourceProofBytes)
    try preimage.append(tronBytesFromHex32(proofContext.statementHash, field: "statementHash"))
    try preimage.append(tronBytesFromHex32(proofContext.destinationBindingHash, field: "destinationBindingHash"))
    for signal in publicSignalWords {
        try preimage.append(tronBytesFromHex32(signal, field: "publicSignalWords"))
    }
    return TronSccpProofRequest(
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
        requestHash: tronHashHex(prefix: "sccp:tron:groth16-proof-request:v1", payload: preimage),
        destinationBinding: input.destinationBinding
    )
}

private func normalizeTronSccpProofContext(statementHash: String,
                                           destinationBindingHash: String) throws -> TronSccpProofContext {
    TronSccpProofContext(
        version: 1,
        statementHash: try tronNormalizeHex32(statementHash, field: "statementHash"),
        destinationBindingHash: try tronNormalizeHex32(destinationBindingHash, field: "destinationBindingHash")
    )
}

public func wrapTronSccpProofResult(proofBytes: Data,
                                    request: TronSccpProofRequest) throws -> TronSccpProofResult {
    try requireProductionTronSccpProofRequest(request)
    try requireTronGroth16ProofBytes(
        proofBytes,
        publicInputs: request.publicInputs,
        sourceDomain: request.sourceDomain
    )
    var envelopePayload = try tronBytesFromHex32(request.requestHash, field: "requestHash")
    envelopePayload.append(proofBytes)
    return TronSccpProofResult(
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
        envelopeHash: tronHashHex(prefix: "sccp:tron:groth16-proof-envelope:v1", payload: envelopePayload),
        destinationBinding: request.destinationBinding
    )
}

private func requireWrappedTronProofResultForSubmission(
    _ proofResult: TronSccpProofResult
) throws -> TronSccpProofResult {
    guard proofResult.backend == sccpTronGroth16Bn254ProofBackendV1 else {
        throw TronSccpProverError.invalidPublicInputs("proofResult.backend")
    }
    let expectedProofContext = try normalizeTronSccpProofContext(
        statementHash: proofResult.statementHash,
        destinationBindingHash: proofResult.destinationBindingHash
    )
    guard proofResult.proofContext == expectedProofContext else {
        throw TronSccpProverError.invalidPublicInputs("proofResult.proofContext")
    }
    try requireTronGroth16ProofBytes(proofResult.proofBytes, publicInputs: proofResult.publicInputs)
    guard proofResult.proofBase64 == proofResult.proofBytes.base64EncodedString() else {
        throw TronSccpProverError.invalidPublicInputs("proofResult.proofBase64")
    }
    let requestHash = try tronNormalizeHex32(proofResult.requestHash, field: "proofResult.requestHash")
    let envelopeHash = try tronNormalizeHex32(proofResult.envelopeHash, field: "proofResult.envelopeHash")
    var envelopePayload = try tronBytesFromHex32(requestHash, field: "proofResult.requestHash")
    envelopePayload.append(proofResult.proofBytes)
    guard envelopeHash == tronHashHex(prefix: "sccp:tron:groth16-proof-envelope:v1", payload: envelopePayload) else {
        throw TronSccpProverError.invalidPublicInputs("proofResult.envelopeHash")
    }
    guard proofResult.sourceProofBytes.isEmpty || proofResult.sourceProofBytes.contains(where: { $0 != 0 }) else {
        throw TronSccpProverError.invalidPublicInputs("proofResult.sourceProofBytes")
    }
    let expectedRequest = try buildTronSccpProofRequest(TronSccpProofRequestInput(
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
        throw TronSccpProverError.invalidPublicInputs("proofResult.requestHash")
    }
    return proofResult
}

/// ABI words for the transparent public inputs passed to the TRON verifier contract.
public func tronSccpMessageTransparentPublicInputAbiWords(_ input: TronSccpPublicInputsInput) throws -> [String] {
    try tronSccpMessageTransparentPublicInputAbiWordBytes(input).map { "0x" + $0.hexEncodedString() }
}

/// ABI call data for `submitSccpMessageProof(bytes,bytes32[6],bytes32)`.
public func tronSccpSubmitMessageProofCallData(proofBytes: Data,
                                               publicInputs: TronSccpPublicInputsInput,
                                               statementHash: String,
                                               sourceDomain: UInt32 = sccpDomainSora) throws -> Data {
    guard sourceDomain == sccpDomainSora else {
        throw TronSccpProverError.invalidPublicInputs("sourceDomain")
    }
    try requireTronGroth16ProofBytes(proofBytes, publicInputs: publicInputs, sourceDomain: sourceDomain)
    let publicInputWords = try tronSccpMessageTransparentPublicInputAbiWordBytes(publicInputs)
    var out = Data(sccpTronSubmitMessageProofSelectorBytesV1)
    out.append(tronAbiWordU256(32 * 8))
    for word in publicInputWords {
        out.append(word)
    }
    try out.append(tronNonZeroBytesFromHex32(statementHash, field: "statementHash"))
    out.append(tronAbiWordU256(UInt64(proofBytes.count)))
    out.append(proofBytes)
    let padding = (32 - (proofBytes.count % 32)) % 32
    if padding > 0 {
        out.append(Data(repeating: 0, count: padding))
    }
    return out
}

/// Build TRON verifier-contract call data from UI-generated proof bytes.
public func buildTronSccpSubmission(_ input: TronSccpSubmissionInput) throws -> TronSccpSubmission {
    guard input.sourceDomain == sccpDomainSora else {
        throw TronSccpProverError.invalidPublicInputs("sourceDomain")
    }
    _ = try canonicalTronSccpPublicInputsBytes(input.publicInputs)
    try requireTronGroth16ProofBytes(
        input.proofBytes,
        publicInputs: input.publicInputs,
        sourceDomain: input.sourceDomain
    )
    let statementHash = try tronNormalizeHex32(input.statementHash, field: "statementHash")
    let destinationBindingHash = try tronNormalizeHex32(
        input.destinationBindingHash,
        field: "destinationBindingHash"
    )
    if let proofResult = input.proofResult {
        let proofResult = try requireWrappedTronProofResultForSubmission(proofResult)
        guard proofResult.backend == sccpTronGroth16Bn254ProofBackendV1 else {
            throw TronSccpProverError.invalidPublicInputs("proofResult.backend")
        }
        guard proofResult.publicInputs == input.publicInputs else {
            throw TronSccpProverError.invalidPublicInputs("proofResult.publicInputs")
        }
        guard proofResult.proofBytes == input.proofBytes else {
            throw TronSccpProverError.invalidPublicInputs("proofBytes")
        }
        guard proofResult.statementHash == statementHash else {
            throw TronSccpProverError.invalidPublicInputs("statementHash")
        }
        guard proofResult.destinationBindingHash == destinationBindingHash else {
            throw TronSccpProverError.invalidPublicInputs("destinationBindingHash")
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
            throw TronSccpProverError.invalidPublicInputs("publicSignalWords")
        }
        let normalizedSignals = try suppliedSignals.enumerated().map { index, word in
            "0x" + (try tronBytesFromHex32(word, field: "publicSignalWords[\(index)]")).hexEncodedString()
        }
        guard normalizedSignals == publicSignalWords else {
            throw TronSccpProverError.invalidPublicInputs("publicSignalWords")
        }
    }
    let publicInputWordsBytesArray = try tronSccpMessageTransparentPublicInputAbiWordBytes(input.publicInputs)
    let publicInputWordsBytes = publicInputWordsBytesArray.reduce(into: Data()) { out, word in
        out.append(word)
    }
    let publicInputWords = publicInputWordsBytesArray.map { "0x" + $0.hexEncodedString() }
    let callData = try tronSccpSubmitMessageProofCallData(
        proofBytes: input.proofBytes,
        publicInputs: input.publicInputs,
        statementHash: statementHash,
        sourceDomain: input.sourceDomain
    )
    let arguments = [
        TronSccpSubmissionArgument(
            key: "proof_bytes",
            encoding: "raw_bytes",
            bytesHex: "0x" + input.proofBytes.hexEncodedString()
        ),
        TronSccpSubmissionArgument(
            key: "public_inputs",
            encoding: "abi_bytes32x6",
            bytesHex: "0x" + publicInputWordsBytes.hexEncodedString()
        ),
        TronSccpSubmissionArgument(
            key: "statement_hash",
            encoding: "abi_bytes32",
            bytesHex: statementHash
        ),
    ]
    return TronSccpSubmission(
        version: 1,
        proofFamily: "stark-fri-v1",
        verifierBackend: sccpTronGroth16Bn254ProofBackendV1,
        platformPayload: "tron_contract_call",
        envelopeEncoding: sccpTronContractCallAbiTupleV1,
        submissionKind: "contract_call",
        verifierEntrypoint: sccpTronSubmitMessageProofEntrypointV1,
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

private func requireCanonicalTronSccpProofRequest(_ request: TronSccpProofRequest) throws {
    let expected = try buildTronSccpProofRequest(TronSccpProofRequestInput(
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
        throw TronSccpProverError.invalidPublicInputs("request")
    }
}

private func requireProductionTronSccpProofRequest(_ request: TronSccpProofRequest) throws {
    try requireCanonicalTronSccpProofRequest(request)
    guard request.version == 1 else {
        throw TronSccpProverError.invalidPublicInputs("request.version")
    }
    guard request.backend == sccpTronGroth16Bn254ProofBackendV1 else {
        throw TronSccpProverError.invalidPublicInputs("request.backend")
    }
    guard request.sourceDomain == sccpDomainSora else {
        throw TronSccpProverError.invalidPublicInputs("request.sourceDomain")
    }
    guard request.targetDomain == request.publicInputs.targetDomain,
          request.targetDomain == sccpDomainTron else {
        throw TronSccpProverError.invalidPublicInputs("request.targetDomain")
    }
    guard !request.bundleBytes.isEmpty else {
        throw TronSccpProverError.invalidPublicInputs("request.bundleBytes")
    }
    guard request.sourceProofBytes.isEmpty || request.sourceProofBytes.contains(where: { $0 != 0 }) else {
        throw TronSccpProverError.invalidPublicInputs("request.sourceProofBytes")
    }
    try requireProductionTronDestinationBinding(request)
}

private func requireProductionTronDestinationBinding(_ request: TronSccpProofRequest) throws {
    guard let destinationBinding = request.destinationBinding else {
        throw TronSccpProverError.invalidPublicInputs("request.destinationBinding")
    }
    let destinationBindingHash = try requireTronDestinationBindingForProofRequest(
        publicInputs: request.publicInputs,
        destinationBinding: destinationBinding,
        backend: request.backend,
        sourceDomain: request.sourceDomain
    )
    guard request.destinationBindingHash == destinationBindingHash else {
        throw TronSccpProverError.invalidPublicInputs("destinationBindingHash")
    }
    guard request.proofContext.destinationBindingHash == destinationBindingHash else {
        throw TronSccpProverError.invalidPublicInputs("proofContext.destinationBindingHash")
    }
}

private let tronGroth16SignalLabels = [
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

private let tronBn254ScalarFieldModulus = Data(hexString:
    "30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001"
)!
private let tronBn254BaseFieldModulus = Data(hexString:
    "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47"
)!

private func tronGroth16Bn254SignalWord(label: String, value: Data) throws -> String {
    let labelHash = irohaKeccak256(Data(label.utf8))
    var payload = Data(labelHash)
    payload.append(value)
    return "0x" + tronReduceModBn254(irohaKeccak256(payload)).hexEncodedString()
}

private func tronReduceModBn254(_ value: Data) -> Data {
    var out = Data(value)
    while tronCompareBytes(out, tronBn254ScalarFieldModulus) != .orderedAscending {
        out = tronSubtractBytes(out, tronBn254ScalarFieldModulus)
    }
    return out
}

private func tronCompareBytes(_ lhs: Data, _ rhs: Data) -> ComparisonResult {
    for (left, right) in zip(lhs, rhs) where left != right {
        return left < right ? .orderedAscending : .orderedDescending
    }
    if lhs.count == rhs.count {
        return .orderedSame
    }
    return lhs.count < rhs.count ? .orderedAscending : .orderedDescending
}

private func tronSubtractBytes(_ lhs: Data, _ rhs: Data) -> Data {
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

private func tronAbiWordU32(_ value: UInt32) -> Data {
    var out = Data(repeating: 0, count: 32)
    out[28] = UInt8((value >> 24) & 0xff)
    out[29] = UInt8((value >> 16) & 0xff)
    out[30] = UInt8((value >> 8) & 0xff)
    out[31] = UInt8(value & 0xff)
    return out
}

private func tronAbiWordU64(_ value: UInt64) -> Data {
    var out = Data(repeating: 0, count: 32)
    for offset in 0..<8 {
        out[24 + offset] = UInt8((value >> UInt64((7 - offset) * 8)) & 0xff)
    }
    return out
}

private func tronAbiWordU256(_ value: UInt64) -> Data {
    var out = Data(repeating: 0, count: 32)
    for offset in 0..<8 {
        out[24 + offset] = UInt8((value >> UInt64((7 - offset) * 8)) & 0xff)
    }
    return out
}

private func tronSccpMessageTransparentPublicInputAbiWordBytes(_ input: TronSccpPublicInputsInput) throws -> [Data] {
    _ = try canonicalTronSccpPublicInputsBytes(input)
    return try [
        tronNonZeroBytesFromHex32(input.messageId, field: "messageId"),
        tronNonZeroBytesFromHex32(input.payloadHash, field: "payloadHash"),
        tronAbiWordU32(input.targetDomain),
        tronNonZeroBytesFromHex32(input.commitmentRoot, field: "commitmentRoot"),
        tronAbiWordU64(input.finalityHeight),
        tronNonZeroBytesFromHex32(input.finalityBlockHash, field: "finalityBlockHash"),
    ]
}

private func requireTronGroth16ProofBytes(_ proofBytes: Data) throws {
    guard !proofBytes.isEmpty else {
        throw TronSccpProverError.emptyProof
    }
    guard proofBytes.contains(where: { $0 != 0 }) else {
        throw TronSccpProverError.allZeroProof
    }
    guard proofBytes.count == sccpGroth16Bn254ProofAbiByteLengthV1 else {
        throw TronSccpProverError.invalidProofLength(proofBytes.count)
    }
    try requireTronGroth16ProofTuple(proofBytes)
}

private func requireTronGroth16ProofBytes(_ proofBytes: Data,
                                          publicInputs: TronSccpPublicInputsInput,
                                          sourceDomain: UInt32? = nil) throws {
    try requireTronGroth16ProofBytes(proofBytes)
    guard tronGroth16ProofWord(proofBytes, index: 1) == (try tronNonZeroBytesFromHex32(
        publicInputs.messageId,
        field: "publicInputs.messageId"
    )) else {
        throw TronSccpProverError.invalidPublicInputs("proofBytes.messageId")
    }
    guard tronGroth16ProofWord(proofBytes, index: 3) == (try tronNonZeroBytesFromHex32(
        publicInputs.commitmentRoot,
        field: "publicInputs.commitmentRoot"
    )) else {
        throw TronSccpProverError.invalidPublicInputs("proofBytes.commitmentRoot")
    }
    if let sourceDomain {
        guard tronGroth16ProofWord(proofBytes, index: 2) == tronAbiWordU32(sourceDomain) else {
            throw TronSccpProverError.invalidPublicInputs("proofBytes.sourceDomain")
        }
    }
}

private func tronGroth16ProofWord(_ proofBytes: Data, index: Int) -> Data {
    let start = index * 32
    return Data(proofBytes[start..<(start + 32)])
}

private func tronGroth16ProofWordIsZero(_ proofBytes: Data, index: Int) -> Bool {
    !tronGroth16ProofWord(proofBytes, index: index).contains { $0 != 0 }
}

private func requireTronGroth16BaseFieldWord(_ proofBytes: Data, index: Int, field: String) throws {
    let word = tronGroth16ProofWord(proofBytes, index: index)
    guard tronCompareBytes(word, tronBn254BaseFieldModulus) == .orderedAscending else {
        throw TronSccpProverError.invalidPublicInputs(field)
    }
}

private func requireTronGroth16NonZeroPoint(_ proofBytes: Data, indexes: [Int], field: String) throws {
    guard indexes.contains(where: { !tronGroth16ProofWordIsZero(proofBytes, index: $0) }) else {
        throw TronSccpProverError.invalidPublicInputs(field)
    }
}

private func requireTronGroth16ProofTuple(_ proofBytes: Data) throws {
    if let field = sccpGroth16Bn254ProofTupleInvalidField(proofBytes) {
        throw TronSccpProverError.invalidPublicInputs(field)
    }
}

private func tronBytesFromHex32(_ value: String, field: String) throws -> Data {
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
        throw TronSccpProverError.invalidHex32(field)
    }
    var hex = value
    if hex.lowercased().hasPrefix("0x") {
        hex.removeFirst(2)
    }
    guard hex.unicodeScalars.allSatisfy({ !CharacterSet.whitespacesAndNewlines.contains($0) }) else {
        throw TronSccpProverError.invalidHex32(field)
    }
    hex = hex.lowercased()
    guard hex.count == 64, let bytes = Data(hexString: hex), bytes.count == 32 else {
        throw TronSccpProverError.invalidHex32(field)
    }
    return bytes
}

private func tronNormalizeHex32(_ value: String, field: String) throws -> String {
    "0x" + (try tronNonZeroBytesFromHex32(value, field: field)).hexEncodedString()
}

private func tronNonZeroBytesFromHex32(_ value: String, field: String) throws -> Data {
    let bytes = try tronBytesFromHex32(value, field: field)
    guard bytes.contains(where: { $0 != 0 }) else {
        throw TronSccpProverError.zeroField(field)
    }
    return bytes
}

private func tronAppendU32Le(_ value: UInt32, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 24) & 0xff))
}

private func tronAppendU64Le(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 0, through: 56, by: 8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func tronHashHex(prefix: String, payload: Data) -> String {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return "0x" + Blake2b.hash256(preimage).hexEncodedString()
}

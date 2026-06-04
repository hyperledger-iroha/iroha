import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// Proof backend id expected by EVM-family SCCP Groth16 verifier contracts.
public let sccpEvmGroth16Bn254ProofBackendV1 = "evm-groth16-bn254-v1"
/// Canonical byte length of the static BN254 Groth16 ABI proof tuple.
public let sccpGroth16Bn254ProofAbiByteLengthV1 = 384
/// EVM-family contract-call envelope encoding used by SCCP verifier submissions.
public let sccpEvmContractCallAbiTupleV1 = "abi_tuple_v1"
/// Local-admission Norito envelope encoding used by EVM-family inbound submissions.
public let sccpLocalAdmissionEnvelopeEncodingV1 = "norito:sccp-local-admission:v1"
/// Local-admission submission kind used by EVM-family inbound submissions.
public let sccpLocalAdmissionSubmissionKindV1 = "local_admission"
/// Local-admission Torii/core entrypoint used by EVM-family inbound submissions.
public let sccpLocalAdmissionEntrypointV1 = "SubmitBridgeProof"
/// Solidity ABI signature for SCCP Groth16 message proof submission.
public let sccpSubmitMessageProofAbiV1 = "submitSccpMessageProof(bytes,bytes32[6],bytes32)"
/// Keccak function selector for `submitSccpMessageProof(bytes,bytes32[6],bytes32)`.
public let sccpSubmitMessageProofSelectorV1 = "0xbd57826c"
/// Solidity ABI signature for SCCP EVM-family source-event logs.
public let sccpEvmSourceEventAbiV1 = "SccpSourceEvent(bytes32)"

/// Return the EVM SCCP source-event topic for `SccpSourceEvent(bytes32)`.
public func evmSccpSourceEventTopic() -> String {
    "0x" + irohaKeccak256(Data(sccpEvmSourceEventAbiV1.utf8)).hexEncodedString()
}

private let sccpSubmitMessageProofSelectorBytesV1 = Data([0xbd, 0x57, 0x82, 0x6c])
private let sccpEvmSubmitMessageProofEntrypointV1 =
    "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)"
private let ethereumMainnetBeaconRestMaxResponseBytes = 1024 * 1024
private let ethereumMainnetSecondsPerSlot: UInt64 = 12

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

/// Inputs used to package BSC -> SORA local-admission verifier output.
public struct BscMainnetLocalAdmissionSubmissionInput: Equatable {
    public let proofBytes: Data
    public let publicInputsBytes: Data
    public let bundleBytes: Data
    public let envelopeBytes: Data
    public let statementHash: String
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterEngineDeploymentHash: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let proofFamily: String
    public let verifierBackend: String
    public let envelopeEncoding: String
    public let submissionKind: String
    public let verifierEntrypoint: String

    public init(proofBytes: Data,
                publicInputsBytes: Data,
                bundleBytes: Data,
                envelopeBytes: Data,
                statementHash: String,
                sourceVerifierMaterialHash: String,
                sourceAdapterEngineDeploymentHash: String,
                sourceDomain: UInt32 = sccpDomainBsc,
                targetDomain: UInt32 = sccpDomainSora,
                proofFamily: String = sccpStarkFriProofFamilyV1,
                verifierBackend: String = sccpEvmGroth16Bn254ProofBackendV1,
                envelopeEncoding: String = sccpLocalAdmissionEnvelopeEncodingV1,
                submissionKind: String = sccpLocalAdmissionSubmissionKindV1,
                verifierEntrypoint: String = sccpLocalAdmissionEntrypointV1) {
        self.proofBytes = proofBytes
        self.publicInputsBytes = publicInputsBytes
        self.bundleBytes = bundleBytes
        self.envelopeBytes = envelopeBytes
        self.statementHash = statementHash
        self.sourceVerifierMaterialHash = sourceVerifierMaterialHash
        self.sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash
        self.sourceDomain = sourceDomain
        self.targetDomain = targetDomain
        self.proofFamily = proofFamily
        self.verifierBackend = verifierBackend
        self.envelopeEncoding = envelopeEncoding
        self.submissionKind = submissionKind
        self.verifierEntrypoint = verifierEntrypoint
    }
}

/// BSC local-admission payload mirrored from the core SCCP package.
public struct BscMainnetLocalAdmissionPayload: Equatable {
    public let version: UInt8
    public let proofBytes: Data
    public let proofBytesHex: String
    public let publicInputsBytes: Data
    public let publicInputsBytesHex: String
    public let bundleBytes: Data
    public let bundleBytesHex: String
    public let statementHash: String
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterEngineDeploymentHash: String
}

/// BSC -> SORA local-admission package ready for Torii bridge-proof submission.
public struct BscMainnetLocalAdmissionSubmission: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let verifierBackend: String
    public let platformPayload: String
    public let envelopeEncoding: String
    public let submissionKind: String
    public let verifierEntrypoint: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let statementHash: String
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterEngineDeploymentHash: String
    public let arguments: [EvmSccpSubmissionArgument]
    public let localAdmission: BscMainnetLocalAdmissionPayload
    public let proofBytes: Data
    public let proofBytesHex: String
    public let publicInputsBytes: Data
    public let publicInputsBytesHex: String
    public let bundleBytes: Data
    public let bundleBytesHex: String
    public let envelopeBytes: Data
    public let envelopeHex: String
}

/// Inputs used to package Ethereum mainnet -> SORA local-admission verifier output.
public struct EthereumMainnetLocalAdmissionSubmissionInput: Equatable {
    public let proofBytes: Data
    public let publicInputsBytes: Data
    public let bundleBytes: Data
    public let envelopeBytes: Data
    public let statementHash: String
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterEngineDeploymentHash: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let proofFamily: String
    public let verifierBackend: String
    public let envelopeEncoding: String
    public let submissionKind: String
    public let verifierEntrypoint: String

    public init(proofBytes: Data,
                publicInputsBytes: Data,
                bundleBytes: Data,
                envelopeBytes: Data,
                statementHash: String,
                sourceVerifierMaterialHash: String,
                sourceAdapterEngineDeploymentHash: String,
                sourceDomain: UInt32 = sccpDomainEthereum,
                targetDomain: UInt32 = sccpDomainSora,
                proofFamily: String = sccpStarkFriProofFamilyV1,
                verifierBackend: String = sccpEvmGroth16Bn254ProofBackendV1,
                envelopeEncoding: String = sccpLocalAdmissionEnvelopeEncodingV1,
                submissionKind: String = sccpLocalAdmissionSubmissionKindV1,
                verifierEntrypoint: String = sccpLocalAdmissionEntrypointV1) {
        self.proofBytes = proofBytes
        self.publicInputsBytes = publicInputsBytes
        self.bundleBytes = bundleBytes
        self.envelopeBytes = envelopeBytes
        self.statementHash = statementHash
        self.sourceVerifierMaterialHash = sourceVerifierMaterialHash
        self.sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash
        self.sourceDomain = sourceDomain
        self.targetDomain = targetDomain
        self.proofFamily = proofFamily
        self.verifierBackend = verifierBackend
        self.envelopeEncoding = envelopeEncoding
        self.submissionKind = submissionKind
        self.verifierEntrypoint = verifierEntrypoint
    }
}

/// Ethereum mainnet local-admission payload mirrored from the core SCCP package.
public struct EthereumMainnetLocalAdmissionPayload: Equatable {
    public let version: UInt8
    public let proofBytes: Data
    public let proofBytesHex: String
    public let publicInputsBytes: Data
    public let publicInputsBytesHex: String
    public let bundleBytes: Data
    public let bundleBytesHex: String
    public let statementHash: String
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterEngineDeploymentHash: String
}

/// Ethereum mainnet -> SORA local-admission package ready for Torii bridge-proof submission.
public struct EthereumMainnetLocalAdmissionSubmission: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let verifierBackend: String
    public let platformPayload: String
    public let envelopeEncoding: String
    public let submissionKind: String
    public let verifierEntrypoint: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let statementHash: String
    public let sourceVerifierMaterialHash: String
    public let sourceAdapterEngineDeploymentHash: String
    public let arguments: [EvmSccpSubmissionArgument]
    public let localAdmission: EthereumMainnetLocalAdmissionPayload
    public let proofBytes: Data
    public let proofBytesHex: String
    public let publicInputsBytes: Data
    public let publicInputsBytesHex: String
    public let bundleBytes: Data
    public let bundleBytesHex: String
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
        return try await prove(request)
    }

    public func prove(_ request: EvmSccpProofRequest) async throws -> EvmSccpProofResult {
        guard let proveFunction else {
            throw EvmSccpProverError.localProverUnavailable
        }
        try requireProductionEvmSccpProofRequest(request)
        let proofBytes = try await proveFunction(evmSccpProofRequestCallbackSnapshot(request))
        return try wrapEvmSccpProofResult(proofBytes: proofBytes, request: request)
    }
}

private func requireBscMainnetDestinationBinding(_ request: EvmSccpProofRequest) throws {
    guard request.targetDomain == sccpDomainBsc,
          request.publicInputs.targetDomain == sccpDomainBsc else {
        throw EvmSccpProverError.invalidPublicInputs("request.targetDomain")
    }
    guard let destinationBinding = request.destinationBinding else {
        throw EvmSccpProverError.invalidPublicInputs("request.destinationBinding")
    }
    guard destinationBinding.targetDomain == sccpDomainBsc else {
        throw EvmSccpProverError.invalidPublicInputs("destinationBinding.targetDomain")
    }
    guard destinationBinding.networkId == sccpBscMainnetNetworkId else {
        throw EvmSccpProverError.invalidPublicInputs("destinationBinding.networkId")
    }
    let expectedHash = try requireEvmDestinationBindingForProofRequest(
        publicInputs: request.publicInputs,
        destinationBinding: destinationBinding,
        backend: request.backend,
        sourceDomain: request.sourceDomain
    )
    guard request.destinationBindingHash == expectedHash else {
        throw EvmSccpProverError.invalidPublicInputs("destinationBindingHash")
    }
}

/// Build a BSC mainnet-only SCCP Groth16 proof request.
public func buildBscMainnetSccpDestinationProofRequest(
    _ input: EvmSccpProofRequestInput
) throws -> EvmSccpProofRequest {
    let request = try buildEvmSccpProofRequest(input)
    try requireBscMainnetDestinationBinding(request)
    return request
}

/// Wrap externally generated BSC mainnet SCCP Groth16 proof bytes against a checked request.
public func wrapBscMainnetSccpDestinationProofResult(
    proofBytes: Data,
    request: EvmSccpProofRequest
) throws -> EvmSccpProofResult {
    try requireBscMainnetDestinationBinding(request)
    return try wrapEvmSccpProofResult(proofBytes: proofBytes, request: request)
}

/// Build BSC mainnet verifier-contract call data from a wrapped proof result.
public func buildBscMainnetSccpDestinationSubmission(
    _ input: EvmSccpSubmissionInput
) throws -> EvmSccpSubmission {
    let submission = try buildEvmSccpSubmission(input)
    guard submission.targetDomain == sccpDomainBsc,
          input.publicInputs.targetDomain == sccpDomainBsc else {
        throw EvmSccpProverError.invalidPublicInputs("publicInputs.targetDomain")
    }
    guard let proofResult = input.proofResult,
          let destinationBinding = proofResult.destinationBinding,
          destinationBinding.targetDomain == sccpDomainBsc,
          destinationBinding.networkId == sccpBscMainnetNetworkId,
          destinationBinding.hash == proofResult.destinationBindingHash else {
        throw EvmSccpProverError.invalidPublicInputs("proofResult.destinationBinding")
    }
    return submission
}

/// Build a BSC -> SORA local-admission package from native verifier output.
public func buildBscMainnetSccpLocalAdmissionSubmission(
    _ input: BscMainnetLocalAdmissionSubmissionInput
) throws -> BscMainnetLocalAdmissionSubmission {
    guard input.sourceDomain == sccpDomainBsc, input.targetDomain == sccpDomainSora else {
        throw EvmSccpProverError.invalidPublicInputs("BSC -> SORA")
    }
    guard input.proofFamily == sccpStarkFriProofFamilyV1,
          input.verifierBackend == sccpEvmGroth16Bn254ProofBackendV1,
          input.envelopeEncoding == sccpLocalAdmissionEnvelopeEncodingV1,
          input.submissionKind == sccpLocalAdmissionSubmissionKindV1,
          input.verifierEntrypoint == sccpLocalAdmissionEntrypointV1 else {
        throw EvmSccpProverError.invalidPublicInputs("localAdmission.metadata")
    }
    let proofBytes = try requireEvmLocalAdmissionBytes(input.proofBytes, field: "proofBytes")
    let publicInputsBytes = try requireEvmLocalAdmissionBytes(
        input.publicInputsBytes,
        field: "publicInputsBytes"
    )
    let bundleBytes = try requireEvmLocalAdmissionBytes(input.bundleBytes, field: "bundleBytes")
    let envelopeBytes = try requireEvmLocalAdmissionBytes(input.envelopeBytes, field: "envelopeBytes")
    let statementHash = try evmNormalizeHex32(input.statementHash, field: "statementHash")
    let sourceVerifierMaterialHash = try evmNormalizeHex32(
        input.sourceVerifierMaterialHash,
        field: "sourceVerifierMaterialHash"
    )
    let sourceAdapterEngineDeploymentHash = try evmNormalizeHex32(
        input.sourceAdapterEngineDeploymentHash,
        field: "sourceAdapterEngineDeploymentHash"
    )
    let payload = BscMainnetLocalAdmissionPayload(
        version: 1,
        proofBytes: proofBytes,
        proofBytesHex: "0x" + proofBytes.hexEncodedString(),
        publicInputsBytes: publicInputsBytes,
        publicInputsBytesHex: "0x" + publicInputsBytes.hexEncodedString(),
        bundleBytes: bundleBytes,
        bundleBytesHex: "0x" + bundleBytes.hexEncodedString(),
        statementHash: statementHash,
        sourceVerifierMaterialHash: sourceVerifierMaterialHash,
        sourceAdapterEngineDeploymentHash: sourceAdapterEngineDeploymentHash
    )
    return BscMainnetLocalAdmissionSubmission(
        version: 1,
        proofFamily: input.proofFamily,
        verifierBackend: input.verifierBackend,
        platformPayload: sccpLocalAdmissionSubmissionKindV1,
        envelopeEncoding: sccpLocalAdmissionEnvelopeEncodingV1,
        submissionKind: sccpLocalAdmissionSubmissionKindV1,
        verifierEntrypoint: sccpLocalAdmissionEntrypointV1,
        sourceDomain: sccpDomainBsc,
        targetDomain: sccpDomainSora,
        statementHash: statementHash,
        sourceVerifierMaterialHash: sourceVerifierMaterialHash,
        sourceAdapterEngineDeploymentHash: sourceAdapterEngineDeploymentHash,
        arguments: [],
        localAdmission: payload,
        proofBytes: proofBytes,
        proofBytesHex: "0x" + proofBytes.hexEncodedString(),
        publicInputsBytes: publicInputsBytes,
        publicInputsBytesHex: "0x" + publicInputsBytes.hexEncodedString(),
        bundleBytes: bundleBytes,
        bundleBytesHex: "0x" + bundleBytes.hexEncodedString(),
        envelopeBytes: envelopeBytes,
        envelopeHex: "0x" + envelopeBytes.hexEncodedString()
    )
}

/// Build an Ethereum mainnet -> SORA local-admission package from native verifier output.
public func buildEthereumMainnetSccpLocalAdmissionSubmission(
    _ input: EthereumMainnetLocalAdmissionSubmissionInput
) throws -> EthereumMainnetLocalAdmissionSubmission {
    guard input.sourceDomain == sccpDomainEthereum, input.targetDomain == sccpDomainSora else {
        throw EvmSccpProverError.invalidPublicInputs("ETH -> SORA")
    }
    guard input.proofFamily == sccpStarkFriProofFamilyV1,
          input.verifierBackend == sccpEvmGroth16Bn254ProofBackendV1,
          input.envelopeEncoding == sccpLocalAdmissionEnvelopeEncodingV1,
          input.submissionKind == sccpLocalAdmissionSubmissionKindV1,
          input.verifierEntrypoint == sccpLocalAdmissionEntrypointV1 else {
        throw EvmSccpProverError.invalidPublicInputs("localAdmission.metadata")
    }
    let proofBytes = try requireEvmLocalAdmissionBytes(input.proofBytes, field: "proofBytes")
    let publicInputsBytes = try requireEvmLocalAdmissionBytes(
        input.publicInputsBytes,
        field: "publicInputsBytes"
    )
    let bundleBytes = try requireEvmLocalAdmissionBytes(input.bundleBytes, field: "bundleBytes")
    let envelopeBytes = try requireEvmLocalAdmissionBytes(input.envelopeBytes, field: "envelopeBytes")
    let statementHash = try evmNormalizeHex32(input.statementHash, field: "statementHash")
    let sourceVerifierMaterialHash = try evmNormalizeHex32(
        input.sourceVerifierMaterialHash,
        field: "sourceVerifierMaterialHash"
    )
    let sourceAdapterEngineDeploymentHash = try evmNormalizeHex32(
        input.sourceAdapterEngineDeploymentHash,
        field: "sourceAdapterEngineDeploymentHash"
    )
    let payload = EthereumMainnetLocalAdmissionPayload(
        version: 1,
        proofBytes: proofBytes,
        proofBytesHex: "0x" + proofBytes.hexEncodedString(),
        publicInputsBytes: publicInputsBytes,
        publicInputsBytesHex: "0x" + publicInputsBytes.hexEncodedString(),
        bundleBytes: bundleBytes,
        bundleBytesHex: "0x" + bundleBytes.hexEncodedString(),
        statementHash: statementHash,
        sourceVerifierMaterialHash: sourceVerifierMaterialHash,
        sourceAdapterEngineDeploymentHash: sourceAdapterEngineDeploymentHash
    )
    return EthereumMainnetLocalAdmissionSubmission(
        version: 1,
        proofFamily: input.proofFamily,
        verifierBackend: input.verifierBackend,
        platformPayload: sccpLocalAdmissionSubmissionKindV1,
        envelopeEncoding: sccpLocalAdmissionEnvelopeEncodingV1,
        submissionKind: sccpLocalAdmissionSubmissionKindV1,
        verifierEntrypoint: sccpLocalAdmissionEntrypointV1,
        sourceDomain: sccpDomainEthereum,
        targetDomain: sccpDomainSora,
        statementHash: statementHash,
        sourceVerifierMaterialHash: sourceVerifierMaterialHash,
        sourceAdapterEngineDeploymentHash: sourceAdapterEngineDeploymentHash,
        arguments: [],
        localAdmission: payload,
        proofBytes: proofBytes,
        proofBytesHex: "0x" + proofBytes.hexEncodedString(),
        publicInputsBytes: publicInputsBytes,
        publicInputsBytesHex: "0x" + publicInputsBytes.hexEncodedString(),
        bundleBytes: bundleBytes,
        bundleBytesHex: "0x" + bundleBytes.hexEncodedString(),
        envelopeBytes: envelopeBytes,
        envelopeHex: "0x" + envelopeBytes.hexEncodedString()
    )
}

/// Local-first BSC mainnet SCCP proof wrapper. It enforces BSC target domain and chain id 56.
public final class BscMainnetSccpProver {
    public typealias ProveFunction = EvmSccpProver.ProveFunction

    private let witnessProvider: EvmSccpWitnessProvider?
    private let proveFunction: ProveFunction?

    public init(witnessProvider: EvmSccpWitnessProvider? = nil,
                proveFunction: ProveFunction? = nil) {
        self.witnessProvider = witnessProvider
        self.proveFunction = proveFunction
    }

    public func buildRequest(_ input: EvmSccpProofRequestInput) async throws -> EvmSccpProofRequest {
        let resolved = try await witnessProvider?.resolveWitness(evmSccpWitnessProviderInputSnapshot(input)) ?? input
        return try buildBscMainnetSccpDestinationProofRequest(resolved)
    }

    public func prove(_ input: EvmSccpProofRequestInput) async throws -> EvmSccpProofResult {
        let request = try await buildRequest(input)
        guard let proveFunction else {
            throw EvmSccpProverError.localProverUnavailable
        }
        try requireProductionEvmSccpProofRequest(request)
        try requireBscMainnetDestinationBinding(request)
        let proofBytes = try await proveFunction(evmSccpProofRequestCallbackSnapshot(request))
        return try wrapBscMainnetSccpDestinationProofResult(proofBytes: proofBytes, request: request)
    }
}

/// App-supplied BSC JSON-RPC execution provider for native SCCP evidence collection.
public protocol BscMainnetExecutionProvider {
    func request(method: String, params: [Any]) async throws -> Any
}

/// App-supplied BSC Parlia finality collector for native SCCP evidence collection.
public protocol BscMainnetConsensusProvider {
    func collectFinalityEvidence(
        receipt: [String: Any]?,
        block: [String: Any]?,
        transactionHash: String?
    ) async throws -> [String: Any]
}

/// Typed BSC Parlia finality evidence required before inbound source proving.
public struct BscMainnetParliaFinalityEvidence {
    public let executionBlockNumber: String
    public let executionBlockHash: String
    public let executionReceiptsRoot: String
    public let additionalFields: [String: Any]

    public init(executionBlockNumber: String,
                executionBlockHash: String,
                executionReceiptsRoot: String,
                additionalFields: [String: Any] = [:]) {
        self.executionBlockNumber = executionBlockNumber
        self.executionBlockHash = executionBlockHash
        self.executionReceiptsRoot = executionReceiptsRoot
        self.additionalFields = additionalFields
    }

    public var dictionary: [String: Any] {
        var value = additionalFields
        value["executionBlockNumber"] = executionBlockNumber
        value["executionBlockHash"] = executionBlockHash
        value["executionReceiptsRoot"] = executionReceiptsRoot
        return value
    }
}

/// BSC mainnet receipt-proof transcript collected from app-supplied providers.
public struct BscMainnetReceiptProof {
    public let sourceDomain: UInt32
    public let sourceEventDigest: String
    public let validatorEpoch: UInt64
    public let blockNumber: UInt64
    public let blockHash: String
    public let receiptsRoot: String
    public let validatorSetHash: String
    public let commitSealHash: String
    public let receiptRootIndex: UInt64
    public let receiptTrieProofNodes: [Data]
    public let inclusionBranch: [Data]

    public init(sourceDomain: UInt32 = sccpDomainBsc,
                sourceEventDigest: String,
                validatorEpoch: UInt64,
                blockNumber: UInt64,
                blockHash: String,
                receiptsRoot: String,
                validatorSetHash: String,
                commitSealHash: String,
                receiptRootIndex: UInt64,
                receiptTrieProofNodes: [Data],
                inclusionBranch: [Data]) {
        self.sourceDomain = sourceDomain
        self.sourceEventDigest = sourceEventDigest
        self.validatorEpoch = validatorEpoch
        self.blockNumber = blockNumber
        self.blockHash = blockHash
        self.receiptsRoot = receiptsRoot
        self.validatorSetHash = validatorSetHash
        self.commitSealHash = commitSealHash
        self.receiptRootIndex = receiptRootIndex
        self.receiptTrieProofNodes = receiptTrieProofNodes
        self.inclusionBranch = inclusionBranch
    }
}

/// Locally collected BSC mainnet inbound evidence before source-proof generation.
public struct BscMainnetInboundEvidence {
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let transactionHash: String?
    public let receipt: [String: Any]?
    public let block: [String: Any]?
    public let parliaFinality: [String: Any]?
    public let receiptProof: BscMainnetReceiptProof?
    public let receiptProofHash: String?
    public let sourceEventDigest: String?
    public let sourceBridgeEmitterAddress: String?

    public init(sourceDomain: UInt32 = sccpDomainBsc,
                targetDomain: UInt32 = sccpDomainSora,
                transactionHash: String? = nil,
                receipt: [String: Any]? = nil,
                block: [String: Any]? = nil,
                parliaFinality: [String: Any]? = nil,
                receiptProof: BscMainnetReceiptProof? = nil,
                receiptProofHash: String? = nil,
                sourceEventDigest: String? = nil,
                sourceBridgeEmitterAddress: String? = nil) {
        self.sourceDomain = sourceDomain
        self.targetDomain = targetDomain
        self.transactionHash = transactionHash
        self.receipt = receipt
        self.block = block
        self.parliaFinality = parliaFinality
        self.receiptProof = receiptProof
        self.receiptProofHash = receiptProofHash
        self.sourceEventDigest = sourceEventDigest
        self.sourceBridgeEmitterAddress = sourceBridgeEmitterAddress
    }

    public init(sourceDomain: UInt32 = sccpDomainBsc,
                targetDomain: UInt32 = sccpDomainSora,
                transactionHash: String? = nil,
                receipt: [String: Any]? = nil,
                block: [String: Any]? = nil,
                parliaFinalityEvidence: BscMainnetParliaFinalityEvidence?,
                receiptProof: BscMainnetReceiptProof? = nil,
                receiptProofHash: String? = nil,
                sourceEventDigest: String? = nil,
                sourceBridgeEmitterAddress: String? = nil) {
        self.init(
            sourceDomain: sourceDomain,
            targetDomain: targetDomain,
            transactionHash: transactionHash,
            receipt: receipt,
            block: block,
            parliaFinality: parliaFinalityEvidence?.dictionary,
            receiptProof: receiptProof,
            receiptProofHash: receiptProofHash,
            sourceEventDigest: sourceEventDigest,
            sourceBridgeEmitterAddress: sourceBridgeEmitterAddress
        )
    }
}

/// Local-first BSC mainnet SCCP API for native proof generation and EVM submission payloads.
public final class BscMainnetSccp {
    public typealias ProveFunction = EvmSccpProver.ProveFunction
    public typealias InboundProveFunction = (BscMainnetInboundEvidence) async throws -> Data
    public typealias InboundSubmitFunction = (Data) async throws -> Any
    public typealias OutboundSubmitFunction = (EvmSccpSubmission) async throws -> Any

    private let prover: BscMainnetSccpProver
    private let executionProvider: BscMainnetExecutionProvider?
    private let consensusProvider: BscMainnetConsensusProvider?
    private let inboundProveFunction: InboundProveFunction?
    private let inboundSubmitFunction: InboundSubmitFunction?
    private let outboundSubmitFunction: OutboundSubmitFunction?
    private let sourceBridgeEmitterAddress: String?

    public init(witnessProvider: EvmSccpWitnessProvider? = nil,
                proveFunction: ProveFunction? = nil,
                executionProvider: BscMainnetExecutionProvider? = nil,
                consensusProvider: BscMainnetConsensusProvider? = nil,
                inboundProveFunction: InboundProveFunction? = nil,
                inboundSubmitFunction: InboundSubmitFunction? = nil,
                outboundSubmitFunction: OutboundSubmitFunction? = nil,
                sourceBridgeEmitterAddress: String? = nil) {
        self.prover = BscMainnetSccpProver(witnessProvider: witnessProvider, proveFunction: proveFunction)
        self.executionProvider = executionProvider
        self.consensusProvider = consensusProvider
        self.inboundProveFunction = inboundProveFunction
        self.inboundSubmitFunction = inboundSubmitFunction
        self.outboundSubmitFunction = outboundSubmitFunction
        self.sourceBridgeEmitterAddress = sourceBridgeEmitterAddress
    }

    public static func requireMainnetChainId(_ chainId: UInt64) throws {
        guard chainId == sccpBscMainnetChainId else {
            throw EvmSccpProverError.invalidPublicInputs("eth_chainId")
        }
    }

    public static func destinationBinding(verifierAddress: String,
                                          bridgeAddress: String,
                                          verifierCodeHash: String,
                                          verifierKeyHash: String,
                                          networkId: String = sccpBscMainnetNetworkId) throws -> EvmSccpDestinationBinding {
        try sccpBscMainnetDestinationBinding(
            verifierAddress: verifierAddress,
            bridgeAddress: bridgeAddress,
            verifierCodeHash: verifierCodeHash,
            verifierKeyHash: verifierKeyHash,
            networkId: networkId
        )
    }

    public func validateExecutionProviderMainnet(_ provider: BscMainnetExecutionProvider? = nil) async throws -> Any {
        guard let selectedProvider = provider ?? executionProvider else {
            throw EvmSccpProverError.localProverUnavailable
        }
        let chainId = try await selectedProvider.request(method: "eth_chainId", params: [])
        try Self.requireMainnetChainId(Self.normalizeRpcChainId(chainId))
        return chainId
    }

    private static func evmReceiptSourceEvent(
        receipt: [String: Any],
        sourceEventDigest inputDigest: String?,
        sourceBridgeEmitterAddress inputAddress: String?,
        transactionHash expectedTransactionHash: String?,
        blockHash expectedBlockHash: String?,
        blockNumber expectedBlockNumber: String?
    ) throws -> (sourceEventDigest: String?, sourceBridgeEmitterAddress: String?) {
        let expectedDigest = try inputDigest.map {
            try normalizeRpcHex($0, label: "sourceEventDigest", byteLength: 32)
        }
        let expectedAddress = try inputAddress.map {
            try normalizeRpcHex($0, label: "sourceBridgeEmitterAddress", byteLength: 20)
        }
        if expectedDigest == nil && expectedAddress == nil {
            return (nil, nil)
        }
        guard let expectedAddress else {
            throw EvmSccpProverError.invalidPublicInputs("sourceBridgeEmitterAddress")
        }
        guard let logs = receipt["logs"] as? [[String: Any]] else {
            throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
        }
        let sourceEventTopic = evmSccpSourceEventTopic()
        var matchedDigest: String?
        for (index, log) in logs.enumerated() {
            if (log["removed"] as? Bool) == true {
                throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
            }
            let address = try normalizeRpcHex(
                log["address"],
                label: "receipt.logs[\(index)].address",
                byteLength: 20,
                allowZero: true
            )
            guard let topics = log["topics"] as? [Any], topics.count <= 4 else {
                throw EvmSccpProverError.invalidPublicInputs("receipt.logs[\(index)].topics")
            }
            let normalizedTopics = try topics.enumerated().map { topicIndex, topic in
                try normalizeRpcHex(
                    topic,
                    label: "receipt.logs[\(index)].topics[\(topicIndex)]",
                    byteLength: 32,
                    allowZero: true
                )
            }
            if address == expectedAddress,
               normalizedTopics.first == sourceEventTopic {
                guard normalizedTopics.count == 2 else {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs[\(index)].topics")
                }
                guard let data = log["data"] as? String else {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs[\(index)].data")
                }
                guard data == "0x" else {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs[\(index)].data")
                }
                let logTransactionHash = try normalizeRpcHex(
                    strictFirstPresent(
                        log,
                        label: "receipt.logs[\(index)].transactionHash",
                        "transactionHash",
                        "transaction_hash"
                    ),
                    label: "receipt.logs[\(index)].transactionHash",
                    byteLength: 32
                )
                if let expectedTransactionHash, logTransactionHash != expectedTransactionHash {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
                }
                let logBlockHash = try normalizeRpcHex(
                    strictFirstPresent(
                        log,
                        label: "receipt.logs[\(index)].blockHash",
                        "blockHash",
                        "block_hash"
                    ),
                    label: "receipt.logs[\(index)].blockHash",
                    byteLength: 32
                )
                if let expectedBlockHash, logBlockHash != expectedBlockHash {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
                }
                let logBlockNumber = try normalizePositiveRpcQuantity(
                    strictFirstPresent(
                        log,
                        label: "receipt.logs[\(index)].blockNumber",
                        "blockNumber",
                        "block_number"
                    ),
                    label: "receipt.logs[\(index)].blockNumber"
                )
                if let expectedBlockNumber, logBlockNumber != expectedBlockNumber {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
                }
                let candidateDigest = normalizedTopics[1]
                guard !candidateDigest.dropFirst(2).allSatisfy({ $0 == "0" }) else {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs[\(index)].topics[1]")
                }
                if let expectedDigest, candidateDigest != expectedDigest {
                    continue
                }
                if matchedDigest != nil {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
                }
                matchedDigest = candidateDigest
            }
        }
        guard let matchedDigest else {
            throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
        }
        return (matchedDigest, expectedAddress)
    }

    private static func resolveSourceBridgeEmitterAddress(
        _ inputAddress: String?,
        defaultAddress: String?
    ) throws -> String? {
        let normalizedInput = try inputAddress.map {
            try normalizeRpcHex($0, label: "sourceBridgeEmitterAddress", byteLength: 20)
        }
        let normalizedDefault = try defaultAddress.map {
            try normalizeRpcHex($0, label: "sourceBridgeEmitterAddress", byteLength: 20)
        }
        if let normalizedInput, let normalizedDefault, normalizedInput != normalizedDefault {
            throw EvmSccpProverError.invalidPublicInputs("sourceBridgeEmitterAddress")
        }
        return normalizedInput ?? normalizedDefault
    }

    public func collectInboundEvidenceFromReceipt(
        _ input: BscMainnetInboundEvidence,
        executionProvider provider: BscMainnetExecutionProvider? = nil,
        consensusProvider finalityProvider: BscMainnetConsensusProvider? = nil
    ) async throws -> BscMainnetInboundEvidence {
        guard input.sourceDomain == sccpDomainBsc else {
            throw EvmSccpProverError.invalidPublicInputs("sourceDomain")
        }
        guard input.targetDomain == sccpDomainSora else {
            throw EvmSccpProverError.invalidPublicInputs("targetDomain")
        }
        let selectedProvider = provider ?? executionProvider
        if let selectedProvider {
            _ = try await validateExecutionProviderMainnet(selectedProvider)
        }

        var transactionHash = try input.transactionHash.map {
            try Self.normalizeRpcHex($0, label: "transactionHash", byteLength: 32)
        }
        var receipt = input.receipt
        if receipt == nil, let transactionHash {
            guard let selectedProvider else {
                throw EvmSccpProverError.invalidPublicInputs("executionProvider")
            }
            guard let fetched = try await selectedProvider.request(
                method: "eth_getTransactionReceipt",
                params: [transactionHash]
            ) as? [String: Any] else {
                throw EvmSccpProverError.invalidPublicInputs("eth_getTransactionReceipt")
            }
            receipt = fetched
        }
        let receiptProof = input.receiptProof
        if receipt == nil, receiptProof == nil, input.receiptProofHash == nil {
            throw EvmSccpProverError.invalidPublicInputs("receipt")
        }

        var blockHash: String?
        var receiptBlockNumber: String?
        var blockReceiptsRoot: String?
        var sourceEventDigest: String?
        var normalizedSourceBridgeEmitterAddress: String?
        if let currentReceipt = receipt {
            guard currentReceipt["status"] as? String == "0x1" else {
                throw EvmSccpProverError.invalidPublicInputs("receipt.status")
            }
            let receiptTransactionHash = try Self.normalizeRpcHex(
                Self.firstPresent(currentReceipt, "transactionHash", "transaction_hash"),
                label: "receipt.transactionHash",
                byteLength: 32
            )
            if let transactionHash, transactionHash != receiptTransactionHash {
                throw EvmSccpProverError.invalidPublicInputs("receipt.transactionHash")
            }
            transactionHash = receiptTransactionHash
            blockHash = try Self.normalizeRpcHex(
                Self.firstPresent(currentReceipt, "blockHash", "block_hash"),
                label: "receipt.blockHash",
                byteLength: 32
            )
            receiptBlockNumber = try Self.normalizePositiveRpcQuantity(
                Self.firstPresent(currentReceipt, "blockNumber", "block_number"),
                label: "receipt.blockNumber"
            )
            let sourceEvent = try Self.evmReceiptSourceEvent(
                receipt: currentReceipt,
                sourceEventDigest: input.sourceEventDigest,
                sourceBridgeEmitterAddress: try Self.resolveSourceBridgeEmitterAddress(
                    input.sourceBridgeEmitterAddress,
                    defaultAddress: self.sourceBridgeEmitterAddress
                ),
                transactionHash: transactionHash,
                blockHash: blockHash,
                blockNumber: receiptBlockNumber
            )
            sourceEventDigest = sourceEvent.sourceEventDigest
            normalizedSourceBridgeEmitterAddress = sourceEvent.sourceBridgeEmitterAddress
        } else if input.sourceEventDigest != nil || input.sourceBridgeEmitterAddress != nil {
            throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
        }

        var block = input.block
        if block == nil, let blockHash, let selectedProvider {
            guard let fetched = try await selectedProvider.request(
                method: "eth_getBlockByHash",
                params: [blockHash, false]
            ) as? [String: Any] else {
                throw EvmSccpProverError.invalidPublicInputs("eth_getBlockByHash")
            }
            block = fetched
        }
        if let currentBlock = block {
            let normalizedBlockHash = try Self.normalizeRpcHex(
                currentBlock["hash"],
                label: "block.hash",
                byteLength: 32
            )
            if let blockHash, blockHash != normalizedBlockHash {
                throw EvmSccpProverError.invalidPublicInputs("block.hash")
            }
            blockHash = normalizedBlockHash
            let normalizedBlockNumber = try Self.normalizePositiveRpcQuantity(
                Self.firstPresent(currentBlock, "number", "blockNumber", "block_number"),
                label: "block.number"
            )
            if let receiptBlockNumber, receiptBlockNumber != normalizedBlockNumber {
                throw EvmSccpProverError.invalidPublicInputs("block.number")
            }
            receiptBlockNumber = normalizedBlockNumber
            blockReceiptsRoot = try Self.normalizeRpcHex(
                Self.firstPresent(currentBlock, "receiptsRoot", "receipts_root"),
                label: "block.receiptsRoot",
                byteLength: 32
            )
        }

        let selectedConsensusProvider = finalityProvider ?? consensusProvider
        let sourceParliaFinality: [String: Any]?
        if let supplied = input.parliaFinality {
            sourceParliaFinality = supplied
        } else {
            sourceParliaFinality = try await selectedConsensusProvider?.collectFinalityEvidence(
                receipt: receipt,
                block: block,
                transactionHash: transactionHash
            )
        }
        let parliaFinality = try sourceParliaFinality.map {
            try Self.normalizeParliaFinality(
                $0,
                expectedBlockHash: blockHash,
                expectedBlockNumber: receiptBlockNumber,
                expectedReceiptsRoot: blockReceiptsRoot
            )
        }
        try Self.requireReceiptProofMatchesEvidence(
            receiptProof,
            blockHash: blockHash,
            receiptBlockNumber: receiptBlockNumber,
            blockReceiptsRoot: blockReceiptsRoot,
            parliaFinality: parliaFinality,
            sourceEventDigest: sourceEventDigest
        )

        return BscMainnetInboundEvidence(
            sourceDomain: sccpDomainBsc,
            targetDomain: sccpDomainSora,
            transactionHash: transactionHash,
            receipt: receipt,
            block: block,
            parliaFinality: parliaFinality,
            receiptProof: receiptProof,
            receiptProofHash: try Self.normalizeReceiptProofHash(
                receiptProof: receiptProof,
                suppliedHash: input.receiptProofHash
            ),
            sourceEventDigest: sourceEventDigest,
            sourceBridgeEmitterAddress: normalizedSourceBridgeEmitterAddress
        )
    }

    public func proveInboundToSora(
        _ input: BscMainnetInboundEvidence,
        executionProvider provider: BscMainnetExecutionProvider? = nil,
        consensusProvider finalityProvider: BscMainnetConsensusProvider? = nil
    ) async throws -> Data {
        guard let inboundProveFunction else {
            throw EvmSccpProverError.localProverUnavailable
        }
        let evidence = try await collectInboundEvidenceFromReceipt(
            input,
            executionProvider: provider,
            consensusProvider: finalityProvider
        )
        guard let parliaFinality = evidence.parliaFinality, !parliaFinality.isEmpty else {
            throw EvmSccpProverError.invalidPublicInputs("parliaFinality")
        }
        guard evidence.receiptProof != nil else {
            throw EvmSccpProverError.invalidPublicInputs("receiptProof")
        }
        guard evidence.sourceEventDigest != nil else {
            throw EvmSccpProverError.invalidPublicInputs("receipt.sourceEvent")
        }
        let proofBytes = try await inboundProveFunction(evidence)
        guard !proofBytes.isEmpty else {
            throw EvmSccpProverError.emptyProof
        }
        guard proofBytes.contains(where: { $0 != 0 }) else {
            throw EvmSccpProverError.allZeroProof
        }
        return Data(proofBytes)
    }

    public func submitInboundToIroha(_ proofBytes: Data) async throws -> Any {
        guard !proofBytes.isEmpty else {
            throw EvmSccpProverError.emptyProof
        }
        guard proofBytes.contains(where: { $0 != 0 }) else {
            throw EvmSccpProverError.allZeroProof
        }
        guard let inboundSubmitFunction else {
            throw EvmSccpProverError.localProverUnavailable
        }
        return try await inboundSubmitFunction(Data(proofBytes))
    }

    public func buildOutboundProofRequest(_ input: EvmSccpProofRequestInput) async throws -> EvmSccpProofRequest {
        try await prover.buildRequest(input)
    }

    public func proveOutboundToBsc(_ input: EvmSccpProofRequestInput) async throws -> EvmSccpProofResult {
        try await prover.prove(input)
    }

    public func buildBscCalldata(_ input: EvmSccpSubmissionInput) throws -> EvmSccpSubmission {
        try buildBscMainnetSccpDestinationSubmission(input)
    }

    public func buildLocalAdmissionSubmission(
        _ input: BscMainnetLocalAdmissionSubmissionInput
    ) throws -> BscMainnetLocalAdmissionSubmission {
        try buildBscMainnetSccpLocalAdmissionSubmission(input)
    }

    public func submitOutboundToBsc(_ input: EvmSccpSubmissionInput) async throws -> Any {
        let submission = try buildBscCalldata(input)
        guard let outboundSubmitFunction else {
            throw EvmSccpProverError.localProverUnavailable
        }
        return try await outboundSubmitFunction(submission)
    }

    private static func normalizeRpcChainId(_ value: Any) throws -> UInt64 {
        let quantity = try Self.normalizeRpcQuantity(value, label: "eth_chainId")
        guard let parsed = UInt64(String(quantity.dropFirst(2)), radix: 16) else {
            throw EvmSccpProverError.invalidPublicInputs("eth_chainId")
        }
        return parsed
    }

    private static func firstPresent(_ input: [String: Any], _ keys: String...) -> Any? {
        for key in keys where input.keys.contains(key) {
            return input[key]
        }
        return nil
    }

    private static func strictFirstPresent(_ input: [String: Any], label: String, _ keys: String...) throws -> Any? {
        var selected: Any?
        var found = false
        for key in keys where input.keys.contains(key) {
            guard !found else {
                throw EvmSccpProverError.invalidPublicInputs(label)
            }
            selected = input[key]
            found = true
        }
        return selected
    }

    private static func requireMapList(_ value: Any, label: String) throws -> [[String: Any]] {
        if let maps = value as? [[String: Any]] {
            return maps
        }
        if let values = value as? [Any] {
            var maps: [[String: Any]] = []
            maps.reserveCapacity(values.count)
            for item in values {
                guard let map = item as? [String: Any] else {
                    throw EvmSccpProverError.invalidPublicInputs(label)
                }
                maps.append(map)
            }
            return maps
        }
        throw EvmSccpProverError.invalidPublicInputs(label)
    }

    private static func normalizeUnsignedInteger(_ value: Any?, label: String) throws -> UInt64 {
        if let value = value as? UInt64 {
            return value
        }
        if let value = value as? UInt32 {
            return UInt64(value)
        }
        if let value = value as? UInt {
            return UInt64(value)
        }
        if let value = value as? Int {
            guard value >= 0 else {
                throw EvmSccpProverError.invalidPublicInputs(label)
            }
            return UInt64(value)
        }
        guard let text = value as? String, text.trimmingCharacters(in: .whitespacesAndNewlines) == text else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        if text.hasPrefix("0x") {
            let hex = String(text.dropFirst(2))
            guard !hex.isEmpty,
                  hex == "0" || (hex.first != "0" && hex.allSatisfy { Self.isLowerHex($0) }),
                  let parsed = UInt64(hex, radix: 16) else {
                throw EvmSccpProverError.invalidPublicInputs(label)
            }
            return parsed
        }
        guard !text.isEmpty,
              text == "0" || (text.first != "0" && text.allSatisfy { Self.isDecimalDigit($0) }),
              let parsed = UInt64(text, radix: 10) else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        return parsed
    }

    private static func normalizeRpcHex(
        _ value: Any?,
        label: String,
        byteLength: Int,
        allowZero: Bool = false
    ) throws -> String {
        guard let text = value as? String,
              text.trimmingCharacters(in: .whitespacesAndNewlines) == text,
              text.hasPrefix("0x") else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        let hex = String(text.dropFirst(2))
        guard hex.count == byteLength * 2,
              hex.allSatisfy({ Self.isLowerHex($0) }) else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        guard allowZero || hex.contains(where: { $0 != "0" }) else {
            throw EvmSccpProverError.zeroField(label)
        }
        return text
    }

    private static func normalizeReceiptProofHash(
        receiptProof: BscMainnetReceiptProof?,
        suppliedHash: String?
    ) throws -> String? {
        var normalizedHash = try suppliedHash.map {
            try normalizeRpcHex($0, label: "receiptProofHash", byteLength: 32)
        }
        guard let receiptProof else {
            return normalizedHash
        }
        guard receiptProof.sourceDomain == sccpDomainBsc else {
            throw EvmSccpProverError.invalidPublicInputs("receiptProof.sourceDomain")
        }
        let computedHash = try bscSccpReceiptProofHash(
            sourceDomain: receiptProof.sourceDomain,
            sourceEventDigest: receiptProof.sourceEventDigest,
            validatorEpoch: receiptProof.validatorEpoch,
            blockNumber: receiptProof.blockNumber,
            blockHash: receiptProof.blockHash,
            receiptsRoot: receiptProof.receiptsRoot,
            validatorSetHash: receiptProof.validatorSetHash,
            commitSealHash: receiptProof.commitSealHash,
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
            inclusionBranch: receiptProof.inclusionBranch
        )
        if let normalizedHash, normalizedHash != computedHash {
            throw EvmSccpProverError.invalidPublicInputs("receiptProofHash")
        }
        normalizedHash = computedHash
        return normalizedHash
    }

    private static func requireReceiptProofMatchesEvidence(
        _ receiptProof: BscMainnetReceiptProof?,
        blockHash: String?,
        receiptBlockNumber: String?,
        blockReceiptsRoot: String?,
        parliaFinality: [String: Any]?,
        sourceEventDigest: String?
    ) throws {
        guard let receiptProof else {
            return
        }
        if let receiptBlockNumber {
            let expected = try normalizeUnsignedInteger(receiptBlockNumber, label: "block.number")
            guard receiptProof.blockNumber == expected else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.blockNumber")
            }
        }
        if let parliaFinality {
            let finalityBlockNumber = try normalizeUnsignedInteger(
                firstPresent(parliaFinality, "executionBlockNumber", "execution_block_number"),
                label: "parliaFinality.executionBlockNumber"
            )
            guard receiptProof.blockNumber == finalityBlockNumber else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.blockNumber")
            }
        }
        let proofBlockHash = try normalizeRpcHex(
            receiptProof.blockHash,
            label: "receiptProof.blockHash",
            byteLength: 32
        )
        if let blockHash, proofBlockHash != blockHash {
            throw EvmSccpProverError.invalidPublicInputs("receiptProof.blockHash")
        }
        if let parliaFinality {
            let finalityBlockHash = try normalizeRpcHex(
                firstPresent(parliaFinality, "executionBlockHash", "execution_block_hash"),
                label: "parliaFinality.executionBlockHash",
                byteLength: 32
            )
            guard proofBlockHash == finalityBlockHash else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.blockHash")
            }
        }
        let proofReceiptsRoot = try normalizeRpcHex(
            receiptProof.receiptsRoot,
            label: "receiptProof.receiptsRoot",
            byteLength: 32
        )
        if let blockReceiptsRoot, proofReceiptsRoot != blockReceiptsRoot {
            throw EvmSccpProverError.invalidPublicInputs("receiptProof.receiptsRoot")
        }
        if let parliaFinality {
            let finalityReceiptsRoot = try normalizeRpcHex(
                firstPresent(parliaFinality, "executionReceiptsRoot", "execution_receipts_root"),
                label: "parliaFinality.executionReceiptsRoot",
                byteLength: 32
            )
            guard proofReceiptsRoot == finalityReceiptsRoot else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.receiptsRoot")
            }
            if let finalityValidatorEpochInput = firstPresent(
                parliaFinality,
                "validatorEpoch",
                "validator_epoch"
            ) {
                let finalityValidatorEpoch = try normalizeUnsignedInteger(
                    finalityValidatorEpochInput,
                    label: "parliaFinality.validatorEpoch"
                )
                guard receiptProof.validatorEpoch == finalityValidatorEpoch else {
                    throw EvmSccpProverError.invalidPublicInputs("receiptProof.validatorEpoch")
                }
            }
            if let finalityValidatorSetHashInput = firstPresent(
                parliaFinality,
                "validatorSetHash",
                "validator_set_hash"
            ) {
                let finalityValidatorSetHash = try normalizeRpcHex(
                    finalityValidatorSetHashInput,
                    label: "parliaFinality.validatorSetHash",
                    byteLength: 32
                )
                let proofValidatorSetHash = try normalizeRpcHex(
                    receiptProof.validatorSetHash,
                    label: "receiptProof.validatorSetHash",
                    byteLength: 32
                )
                guard proofValidatorSetHash == finalityValidatorSetHash else {
                    throw EvmSccpProverError.invalidPublicInputs("receiptProof.validatorSetHash")
                }
            }
            if let finalityCommitSealHashInput = firstPresent(
                parliaFinality,
                "commitSealHash",
                "commit_seal_hash"
            ) {
                let finalityCommitSealHash = try normalizeRpcHex(
                    finalityCommitSealHashInput,
                    label: "parliaFinality.commitSealHash",
                    byteLength: 32
                )
                let proofCommitSealHash = try normalizeRpcHex(
                    receiptProof.commitSealHash,
                    label: "receiptProof.commitSealHash",
                    byteLength: 32
                )
                guard proofCommitSealHash == finalityCommitSealHash else {
                    throw EvmSccpProverError.invalidPublicInputs("receiptProof.commitSealHash")
                }
            }
        }
        if let sourceEventDigest {
            let proofSourceEventDigest = try normalizeRpcHex(
                receiptProof.sourceEventDigest,
                label: "receiptProof.sourceEventDigest",
                byteLength: 32
            )
            guard proofSourceEventDigest == sourceEventDigest else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.sourceEventDigest")
            }
        }
    }

    private static func normalizeRpcQuantity(_ value: Any?, label: String) throws -> String {
        guard let text = value as? String,
              text.trimmingCharacters(in: .whitespacesAndNewlines) == text,
              text.hasPrefix("0x") else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        let hex = String(text.dropFirst(2))
        guard !hex.isEmpty,
              hex == "0" || (hex.first != "0" && hex.allSatisfy { Self.isLowerHex($0) }),
              let parsed = UInt64(hex, radix: 16) else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        return "0x" + String(parsed, radix: 16)
    }

    private static func normalizePositiveRpcQuantity(_ value: Any?, label: String) throws -> String {
        let quantity = try Self.normalizeRpcQuantity(value, label: label)
        if quantity == "0x0" {
            throw EvmSccpProverError.zeroField(label)
        }
        return quantity
    }

    private static func normalizeParliaFinality(
        _ finality: [String: Any],
        expectedBlockHash: String?,
        expectedBlockNumber: String?,
        expectedReceiptsRoot: String?
    ) throws -> [String: Any] {
        let executionBlockNumber = try Self.normalizeUnsignedInteger(
            Self.firstPresent(finality, "executionBlockNumber", "execution_block_number", "finalityHeight", "finality_height"),
            label: "parliaFinality.executionBlockNumber"
        )
        guard executionBlockNumber != 0 else {
            throw EvmSccpProverError.zeroField("parliaFinality.executionBlockNumber")
        }
        if let expectedBlockNumber {
            let expected = try Self.normalizeUnsignedInteger(expectedBlockNumber, label: "block.number")
            guard executionBlockNumber == expected else {
                throw EvmSccpProverError.invalidPublicInputs("parliaFinality.executionBlockNumber")
            }
        }
        let executionBlockHash = try Self.normalizeRpcHex(
            Self.firstPresent(finality, "executionBlockHash", "execution_block_hash", "finalityBlockHash", "finality_block_hash"),
            label: "parliaFinality.executionBlockHash",
            byteLength: 32
        )
        if let expectedBlockHash, executionBlockHash != expectedBlockHash {
            throw EvmSccpProverError.invalidPublicInputs("parliaFinality.executionBlockHash")
        }
        let executionReceiptsRoot = try Self.normalizeRpcHex(
            Self.firstPresent(finality, "executionReceiptsRoot", "execution_receipts_root", "receiptsRoot", "receipts_root"),
            label: "parliaFinality.executionReceiptsRoot",
            byteLength: 32
        )
        if let expectedReceiptsRoot, executionReceiptsRoot != expectedReceiptsRoot {
            throw EvmSccpProverError.invalidPublicInputs("parliaFinality.executionReceiptsRoot")
        }
        var normalized = finality
        normalized["executionBlockNumber"] = String(executionBlockNumber)
        normalized["executionBlockHash"] = executionBlockHash
        normalized["executionReceiptsRoot"] = executionReceiptsRoot
        return normalized
    }

    private static func isLowerHex(_ character: Character) -> Bool {
        "0123456789abcdef".contains(character)
    }

    private static func isDecimalDigit(_ character: Character) -> Bool {
        "0123456789".contains(character)
    }
}

/// App-supplied Ethereum JSON-RPC execution provider for native SCCP evidence collection.
public protocol EthereumMainnetExecutionProvider {
    func request(method: String, params: [Any]) async throws -> Any
}

/// App-supplied Ethereum Beacon REST finality collector for native SCCP evidence collection.
public protocol EthereumMainnetConsensusProvider {
    func collectFinalityEvidence(
        receipt: [String: Any]?,
        block: [String: Any]?,
        transactionHash: String?
    ) async throws -> [String: Any]
}

/// HTTP response returned by an Ethereum Beacon REST transport.
public struct EthereumMainnetBeaconRestResponse {
    public let statusCode: Int
    public let body: Data
    public let statusMessage: String?

    public init(statusCode: Int,
                body: Data,
                statusMessage: String? = nil) {
        self.statusCode = statusCode
        self.body = body
        self.statusMessage = statusMessage
    }
}

/// Fetch transport used by the Ethereum mainnet Beacon REST consensus provider.
public protocol EthereumMainnetBeaconRestTransport {
    func get(url: URL, headers: [String: String]) async throws -> EthereumMainnetBeaconRestResponse
}

/// URLSession-backed Ethereum Beacon REST transport.
public final class EthereumMainnetBeaconRestURLSessionTransport: EthereumMainnetBeaconRestTransport {
    private let session: URLSession

    public init(session: URLSession = .shared) {
        self.session = session
    }

    public func get(url: URL, headers: [String: String]) async throws -> EthereumMainnetBeaconRestResponse {
        var request = URLRequest(url: url)
        request.httpMethod = "GET"
        for (header, value) in headers {
            request.setValue(value, forHTTPHeaderField: header)
        }
        let (bytes, response) = try await session.bytes(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.response")
        }
        if http.expectedContentLength > Int64(ethereumMainnetBeaconRestMaxResponseBytes) {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.response")
        }
        var data = Data()
        if http.expectedContentLength > 0 {
            data.reserveCapacity(Int(http.expectedContentLength))
        }
        for try await byte in bytes {
            guard data.count < ethereumMainnetBeaconRestMaxResponseBytes else {
                throw EvmSccpProverError.invalidPublicInputs("beaconRest.response")
            }
            data.append(byte)
        }
        return EthereumMainnetBeaconRestResponse(
            statusCode: http.statusCode,
            body: data,
            statusMessage: HTTPURLResponse.localizedString(forStatusCode: http.statusCode)
        )
    }
}

/// Ethereum mainnet Beacon REST finality collector.
public final class EthereumMainnetBeaconRestConsensusProvider: EthereumMainnetConsensusProvider {
    private struct BeaconRestHeaderSummary {
        let root: String
        let slot: UInt64
    }

    private struct BeaconRestBlockId {
        let id: String
        let slot: UInt64?
        let root: String?
    }

    private struct BeaconRestFinalityUpdateSummary {
        let finalizedHeaderRoot: String
        let beaconSlot: UInt64
        let syncCommitteeBits: String
        let syncCommitteeSignature: String
        let syncCommitteeParticipation: UInt64
        let syncSignatureSlot: UInt64
    }

    private let endpoint: URL
    private let syncCommitteeRoot: String?
    private let syncCommitteePayload: Data?
    private let headers: [String: String]
    private let verifyFinalityCheckpoint: Bool
    private let transport: EthereumMainnetBeaconRestTransport

    public init(endpoint: String,
                syncCommitteeRoot: String? = nil,
                syncCommitteePayload: Data? = nil,
                headers: [String: String] = [:],
                verifyFinalityCheckpoint: Bool = true,
                transport: EthereumMainnetBeaconRestTransport = EthereumMainnetBeaconRestURLSessionTransport()) throws {
        self.endpoint = try Self.normalizeEndpoint(endpoint)
        self.syncCommitteeRoot = try syncCommitteeRoot.map {
            try Self.normalizeRpcHex($0, label: "syncCommitteeRoot", byteLength: 32)
        }
        self.syncCommitteePayload = syncCommitteePayload
        self.headers = try Self.normalizeHeaders(headers)
        self.verifyFinalityCheckpoint = verifyFinalityCheckpoint
        self.transport = transport
        if self.syncCommitteeRoot == nil, syncCommitteePayload == nil {
            throw EvmSccpProverError.invalidPublicInputs("syncCommitteeRoot")
        }
        if let root = self.syncCommitteeRoot, let payload = syncCommitteePayload {
            guard try ethSyncCommitteeHashFromPayload(payload: payload) == root else {
                throw EvmSccpProverError.invalidPublicInputs("syncCommitteePayload")
            }
        }
    }

    public convenience init(endpoint: URL,
                            syncCommitteeRoot: String? = nil,
                            syncCommitteePayload: Data? = nil,
                            headers: [String: String] = [:],
                            verifyFinalityCheckpoint: Bool = true,
                            transport: EthereumMainnetBeaconRestTransport = EthereumMainnetBeaconRestURLSessionTransport()) throws {
        try self.init(
            endpoint: endpoint.absoluteString,
            syncCommitteeRoot: syncCommitteeRoot,
            syncCommitteePayload: syncCommitteePayload,
            headers: headers,
            verifyFinalityCheckpoint: verifyFinalityCheckpoint,
            transport: transport
        )
    }

    public func collectFinalityEvidence(
        receipt: [String: Any]?,
        block: [String: Any]?,
        transactionHash: String?
    ) async throws -> [String: Any] {
        guard let block else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.block")
        }
        let blockHash = try Self.normalizeRpcHex(block["hash"], label: "block.hash", byteLength: 32)
        let blockNumber = try Self.normalizeRpcQuantity(
            Self.firstPresent(block, "number", "blockNumber", "block_number"),
            label: "block.number"
        )
        guard blockNumber != "0x0" else {
            throw EvmSccpProverError.zeroField("block.number")
        }
        let receiptsRoot = try Self.normalizeRpcHex(
            Self.firstPresent(block, "receiptsRoot", "receipts_root"),
            label: "block.receiptsRoot",
            byteLength: 32
        )
        let targetBlockId = try await beaconRestBlockIdForTarget(block: block)
        let finalizedHeaderResponse = try await fetchJsonObject(
            path: "/eth/v1/beacon/headers/finalized",
            label: "Ethereum mainnet Beacon REST finalized header"
        )
        let finalizedHeader = try Self.beaconRestHeaderSummary(
            finalizedHeaderResponse,
            label: "Ethereum mainnet Beacon REST finalized header"
        )

        let targetHeader: BeaconRestHeaderSummary
        if targetBlockId.id == "finalized" {
            targetHeader = finalizedHeader
        } else {
            let targetHeaderResponse = try await fetchJsonObject(
                path: "/eth/v1/beacon/headers/\(targetBlockId.id)",
                label: "Ethereum mainnet Beacon REST finalized target header"
            )
            targetHeader = try Self.beaconRestHeaderSummary(
                targetHeaderResponse,
                label: "Ethereum mainnet Beacon REST finalized target header"
            )
        }
        if let expectedSlot = targetBlockId.slot, targetHeader.slot != expectedSlot {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.targetHeader.slot")
        }
        if let expectedRoot = targetBlockId.root, targetHeader.root != expectedRoot {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.targetHeader.root")
        }
        guard targetHeader.slot <= finalizedHeader.slot else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.targetHeader.finalizedSlot")
        }
        if targetHeader.slot == finalizedHeader.slot, targetHeader.root != finalizedHeader.root {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.targetHeader.finalizedRoot")
        }

        let finalizedBlockRootResponse = try await fetchJsonObject(
            path: "/eth/v1/beacon/blocks/\(targetBlockId.id)/root",
            label: "Ethereum mainnet Beacon REST finalized block root"
        )
        try Self.rejectUnsafeBeaconRestPayload(
            finalizedBlockRootResponse,
            label: "Ethereum mainnet Beacon REST finalized block root"
        )
        let finalizedBlockRootData = try Self.expectObject(
            Self.requireField(
                finalizedBlockRootResponse,
                label: "Ethereum mainnet Beacon REST finalized block root",
                field: "data"
            ),
            label: "Ethereum mainnet Beacon REST finalized block root.data"
        )
        let finalizedBlockRootHash = try Self.normalizeRpcHex(
            Self.requireField(
                finalizedBlockRootData,
                label: "Ethereum mainnet Beacon REST finalized block root.data",
                field: "root"
            ),
            label: "finalizedBlockRoot",
            byteLength: 32
        )
        guard finalizedBlockRootHash == targetHeader.root else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.finalizedBlockRoot")
        }
        let finalizedBlockRoot = try await fetchJsonObject(
            path: "/eth/v2/beacon/blocks/\(targetBlockId.id)",
            label: "Ethereum mainnet Beacon REST finalized block"
        )
        try Self.rejectUnsafeBeaconRestPayload(
            finalizedBlockRoot,
            label: "Ethereum mainnet Beacon REST finalized block"
        )
        let blockData = try Self.expectObject(
            Self.requireField(
                finalizedBlockRoot,
                label: "Ethereum mainnet Beacon REST finalized block",
                field: "data"
            ),
            label: "Ethereum mainnet Beacon REST finalized block.data"
        )
        let blockMessage = try Self.expectObject(
            Self.requireField(
                blockData,
                label: "Ethereum mainnet Beacon REST finalized block.data",
                field: "message"
            ),
            label: "Ethereum mainnet Beacon REST finalized block.data.message"
        )
        let finalizedBlockSlot = try Self.normalizeUnsignedInteger(
            Self.requireField(
                blockMessage,
                label: "Ethereum mainnet Beacon REST finalized block.data.message",
                field: "slot"
            ),
            label: "Ethereum mainnet Beacon REST finalized block.data.message.slot"
        )
        guard finalizedBlockSlot == targetHeader.slot else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.executionPayload.slot")
        }
        let blockBody = try Self.expectObject(
            Self.requireField(
                blockMessage,
                label: "Ethereum mainnet Beacon REST finalized block.data.message",
                field: "body"
            ),
            label: "Ethereum mainnet Beacon REST finalized block.data.message.body"
        )
        let executionPayload = try Self.expectObject(
            Self.requireField(
                blockBody,
                label: "Ethereum mainnet Beacon REST finalized block.data.message.body",
                field: "execution_payload"
            ),
            label: "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload"
        )
        let payloadBlockHash = try Self.normalizeRpcHex(
            Self.requireField(
                executionPayload,
                label: "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                field: "block_hash"
            ),
            label: "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.block_hash",
            byteLength: 32
        )
        guard payloadBlockHash == blockHash else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.executionPayload.blockHash")
        }
        let payloadBlockNumber = try Self.normalizeUnsignedInteger(
            Self.requireField(
                executionPayload,
                label: "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                field: "block_number"
            ),
            label: "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.block_number"
        )
        let expectedPayloadBlockNumber = try Self.normalizeUnsignedInteger(blockNumber, label: "block.number")
        guard payloadBlockNumber == expectedPayloadBlockNumber else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.executionPayload.blockNumber")
        }
        let payloadReceiptsRoot = try Self.normalizeRpcHex(
            Self.requireField(
                executionPayload,
                label: "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                field: "receipts_root"
            ),
            label: "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.receipts_root",
            byteLength: 32
        )
        guard payloadReceiptsRoot == receiptsRoot else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.executionPayload.receiptsRoot")
        }
        if verifyFinalityCheckpoint {
            let checkpointRoot = try await fetchJsonObject(
                path: "/eth/v1/beacon/states/finalized/finality_checkpoints",
                label: "Ethereum mainnet Beacon REST finality checkpoints"
            )
            try Self.rejectUnsafeBeaconRestPayload(
                checkpointRoot,
                label: "Ethereum mainnet Beacon REST finality checkpoints"
            )
            let checkpointData = try Self.expectObject(
                Self.requireField(
                    checkpointRoot,
                    label: "Ethereum mainnet Beacon REST finality checkpoints",
                    field: "data"
                ),
                label: "Ethereum mainnet Beacon REST finality checkpoints.data"
            )
            let finalizedCheckpoint = try Self.expectObject(
                Self.requireField(
                    checkpointData,
                    label: "Ethereum mainnet Beacon REST finality checkpoints.data",
                    field: "finalized"
                ),
                label: "Ethereum mainnet Beacon REST finality checkpoints.data.finalized"
            )
            let checkpointFinalizedRoot = try Self.normalizeRpcHex(
                Self.requireField(
                    finalizedCheckpoint,
                    label: "Ethereum mainnet Beacon REST finality checkpoints.data.finalized",
                    field: "root"
                ),
                label: "finalizedCheckpointRoot",
                byteLength: 32
            )
            guard checkpointFinalizedRoot == finalizedHeader.root else {
                throw EvmSccpProverError.invalidPublicInputs("beaconRest.finalityCheckpoint")
            }
        }
        let finalityUpdateResponse = try await fetchJsonObject(
            path: "/eth/v1/beacon/light_client/finality_update",
            label: "Ethereum mainnet Beacon REST light-client finality update"
        )
        let finalityUpdate = try Self.beaconRestFinalityUpdateSummary(
            finalityUpdateResponse,
            expectedFinalizedSlot: finalizedHeader.slot,
            expectedFinalizedRoot: finalizedHeader.root
        )
        return [
            "executionBlockNumber": String(
                try Self.normalizeUnsignedInteger(blockNumber, label: "block.number")
            ),
            "executionBlockHash": blockHash,
            "executionReceiptsRoot": receiptsRoot,
            "finalizedHeaderRoot": finalityUpdate.finalizedHeaderRoot,
            "syncCommitteeRoot": try resolvedSyncCommitteeRoot(),
            "beaconSlot": String(finalityUpdate.beaconSlot),
            "syncCommitteeBits": finalityUpdate.syncCommitteeBits,
            "syncCommitteeSignature": finalityUpdate.syncCommitteeSignature,
            "syncCommitteeParticipation": String(finalityUpdate.syncCommitteeParticipation),
            "syncSignatureSlot": String(finalityUpdate.syncSignatureSlot),
        ]
    }

    private func beaconRestBlockIdForTarget(block: [String: Any]) async throws -> BeaconRestBlockId {
        if let rootInput = Self.firstPresent(
            block,
            "beaconBlockRoot",
            "beacon_block_root",
            "targetBeaconBlockRoot",
            "target_beacon_block_root"
        ) {
            let root = try Self.normalizeRpcHex(rootInput, label: "block.beaconBlockRoot", byteLength: 32)
            return BeaconRestBlockId(id: root, slot: nil, root: root)
        }
        if let idInput = Self.firstPresent(
            block,
            "beaconBlockId",
            "beacon_block_id",
            "targetBeaconBlockId",
            "target_beacon_block_id"
        ) {
            return try Self.beaconRestBlockIdFromValue(idInput, label: "block.beaconBlockId")
        }
        if let slotInput = Self.firstPresent(
            block,
            "beaconSlot",
            "beacon_slot",
            "finalizedSlot",
            "finalized_slot",
            "slot"
        ) {
            let slot = try Self.normalizeBeaconSlot(slotInput, label: "block.beaconSlot")
            return BeaconRestBlockId(id: String(slot), slot: slot, root: nil)
        }
        if let timestampInput = Self.firstPresent(block, "timestamp", "blockTimestamp", "block_timestamp") {
            let timestamp = try Self.normalizeUnsignedInteger(timestampInput, label: "block.timestamp")
            let genesisTime = try await beaconRestGenesisTime()
            guard timestamp >= genesisTime else {
                throw EvmSccpProverError.invalidPublicInputs("block.timestamp")
            }
            let elapsed = timestamp - genesisTime
            guard elapsed % ethereumMainnetSecondsPerSlot == 0 else {
                throw EvmSccpProverError.invalidPublicInputs("block.timestamp")
            }
            let slot = elapsed / ethereumMainnetSecondsPerSlot
            guard slot != 0 else {
                throw EvmSccpProverError.zeroField("beaconFinality.beaconSlot")
            }
            return BeaconRestBlockId(id: String(slot), slot: slot, root: nil)
        }
        return BeaconRestBlockId(id: "finalized", slot: nil, root: nil)
    }

    private func beaconRestGenesisTime() async throws -> UInt64 {
        let genesis = try await fetchJsonObject(
            path: "/eth/v1/beacon/genesis",
            label: "Ethereum mainnet Beacon REST genesis"
        )
        let data = try Self.expectObject(
            Self.requireField(
                genesis,
                label: "Ethereum mainnet Beacon REST genesis",
                field: "data"
            ),
            label: "Ethereum mainnet Beacon REST genesis.data"
        )
        return try Self.normalizeUnsignedInteger(
            Self.requireField(
                data,
                label: "Ethereum mainnet Beacon REST genesis.data",
                field: "genesis_time"
            ),
            label: "Ethereum mainnet Beacon REST genesis.data.genesis_time"
        )
    }

    private func fetchJsonObject(path: String, label: String) async throws -> [String: Any] {
        let response = try await transport.get(url: beaconRestUrl(path), headers: headers)
        guard (200..<300).contains(response.statusCode) else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.response")
        }
        guard response.body.count <= ethereumMainnetBeaconRestMaxResponseBytes else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.response")
        }
        let parsed = try JSONSerialization.jsonObject(with: response.body)
        return try Self.expectObject(parsed, label: label)
    }

    private func beaconRestUrl(_ path: String) throws -> URL {
        guard var components = URLComponents(url: endpoint, resolvingAgainstBaseURL: false) else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.endpoint")
        }
        var basePath = components.path
        while basePath.hasSuffix("/") {
            basePath.removeLast()
        }
        let suffix = path
        if let versionRange = basePath.range(of: #"/eth/v[0-9]+$"#, options: .regularExpression),
           suffix.range(of: #"^/eth/v[0-9]+/"#, options: .regularExpression) != nil {
            basePath.removeSubrange(versionRange)
        }
        components.path = basePath + suffix
        components.fragment = nil
        guard let url = components.url else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.endpoint")
        }
        return url
    }

    private func resolvedSyncCommitteeRoot() throws -> String {
        if let syncCommitteeRoot {
            return syncCommitteeRoot
        }
        guard let syncCommitteePayload else {
            throw EvmSccpProverError.invalidPublicInputs("syncCommitteeRoot")
        }
        return try ethSyncCommitteeHashFromPayload(payload: syncCommitteePayload)
    }

    private static func beaconRestHeaderSummary(
        _ payload: [String: Any],
        label: String
    ) throws -> BeaconRestHeaderSummary {
        try rejectUnsafeBeaconRestPayload(payload, label: label)
        let headerData = try expectObject(
            requireField(payload, label: label, field: "data"),
            label: "\(label).data"
        )
        try rejectNonBooleanBeaconRestCanonical(headerData, label: label)
        let rootLabel = label.contains("target") ? "targetHeaderRoot" : "finalizedHeaderRoot"
        let root = try normalizeRpcHex(
            requireField(headerData, label: "\(label).data", field: "root"),
            label: rootLabel,
            byteLength: 32
        )
        let header = try expectObject(
            requireField(headerData, label: "\(label).data", field: "header"),
            label: "\(label).data.header"
        )
        let message = try expectObject(
            requireField(header, label: "\(label).data.header", field: "message"),
            label: "\(label).data.header.message"
        )
        for field in ["parent_root", "state_root", "body_root"] {
            _ = try normalizeRpcHex(
                requireField(message, label: "\(label).data.header.message", field: field),
                label: "\(label).data.header.message.\(field)",
                byteLength: 32
            )
        }
        _ = try normalizeRpcHex(
            requireField(header, label: "\(label).data.header", field: "signature"),
            label: "\(label).data.header.signature",
            byteLength: 96
        )
        let slot = try normalizeBeaconSlot(
            requireField(message, label: "\(label).data.header.message", field: "slot"),
            label: "beaconFinality.beaconSlot"
        )
        return BeaconRestHeaderSummary(root: root, slot: slot)
    }

    private static func beaconRestFinalityUpdateSummary(
        _ payload: [String: Any],
        expectedFinalizedSlot: UInt64,
        expectedFinalizedRoot: String
    ) throws -> BeaconRestFinalityUpdateSummary {
        try rejectUnsafeBeaconRestPayload(payload, label: "Ethereum mainnet Beacon REST light-client finality update")
        let data = try expectObject(
            requireField(
                payload,
                label: "Ethereum mainnet Beacon REST light-client finality update",
                field: "data"
            ),
            label: "Ethereum mainnet Beacon REST light-client finality update.data"
        )
        let finalizedHeader = try expectObject(
            requireField(
                data,
                label: "Ethereum mainnet Beacon REST light-client finality update.data",
                field: "finalized_header"
            ),
            label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header"
        )
        let finalizedBeacon = try expectObject(
            requireField(
                finalizedHeader,
                label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header",
                field: "beacon"
            ),
            label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon"
        )
        let finalizedSlot = try normalizeBeaconSlot(
            requireField(
                finalizedBeacon,
                label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon",
                field: "slot"
            ),
            label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon.slot"
        )
        guard finalizedSlot == expectedFinalizedSlot else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.finalityUpdate.finalizedSlot")
        }
        let proposerIndex = try normalizeUnsignedInteger(
            requireField(
                finalizedBeacon,
                label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon",
                field: "proposer_index"
            ),
            label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon.proposer_index"
        )
        let finalizedHeaderRoot = try ethBeaconBlockHeaderRoot(
            beaconSlot: finalizedSlot,
            beaconProposerIndex: proposerIndex,
            beaconParentRoot: normalizeRpcHex(
                requireField(
                    finalizedBeacon,
                    label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon",
                    field: "parent_root"
                ),
                label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon.parent_root",
                byteLength: 32
            ),
            beaconStateRoot: normalizeRpcHex(
                requireField(
                    finalizedBeacon,
                    label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon",
                    field: "state_root"
                ),
                label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon.state_root",
                byteLength: 32
            ),
            beaconBodyRoot: normalizeRpcHex(
                requireField(
                    finalizedBeacon,
                    label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon",
                    field: "body_root"
                ),
                label: "Ethereum mainnet Beacon REST light-client finality update.data.finalized_header.beacon.body_root",
                byteLength: 32
            )
        )
        guard finalizedHeaderRoot == expectedFinalizedRoot else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.finalityUpdate.finalizedRoot")
        }
        let syncSignatureSlot = try normalizeBeaconSlot(
            requireField(
                data,
                label: "Ethereum mainnet Beacon REST light-client finality update.data",
                field: "signature_slot"
            ),
            label: "Ethereum mainnet Beacon REST light-client finality update.data.signature_slot"
        )
        guard syncSignatureSlot >= expectedFinalizedSlot else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.finalityUpdate.signatureSlot")
        }
        let syncAggregate = try expectObject(
            requireField(
                data,
                label: "Ethereum mainnet Beacon REST light-client finality update.data",
                field: "sync_aggregate"
            ),
            label: "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate"
        )
        let syncCommitteeBits = try normalizeSyncCommitteeBits(
            requireField(
                syncAggregate,
                label: "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate",
                field: "sync_committee_bits"
            ),
            label: "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits"
        )
        let syncCommitteeSignature = try normalizeRpcHex(
            requireField(
                syncAggregate,
                label: "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate",
                field: "sync_committee_signature"
            ),
            label: "Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_signature",
            byteLength: 96
        )
        return BeaconRestFinalityUpdateSummary(
            finalizedHeaderRoot: finalizedHeaderRoot,
            beaconSlot: finalizedSlot,
            syncCommitteeBits: syncCommitteeBits,
            syncCommitteeSignature: syncCommitteeSignature,
            syncCommitteeParticipation: syncCommitteeParticipation(syncCommitteeBits),
            syncSignatureSlot: syncSignatureSlot
        )
    }

    private static func normalizeSyncCommitteeBits(_ value: Any?, label: String) throws -> String {
        let bits = try normalizeRpcHex(value, label: label, byteLength: 64, allowZero: true)
        guard syncCommitteeParticipation(bits) != 0 else {
            throw EvmSccpProverError.zeroField(label)
        }
        return bits
    }

    private static func syncCommitteeParticipation(_ bits: String) -> UInt64 {
        guard bits.hasPrefix("0x"),
              let bytes = Data(hexString: String(bits.dropFirst(2))) else {
            return 0
        }
        var count: UInt64 = 0
        for byte in bytes {
            var value = byte
            while value != 0 {
                count += UInt64(value & 1)
                value >>= 1
            }
        }
        return count
    }

    private static func beaconRestBlockIdFromValue(_ value: Any, label: String) throws -> BeaconRestBlockId {
        if let text = value as? String {
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed == text,
               text.hasPrefix("0x"),
               text.dropFirst(2).count == 64 {
                let root = try normalizeRpcHex(text, label: label, byteLength: 32)
                return BeaconRestBlockId(id: root, slot: nil, root: root)
            }
        }
        let slot = try normalizeBeaconSlot(value, label: label)
        return BeaconRestBlockId(id: String(slot), slot: slot, root: nil)
    }

    private static func normalizeBeaconSlot(_ value: Any?, label: String) throws -> UInt64 {
        let slot = try normalizeUnsignedInteger(value, label: label)
        guard slot != 0 else {
            throw EvmSccpProverError.zeroField("beaconFinality.beaconSlot")
        }
        return slot
    }

    private static func normalizeEndpoint(_ value: String) throws -> URL {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed == value,
              let url = URL(string: value),
              let scheme = url.scheme?.lowercased(),
              scheme == "http" || scheme == "https",
              url.host != nil else {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.endpoint")
        }
        return url
    }

    private static func normalizeHeaders(_ input: [String: String]) throws -> [String: String] {
        var headers: [String: String] = [:]
        for (name, value) in input {
            guard !name.isEmpty,
                  name.trimmingCharacters(in: .whitespacesAndNewlines) == name,
                  value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
                throw EvmSccpProverError.invalidPublicInputs("beaconRest.headers")
            }
            headers[name] = value
        }
        return headers
    }

    private static func rejectUnsafeBeaconRestPayload(_ payload: [String: Any], label: String) throws {
        let executionOptimistic = try optionalBeaconRestBoolean(
            payload,
            field: "execution_optimistic",
            label: label
        )
        let executionOptimisticAlias = try optionalBeaconRestBoolean(
            payload,
            field: "executionOptimistic",
            label: label
        )
        let finalized = try optionalBeaconRestBoolean(payload, field: "finalized", label: label)
        if executionOptimistic == true || executionOptimisticAlias == true {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.finalizedHeader")
        }
        if finalized == false {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.finalizedHeader")
        }
        guard payload["data"] != nil else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
    }

    private static func rejectNonBooleanBeaconRestCanonical(_ payload: [String: Any], label: String) throws {
        if try optionalBeaconRestBoolean(payload, field: "canonical", label: label) == false {
            throw EvmSccpProverError.invalidPublicInputs("beaconRest.finalizedHeader")
        }
    }

    private static func optionalBeaconRestBoolean(
        _ payload: [String: Any],
        field: String,
        label: String
    ) throws -> Bool? {
        guard let value = payload[field] else {
            return nil
        }
        guard let boolean = value as? Bool else {
            throw EvmSccpProverError.invalidPublicInputs("\(label).\(field)")
        }
        return boolean
    }

    private static func expectObject(_ value: Any, label: String) throws -> [String: Any] {
        guard let object = value as? [String: Any] else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        return object
    }

    private static func requireField(_ object: [String: Any], label: String, field: String) throws -> Any {
        guard let value = object[field] else {
            throw EvmSccpProverError.invalidPublicInputs("\(label).\(field)")
        }
        return value
    }

    private static func firstPresent(_ input: [String: Any], _ keys: String...) -> Any? {
        for key in keys where input.keys.contains(key) {
            return input[key]
        }
        return nil
    }

    private static func normalizeUnsignedInteger(_ value: Any?, label: String) throws -> UInt64 {
        if let value = value as? UInt64 {
            return value
        }
        if let value = value as? UInt32 {
            return UInt64(value)
        }
        if let value = value as? UInt {
            return UInt64(value)
        }
        if let value = value as? Int {
            guard value >= 0 else {
                throw EvmSccpProverError.invalidPublicInputs(label)
            }
            return UInt64(value)
        }
        guard let text = value as? String,
              text.trimmingCharacters(in: .whitespacesAndNewlines) == text else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        if text.hasPrefix("0x") {
            let hex = String(text.dropFirst(2))
            guard !hex.isEmpty,
                  hex == "0" || (hex.first != "0" && hex.allSatisfy { Self.isLowerHex($0) }),
                  let parsed = UInt64(hex, radix: 16) else {
                throw EvmSccpProverError.invalidPublicInputs(label)
            }
            return parsed
        }
        guard !text.isEmpty,
              text == "0" || (text.first != "0" && text.allSatisfy { Self.isDecimalDigit($0) }),
              let parsed = UInt64(text, radix: 10) else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        return parsed
    }

    private static func normalizeRpcHex(
        _ value: Any?,
        label: String,
        byteLength: Int,
        allowZero: Bool = false
    ) throws -> String {
        guard let text = value as? String,
              text.trimmingCharacters(in: .whitespacesAndNewlines) == text,
              text.hasPrefix("0x") else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        let hex = String(text.dropFirst(2))
        guard hex.count == byteLength * 2,
              hex.allSatisfy({ Self.isLowerHex($0) }) else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        guard allowZero || hex.contains(where: { $0 != "0" }) else {
            throw EvmSccpProverError.zeroField(label)
        }
        return text
    }

    private static func normalizeRpcQuantity(_ value: Any?, label: String) throws -> String {
        guard let text = value as? String,
              text.trimmingCharacters(in: .whitespacesAndNewlines) == text,
              text.hasPrefix("0x") else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        let hex = String(text.dropFirst(2))
        guard !hex.isEmpty,
              hex == "0" || (hex.first != "0" && hex.allSatisfy { Self.isLowerHex($0) }),
              let parsed = UInt64(hex, radix: 16) else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        return "0x" + String(parsed, radix: 16)
    }

    private static func isLowerHex(_ character: Character) -> Bool {
        "0123456789abcdef".contains(character)
    }

    private static func isDecimalDigit(_ character: Character) -> Bool {
        "0123456789".contains(character)
    }
}

/// Typed Ethereum beacon finality evidence required before inbound source proving.
public struct EthereumMainnetBeaconFinalityEvidence {
    public let executionBlockNumber: String
    public let executionBlockHash: String
    public let executionReceiptsRoot: String
    public let beaconSlot: String?
    public let syncCommitteeBits: String?
    public let syncCommitteeSignature: String?
    public let syncCommitteeParticipation: String?
    public let syncSignatureSlot: String?
    public let additionalFields: [String: Any]

    public init(executionBlockNumber: String,
                executionBlockHash: String,
                executionReceiptsRoot: String,
                beaconSlot: String? = nil,
                syncCommitteeBits: String? = nil,
                syncCommitteeSignature: String? = nil,
                syncCommitteeParticipation: String? = nil,
                syncSignatureSlot: String? = nil,
                additionalFields: [String: Any] = [:]) {
        self.executionBlockNumber = executionBlockNumber
        self.executionBlockHash = executionBlockHash
        self.executionReceiptsRoot = executionReceiptsRoot
        self.beaconSlot = beaconSlot
        self.syncCommitteeBits = syncCommitteeBits
        self.syncCommitteeSignature = syncCommitteeSignature
        self.syncCommitteeParticipation = syncCommitteeParticipation
        self.syncSignatureSlot = syncSignatureSlot
        self.additionalFields = additionalFields
    }

    public var dictionary: [String: Any] {
        var value = additionalFields
        value["executionBlockNumber"] = executionBlockNumber
        value["executionBlockHash"] = executionBlockHash
        value["executionReceiptsRoot"] = executionReceiptsRoot
        if let beaconSlot {
            value["beaconSlot"] = beaconSlot
        }
        if let syncCommitteeBits {
            value["syncCommitteeBits"] = syncCommitteeBits
        }
        if let syncCommitteeSignature {
            value["syncCommitteeSignature"] = syncCommitteeSignature
        }
        if let syncCommitteeParticipation {
            value["syncCommitteeParticipation"] = syncCommitteeParticipation
        }
        if let syncSignatureSlot {
            value["syncSignatureSlot"] = syncSignatureSlot
        }
        return value
    }
}

/// Ethereum mainnet receipt-proof transcript collected from app-supplied RPC and Beacon REST providers.
public struct EthereumMainnetReceiptProof {
    public let sourceDomain: UInt32
    public let sourceEventDigest: String
    public let beaconSlot: UInt64
    public let executionBlockNumber: UInt64
    public let executionBlockHash: String
    public let executionReceiptsRoot: String
    public let beaconFinalizedRoot: String
    public let syncCommitteeRoot: String
    public let receiptRootIndex: UInt64
    public let receiptTrieProofNodes: [Data]
    public let inclusionBranch: [Data]

    public init(sourceDomain: UInt32 = sccpDomainEthereum,
                sourceEventDigest: String,
                beaconSlot: UInt64,
                executionBlockNumber: UInt64,
                executionBlockHash: String,
                executionReceiptsRoot: String,
                beaconFinalizedRoot: String,
                syncCommitteeRoot: String,
                receiptRootIndex: UInt64,
                receiptTrieProofNodes: [Data],
                inclusionBranch: [Data]) {
        self.sourceDomain = sourceDomain
        self.sourceEventDigest = sourceEventDigest
        self.beaconSlot = beaconSlot
        self.executionBlockNumber = executionBlockNumber
        self.executionBlockHash = executionBlockHash
        self.executionReceiptsRoot = executionReceiptsRoot
        self.beaconFinalizedRoot = beaconFinalizedRoot
        self.syncCommitteeRoot = syncCommitteeRoot
        self.receiptRootIndex = receiptRootIndex
        self.receiptTrieProofNodes = receiptTrieProofNodes
        self.inclusionBranch = inclusionBranch
    }
}

/// Locally collected Ethereum mainnet inbound evidence before source-proof generation.
public struct EthereumMainnetInboundEvidence {
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let transactionHash: String?
    public let receipt: [String: Any]?
    public let block: [String: Any]?
    public let beaconFinality: [String: Any]?
    public let blockReceipts: [[String: Any]]?
    public let inclusionBranch: [Data]?
    public let receiptProof: EthereumMainnetReceiptProof?
    public let receiptProofHash: String?
    public let sourceEventDigest: String?
    public let sourceBridgeEmitterAddress: String?

    public init(sourceDomain: UInt32 = sccpDomainEthereum,
                targetDomain: UInt32 = sccpDomainSora,
                transactionHash: String? = nil,
                receipt: [String: Any]? = nil,
                block: [String: Any]? = nil,
                beaconFinality: [String: Any]? = nil,
                blockReceipts: [[String: Any]]? = nil,
                inclusionBranch: [Data]? = nil,
                receiptProof: EthereumMainnetReceiptProof? = nil,
                receiptProofHash: String? = nil,
                sourceEventDigest: String? = nil,
                sourceBridgeEmitterAddress: String? = nil) {
        self.sourceDomain = sourceDomain
        self.targetDomain = targetDomain
        self.transactionHash = transactionHash
        self.receipt = receipt
        self.block = block
        self.beaconFinality = beaconFinality
        self.blockReceipts = blockReceipts
        self.inclusionBranch = inclusionBranch
        self.receiptProof = receiptProof
        self.receiptProofHash = receiptProofHash
        self.sourceEventDigest = sourceEventDigest
        self.sourceBridgeEmitterAddress = sourceBridgeEmitterAddress
    }

    public init(sourceDomain: UInt32 = sccpDomainEthereum,
                targetDomain: UInt32 = sccpDomainSora,
                transactionHash: String? = nil,
                receipt: [String: Any]? = nil,
                block: [String: Any]? = nil,
                beaconFinalityEvidence: EthereumMainnetBeaconFinalityEvidence?,
                blockReceipts: [[String: Any]]? = nil,
                inclusionBranch: [Data]? = nil,
                receiptProof: EthereumMainnetReceiptProof? = nil,
                receiptProofHash: String? = nil,
                sourceEventDigest: String? = nil,
                sourceBridgeEmitterAddress: String? = nil) {
        self.init(
            sourceDomain: sourceDomain,
            targetDomain: targetDomain,
            transactionHash: transactionHash,
            receipt: receipt,
            block: block,
            beaconFinality: beaconFinalityEvidence?.dictionary,
            blockReceipts: blockReceipts,
            inclusionBranch: inclusionBranch,
            receiptProof: receiptProof,
            receiptProofHash: receiptProofHash,
            sourceEventDigest: sourceEventDigest,
            sourceBridgeEmitterAddress: sourceBridgeEmitterAddress
        )
    }
}

/// Local-first Ethereum mainnet SCCP API for native proof generation and EVM submission payloads.
public final class EthereumMainnetSccp {
    public typealias ProveFunction = EvmSccpProver.ProveFunction
    public typealias InboundProveFunction = (EthereumMainnetInboundEvidence) async throws -> Data
    public typealias InboundSubmitFunction = (Data) async throws -> Any
    public typealias OutboundSubmitFunction = (EvmSccpSubmission) async throws -> Any

    private let prover: EvmSccpProver
    private let executionProvider: EthereumMainnetExecutionProvider?
    private let consensusProvider: EthereumMainnetConsensusProvider?
    private let inboundProveFunction: InboundProveFunction?
    private let inboundSubmitFunction: InboundSubmitFunction?
    private let outboundSubmitFunction: OutboundSubmitFunction?
    private let sourceBridgeEmitterAddress: String?

    public init(witnessProvider: EvmSccpWitnessProvider? = nil,
                proveFunction: ProveFunction? = nil,
                executionProvider: EthereumMainnetExecutionProvider? = nil,
                consensusProvider: EthereumMainnetConsensusProvider? = nil,
                inboundProveFunction: InboundProveFunction? = nil,
                inboundSubmitFunction: InboundSubmitFunction? = nil,
                outboundSubmitFunction: OutboundSubmitFunction? = nil,
                sourceBridgeEmitterAddress: String? = nil) {
        self.prover = EvmSccpProver(witnessProvider: witnessProvider, proveFunction: proveFunction)
        self.executionProvider = executionProvider
        self.consensusProvider = consensusProvider
        self.inboundProveFunction = inboundProveFunction
        self.inboundSubmitFunction = inboundSubmitFunction
        self.outboundSubmitFunction = outboundSubmitFunction
        self.sourceBridgeEmitterAddress = sourceBridgeEmitterAddress
    }

    public static func requireMainnetChainId(_ chainId: UInt64) throws {
        guard chainId == sccpEthereumMainnetChainId else {
            throw EvmSccpProverError.invalidPublicInputs("eth_chainId")
        }
    }

    public static func destinationBinding(verifierAddress: String,
                                          bridgeAddress: String,
                                          verifierCodeHash: String,
                                          verifierKeyHash: String,
                                          networkId: String = sccpEthereumMainnetNetworkId) throws -> EvmSccpDestinationBinding {
        try sccpEthereumMainnetDestinationBinding(
            verifierAddress: verifierAddress,
            bridgeAddress: bridgeAddress,
            verifierCodeHash: verifierCodeHash,
            verifierKeyHash: verifierKeyHash,
            networkId: networkId
        )
    }

    public func validateExecutionProviderMainnet(_ provider: EthereumMainnetExecutionProvider? = nil) async throws -> Any {
        guard let selectedProvider = provider ?? executionProvider else {
            throw EvmSccpProverError.localProverUnavailable
        }
        let chainId = try await selectedProvider.request(method: "eth_chainId", params: [])
        try Self.requireMainnetChainId(Self.normalizeRpcChainId(chainId))
        return chainId
    }

    private static func ethereumReceiptSourceEvent(
        receipt: [String: Any],
        sourceEventDigest inputDigest: String?,
        sourceBridgeEmitterAddress inputAddress: String?,
        transactionHash expectedTransactionHash: String?,
        blockHash expectedBlockHash: String?,
        blockNumber expectedBlockNumber: String?
    ) throws -> (sourceEventDigest: String?, sourceBridgeEmitterAddress: String?) {
        let expectedDigest = try inputDigest.map {
            try normalizeRpcHex($0, label: "sourceEventDigest", byteLength: 32)
        }
        let expectedAddress = try inputAddress.map {
            try normalizeRpcHex($0, label: "sourceBridgeEmitterAddress", byteLength: 20)
        }
        if expectedDigest == nil && expectedAddress == nil {
            return (nil, nil)
        }
        guard let expectedAddress else {
            throw EvmSccpProverError.invalidPublicInputs("sourceBridgeEmitterAddress")
        }
        guard let logs = receipt["logs"] as? [[String: Any]] else {
            throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
        }
        let sourceEventTopic = evmSccpSourceEventTopic()
        var matchedDigest: String?
        for (index, log) in logs.enumerated() {
            if (log["removed"] as? Bool) == true {
                throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
            }
            let address = try normalizeRpcHex(
                log["address"],
                label: "receipt.logs[\(index)].address",
                byteLength: 20,
                allowZero: true
            )
            guard let topics = log["topics"] as? [Any], topics.count <= 4 else {
                throw EvmSccpProverError.invalidPublicInputs("receipt.logs[\(index)].topics")
            }
            let normalizedTopics = try topics.enumerated().map { topicIndex, topic in
                try normalizeRpcHex(
                    topic,
                    label: "receipt.logs[\(index)].topics[\(topicIndex)]",
                    byteLength: 32,
                    allowZero: true
                )
            }
            if address == expectedAddress,
               normalizedTopics.first == sourceEventTopic {
                guard normalizedTopics.count == 2 else {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs[\(index)].topics")
                }
                guard let data = log["data"] as? String else {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs[\(index)].data")
                }
                guard data == "0x" else {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs[\(index)].data")
                }
                let logTransactionHash = try normalizeRpcHex(
                    strictFirstPresent(
                        log,
                        label: "receipt.logs[\(index)].transactionHash",
                        "transactionHash",
                        "transaction_hash"
                    ),
                    label: "receipt.logs[\(index)].transactionHash",
                    byteLength: 32
                )
                if let expectedTransactionHash, logTransactionHash != expectedTransactionHash {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
                }
                let logBlockHash = try normalizeRpcHex(
                    strictFirstPresent(
                        log,
                        label: "receipt.logs[\(index)].blockHash",
                        "blockHash",
                        "block_hash"
                    ),
                    label: "receipt.logs[\(index)].blockHash",
                    byteLength: 32
                )
                if let expectedBlockHash, logBlockHash != expectedBlockHash {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
                }
                let logBlockNumber = try normalizePositiveRpcQuantity(
                    strictFirstPresent(
                        log,
                        label: "receipt.logs[\(index)].blockNumber",
                        "blockNumber",
                        "block_number"
                    ),
                    label: "receipt.logs[\(index)].blockNumber"
                )
                if let expectedBlockNumber, logBlockNumber != expectedBlockNumber {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
                }
                let candidateDigest = normalizedTopics[1]
                guard !candidateDigest.dropFirst(2).allSatisfy({ $0 == "0" }) else {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs[\(index)].topics[1]")
                }
                if let expectedDigest, candidateDigest != expectedDigest {
                    continue
                }
                if matchedDigest != nil {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
                }
                matchedDigest = candidateDigest
            }
        }
        guard let matchedDigest else {
            throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
        }
        return (matchedDigest, expectedAddress)
    }

    private static func resolveSourceBridgeEmitterAddress(
        _ inputAddress: String?,
        defaultAddress: String?
    ) throws -> String? {
        let normalizedInput = try inputAddress.map {
            try normalizeRpcHex($0, label: "sourceBridgeEmitterAddress", byteLength: 20)
        }
        let normalizedDefault = try defaultAddress.map {
            try normalizeRpcHex($0, label: "sourceBridgeEmitterAddress", byteLength: 20)
        }
        if let normalizedInput, let normalizedDefault, normalizedInput != normalizedDefault {
            throw EvmSccpProverError.invalidPublicInputs("sourceBridgeEmitterAddress")
        }
        return normalizedInput ?? normalizedDefault
    }

    public func collectInboundEvidenceFromReceipt(
        _ input: EthereumMainnetInboundEvidence,
        executionProvider provider: EthereumMainnetExecutionProvider? = nil,
        consensusProvider finalityProvider: EthereumMainnetConsensusProvider? = nil
    ) async throws -> EthereumMainnetInboundEvidence {
        guard input.sourceDomain == sccpDomainEthereum else {
            throw EvmSccpProverError.invalidPublicInputs("sourceDomain")
        }
        guard input.targetDomain == sccpDomainSora else {
            throw EvmSccpProverError.invalidPublicInputs("targetDomain")
        }
        let selectedProvider = provider ?? executionProvider
        if let selectedProvider {
            _ = try await validateExecutionProviderMainnet(selectedProvider)
        }

        var transactionHash = try input.transactionHash.map {
            try Self.normalizeRpcHex($0, label: "transactionHash", byteLength: 32)
        }
        var receipt = input.receipt
        if receipt == nil, let transactionHash {
            guard let selectedProvider else {
                throw EvmSccpProverError.invalidPublicInputs("executionProvider")
            }
            guard let fetched = try await selectedProvider.request(
                method: "eth_getTransactionReceipt",
                params: [transactionHash]
            ) as? [String: Any] else {
                throw EvmSccpProverError.invalidPublicInputs("eth_getTransactionReceipt")
            }
            receipt = fetched
        }
        var receiptProof = input.receiptProof
        if receipt == nil, receiptProof == nil, input.receiptProofHash == nil {
            throw EvmSccpProverError.invalidPublicInputs("receipt")
        }

        var blockHash: String?
        var receiptBlockNumber: String?
        var blockReceiptsRoot: String?
        var sourceEventDigest: String?
        var sourceBridgeEmitterAddress: String?
        if let currentReceipt = receipt {
            guard currentReceipt["status"] as? String == "0x1" else {
                throw EvmSccpProverError.invalidPublicInputs("receipt.status")
            }
            let receiptTransactionHash = try Self.normalizeRpcHex(
                Self.firstPresent(currentReceipt, "transactionHash", "transaction_hash"),
                label: "receipt.transactionHash",
                byteLength: 32
            )
            if let transactionHash, transactionHash != receiptTransactionHash {
                throw EvmSccpProverError.invalidPublicInputs("receipt.transactionHash")
            }
            transactionHash = receiptTransactionHash
            blockHash = try Self.normalizeRpcHex(
                Self.firstPresent(currentReceipt, "blockHash", "block_hash"),
                label: "receipt.blockHash",
                byteLength: 32
            )
            receiptBlockNumber = try Self.normalizePositiveRpcQuantity(
                Self.firstPresent(currentReceipt, "blockNumber", "block_number"),
                label: "receipt.blockNumber"
            )
            let sourceEvent = try Self.ethereumReceiptSourceEvent(
                receipt: currentReceipt,
                sourceEventDigest: input.sourceEventDigest,
                sourceBridgeEmitterAddress: try Self.resolveSourceBridgeEmitterAddress(
                    input.sourceBridgeEmitterAddress,
                    defaultAddress: self.sourceBridgeEmitterAddress
                ),
                transactionHash: transactionHash,
                blockHash: blockHash,
                blockNumber: receiptBlockNumber
            )
            sourceEventDigest = sourceEvent.sourceEventDigest
            sourceBridgeEmitterAddress = sourceEvent.sourceBridgeEmitterAddress
        } else if input.sourceEventDigest != nil || input.sourceBridgeEmitterAddress != nil {
            throw EvmSccpProverError.invalidPublicInputs("receipt.logs")
        }

        var block = input.block
        if block == nil, let blockHash, let selectedProvider {
            guard let fetched = try await selectedProvider.request(
                method: "eth_getBlockByHash",
                params: [blockHash, false]
            ) as? [String: Any] else {
                throw EvmSccpProverError.invalidPublicInputs("eth_getBlockByHash")
            }
            block = fetched
        }
        if let currentBlock = block {
            let normalizedBlockHash = try Self.normalizeRpcHex(
                currentBlock["hash"],
                label: "block.hash",
                byteLength: 32
            )
            if let blockHash, blockHash != normalizedBlockHash {
                throw EvmSccpProverError.invalidPublicInputs("block.hash")
            }
            let normalizedBlockNumber = try Self.normalizePositiveRpcQuantity(
                Self.firstPresent(currentBlock, "number", "blockNumber", "block_number"),
                label: "block.number"
            )
            if let receiptBlockNumber, receiptBlockNumber != normalizedBlockNumber {
                throw EvmSccpProverError.invalidPublicInputs("block.number")
            }
            receiptBlockNumber = normalizedBlockNumber
            blockReceiptsRoot = try Self.normalizeRpcHex(
                Self.firstPresent(currentBlock, "receiptsRoot", "receipts_root"),
                label: "block.receiptsRoot",
                byteLength: 32
            )
        }

        let selectedConsensusProvider = finalityProvider ?? consensusProvider
        let beaconFinality: [String: Any]?
        if let suppliedFinality = input.beaconFinality {
            beaconFinality = try Self.normalizeBeaconFinality(
                suppliedFinality,
                expectedBlockHash: blockHash,
                expectedBlockNumber: receiptBlockNumber,
                expectedReceiptsRoot: blockReceiptsRoot
            )
        } else if let selectedConsensusProvider {
            let collectedFinality = try await selectedConsensusProvider.collectFinalityEvidence(
                receipt: receipt,
                block: block,
                transactionHash: transactionHash
            )
            beaconFinality = try Self.normalizeBeaconFinality(
                collectedFinality,
                expectedBlockHash: blockHash,
                expectedBlockNumber: receiptBlockNumber,
                expectedReceiptsRoot: blockReceiptsRoot
            )
        } else {
            beaconFinality = nil
        }

        var blockReceipts = input.blockReceipts
        if receiptProof == nil,
           let currentReceipt = receipt,
           let beaconFinality,
           let sourceEventDigest,
           let inclusionBranch = input.inclusionBranch {
            if blockReceipts == nil {
                guard let selectedProvider else {
                    throw EvmSccpProverError.invalidPublicInputs("executionProvider")
                }
                guard let receiptBlockNumber else {
                    throw EvmSccpProverError.invalidPublicInputs("receipt.blockNumber")
                }
                guard let fetched = try await selectedProvider.request(
                    method: "eth_getBlockReceipts",
                    params: [receiptBlockNumber]
                ) as? [[String: Any]] else {
                    throw EvmSccpProverError.invalidPublicInputs("eth_getBlockReceipts")
                }
                blockReceipts = fetched
            }
            guard let blockReceipts else {
                throw EvmSccpProverError.invalidPublicInputs("blockReceipts")
            }
            guard let receiptTransactionIndex = Self.firstPresent(
                currentReceipt,
                "transactionIndex",
                "transaction_index"
            ) else {
                throw EvmSccpProverError.invalidPublicInputs("receipt.transactionIndex")
            }
            let receiptTrieProof = try buildEvmReceiptTrieProofFromReceipts(
                blockReceipts,
                transactionIndex: receiptTransactionIndex
            )
            let expectedReceiptsRoot: String?
            if let blockReceiptsRoot {
                expectedReceiptsRoot = blockReceiptsRoot
            } else {
                expectedReceiptsRoot = try Self.strictFirstPresent(
                    beaconFinality,
                    label: "beaconFinality.executionReceiptsRoot",
                    "executionReceiptsRoot",
                    "execution_receipts_root"
                ) as? String
            }
            guard let expectedReceiptsRoot, receiptTrieProof.receiptsRoot == expectedReceiptsRoot else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.executionReceiptsRoot")
            }
            let targetIndex = try Self.normalizeUnsignedInteger(
                receiptTransactionIndex,
                label: "receipt.transactionIndex"
            )
            guard targetIndex < UInt64(blockReceipts.count),
                  targetIndex <= UInt64(Int.max) else {
                throw EvmSccpProverError.invalidPublicInputs("receipt.transactionIndex")
            }
            let indexedReceipt = blockReceipts[Int(targetIndex)]
            let indexedTransactionHash = try Self.normalizeRpcHex(
                Self.firstPresent(indexedReceipt, "transactionHash", "transaction_hash"),
                label: "blockReceipts.transactionHash",
                byteLength: 32
            )
            guard let transactionHash else {
                throw EvmSccpProverError.invalidPublicInputs("transactionHash")
            }
            guard indexedTransactionHash == transactionHash else {
                throw EvmSccpProverError.invalidPublicInputs("blockReceipts.transactionHash")
            }
            let indexedBlockHash = try Self.normalizeRpcHex(
                Self.firstPresent(indexedReceipt, "blockHash", "block_hash"),
                label: "blockReceipts.blockHash",
                byteLength: 32
            )
            guard indexedBlockHash == blockHash else {
                throw EvmSccpProverError.invalidPublicInputs("blockReceipts.blockHash")
            }
            let indexedBlockNumber = try Self.normalizePositiveRpcQuantity(
                Self.firstPresent(indexedReceipt, "blockNumber", "block_number"),
                label: "blockReceipts.blockNumber"
            )
            guard indexedBlockNumber == receiptBlockNumber else {
                throw EvmSccpProverError.invalidPublicInputs("blockReceipts.blockNumber")
            }
            let receiptRlp = "0x" + (try canonicalEvmReceiptRlp(currentReceipt)).hexEncodedString()
            guard receiptTrieProof.receiptRlp == receiptRlp else {
                throw EvmSccpProverError.invalidPublicInputs("blockReceipts.receiptRlp")
            }
            guard let beaconSlotInput = try Self.strictFirstPresent(
                beaconFinality,
                label: "beaconFinality.beaconSlot",
                "beaconSlot",
                "beacon_slot",
                "finalizedSlot",
                "finalized_slot",
                "slot"
            ) else {
                throw EvmSccpProverError.invalidPublicInputs("beaconFinality.beaconSlot")
            }
            guard let finalizedRootInput = try Self.strictFirstPresent(
                beaconFinality,
                label: "beaconFinality.finalizedHeaderRoot",
                "finalizedHeaderRoot",
                "finalized_header_root",
                "beaconFinalizedRoot",
                "beacon_finalized_root"
            ) else {
                throw EvmSccpProverError.invalidPublicInputs("beaconFinality.finalizedHeaderRoot")
            }
            guard let syncCommitteeRootInput = try Self.strictFirstPresent(
                beaconFinality,
                label: "beaconFinality.syncCommitteeRoot",
                "syncCommitteeRoot",
                "sync_committee_root"
            ) else {
                throw EvmSccpProverError.invalidPublicInputs("beaconFinality.syncCommitteeRoot")
            }
            receiptProof = EthereumMainnetReceiptProof(
                sourceEventDigest: sourceEventDigest,
                beaconSlot: try Self.normalizeUnsignedInteger(
                    beaconSlotInput,
                    label: "beaconFinality.beaconSlot"
                ),
                executionBlockNumber: try Self.normalizeUnsignedInteger(
                    Self.strictFirstPresent(
                        beaconFinality,
                        label: "beaconFinality.executionBlockNumber",
                        "executionBlockNumber",
                        "execution_block_number"
                    ),
                    label: "beaconFinality.executionBlockNumber"
                ),
                executionBlockHash: try Self.normalizeRpcHex(
                    Self.strictFirstPresent(
                        beaconFinality,
                        label: "beaconFinality.executionBlockHash",
                        "executionBlockHash",
                        "execution_block_hash"
                    ),
                    label: "beaconFinality.executionBlockHash",
                    byteLength: 32
                ),
                executionReceiptsRoot: try Self.normalizeRpcHex(
                    Self.strictFirstPresent(
                        beaconFinality,
                        label: "beaconFinality.executionReceiptsRoot",
                        "executionReceiptsRoot",
                        "execution_receipts_root"
                    ),
                    label: "beaconFinality.executionReceiptsRoot",
                    byteLength: 32
                ),
                beaconFinalizedRoot: try Self.normalizeRpcHex(
                    finalizedRootInput,
                    label: "beaconFinality.finalizedHeaderRoot",
                    byteLength: 32
                ),
                syncCommitteeRoot: try Self.normalizeRpcHex(
                    syncCommitteeRootInput,
                    label: "beaconFinality.syncCommitteeRoot",
                    byteLength: 32
                ),
                receiptRootIndex: targetIndex,
                receiptTrieProofNodes: receiptTrieProof.receiptTrieProofNodes,
                inclusionBranch: inclusionBranch.map { Data($0) }
            )
        }

        try Self.requireReceiptProofMatchesEvidence(
            receiptProof,
            blockHash: blockHash,
            receiptBlockNumber: receiptBlockNumber,
            blockReceiptsRoot: blockReceiptsRoot,
            beaconFinality: beaconFinality,
            sourceEventDigest: sourceEventDigest
        )

        return EthereumMainnetInboundEvidence(
            sourceDomain: sccpDomainEthereum,
            targetDomain: sccpDomainSora,
            transactionHash: transactionHash,
            receipt: receipt,
            block: block,
            beaconFinality: beaconFinality,
            blockReceipts: blockReceipts,
            inclusionBranch: input.inclusionBranch?.map { Data($0) },
            receiptProof: receiptProof,
            receiptProofHash: try Self.normalizeReceiptProofHash(
                receiptProof: receiptProof,
                suppliedHash: input.receiptProofHash
            ),
            sourceEventDigest: sourceEventDigest,
            sourceBridgeEmitterAddress: sourceBridgeEmitterAddress
        )
    }

    public func proveInboundToSora(
        _ input: EthereumMainnetInboundEvidence,
        executionProvider provider: EthereumMainnetExecutionProvider? = nil,
        consensusProvider finalityProvider: EthereumMainnetConsensusProvider? = nil
    ) async throws -> Data {
        guard let inboundProveFunction else {
            throw EvmSccpProverError.localProverUnavailable
        }
        let evidence = try await collectInboundEvidenceFromReceipt(
            input,
            executionProvider: provider,
            consensusProvider: finalityProvider
        )
        guard evidence.beaconFinality != nil else {
            throw EvmSccpProverError.invalidPublicInputs("beaconFinality")
        }
        guard evidence.receiptProof != nil else {
            throw EvmSccpProverError.invalidPublicInputs("receiptProof")
        }
        guard evidence.sourceEventDigest != nil else {
            throw EvmSccpProverError.invalidPublicInputs("receipt.sourceEvent")
        }
        guard evidence.beaconFinality?["finalizedHeaderRoot"] != nil else {
            throw EvmSccpProverError.invalidPublicInputs("beaconFinality.finalizedHeaderRoot")
        }
        guard evidence.beaconFinality?["syncCommitteeRoot"] != nil else {
            throw EvmSccpProverError.invalidPublicInputs("beaconFinality.syncCommitteeRoot")
        }
        guard evidence.beaconFinality?["beaconSlot"] != nil else {
            throw EvmSccpProverError.invalidPublicInputs("beaconFinality.beaconSlot")
        }
        for field in [
            "syncCommitteeBits",
            "syncCommitteeSignature",
            "syncCommitteeParticipation",
            "syncSignatureSlot",
        ] {
            guard evidence.beaconFinality?[field] != nil else {
                throw EvmSccpProverError.invalidPublicInputs("beaconFinality.\(field)")
            }
        }
        let proofBytes = try await inboundProveFunction(evidence)
        guard !proofBytes.isEmpty else {
            throw EvmSccpProverError.emptyProof
        }
        guard proofBytes.contains(where: { $0 != 0 }) else {
            throw EvmSccpProverError.allZeroProof
        }
        return Data(proofBytes)
    }

    public func submitInboundToIroha(_ proofBytes: Data) async throws -> Any {
        guard !proofBytes.isEmpty else {
            throw EvmSccpProverError.emptyProof
        }
        guard proofBytes.contains(where: { $0 != 0 }) else {
            throw EvmSccpProverError.allZeroProof
        }
        guard let inboundSubmitFunction else {
            throw EvmSccpProverError.localProverUnavailable
        }
        return try await inboundSubmitFunction(Data(proofBytes))
    }

    public func buildOutboundProofRequest(_ input: EvmSccpProofRequestInput) async throws -> EvmSccpProofRequest {
        let request = try await prover.buildRequest(input)
        try Self.requireEthereumMainnetRequest(request)
        return request
    }

    public func proveOutboundToEthereum(_ input: EvmSccpProofRequestInput) async throws -> EvmSccpProofResult {
        let request = try await buildOutboundProofRequest(input)
        let result = try await prover.prove(request)
        guard result.publicInputs.targetDomain == sccpDomainEthereum else {
            throw EvmSccpProverError.invalidPublicInputs("proofResult.publicInputs.targetDomain")
        }
        guard result.destinationBinding?.sourceDomain == sccpDomainSora else {
            throw EvmSccpProverError.invalidPublicInputs("proofResult.destinationBinding.sourceDomain")
        }
        guard result.destinationBinding?.networkId == sccpEthereumMainnetNetworkId else {
            throw EvmSccpProverError.invalidPublicInputs("proofResult.destinationBinding.networkId")
        }
        guard result.destinationBinding?.hash == result.destinationBindingHash else {
            throw EvmSccpProverError.invalidPublicInputs("proofResult.destinationBinding")
        }
        return result
    }

    public func buildEthereumCalldata(_ input: EvmSccpSubmissionInput) throws -> EvmSccpSubmission {
        let submission = try buildEvmSccpSubmission(input)
        guard submission.targetDomain == sccpDomainEthereum else {
            throw EvmSccpProverError.invalidPublicInputs("submission.targetDomain")
        }
        guard let proofResult = input.proofResult,
              let destinationBinding = proofResult.destinationBinding,
              destinationBinding.sourceDomain == sccpDomainSora,
              destinationBinding.targetDomain == sccpDomainEthereum,
              destinationBinding.networkId == sccpEthereumMainnetNetworkId,
              destinationBinding.hash == proofResult.destinationBindingHash else {
            throw EvmSccpProverError.invalidPublicInputs("proofResult.destinationBinding")
        }
        return submission
    }

    public func buildLocalAdmissionSubmission(
        _ input: EthereumMainnetLocalAdmissionSubmissionInput
    ) throws -> EthereumMainnetLocalAdmissionSubmission {
        try buildEthereumMainnetSccpLocalAdmissionSubmission(input)
    }

    public func submitOutboundToEthereum(_ input: EvmSccpSubmissionInput) async throws -> Any {
        let submission = try buildEthereumCalldata(input)
        guard let outboundSubmitFunction else {
            throw EvmSccpProverError.localProverUnavailable
        }
        if let executionProvider {
            _ = try await validateExecutionProviderMainnet(executionProvider)
        }
        return try await outboundSubmitFunction(submission)
    }

    private static func requireEthereumMainnetRequest(_ request: EvmSccpProofRequest) throws {
        guard request.sourceDomain == sccpDomainSora else {
            throw EvmSccpProverError.invalidPublicInputs("request.sourceDomain")
        }
        guard request.targetDomain == sccpDomainEthereum,
              request.publicInputs.targetDomain == sccpDomainEthereum else {
            throw EvmSccpProverError.invalidPublicInputs("request.targetDomain")
        }
        guard request.destinationBinding?.sourceDomain == sccpDomainSora else {
            throw EvmSccpProverError.invalidPublicInputs("request.destinationBinding.sourceDomain")
        }
        guard request.destinationBinding?.networkId == sccpEthereumMainnetNetworkId else {
            throw EvmSccpProverError.invalidPublicInputs("request.destinationBinding.networkId")
        }
    }

    private static func normalizeRpcChainId(_ value: Any) throws -> UInt64 {
        let quantity = try Self.normalizeRpcQuantity(value, label: "eth_chainId")
        guard let parsed = UInt64(String(quantity.dropFirst(2)), radix: 16) else {
            throw EvmSccpProverError.invalidPublicInputs("eth_chainId")
        }
        return parsed
    }

    private static func normalizeReceiptProofHash(
        receiptProof: EthereumMainnetReceiptProof?,
        suppliedHash: String?
    ) throws -> String? {
        var normalizedHash = try suppliedHash.map {
            try normalizeRpcHex($0, label: "receiptProofHash", byteLength: 32)
        }
        guard let receiptProof else {
            return normalizedHash
        }
        guard receiptProof.sourceDomain == sccpDomainEthereum else {
            throw EvmSccpProverError.invalidPublicInputs("receiptProof.sourceDomain")
        }
        let computedHash = try evmSccpReceiptProofHash(
            sourceDomain: receiptProof.sourceDomain,
            sourceEventDigest: receiptProof.sourceEventDigest,
            beaconSlot: receiptProof.beaconSlot,
            executionBlockNumber: receiptProof.executionBlockNumber,
            executionBlockHash: receiptProof.executionBlockHash,
            executionReceiptsRoot: receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot: receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot: receiptProof.syncCommitteeRoot,
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
            inclusionBranch: receiptProof.inclusionBranch
        )
        if let normalizedHash, normalizedHash != computedHash {
            throw EvmSccpProverError.invalidPublicInputs("receiptProofHash")
        }
        normalizedHash = computedHash
        return normalizedHash
    }

    private static func requireReceiptProofMatchesEvidence(
        _ receiptProof: EthereumMainnetReceiptProof?,
        blockHash: String?,
        receiptBlockNumber: String?,
        blockReceiptsRoot: String?,
        beaconFinality: [String: Any]?,
        sourceEventDigest: String?
    ) throws {
        guard let receiptProof else {
            return
        }
        if let receiptBlockNumber {
            let expected = try normalizeUnsignedInteger(receiptBlockNumber, label: "block.number")
            guard receiptProof.executionBlockNumber == expected else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.executionBlockNumber")
            }
        }
        if let beaconFinality {
            let finalityBlockNumber = try normalizeUnsignedInteger(
                strictFirstPresent(
                    beaconFinality,
                    label: "beaconFinality.executionBlockNumber",
                    "executionBlockNumber",
                    "execution_block_number"
                ),
                label: "beaconFinality.executionBlockNumber"
            )
            guard receiptProof.executionBlockNumber == finalityBlockNumber else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.executionBlockNumber")
            }
        }
        let proofBlockHash = try normalizeRpcHex(
            receiptProof.executionBlockHash,
            label: "receiptProof.executionBlockHash",
            byteLength: 32
        )
        if let blockHash, proofBlockHash != blockHash {
            throw EvmSccpProverError.invalidPublicInputs("receiptProof.executionBlockHash")
        }
        if let beaconFinality {
            let finalityBlockHash = try normalizeRpcHex(
                strictFirstPresent(
                    beaconFinality,
                    label: "beaconFinality.executionBlockHash",
                    "executionBlockHash",
                    "execution_block_hash"
                ),
                label: "beaconFinality.executionBlockHash",
                byteLength: 32
            )
            guard proofBlockHash == finalityBlockHash else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.executionBlockHash")
            }
        }
        let proofReceiptsRoot = try normalizeRpcHex(
            receiptProof.executionReceiptsRoot,
            label: "receiptProof.executionReceiptsRoot",
            byteLength: 32
        )
        if let blockReceiptsRoot, proofReceiptsRoot != blockReceiptsRoot {
            throw EvmSccpProverError.invalidPublicInputs("receiptProof.executionReceiptsRoot")
        }
        if let beaconFinality {
            let finalityReceiptsRoot = try normalizeRpcHex(
                strictFirstPresent(
                    beaconFinality,
                    label: "beaconFinality.executionReceiptsRoot",
                    "executionReceiptsRoot",
                    "execution_receipts_root"
                ),
                label: "beaconFinality.executionReceiptsRoot",
                byteLength: 32
            )
            guard proofReceiptsRoot == finalityReceiptsRoot else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.executionReceiptsRoot")
            }
            if let finalityFinalizedRootInput = try strictFirstPresent(
                beaconFinality,
                label: "beaconFinality.finalizedHeaderRoot",
                "finalizedHeaderRoot",
                "finalized_header_root",
                "beaconFinalizedRoot",
                "beacon_finalized_root"
            ) {
                let finalityFinalizedRoot = try normalizeRpcHex(
                    finalityFinalizedRootInput,
                    label: "beaconFinality.finalizedHeaderRoot",
                    byteLength: 32
                )
                let proofFinalizedRoot = try normalizeRpcHex(
                    receiptProof.beaconFinalizedRoot,
                    label: "receiptProof.beaconFinalizedRoot",
                    byteLength: 32
                )
                guard proofFinalizedRoot == finalityFinalizedRoot else {
                    throw EvmSccpProverError.invalidPublicInputs("receiptProof.beaconFinalizedRoot")
                }
            }
            if let finalitySyncCommitteeRootInput = try strictFirstPresent(
                beaconFinality,
                label: "beaconFinality.syncCommitteeRoot",
                "syncCommitteeRoot",
                "sync_committee_root"
            ) {
                let finalitySyncCommitteeRoot = try normalizeRpcHex(
                    finalitySyncCommitteeRootInput,
                    label: "beaconFinality.syncCommitteeRoot",
                    byteLength: 32
                )
                let proofSyncCommitteeRoot = try normalizeRpcHex(
                    receiptProof.syncCommitteeRoot,
                    label: "receiptProof.syncCommitteeRoot",
                    byteLength: 32
                )
                guard proofSyncCommitteeRoot == finalitySyncCommitteeRoot else {
                    throw EvmSccpProverError.invalidPublicInputs("receiptProof.syncCommitteeRoot")
                }
            }
            if let finalityBeaconSlotInput = try strictFirstPresent(
                beaconFinality,
                label: "beaconFinality.beaconSlot",
                "beaconSlot",
                "beacon_slot",
                "finalizedSlot",
                "finalized_slot",
                "slot"
            ) {
                let finalityBeaconSlot = try normalizeUnsignedInteger(
                    finalityBeaconSlotInput,
                    label: "beaconFinality.beaconSlot"
                )
                guard receiptProof.beaconSlot == finalityBeaconSlot else {
                    throw EvmSccpProverError.invalidPublicInputs("receiptProof.beaconSlot")
                }
            }
        }
        if let sourceEventDigest {
            let proofSourceEventDigest = try normalizeRpcHex(
                receiptProof.sourceEventDigest,
                label: "receiptProof.sourceEventDigest",
                byteLength: 32
            )
            guard proofSourceEventDigest == sourceEventDigest else {
                throw EvmSccpProverError.invalidPublicInputs("receiptProof.sourceEventDigest")
            }
        }
    }

    private static func normalizeUnsignedInteger(_ value: Any?, label: String) throws -> UInt64 {
        if let value = value as? UInt64 {
            return value
        }
        if let value = value as? UInt32 {
            return UInt64(value)
        }
        if let value = value as? UInt {
            return UInt64(value)
        }
        if let value = value as? Int {
            guard value >= 0 else {
                throw EvmSccpProverError.invalidPublicInputs(label)
            }
            return UInt64(value)
        }
        guard let text = value as? String, text.trimmingCharacters(in: .whitespacesAndNewlines) == text else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        if text.hasPrefix("0x") {
            let hex = String(text.dropFirst(2))
            guard !hex.isEmpty,
                  hex == "0" || (hex.first != "0" && hex.allSatisfy { Self.isLowerHex($0) }),
                  let parsed = UInt64(hex, radix: 16) else {
                throw EvmSccpProverError.invalidPublicInputs(label)
            }
            return parsed
        }
        guard !text.isEmpty,
              text == "0" || (text.first != "0" && text.allSatisfy { Self.isDecimalDigit($0) }),
              let parsed = UInt64(text, radix: 10) else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        return parsed
    }

    private static func firstPresent(_ input: [String: Any], _ keys: String...) -> Any? {
        for key in keys where input.keys.contains(key) {
            return input[key]
        }
        return nil
    }

    private static func strictFirstPresent(_ input: [String: Any], label: String, _ keys: String...) throws -> Any? {
        var selected: Any?
        var found = false
        for key in keys where input.keys.contains(key) {
            guard !found else {
                throw EvmSccpProverError.invalidPublicInputs(label)
            }
            selected = input[key]
            found = true
        }
        return selected
    }

    private static func requireMapList(_ value: Any, label: String) throws -> [[String: Any]] {
        if let maps = value as? [[String: Any]] {
            return maps
        }
        if let values = value as? [Any] {
            var maps: [[String: Any]] = []
            maps.reserveCapacity(values.count)
            for item in values {
                guard let map = item as? [String: Any] else {
                    throw EvmSccpProverError.invalidPublicInputs(label)
                }
                maps.append(map)
            }
            return maps
        }
        throw EvmSccpProverError.invalidPublicInputs(label)
    }

    private static func normalizeRpcHex(
        _ value: Any?,
        label: String,
        byteLength: Int,
        allowZero: Bool = false
    ) throws -> String {
        guard let text = value as? String,
              text.trimmingCharacters(in: .whitespacesAndNewlines) == text,
              text.hasPrefix("0x") else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        let hex = String(text.dropFirst(2))
        guard hex.count == byteLength * 2,
              hex.allSatisfy({ Self.isLowerHex($0) }) else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        guard allowZero || hex.contains(where: { $0 != "0" }) else {
            throw EvmSccpProverError.zeroField(label)
        }
        return text
    }

    private static func normalizeRpcQuantity(_ value: Any?, label: String) throws -> String {
        guard let text = value as? String,
              text.trimmingCharacters(in: .whitespacesAndNewlines) == text,
              text.hasPrefix("0x") else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        let hex = String(text.dropFirst(2))
        guard !hex.isEmpty,
              hex == "0" || (hex.first != "0" && hex.allSatisfy { Self.isLowerHex($0) }),
              let parsed = UInt64(hex, radix: 16) else {
            throw EvmSccpProverError.invalidPublicInputs(label)
        }
        return "0x" + String(parsed, radix: 16)
    }

    private static func normalizePositiveRpcQuantity(_ value: Any?, label: String) throws -> String {
        let quantity = try Self.normalizeRpcQuantity(value, label: label)
        if quantity == "0x0" {
            throw EvmSccpProverError.zeroField(label)
        }
        return quantity
    }

    private static func normalizeBeaconFinality(
        _ finality: [String: Any],
        expectedBlockHash: String?,
        expectedBlockNumber: String?,
        expectedReceiptsRoot: String?
    ) throws -> [String: Any] {
        let executionBlockNumber = try Self.normalizeUnsignedInteger(
            Self.strictFirstPresent(
                finality,
                label: "beaconFinality.executionBlockNumber",
                "executionBlockNumber",
                "execution_block_number",
                "finalityHeight",
                "finality_height"
            ),
            label: "beaconFinality.executionBlockNumber"
        )
        guard executionBlockNumber != 0 else {
            throw EvmSccpProverError.zeroField("beaconFinality.executionBlockNumber")
        }
        if let expectedBlockNumber {
            let expected = try Self.normalizeUnsignedInteger(expectedBlockNumber, label: "block.number")
            guard executionBlockNumber == expected else {
                throw EvmSccpProverError.invalidPublicInputs("beaconFinality.executionBlockNumber")
            }
        }
        let executionBlockHash = try Self.normalizeRpcHex(
            Self.strictFirstPresent(
                finality,
                label: "beaconFinality.executionBlockHash",
                "executionBlockHash",
                "execution_block_hash",
                "finalityBlockHash",
                "finality_block_hash"
            ),
            label: "beaconFinality.executionBlockHash",
            byteLength: 32
        )
        if let expectedBlockHash, executionBlockHash != expectedBlockHash {
            throw EvmSccpProverError.invalidPublicInputs("beaconFinality.executionBlockHash")
        }
        let executionReceiptsRoot = try Self.normalizeRpcHex(
            Self.strictFirstPresent(
                finality,
                label: "beaconFinality.executionReceiptsRoot",
                "executionReceiptsRoot",
                "execution_receipts_root",
                "receiptsRoot",
                "receipts_root"
            ),
            label: "beaconFinality.executionReceiptsRoot",
            byteLength: 32
        )
        if let expectedReceiptsRoot, executionReceiptsRoot != expectedReceiptsRoot {
            throw EvmSccpProverError.invalidPublicInputs("beaconFinality.executionReceiptsRoot")
        }
        var normalized = finality
        normalized["executionBlockNumber"] = String(executionBlockNumber)
        normalized["executionBlockHash"] = executionBlockHash
        normalized["executionReceiptsRoot"] = executionReceiptsRoot
        if let finalizedHeaderRootInput = try Self.strictFirstPresent(
            finality,
            label: "beaconFinality.finalizedHeaderRoot",
            "finalizedHeaderRoot",
            "finalized_header_root",
            "beaconFinalizedRoot",
            "beacon_finalized_root"
        ) {
            normalized["finalizedHeaderRoot"] = try Self.normalizeRpcHex(
                finalizedHeaderRootInput,
                label: "beaconFinality.finalizedHeaderRoot",
                byteLength: 32
            )
        }
        if let syncCommitteeRootInput = try Self.strictFirstPresent(
            finality,
            label: "beaconFinality.syncCommitteeRoot",
            "syncCommitteeRoot",
            "sync_committee_root"
        ) {
            normalized["syncCommitteeRoot"] = try Self.normalizeRpcHex(
                syncCommitteeRootInput,
                label: "beaconFinality.syncCommitteeRoot",
                byteLength: 32
            )
        }
        if let beaconSlotInput = try Self.strictFirstPresent(
            finality,
            label: "beaconFinality.beaconSlot",
            "beaconSlot",
            "beacon_slot",
            "finalizedSlot",
            "finalized_slot",
            "slot"
        ) {
            let beaconSlot = try Self.normalizeUnsignedInteger(
                beaconSlotInput,
                label: "beaconFinality.beaconSlot"
            )
            guard beaconSlot != 0 else {
                throw EvmSccpProverError.zeroField("beaconFinality.beaconSlot")
            }
            normalized["beaconSlot"] = String(beaconSlot)
        }
        if let syncCommitteeBitsInput = try Self.strictFirstPresent(
            finality,
            label: "beaconFinality.syncCommitteeBits",
            "syncCommitteeBits",
            "sync_committee_bits"
        ) {
            normalized["syncCommitteeBits"] = try Self.normalizeFinalitySyncCommitteeBits(
                syncCommitteeBitsInput,
                label: "beaconFinality.syncCommitteeBits"
            )
        }
        if let syncCommitteeSignatureInput = try Self.strictFirstPresent(
            finality,
            label: "beaconFinality.syncCommitteeSignature",
            "syncCommitteeSignature",
            "sync_committee_signature"
        ) {
            normalized["syncCommitteeSignature"] = try Self.normalizeRpcHex(
                syncCommitteeSignatureInput,
                label: "beaconFinality.syncCommitteeSignature",
                byteLength: 96
            )
        }
        if let syncSignatureSlotInput = try Self.strictFirstPresent(
            finality,
            label: "beaconFinality.syncSignatureSlot",
            "syncSignatureSlot",
            "sync_signature_slot",
            "signatureSlot",
            "signature_slot"
        ) {
            let syncSignatureSlot = try Self.normalizeUnsignedInteger(
                syncSignatureSlotInput,
                label: "beaconFinality.syncSignatureSlot"
            )
            guard syncSignatureSlot != 0 else {
                throw EvmSccpProverError.zeroField("beaconFinality.syncSignatureSlot")
            }
            normalized["syncSignatureSlot"] = String(syncSignatureSlot)
        }
        if let syncCommitteeParticipationInput = try Self.strictFirstPresent(
            finality,
            label: "beaconFinality.syncCommitteeParticipation",
            "syncCommitteeParticipation",
            "sync_committee_participation"
        ) {
            let syncCommitteeParticipation = try Self.normalizeUnsignedInteger(
                syncCommitteeParticipationInput,
                label: "beaconFinality.syncCommitteeParticipation"
            )
            guard syncCommitteeParticipation != 0 else {
                throw EvmSccpProverError.zeroField("beaconFinality.syncCommitteeParticipation")
            }
            normalized["syncCommitteeParticipation"] = String(syncCommitteeParticipation)
        }
        return normalized
    }

    private static func normalizeFinalitySyncCommitteeBits(_ value: Any?, label: String) throws -> String {
        let bits = try normalizeRpcHex(value, label: label, byteLength: 64, allowZero: true)
        guard finalitySyncCommitteeParticipation(bits) != 0 else {
            throw EvmSccpProverError.zeroField(label)
        }
        return bits
    }

    private static func finalitySyncCommitteeParticipation(_ bits: String) -> UInt64 {
        guard bits.hasPrefix("0x"),
              let bytes = Data(hexString: String(bits.dropFirst(2))) else {
            return 0
        }
        var count: UInt64 = 0
        for byte in bytes {
            var value = byte
            while value != 0 {
                count += UInt64(value & 1)
                value >>= 1
            }
        }
        return count
    }

    private static func isLowerHex(_ character: Character) -> Bool {
        "0123456789abcdef".contains(character)
    }

    private static func isDecimalDigit(_ character: Character) -> Bool {
        "0123456789".contains(character)
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
    guard UInt64(input.bundleBytes.count) <= UInt64(UInt32.max) else {
        throw EvmSccpProverError.invalidPublicInputs("proof byte length")
    }
    let sourceProofBytes = try requireEvmOptionalSourceProofBytes(input.sourceProofBytes, field: "sourceProofBytes")
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
    evmAppendU32Le(UInt32(sourceProofBytes.count), to: &preimage)
    preimage.append(sourceProofBytes)
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
        sourceProofBytes: sourceProofBytes,
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
    let sourceProofBytes = try requireEvmOptionalSourceProofBytes(
        proofResult.sourceProofBytes,
        field: "proofResult.sourceProofBytes"
    )
    let expectedRequest = try buildEvmSccpProofRequest(EvmSccpProofRequestInput(
        publicInputs: proofResult.publicInputs,
        bundleBytes: proofResult.bundleBytes,
        sourceProofBytes: sourceProofBytes,
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
    try requireEvmOptionalSourceProofBytes(request.sourceProofBytes, field: "request.sourceProofBytes")
    try requireProductionEvmDestinationBinding(request)
}

@discardableResult
private func requireEvmOptionalSourceProofBytes(_ bytes: Data, field: String) throws -> Data {
    guard bytes.count <= sccpSourceStateMaxProofBytes else {
        throw EvmSccpProverError.invalidPublicInputs(field)
    }
    guard bytes.isEmpty || bytes.contains(where: { $0 != 0 }) else {
        throw EvmSccpProverError.invalidPublicInputs(field)
    }
    return bytes
}

private func requireEvmLocalAdmissionBytes(_ bytes: Data, field: String) throws -> Data {
    guard !bytes.isEmpty else {
        throw EvmSccpProverError.emptyProof
    }
    guard bytes.contains(where: { $0 != 0 }) else {
        throw EvmSccpProverError.allZeroProof
    }
    guard bytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw EvmSccpProverError.invalidPublicInputs(field)
    }
    return Data(bytes)
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

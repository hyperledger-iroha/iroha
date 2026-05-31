import Foundation

/// Proof backend id expected by Substrate-family SCCP runtime verifier calls.
public let sccpSubstrateRuntimeProofBackendV1 = "substrate-runtime-v1"

/// SCALE runtime-call envelope encoding emitted for Substrate-family SCCP submissions.
public let sccpSubstrateRuntimeCallScaleV1 = "scale_call_v1"

/// Runtime verifier entrypoint used by Substrate-family SCCP submit calls.
public let sccpSubstrateSubmitMessageProofEntrypointV1 = "SccpBridge.submit_message_proof"

/// SCCP public inputs shared by Substrate runtime proof requests.
public struct SubstrateSccpPublicInputsInput: Equatable {
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
                targetDomain: UInt32 = sccpDomainSora2,
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

/// Inputs used to build a local Substrate SCCP runtime proof request.
public struct SubstrateSccpProofRequestInput: Equatable {
    public let publicInputs: SubstrateSccpPublicInputsInput
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let backend: String
    public let sourceDomain: UInt32

    public init(publicInputs: SubstrateSccpPublicInputsInput,
                bundleBytes: Data,
                sourceProofBytes: Data = Data(),
                statementHash: String,
                destinationBindingHash: String,
                backend: String = sccpSubstrateRuntimeProofBackendV1,
                sourceDomain: UInt32 = sccpDomainSora) {
        self.publicInputs = publicInputs
        self.bundleBytes = bundleBytes
        self.sourceProofBytes = sourceProofBytes
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.backend = backend
        self.sourceDomain = sourceDomain
    }
}

/// Statement and destination context proved by the local Substrate SCCP prover.
public struct SubstrateSccpProofContext: Equatable {
    public let version: UInt8
    public let statementHash: String
    public let destinationBindingHash: String
}

/// Request passed to a linked local Substrate runtime prover.
public struct SubstrateSccpProofRequest: Equatable {
    public let version: UInt8
    public let backend: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let publicInputs: SubstrateSccpPublicInputsInput
    public let publicInputsBytes: Data
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let proofContext: SubstrateSccpProofContext
    public let statementHash: String
    public let destinationBindingHash: String
    public let requestHash: String
}

/// Proof bytes returned by a linked local Substrate runtime prover.
public struct SubstrateSccpProofResult: Equatable {
    public let version: UInt8
    public let backend: String
    public let proofBytes: Data
    public let proofBase64: String
    public let publicInputs: SubstrateSccpPublicInputsInput
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let proofContext: SubstrateSccpProofContext
    public let statementHash: String
    public let destinationBindingHash: String
    public let requestHash: String
    public let envelopeHash: String

    public init(version: UInt8,
                backend: String,
                proofBytes: Data,
                proofBase64: String,
                publicInputs: SubstrateSccpPublicInputsInput,
                bundleBytes: Data = Data(),
                sourceProofBytes: Data = Data(),
                proofContext: SubstrateSccpProofContext,
                statementHash: String,
                destinationBindingHash: String,
                requestHash: String,
                envelopeHash: String) {
        self.version = version
        self.backend = backend
        self.proofBytes = proofBytes
        self.proofBase64 = proofBase64
        self.publicInputs = publicInputs
        self.bundleBytes = bundleBytes
        self.sourceProofBytes = sourceProofBytes
        self.proofContext = proofContext
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.requestHash = requestHash
        self.envelopeHash = envelopeHash
    }
}

/// Inputs used to package a completed Substrate proof for runtime submission.
public struct SubstrateSccpSubmissionInput: Equatable {
    public let publicInputs: SubstrateSccpPublicInputsInput
    public let proofBytes: Data
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let sourceDomain: UInt32
    public let proofResult: SubstrateSccpProofResult?

    public init(publicInputs: SubstrateSccpPublicInputsInput,
                proofBytes: Data,
                bundleBytes: Data,
                sourceProofBytes: Data = Data(),
                statementHash: String,
                destinationBindingHash: String,
                sourceDomain: UInt32 = sccpDomainSora,
                proofResult: SubstrateSccpProofResult? = nil) {
        self.publicInputs = publicInputs
        self.proofBytes = proofBytes
        self.bundleBytes = bundleBytes
        self.sourceProofBytes = sourceProofBytes
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.sourceDomain = sourceDomain
        self.proofResult = proofResult
    }

    public init(proofResult: SubstrateSccpProofResult,
                sourceDomain: UInt32 = sccpDomainSora) {
        self.init(
            publicInputs: proofResult.publicInputs,
            proofBytes: proofResult.proofBytes,
            bundleBytes: proofResult.bundleBytes,
            sourceProofBytes: proofResult.sourceProofBytes,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            sourceDomain: sourceDomain,
            proofResult: proofResult
        )
    }
}

/// One Substrate SCCP runtime-call argument in verifier order.
public struct SubstrateSccpSubmissionArgument: Equatable {
    public let key: String
    public let encoding: String
    public let bytesHex: String
}

/// Substrate-family SCCP runtime call ready for wallet or relayer submission.
public struct SubstrateSccpSubmission: Equatable {
    public let version: UInt8
    public let proofFamily: String
    public let verifierBackend: String
    public let platformPayload: String
    public let envelopeEncoding: String
    public let submissionKind: String
    public let verifierEntrypoint: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let publicInputs: SubstrateSccpPublicInputsInput
    public let proofContext: SubstrateSccpProofContext
    public let statementHash: String
    public let destinationBindingHash: String
    public let requestHash: String
    public let proofBytes: Data
    public let publicInputsBytes: Data
    public let bundleBytes: Data
    public let arguments: [SubstrateSccpSubmissionArgument]
    public let runtimeCall: Data
    public let runtimeCallHex: String
    public let envelopeBytes: Data
    public let envelopeHex: String
}

/// Error cases for Substrate SCCP local proof request construction.
public enum SubstrateSccpProverError: Error, Equatable {
    case invalidHex32(String)
    case zeroField(String)
    case invalidPublicInputs(String)
    case localProverUnavailable
    case emptyProof
    case allZeroProof
}

/// Optional async witness resolver backed by app-controlled Substrate RPC calls.
public protocol SubstrateSccpWitnessProvider {
    func resolveWitness(_ input: SubstrateSccpProofRequestInput) async throws -> SubstrateSccpProofRequestInput
}

/// Local-first Substrate SCCP proof wrapper. It never fabricates proofs; callers must link a prover.
public final class SubstrateSccpProver {
    public typealias ProveFunction = (SubstrateSccpProofRequest) async throws -> Data

    private let witnessProvider: SubstrateSccpWitnessProvider?
    private let proveFunction: ProveFunction?

    public init(witnessProvider: SubstrateSccpWitnessProvider? = nil,
                proveFunction: ProveFunction? = nil) {
        self.witnessProvider = witnessProvider
        self.proveFunction = proveFunction
    }

    public func buildRequest(_ input: SubstrateSccpProofRequestInput) async throws -> SubstrateSccpProofRequest {
        let resolved = try await witnessProvider?.resolveWitness(substrateSccpWitnessProviderInputSnapshot(input)) ?? input
        return try buildSubstrateSccpProofRequest(resolved)
    }

    public func prove(_ input: SubstrateSccpProofRequestInput) async throws -> SubstrateSccpProofResult {
        let request = try await buildRequest(input)
        guard let proveFunction else {
            throw SubstrateSccpProverError.localProverUnavailable
        }
        try requireProductionSubstrateSccpProofRequest(request)
        let proofBytes = try await proveFunction(substrateSccpProofRequestCallbackSnapshot(request))
        return try wrapSubstrateSccpProofResult(proofBytes: proofBytes, request: request)
    }
}

private func substrateSccpWitnessProviderInputSnapshot(
    _ input: SubstrateSccpProofRequestInput
) -> SubstrateSccpProofRequestInput {
    SubstrateSccpProofRequestInput(
        publicInputs: input.publicInputs,
        bundleBytes: Data(input.bundleBytes),
        sourceProofBytes: Data(input.sourceProofBytes),
        statementHash: input.statementHash,
        destinationBindingHash: input.destinationBindingHash,
        backend: input.backend,
        sourceDomain: input.sourceDomain
    )
}

private func substrateSccpProofRequestCallbackSnapshot(
    _ request: SubstrateSccpProofRequest
) -> SubstrateSccpProofRequest {
    SubstrateSccpProofRequest(
        version: request.version,
        backend: request.backend,
        sourceDomain: request.sourceDomain,
        targetDomain: request.targetDomain,
        publicInputs: request.publicInputs,
        publicInputsBytes: Data(request.publicInputsBytes),
        bundleBytes: Data(request.bundleBytes),
        sourceProofBytes: Data(request.sourceProofBytes),
        proofContext: request.proofContext,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
        requestHash: request.requestHash
    )
}

/// Canonical SCCP public-input bytes used by Substrate runtime proof requests.
public func canonicalSubstrateSccpPublicInputsBytes(_ input: SubstrateSccpPublicInputsInput) throws -> Data {
    guard input.version == 1 else {
        throw SubstrateSccpProverError.invalidPublicInputs("publicInputs.version")
    }
    guard substrateTargetDomainIsSupported(input.targetDomain) else {
        throw SubstrateSccpProverError.invalidPublicInputs("publicInputs.targetDomain")
    }
    guard input.finalityHeight != 0 else {
        throw SubstrateSccpProverError.invalidPublicInputs("publicInputs.finalityHeight")
    }
    var out = Data()
    out.append(input.version)
    try out.append(substrateNonZeroBytesFromHex32(input.messageId, field: "messageId"))
    try out.append(substrateNonZeroBytesFromHex32(input.payloadHash, field: "payloadHash"))
    substrateAppendU32Le(input.targetDomain, to: &out)
    try out.append(substrateNonZeroBytesFromHex32(input.commitmentRoot, field: "commitmentRoot"))
    substrateAppendU64Le(input.finalityHeight, to: &out)
    try out.append(substrateNonZeroBytesFromHex32(input.finalityBlockHash, field: "finalityBlockHash"))
    return out
}

/// Build a Substrate SCCP runtime proof request for a linked local prover.
public func buildSubstrateSccpProofRequest(_ input: SubstrateSccpProofRequestInput) throws -> SubstrateSccpProofRequest {
    guard input.backend == sccpSubstrateRuntimeProofBackendV1 else {
        throw SubstrateSccpProverError.invalidPublicInputs("backend")
    }
    guard !input.bundleBytes.isEmpty else {
        throw SubstrateSccpProverError.invalidPublicInputs("bundleBytes")
    }
    guard input.bundleBytes.count <= Int(UInt32.max) else {
        throw SubstrateSccpProverError.invalidPublicInputs("bundleBytes")
    }
    guard input.sourceProofBytes.count <= Int(UInt32.max) else {
        throw SubstrateSccpProverError.invalidPublicInputs("sourceProofBytes")
    }
    guard input.sourceProofBytes.isEmpty || input.sourceProofBytes.contains(where: { $0 != 0 }) else {
        throw SubstrateSccpProverError.invalidPublicInputs("sourceProofBytes")
    }
    guard input.sourceDomain == sccpDomainSora else {
        throw SubstrateSccpProverError.invalidPublicInputs("sourceDomain")
    }
    guard input.sourceDomain != input.publicInputs.targetDomain else {
        throw SubstrateSccpProverError.invalidPublicInputs("sourceDomain")
    }
    let publicInputsBytes = try canonicalSubstrateSccpPublicInputsBytes(input.publicInputs)
    let proofContext = try normalizeSubstrateSccpProofContext(
        statementHash: input.statementHash,
        destinationBindingHash: input.destinationBindingHash
    )
    var preimage = Data()
    substrateAppendU32Le(input.sourceDomain, to: &preimage)
    preimage.append(publicInputsBytes)
    substrateAppendU32Le(UInt32(input.bundleBytes.count), to: &preimage)
    preimage.append(input.bundleBytes)
    substrateAppendU32Le(UInt32(input.sourceProofBytes.count), to: &preimage)
    preimage.append(input.sourceProofBytes)
    try preimage.append(substrateBytesFromHex32(proofContext.statementHash, field: "statementHash"))
    try preimage.append(substrateBytesFromHex32(proofContext.destinationBindingHash, field: "destinationBindingHash"))
    return SubstrateSccpProofRequest(
        version: 1,
        backend: input.backend,
        sourceDomain: input.sourceDomain,
        targetDomain: input.publicInputs.targetDomain,
        publicInputs: input.publicInputs,
        publicInputsBytes: publicInputsBytes,
        bundleBytes: input.bundleBytes,
        sourceProofBytes: input.sourceProofBytes,
        proofContext: proofContext,
        statementHash: proofContext.statementHash,
        destinationBindingHash: proofContext.destinationBindingHash,
        requestHash: substrateHashHex(prefix: "sccp:substrate:runtime-proof-request:v1", payload: preimage)
    )
}

public func wrapSubstrateSccpProofResult(proofBytes: Data,
                                         request: SubstrateSccpProofRequest) throws -> SubstrateSccpProofResult {
    guard !proofBytes.isEmpty else {
        throw SubstrateSccpProverError.emptyProof
    }
    guard proofBytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw SubstrateSccpProverError.invalidPublicInputs("proofBytes")
    }
    guard proofBytes.contains(where: { $0 != 0 }) else {
        throw SubstrateSccpProverError.allZeroProof
    }
    try requireProductionSubstrateSccpProofRequest(request)
    var envelopePayload = try substrateBytesFromHex32(request.requestHash, field: "requestHash")
    envelopePayload.append(proofBytes)
    return SubstrateSccpProofResult(
        version: 1,
        backend: request.backend,
        proofBytes: proofBytes,
        proofBase64: proofBytes.base64EncodedString(),
        publicInputs: request.publicInputs,
        bundleBytes: request.bundleBytes,
        sourceProofBytes: request.sourceProofBytes,
        proofContext: request.proofContext,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
        requestHash: request.requestHash,
        envelopeHash: substrateHashHex(prefix: "sccp:substrate:runtime-proof-envelope:v1", payload: envelopePayload)
    )
}

/// Build a SCALE runtime-call envelope for a completed Substrate SCCP proof.
public func buildSubstrateSccpSubmission(_ input: SubstrateSccpSubmissionInput) throws -> SubstrateSccpSubmission {
    guard input.sourceDomain == sccpDomainSora else {
        throw SubstrateSccpProverError.invalidPublicInputs("sourceDomain")
    }
    guard !input.proofBytes.isEmpty else {
        throw SubstrateSccpProverError.emptyProof
    }
    guard input.proofBytes.count <= sccpNativeRecursiveMaxProofBytes else {
        throw SubstrateSccpProverError.invalidPublicInputs("proofBytes")
    }
    guard input.proofBytes.contains(where: { $0 != 0 }) else {
        throw SubstrateSccpProverError.allZeroProof
    }
    let request = try buildSubstrateSccpProofRequest(SubstrateSccpProofRequestInput(
        publicInputs: input.publicInputs,
        bundleBytes: input.bundleBytes,
        sourceProofBytes: input.sourceProofBytes,
        statementHash: input.statementHash,
        destinationBindingHash: input.destinationBindingHash,
        backend: sccpSubstrateRuntimeProofBackendV1,
        sourceDomain: input.sourceDomain
    ))
    if let proofResult = input.proofResult {
        guard proofResult.backend == sccpSubstrateRuntimeProofBackendV1 else {
            throw SubstrateSccpProverError.invalidPublicInputs("proofResult.backend")
        }
        guard proofResult.publicInputs == input.publicInputs else {
            throw SubstrateSccpProverError.invalidPublicInputs("proofResult.publicInputs")
        }
        guard proofResult.bundleBytes == input.bundleBytes else {
            throw SubstrateSccpProverError.invalidPublicInputs("bundleBytes")
        }
        guard proofResult.sourceProofBytes == input.sourceProofBytes else {
            throw SubstrateSccpProverError.invalidPublicInputs("sourceProofBytes")
        }
        guard proofResult.proofBase64 == proofResult.proofBytes.base64EncodedString() else {
            throw SubstrateSccpProverError.invalidPublicInputs("proofResult.proofBase64")
        }
        let expectedResult = try wrapSubstrateSccpProofResult(
            proofBytes: proofResult.proofBytes,
            request: request
        )
        guard expectedResult == proofResult else {
            throw SubstrateSccpProverError.invalidPublicInputs("proofResult")
        }
        guard proofResult.proofBytes == input.proofBytes else {
            throw SubstrateSccpProverError.invalidPublicInputs("proofBytes")
        }
    }
    let argumentPairs: [(String, Data)] = [
        ("proof_bytes", input.proofBytes),
        ("public_inputs", request.publicInputsBytes),
        ("bundle_bytes", input.bundleBytes),
    ]
    var runtimeCall = try substrateScaleVec(
        Data(sccpSubstrateSubmitMessageProofEntrypointV1.utf8),
        field: "verifierEntrypoint"
    )
    for (index, pair) in argumentPairs.enumerated() {
        runtimeCall.append(try substrateScaleVec(pair.1, field: "arguments[\(index)]"))
    }
    let arguments = argumentPairs.map { key, bytes in
        SubstrateSccpSubmissionArgument(
            key: key,
            encoding: "raw_bytes",
            bytesHex: "0x" + bytes.hexEncodedString()
        )
    }
    return SubstrateSccpSubmission(
        version: 1,
        proofFamily: sccpStarkFriProofFamilyV1,
        verifierBackend: sccpSubstrateRuntimeProofBackendV1,
        platformPayload: "substrate_runtime_call",
        envelopeEncoding: sccpSubstrateRuntimeCallScaleV1,
        submissionKind: "runtime_call",
        verifierEntrypoint: sccpSubstrateSubmitMessageProofEntrypointV1,
        sourceDomain: input.sourceDomain,
        targetDomain: input.publicInputs.targetDomain,
        publicInputs: input.publicInputs,
        proofContext: request.proofContext,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
        requestHash: request.requestHash,
        proofBytes: input.proofBytes,
        publicInputsBytes: request.publicInputsBytes,
        bundleBytes: input.bundleBytes,
        arguments: arguments,
        runtimeCall: runtimeCall,
        runtimeCallHex: "0x" + runtimeCall.hexEncodedString(),
        envelopeBytes: runtimeCall,
        envelopeHex: "0x" + runtimeCall.hexEncodedString()
    )
}

private func normalizeSubstrateSccpProofContext(statementHash: String,
                                                destinationBindingHash: String) throws -> SubstrateSccpProofContext {
    SubstrateSccpProofContext(
        version: 1,
        statementHash: try substrateNormalizeHex32(statementHash, field: "statementHash"),
        destinationBindingHash: try substrateNormalizeHex32(destinationBindingHash, field: "destinationBindingHash")
    )
}

private func requireCanonicalSubstrateSccpProofRequest(_ request: SubstrateSccpProofRequest) throws {
    let expected = try buildSubstrateSccpProofRequest(SubstrateSccpProofRequestInput(
        publicInputs: request.publicInputs,
        bundleBytes: request.bundleBytes,
        sourceProofBytes: request.sourceProofBytes,
        statementHash: request.statementHash,
        destinationBindingHash: request.destinationBindingHash,
        backend: request.backend,
        sourceDomain: request.sourceDomain
    ))
    guard expected == request else {
        throw SubstrateSccpProverError.invalidPublicInputs("request")
    }
}

private func requireProductionSubstrateSccpProofRequest(_ request: SubstrateSccpProofRequest) throws {
    try requireCanonicalSubstrateSccpProofRequest(request)
    guard request.version == 1 else {
        throw SubstrateSccpProverError.invalidPublicInputs("request.version")
    }
    guard request.backend == sccpSubstrateRuntimeProofBackendV1 else {
        throw SubstrateSccpProverError.invalidPublicInputs("request.backend")
    }
    guard request.sourceDomain == sccpDomainSora else {
        throw SubstrateSccpProverError.invalidPublicInputs("request.sourceDomain")
    }
    guard request.targetDomain == request.publicInputs.targetDomain,
          substrateTargetDomainIsSupported(request.targetDomain) else {
        throw SubstrateSccpProverError.invalidPublicInputs("request.targetDomain")
    }
    guard !request.bundleBytes.isEmpty else {
        throw SubstrateSccpProverError.invalidPublicInputs("request.bundleBytes")
    }
    guard request.sourceProofBytes.isEmpty || request.sourceProofBytes.contains(where: { $0 != 0 }) else {
        throw SubstrateSccpProverError.invalidPublicInputs("request.sourceProofBytes")
    }
}

private func substrateTargetDomainIsSupported(_ value: UInt32) -> Bool {
    value == sccpDomainSoraKusama || value == sccpDomainSoraPolkadot || value == sccpDomainSora2
}

private func substrateBytesFromHex32(_ value: String, field: String) throws -> Data {
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
        throw SubstrateSccpProverError.invalidHex32(field)
    }
    var hex = value
    if hex.lowercased().hasPrefix("0x") {
        hex.removeFirst(2)
    }
    guard hex.unicodeScalars.allSatisfy({ !CharacterSet.whitespacesAndNewlines.contains($0) }) else {
        throw SubstrateSccpProverError.invalidHex32(field)
    }
    hex = hex.lowercased()
    guard hex.count == 64, let bytes = Data(hexString: hex), bytes.count == 32 else {
        throw SubstrateSccpProverError.invalidHex32(field)
    }
    return bytes
}

private func substrateNormalizeHex32(_ value: String, field: String) throws -> String {
    "0x" + (try substrateNonZeroBytesFromHex32(value, field: field)).hexEncodedString()
}

private func substrateNonZeroBytesFromHex32(_ value: String, field: String) throws -> Data {
    let bytes = try substrateBytesFromHex32(value, field: field)
    guard bytes.contains(where: { $0 != 0 }) else {
        throw SubstrateSccpProverError.zeroField(field)
    }
    return bytes
}

private func substrateAppendU32Le(_ value: UInt32, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 24) & 0xff))
}

private func substrateAppendU64Le(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 0, through: 56, by: 8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func substrateScaleCompactU32(_ value: UInt32) -> Data {
    if value < (UInt32(1) << 6) {
        return Data([UInt8(value << 2)])
    }
    if value < (UInt32(1) << 14) {
        let encoded = UInt16(value << 2) | 0b01
        return Data([
            UInt8(encoded & 0xff),
            UInt8((encoded >> 8) & 0xff),
        ])
    }
    if value < (UInt32(1) << 30) {
        let encoded = (value << 2) | 0b10
        return Data([
            UInt8(encoded & 0xff),
            UInt8((encoded >> 8) & 0xff),
            UInt8((encoded >> 16) & 0xff),
            UInt8((encoded >> 24) & 0xff),
        ])
    }
    var out = Data([0b11])
    substrateAppendU32Le(value, to: &out)
    return out
}

private func substrateScaleVec(_ value: Data, field: String) throws -> Data {
    guard value.count <= Int(UInt32.max) else {
        throw SubstrateSccpProverError.invalidPublicInputs(field)
    }
    var out = substrateScaleCompactU32(UInt32(value.count))
    out.append(value)
    return out
}

private func substrateHashHex(prefix: String, payload: Data) -> String {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return "0x" + Blake2b.hash256(preimage).hexEncodedString()
}

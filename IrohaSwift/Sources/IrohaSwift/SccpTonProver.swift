import Foundation

/// SCCP domain id for TON.
public let sccpDomainTon: UInt32 = 4

/// Proof backend id expected by the TON SCCP verifier contract.
public let sccpTonContractProofBackendV1 = "ton-contract-v1"

/// TON internal-message body envelope encoding for SCCP submissions.
public let sccpTonMessageBodyBocV1 = "ton_message_body_boc_v1"

/// SCCP public inputs shared by TON proof requests and message-body builders.
public struct TonSccpPublicInputsInput: Equatable {
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
                targetDomain: UInt32 = sccpDomainTon,
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

/// Inputs for a TON internal message body carrying an SCCP proof submission.
public struct TonSccpMessageBodyInput: Equatable {
    public let publicInputs: TonSccpPublicInputsInput
    public let proofBytes: Data
    public let bundleBytes: Data
    public let statementHash: String
    public let destinationBindingHash: String
    public let metadataBytes: Data
    public let queryId: UInt64?

    public init(publicInputs: TonSccpPublicInputsInput,
                proofBytes: Data,
                bundleBytes: Data,
                statementHash: String,
                destinationBindingHash: String,
                metadataBytes: Data = Data(),
                queryId: UInt64? = nil) {
        self.publicInputs = publicInputs
        self.proofBytes = proofBytes
        self.bundleBytes = bundleBytes
        self.statementHash = statementHash
        self.destinationBindingHash = destinationBindingHash
        self.metadataBytes = metadataBytes
        self.queryId = queryId
    }
}

/// Prebuilt TON SCCP submission envelope for wallet or liteserver broadcasting.
public struct TonSccpSubmission: Equatable {
    public let envelopeEncoding: String
    public let messageBodyBoc: Data
    public let messageBodyBocHex: String
}

/// Inputs used to build a local TON SCCP proof request.
public struct TonSccpProofRequestInput: Equatable {
    public let publicInputs: TonSccpPublicInputsInput
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let backend: String
    public let sourceDomain: UInt32

    public init(publicInputs: TonSccpPublicInputsInput,
                bundleBytes: Data,
                sourceProofBytes: Data = Data(),
                backend: String = sccpTonContractProofBackendV1,
                sourceDomain: UInt32 = sccpDomainTon) {
        self.publicInputs = publicInputs
        self.bundleBytes = bundleBytes
        self.sourceProofBytes = sourceProofBytes
        self.backend = backend
        self.sourceDomain = sourceDomain
    }
}

/// Request passed to a linked local TON SCCP prover.
public struct TonSccpProofRequest: Equatable {
    public let version: UInt8
    public let backend: String
    public let sourceDomain: UInt32
    public let targetDomain: UInt32
    public let publicInputs: TonSccpPublicInputsInput
    public let publicInputsBytes: Data
    public let bundleBytes: Data
    public let sourceProofBytes: Data
    public let requestHash: String
}

/// Proof bytes returned by a linked local TON SCCP prover.
public struct TonSccpProofResult: Equatable {
    public let version: UInt8
    public let backend: String
    public let proofBytes: Data
    public let proofBase64: String
    public let publicInputs: TonSccpPublicInputsInput
    public let requestHash: String
}

/// Error cases for TON SCCP local proof request construction.
public enum TonSccpProverError: Error, Equatable {
    case invalidHex32(String)
    case localProverUnavailable
    case emptyProof
}

/// Optional async witness resolver backed by app-controlled TON liteserver calls.
public protocol TonSccpWitnessProvider {
    func resolveWitness(_ input: TonSccpProofRequestInput) async throws -> TonSccpProofRequestInput
}

/// Local-first TON SCCP proof wrapper. It never fabricates proofs; callers must link a prover.
public final class TonSccpProver {
    public typealias ProveFunction = (TonSccpProofRequest) async throws -> Data

    private let witnessProvider: TonSccpWitnessProvider?
    private let proveFunction: ProveFunction?

    public init(witnessProvider: TonSccpWitnessProvider? = nil,
                proveFunction: ProveFunction? = nil) {
        self.witnessProvider = witnessProvider
        self.proveFunction = proveFunction
    }

    public func buildRequest(_ input: TonSccpProofRequestInput) async throws -> TonSccpProofRequest {
        let resolved = try await witnessProvider?.resolveWitness(input) ?? input
        return try buildTonSccpProofRequest(resolved)
    }

    public func prove(_ input: TonSccpProofRequestInput) async throws -> TonSccpProofResult {
        let request = try await buildRequest(input)
        guard let proveFunction else {
            throw TonSccpProverError.localProverUnavailable
        }
        let proofBytes = try await proveFunction(request)
        return try wrapTonSccpProofResult(proofBytes: proofBytes, request: request)
    }
}

/// Canonical SCCP public-input bytes used by TON proof requests and message bodies.
public func canonicalTonSccpPublicInputsBytes(_ input: TonSccpPublicInputsInput) throws -> Data {
    var out = Data()
    out.append(input.version)
    try out.append(tonBytesFromHex32(input.messageId, field: "messageId"))
    try out.append(tonBytesFromHex32(input.payloadHash, field: "payloadHash"))
    tonAppendU32Le(input.targetDomain, to: &out)
    try out.append(tonBytesFromHex32(input.commitmentRoot, field: "commitmentRoot"))
    tonAppendU64Le(input.finalityHeight, to: &out)
    try out.append(tonBytesFromHex32(input.finalityBlockHash, field: "finalityBlockHash"))
    return out
}

/// Deterministic TON query id derived from the SCCP message id.
public func tonSccpSubmissionQueryId(_ publicInputs: TonSccpPublicInputsInput) throws -> UInt64 {
    let messageId = try tonBytesFromHex32(publicInputs.messageId, field: "messageId")
    return messageId.prefix(8).reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
}

/// Build the TON BOC internal message body carrying an SCCP proof submission.
public func buildTonSccpMessageBodyBoc(_ input: TonSccpMessageBodyInput) throws -> Data {
    let publicInputsBytes = try canonicalTonSccpPublicInputsBytes(input.publicInputs)
    let statementHash = try tonBytesFromHex32(input.statementHash, field: "statementHash")
    let destinationBindingHash = try tonBytesFromHex32(input.destinationBindingHash, field: "destinationBindingHash")
    let queryId: UInt64
    if let suppliedQueryId = input.queryId {
        queryId = suppliedQueryId
    } else {
        queryId = try tonSccpSubmissionQueryId(input.publicInputs)
    }
    var rootData = Data()
    tonAppendU32Be(0x53434350, to: &rootData)
    tonAppendU64Be(queryId, to: &rootData)
    tonAppendU16Be(1, to: &rootData)
    rootData.append(statementHash)
    rootData.append(destinationBindingHash)

    var cells = [TonCell(data: rootData, refs: [])]
    let publicInputsRoot = tonPushSnakeCells(&cells, bytes: publicInputsBytes)
    let proofRoot = tonPushSnakeCells(&cells, bytes: input.proofBytes)
    let bundleRoot = tonPushSnakeCells(&cells, bytes: input.bundleBytes)
    let metadataRoot = tonPushSnakeCells(&cells, bytes: input.metadataBytes)
    cells[0].refs = [publicInputsRoot, proofRoot, bundleRoot, metadataRoot]
    return try tonEncodeBocSingleRoot(cells, rootIndex: 0)
}

/// Build a TON SCCP submission envelope for wallet or liteserver broadcasting.
public func buildTonSccpSubmission(_ input: TonSccpMessageBodyInput) throws -> TonSccpSubmission {
    let messageBodyBoc = try buildTonSccpMessageBodyBoc(input)
    return TonSccpSubmission(
        envelopeEncoding: sccpTonMessageBodyBocV1,
        messageBodyBoc: messageBodyBoc,
        messageBodyBocHex: "0x" + messageBodyBoc.hexEncodedString()
    )
}

/// Build a TON SCCP proof request for a linked local prover.
public func buildTonSccpProofRequest(_ input: TonSccpProofRequestInput) throws -> TonSccpProofRequest {
    let publicInputsBytes = try canonicalTonSccpPublicInputsBytes(input.publicInputs)
    var preimage = Data()
    preimage.append(publicInputsBytes)
    preimage.append(input.bundleBytes)
    preimage.append(input.sourceProofBytes)
    return TonSccpProofRequest(
        version: 1,
        backend: input.backend,
        sourceDomain: input.sourceDomain,
        targetDomain: input.publicInputs.targetDomain,
        publicInputs: input.publicInputs,
        publicInputsBytes: publicInputsBytes,
        bundleBytes: input.bundleBytes,
        sourceProofBytes: input.sourceProofBytes,
        requestHash: tonHashHex(prefix: "sccp:ton:proof-request:v1", payload: preimage)
    )
}

private func wrapTonSccpProofResult(proofBytes: Data,
                                    request: TonSccpProofRequest) throws -> TonSccpProofResult {
    guard !proofBytes.isEmpty else {
        throw TonSccpProverError.emptyProof
    }
    return TonSccpProofResult(
        version: 1,
        backend: request.backend,
        proofBytes: proofBytes,
        proofBase64: proofBytes.base64EncodedString(),
        publicInputs: request.publicInputs,
        requestHash: request.requestHash
    )
}

private struct TonCell {
    var data: Data
    var refs: [Int]
}

private let tonBocMagic = Data([0xb5, 0xee, 0x9c, 0x72])
private let tonMaxCellDataBytes = 127
private let tonMaxRefs = 4

private func tonPushSnakeCells(_ cells: inout [TonCell], bytes: Data) -> Int {
    let start = cells.count
    guard !bytes.isEmpty else {
        cells.append(TonCell(data: Data(), refs: []))
        return start
    }
    let chunkCount = (bytes.count + tonMaxCellDataBytes - 1) / tonMaxCellDataBytes
    for index in 0..<chunkCount {
        let chunkStart = index * tonMaxCellDataBytes
        let chunkEnd = min(chunkStart + tonMaxCellDataBytes, bytes.count)
        let refs = index + 1 == chunkCount ? [] : [start + index + 1]
        cells.append(TonCell(data: bytes[chunkStart..<chunkEnd], refs: refs))
    }
    return start
}

private func tonEncodeBocSingleRoot(_ cells: [TonCell], rootIndex: Int) throws -> Data {
    guard !cells.isEmpty, rootIndex >= 0, rootIndex < cells.count else {
        throw TonSccpProverError.emptyProof
    }
    let sizeBytes = tonMinSizeBytes(max(cells.count, rootIndex))
    let cellsBytes = try tonSerializeCells(cells, sizeBytes: sizeBytes)
    let offsetBytes = tonMinSizeBytes(cellsBytes.count)
    var out = Data()
    out.append(tonBocMagic)
    out.append(UInt8(sizeBytes))
    out.append(UInt8(offsetBytes))
    out.append(tonSizedUInt(cells.count, size: sizeBytes))
    out.append(tonSizedUInt(1, size: sizeBytes))
    out.append(tonSizedUInt(0, size: sizeBytes))
    out.append(tonSizedUInt(cellsBytes.count, size: offsetBytes))
    out.append(tonSizedUInt(rootIndex, size: sizeBytes))
    out.append(cellsBytes)
    return out
}

private func tonSerializeCells(_ cells: [TonCell], sizeBytes: Int) throws -> Data {
    var out = Data()
    for cell in cells {
        guard cell.data.count <= tonMaxCellDataBytes, cell.refs.count <= tonMaxRefs else {
            throw TonSccpProverError.emptyProof
        }
        out.append(UInt8(cell.refs.count))
        out.append(UInt8(cell.data.count * 2))
        out.append(cell.data)
        for ref in cell.refs {
            guard ref >= 0, ref < cells.count else {
                throw TonSccpProverError.emptyProof
            }
            out.append(tonSizedUInt(ref, size: sizeBytes))
        }
    }
    return out
}

private func tonMinSizeBytes(_ value: Int) -> Int {
    for size in 1...7 where UInt64(value) <= ((UInt64(1) << UInt64(size * 8)) - 1) {
        return size
    }
    return 7
}

private func tonSizedUInt(_ value: Int, size: Int) -> Data {
    var working = UInt64(value)
    var out = Data(repeating: 0, count: size)
    for index in stride(from: size - 1, through: 0, by: -1) {
        out[index] = UInt8(working & 0xff)
        working >>= 8
    }
    return out
}

private func tonBytesFromHex32(_ value: String, field: String) throws -> Data {
    var hex = value.trimmingCharacters(in: .whitespacesAndNewlines)
    if hex.lowercased().hasPrefix("0x") {
        hex.removeFirst(2)
    }
    hex = hex.lowercased()
    guard hex.count == 64, let bytes = Data(hexString: hex), bytes.count == 32 else {
        throw TonSccpProverError.invalidHex32(field)
    }
    return bytes
}

private func tonAppendU16Be(_ value: UInt16, to out: inout Data) {
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8(value & 0xff))
}

private func tonAppendU32Le(_ value: UInt32, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 24) & 0xff))
}

private func tonAppendU32Be(_ value: UInt32, to out: inout Data) {
    out.append(UInt8((value >> 24) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8(value & 0xff))
}

private func tonAppendU64Le(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 0, through: 56, by: 8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func tonAppendU64Be(_ value: UInt64, to out: inout Data) {
    for shift in stride(from: 56, through: 0, by: -8) {
        out.append(UInt8((value >> UInt64(shift)) & 0xff))
    }
}

private func tonHashHex(prefix: String, payload: Data) -> String {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return "0x" + Blake2b.hash256(preimage).hexEncodedString()
}

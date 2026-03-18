import Foundation

public enum ProofAttachmentError: Error, LocalizedError, Sendable {
    case emptyBackend
    case emptyProof
    case missingVerifyingKey
    case emptyVerifyingKeyBackend
    case emptyVerifyingKeyName
    case emptyVerifyingKeyBytes
    case invalidVerifyingKeySeparator
    case invalidVerifyingKeyCommitmentLength(expected: Int, actual: Int)
    case invalidEnvelopeHashLength(expected: Int, actual: Int)

    public var errorDescription: String? {
        switch self {
        case .emptyBackend:
            return "Proof backend identifier must not be empty."
        case .emptyProof:
            return "Proof bytes must not be empty."
        case .missingVerifyingKey:
            return "Proof attachment must include either verifyingKeyReference or verifyingKeyInline."
        case .emptyVerifyingKeyBackend:
            return "Verifying key backend must not be empty."
        case .emptyVerifyingKeyName:
            return "Verifying key name must not be empty."
        case .emptyVerifyingKeyBytes:
            return "Verifying key bytes must not be empty."
        case .invalidVerifyingKeySeparator:
            return "Verifying key backend and name must not contain ':' characters."
        case let .invalidVerifyingKeyCommitmentLength(expected, actual):
            return "Verifying key commitment must be \(expected) bytes (found \(actual))."
        case let .invalidEnvelopeHashLength(expected, actual):
            return "Envelope hash must be \(expected) bytes (found \(actual))."
        }
    }
}

public struct ProofAttachment: Sendable, Equatable {
    public struct VerifyingKeyReference: Sendable, Equatable {
        public let backend: String
        public let name: String

        public init(backend: String, name: String) {
            self.backend = backend
            self.name = name
        }
    }

    public struct VerifyingKeyInline: Sendable, Equatable {
        public let backend: String
        public let bytes: Data

        public init(backend: String, bytes: Data) {
            self.backend = backend
            self.bytes = bytes
        }
    }

    public enum VerifyingKey: Sendable, Equatable {
        case reference(VerifyingKeyReference)
        case inline(VerifyingKeyInline)
    }

    public let backend: String
    public let proof: Data
    public let verifyingKey: VerifyingKey
    public var verifyingKeyCommitment: Data?
    public var envelopeHash: Data?

    public init(backend: String,
                proof: Data,
                verifyingKey: VerifyingKey,
                verifyingKeyCommitment: Data? = nil,
                envelopeHash: Data? = nil) throws {
        let normalizedBackend = try Self.normalizeNonEmpty(backend, error: .emptyBackend)
        guard !proof.isEmpty else {
            throw ProofAttachmentError.emptyProof
        }
        let normalizedKey: VerifyingKey
        switch verifyingKey {
        case .reference(let ref):
            let normalizedRefBackend = try Self.normalizeNonEmpty(ref.backend, error: .emptyVerifyingKeyBackend)
            let normalizedName = try Self.normalizeNonEmpty(ref.name, error: .emptyVerifyingKeyName)
            try Self.ensureNoSeparator(normalizedRefBackend)
            try Self.ensureNoSeparator(normalizedName)
            normalizedKey = .reference(.init(backend: normalizedRefBackend, name: normalizedName))
        case .inline(let inline):
            let normalizedInlineBackend = try Self.normalizeNonEmpty(inline.backend, error: .emptyVerifyingKeyBackend)
            guard !inline.bytes.isEmpty else {
                throw ProofAttachmentError.emptyVerifyingKeyBytes
            }
            try Self.ensureNoSeparator(normalizedInlineBackend)
            normalizedKey = .inline(.init(backend: normalizedInlineBackend, bytes: inline.bytes))
        }
        if let commitment = verifyingKeyCommitment {
            try Self.ensureFixedLength(commitment,
                                       expectedLength: Self.hashLength,
                                       makeError: ProofAttachmentError.invalidVerifyingKeyCommitmentLength)
        }
        if let envelope = envelopeHash {
            try Self.ensureFixedLength(envelope,
                                       expectedLength: Self.hashLength,
                                       makeError: ProofAttachmentError.invalidEnvelopeHashLength)
        }
        self.backend = normalizedBackend
        self.proof = proof
        self.verifyingKey = normalizedKey
        self.verifyingKeyCommitment = verifyingKeyCommitment
        self.envelopeHash = envelopeHash
    }

    private static func normalizeNonEmpty(_ value: String,
                                          error: ProofAttachmentError) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw error
        }
        return trimmed
    }

    private static func ensureNoSeparator(_ value: String) throws {
        if value.contains(":") {
            throw ProofAttachmentError.invalidVerifyingKeySeparator
        }
    }

    private static func ensureFixedLength(
        _ bytes: Data,
        expectedLength: Int,
        makeError: (Int, Int) -> ProofAttachmentError
    ) throws {
        let actual = bytes.count
        guard actual == expectedLength else {
            throw makeError(expectedLength, actual)
        }
    }

    func encodedJSON() throws -> Data {
        var payload: [String: Any] = [
            "backend": backend,
            "proof_b64": proof.base64EncodedString()
        ]
        switch verifyingKey {
        case let .reference(ref):
            payload["vk_ref"] = [
                "backend": ref.backend,
                "name": ref.name
            ]
        case let .inline(inline):
            payload["vk_inline"] = [
                "backend": inline.backend,
                "bytes_b64": inline.bytes.base64EncodedString()
            ]
        }
        if let commitment = verifyingKeyCommitment {
            _ = try Self.fixedBytesPayload(commitment,
                                           expectedLength: Self.hashLength,
                                           makeError: ProofAttachmentError.invalidVerifyingKeyCommitmentLength)
            payload["vk_commitment_hex"] = commitment.hexEncodedString()
        }
        if let envelope = envelopeHash {
            _ = try Self.fixedBytesPayload(envelope,
                                           expectedLength: Self.hashLength,
                                           makeError: ProofAttachmentError.invalidEnvelopeHashLength)
            payload["envelope_hash_hex"] = envelope.hexEncodedString()
        }
        guard payload["vk_ref"] != nil || payload["vk_inline"] != nil else {
            throw ProofAttachmentError.missingVerifyingKey
        }
        return try JSONSerialization.data(withJSONObject: payload, options: [])
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeString(backend))
        writer.writeField(Self.proofBoxPayload(backend: backend, bytes: proof))

        let vkRef: VerifyingKeyReference?
        let vkInline: VerifyingKeyInline?
        switch verifyingKey {
        case let .reference(ref):
            vkRef = ref
            vkInline = nil
        case let .inline(inline):
            vkInline = inline
            vkRef = nil
        }

        writer.writeField(try OfflineNorito.encodeOption(vkRef, encode: Self.verifyingKeyIdPayload))
        writer.writeField(try OfflineNorito.encodeOption(vkInline, encode: Self.verifyingKeyBoxPayload))

        let tail = envelopeHash != nil ? 2 : (verifyingKeyCommitment != nil ? 1 : 0)
        if tail >= 1 {
            writer.writeField(try OfflineNorito.encodeOption(
                verifyingKeyCommitment,
                encode: { try Self.fixedBytesPayload($0,
                                                     expectedLength: Self.hashLength,
                                                     makeError: ProofAttachmentError.invalidVerifyingKeyCommitmentLength) }
            ))
        }
        if tail >= 2 {
            writer.writeField(try OfflineNorito.encodeOption(
                envelopeHash,
                encode: { try Self.fixedBytesPayload($0,
                                                     expectedLength: Self.hashLength,
                                                     makeError: ProofAttachmentError.invalidEnvelopeHashLength) }
            ))
        }
        return writer.data
    }

    private static let hashLength = 32

    private static func proofBoxPayload(backend: String, bytes: Data) -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeString(backend))
        writer.writeField(OfflineNorito.encodeBytesVec(bytes))
        return writer.data
    }

    private static func verifyingKeyIdPayload(_ ref: VerifyingKeyReference) -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeString(ref.backend))
        writer.writeField(OfflineNorito.encodeString(ref.name))
        return writer.data
    }

    private static func verifyingKeyBoxPayload(_ inline: VerifyingKeyInline) -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeString(inline.backend))
        writer.writeField(OfflineNorito.encodeBytesVec(inline.bytes))
        return writer.data
    }

    private static func fixedBytesPayload(
        _ bytes: Data,
        expectedLength: Int,
        makeError: (Int, Int) -> ProofAttachmentError
    ) throws -> Data {
        let actual = bytes.count
        guard actual == expectedLength else {
            throw makeError(expectedLength, actual)
        }
        var writer = OfflineNoritoWriter()
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }
}

private extension Data {
    func hexEncodedString() -> String {
        map { String(format: "%02x", $0) }.joined()
    }
}

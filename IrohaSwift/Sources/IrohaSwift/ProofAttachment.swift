import Foundation

public enum ProofAttachmentError: Error, LocalizedError, Sendable {
    case emptyBackend
    case emptyProof
    case missingVerifyingKey
    case emptyVerifyingKeyBackend
    case emptyVerifyingKeyName
    case invalidVerifyingKeyIdentifier(field: String)
    case verifyingKeyIdentifierTooLong(field: String, maximum: Int, actual: Int)
    case verifyingKeyBackendMismatch(expected: String, actual: String)
    case proofBoxTooLarge(maximum: Int, actual: Int)
    case invalidVerifyingKeyCommitmentLength(expected: Int, actual: Int)
    case zeroVerifyingKeyCommitment
    case invalidEnvelopeHashLength(expected: Int, actual: Int)
    case envelopeHashMismatch
    case invalidLanePrivacyHashLength(expected: Int, actual: Int)
    case invalidLanePrivacyPathLength(actual: Int)
    case invalidLanePrivacyLeafIndex(index: UInt32, pathLength: Int)

    public var errorDescription: String? {
        switch self {
        case .emptyBackend:
            return "Proof backend identifier must not be empty."
        case .emptyProof:
            return "Proof bytes must not be empty."
        case .missingVerifyingKey:
            return "Proof attachment must include verifyingKeyReference."
        case .emptyVerifyingKeyBackend:
            return "Verifying key backend must not be empty."
        case .emptyVerifyingKeyName:
            return "Verifying key name must not be empty."
        case let .invalidVerifyingKeyIdentifier(field):
            return "\(field) must use the canonical portable verifier-key identifier grammar."
        case let .verifyingKeyIdentifierTooLong(field, maximum, actual):
            return "\(field) must not exceed \(maximum) UTF-8 bytes (found \(actual))."
        case let .verifyingKeyBackendMismatch(expected, actual):
            return "Verifying key backend must match proof backend \(expected) (found \(actual))."
        case let .proofBoxTooLarge(maximum, actual):
            return "Encoded ProofBox must not exceed \(maximum) bytes (found \(actual))."
        case let .invalidVerifyingKeyCommitmentLength(expected, actual):
            return "Verifying key commitment must be \(expected) bytes (found \(actual))."
        case .zeroVerifyingKeyCommitment:
            return "Verifying key commitment must not be all zero."
        case let .invalidEnvelopeHashLength(expected, actual):
            return "Envelope hash must be \(expected) bytes (found \(actual))."
        case .envelopeHashMismatch:
            return "Envelope hash must match the proof bytes."
        case let .invalidLanePrivacyHashLength(expected, actual):
            return "Lane privacy hash must be \(expected) bytes (found \(actual))."
        case let .invalidLanePrivacyPathLength(actual):
            return "Lane privacy Merkle path must contain between 1 and 255 siblings (found \(actual))."
        case let .invalidLanePrivacyLeafIndex(index, pathLength):
            return "Lane privacy leaf index \(index) cannot fit a Merkle path of depth \(pathLength)."
        }
    }
}

public struct ProofAttachment: Sendable, Equatable {
    public struct LanePrivacyProof: Sendable, Equatable {
        public struct MerkleWitness: Sendable, Equatable {
            public let leaf: Data
            public let leafIndex: UInt32
            public let auditPath: [Data]

            public init(leaf: Data, leafIndex: UInt32, auditPath: [Data]) throws {
                try ProofAttachment.ensureFixedLength(
                    leaf,
                    expectedLength: ProofAttachment.hashLength,
                    makeError: ProofAttachmentError.invalidLanePrivacyHashLength
                )
                guard !auditPath.isEmpty, auditPath.count <= Int(UInt8.max) else {
                    throw ProofAttachmentError.invalidLanePrivacyPathLength(actual: auditPath.count)
                }
                if auditPath.count < UInt32.bitWidth,
                   UInt64(leafIndex) >= (UInt64(1) << UInt64(auditPath.count)) {
                    throw ProofAttachmentError.invalidLanePrivacyLeafIndex(
                        index: leafIndex,
                        pathLength: auditPath.count
                    )
                }
                let canonicalPath = try auditPath.map { sibling in
                    try ProofAttachment.ensureFixedLength(
                        sibling,
                        expectedLength: ProofAttachment.hashLength,
                        makeError: ProofAttachmentError.invalidLanePrivacyHashLength
                    )
                    var canonical = sibling
                    let markerIndex = canonical.index(before: canonical.endIndex)
                    canonical[markerIndex] |= 1
                    return canonical
                }
                self.leaf = leaf
                self.leafIndex = leafIndex
                self.auditPath = canonicalPath
            }
        }

        public enum Witness: Sendable, Equatable {
            case merkle(MerkleWitness)
        }

        public let commitmentId: UInt16
        public let witness: Witness

        public init(commitmentId: UInt16, witness: Witness) {
            self.commitmentId = commitmentId
            self.witness = witness
        }
    }

    public struct VerifyingKeyReference: Sendable, Equatable {
        public let backend: String
        public let name: String

        public init(backend: String, name: String) {
            self.backend = backend
            self.name = name
        }
    }

    public enum VerifyingKey: Sendable, Equatable {
        case reference(VerifyingKeyReference)
    }

    public let backend: String
    public let proof: Data
    public let verifyingKey: VerifyingKey
    public let verifyingKeyCommitment: Data?
    public let envelopeHash: Data?
    public let lanePrivacy: LanePrivacyProof?

    public init(backend: String,
                proof: Data,
                verifyingKey: VerifyingKey,
                verifyingKeyCommitment: Data? = nil,
                envelopeHash: Data? = nil,
                lanePrivacy: LanePrivacyProof? = nil) throws {
        let normalizedBackend = try Self.requirePortableIdentifier(
            backend,
            field: "backend",
            emptyError: .emptyBackend
        )
        guard !proof.isEmpty else {
            throw ProofAttachmentError.emptyProof
        }
        let maximumProofBytes = try Self.maximumProofByteCount(
            backendUTF8Count: normalizedBackend.utf8.count
        )
        guard proof.count <= maximumProofBytes else {
            let encodedProofBoxLength = try Self.canonicalProofBoxEncodedLength(
                backendUTF8Count: normalizedBackend.utf8.count,
                proofByteCount: proof.count
            )
            throw ProofAttachmentError.proofBoxTooLarge(
                maximum: Self.maximumEncodedProofBoxBytesV1,
                actual: encodedProofBoxLength
            )
        }
        let normalizedKey: VerifyingKey
        switch verifyingKey {
        case .reference(let ref):
            let normalizedRefBackend = try Self.requirePortableIdentifier(
                ref.backend,
                field: "verifyingKey.backend",
                emptyError: .emptyVerifyingKeyBackend
            )
            let normalizedName = try Self.requirePortableIdentifier(
                ref.name,
                field: "verifyingKey.name",
                emptyError: .emptyVerifyingKeyName
            )
            guard normalizedRefBackend == normalizedBackend else {
                throw ProofAttachmentError.verifyingKeyBackendMismatch(
                    expected: normalizedBackend,
                    actual: normalizedRefBackend
                )
            }
            normalizedKey = .reference(.init(backend: normalizedRefBackend, name: normalizedName))
        }
        if let commitment = verifyingKeyCommitment {
            try Self.ensureFixedLength(commitment,
                                       expectedLength: Self.hashLength,
                                       makeError: ProofAttachmentError.invalidVerifyingKeyCommitmentLength)
            guard commitment.contains(where: { $0 != 0 }) else {
                throw ProofAttachmentError.zeroVerifyingKeyCommitment
            }
        }
        if let envelope = envelopeHash {
            try Self.ensureFixedLength(envelope,
                                       expectedLength: Self.hashLength,
                                       makeError: ProofAttachmentError.invalidEnvelopeHashLength)
            guard envelope == Self.canonicalEnvelopeHash(for: proof) else {
                throw ProofAttachmentError.envelopeHashMismatch
            }
        }
        self.backend = normalizedBackend
        self.proof = proof
        self.verifyingKey = normalizedKey
        self.verifyingKeyCommitment = verifyingKeyCommitment
        self.envelopeHash = envelopeHash
        self.lanePrivacy = lanePrivacy
    }

    private static func requirePortableIdentifier(
        _ value: String,
        field: String,
        emptyError: ProofAttachmentError
    ) throws -> String {
        guard !value.isEmpty else {
            throw emptyError
        }
        guard !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw emptyError
        }
        let bytes = Array(value.utf8)
        guard bytes.count <= maximumIdentifierBytes else {
            throw ProofAttachmentError.verifyingKeyIdentifierTooLong(
                field: field,
                maximum: maximumIdentifierBytes,
                actual: bytes.count
            )
        }
        guard let first = bytes.first,
              let last = bytes.last,
              isPortableEndpoint(first),
              isPortableEndpoint(last),
              bytes.allSatisfy(isPortableIdentifierByte),
              !forbiddenIdentifierSeparators.contains(where: { value.range(of: $0) != nil }) else {
            throw ProofAttachmentError.invalidVerifyingKeyIdentifier(field: field)
        }
        return value
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
        }
        if let commitment = verifyingKeyCommitment {
            _ = try Self.fixedBytesPayload(commitment,
                                           expectedLength: Self.hashLength,
                                           makeError: ProofAttachmentError.invalidVerifyingKeyCommitmentLength)
            payload["vk_commitment_hex"] = commitment.hexEncodedString()
        }
        let envelope = envelopeHash ?? Self.canonicalEnvelopeHash(for: proof)
        _ = try Self.fixedBytesPayload(envelope,
                                       expectedLength: Self.hashLength,
                                       makeError: ProofAttachmentError.invalidEnvelopeHashLength)
        payload["envelope_hash_hex"] = envelope.hexEncodedString()
        if let lanePrivacy {
            payload["lane_privacy"] = Self.lanePrivacyJSON(lanePrivacy)
        }
        guard payload["vk_ref"] != nil else {
            throw ProofAttachmentError.missingVerifyingKey
        }
        return try JSONSerialization.data(withJSONObject: payload, options: [])
    }

    func noritoPayload() throws -> Data {
        var writer = CanonicalNoritoWriter()
        writer.writeField(CanonicalNorito.encodeString(backend))
        writer.writeField(Self.proofBoxPayload(backend: backend, bytes: proof))

        let vkRef: VerifyingKeyReference
        switch verifyingKey {
        case let .reference(ref):
            vkRef = ref
        }

        writer.writeField(Self.verifyingKeyIdPayload(vkRef))

        let tail = lanePrivacy != nil ? 3 : (envelopeHash != nil ? 2 : (verifyingKeyCommitment != nil ? 1 : 0))
        if tail >= 1 {
            writer.writeField(try CanonicalNorito.encodeOption(
                verifyingKeyCommitment,
                encode: { try Self.fixedBytesPayload($0,
                                                     expectedLength: Self.hashLength,
                                                     makeError: ProofAttachmentError.invalidVerifyingKeyCommitmentLength) }
            ))
        }
        if tail >= 2 {
            writer.writeField(try CanonicalNorito.encodeOption(
                envelopeHash,
                encode: { try Self.fixedBytesPayload($0,
                                                     expectedLength: Self.hashLength,
                                                     makeError: ProofAttachmentError.invalidEnvelopeHashLength) }
            ))
        }
        if tail >= 3 {
            writer.writeField(try CanonicalNorito.encodeOption(
                lanePrivacy,
                encode: Self.lanePrivacyPayload
            ))
        }
        return writer.data
    }

    private static let hashLength = 32
    private static let maximumIdentifierBytes = 256
    /// Maximum canonical bytes admitted for the complete nested V1 ProofBox.
    public static let maximumEncodedProofBoxBytesV1 = 64 * 1024 * 1024
    private static let forbiddenIdentifierSeparators = [
        "..", "//", ":::", "/:", ":/", "/.", "./", ":.", ".:"
    ]

    private static func isPortableEndpoint(_ byte: UInt8) -> Bool {
        (0x61...0x7A).contains(byte) || (0x30...0x39).contains(byte)
    }

    private static func isPortableIdentifierByte(_ byte: UInt8) -> Bool {
        isPortableEndpoint(byte) || [0x2D, 0x5F, 0x2F, 0x3A, 0x2E].contains(byte)
    }

    static func canonicalProofBoxEncodedLength(
        backendUTF8Count: Int,
        proofByteCount: Int
    ) throws -> Int {
        guard backendUTF8Count >= 0, proofByteCount >= 0 else {
            throw ProofAttachmentError.proofBoxTooLarge(
                maximum: maximumEncodedProofBoxBytesV1,
                actual: Int.max
            )
        }
        // `Ident` encodes as compact UTF-8 length plus bytes. Both ProofBox
        // members then receive their own compact field frame. `Vec<u8>` keeps
        // its fixed-width V1 sequence count inside the proof member.
        let backendValueLength = try checkedProofBoxAdd(
            compactLengthByteCount(backendUTF8Count),
            backendUTF8Count
        )
        let backendFieldLength = try checkedProofBoxAdd(
            compactLengthByteCount(backendValueLength),
            backendValueLength
        )
        let proofValueLength = try checkedProofBoxAdd(8, proofByteCount)
        let proofFieldLength = try checkedProofBoxAdd(
            compactLengthByteCount(proofValueLength),
            proofValueLength
        )
        return try checkedProofBoxAdd(backendFieldLength, proofFieldLength)
    }

    /// Largest proof vector that keeps the complete canonical nested
    /// `ProofBox` payload within the fixed 64 MiB first-release ceiling.
    ///
    /// Compact prefix widths make the size monotone but piecewise-linear, so
    /// this uses a bounded binary search and never allocates proof storage.
    public static func maximumProofByteCountV1(forBackend backend: String) throws -> Int {
        let canonicalBackend = try requirePortableIdentifier(
            backend,
            field: "backend",
            emptyError: .emptyBackend
        )
        return try maximumProofByteCount(backendUTF8Count: canonicalBackend.utf8.count)
    }

    static func maximumProofByteCount(backendUTF8Count: Int) throws -> Int {
        guard backendUTF8Count >= 0 else {
            throw ProofAttachmentError.proofBoxTooLarge(
                maximum: maximumEncodedProofBoxBytesV1,
                actual: Int.max
            )
        }
        guard try canonicalProofBoxEncodedLength(
            backendUTF8Count: backendUTF8Count,
            proofByteCount: 0
        ) <= maximumEncodedProofBoxBytesV1 else {
            throw ProofAttachmentError.proofBoxTooLarge(
                maximum: maximumEncodedProofBoxBytesV1,
                actual: Int.max
            )
        }
        var lower = 0
        var upper = maximumEncodedProofBoxBytesV1
        while lower < upper {
            let distance = upper - lower
            let candidate = lower + distance / 2 + distance % 2
            if try canonicalProofBoxEncodedLength(
                backendUTF8Count: backendUTF8Count,
                proofByteCount: candidate
            ) <= maximumEncodedProofBoxBytesV1 {
                lower = candidate
            } else {
                upper = candidate - 1
            }
        }
        return lower
    }

    private static func compactLengthByteCount(_ value: Int) -> Int {
        precondition(value >= 0)
        var remaining = UInt(value)
        var byteCount = 1
        while remaining >= 0x80 {
            remaining >>= 7
            byteCount += 1
        }
        return byteCount
    }

    private static func checkedProofBoxAdd(_ left: Int, _ right: Int) throws -> Int {
        let (sum, overflow) = left.addingReportingOverflow(right)
        guard !overflow else {
            throw ProofAttachmentError.proofBoxTooLarge(
                maximum: maximumEncodedProofBoxBytesV1,
                actual: Int.max
            )
        }
        return sum
    }

    private static func canonicalEnvelopeHash(for proof: Data) -> Data {
        IrohaHash.hash(proof)
    }

    private static func proofBoxPayload(backend: String, bytes: Data) -> Data {
        var writer = CanonicalNoritoWriter()
        writer.writeField(CanonicalNorito.encodeString(backend))
        writer.writeField(CanonicalNorito.encodeBytesVec(bytes))
        return writer.data
    }

    private static func lanePrivacyPayload(_ proof: LanePrivacyProof) throws -> Data {
        var writer = CanonicalNoritoWriter()
        writer.writeField(CanonicalNorito.encodeUInt16(proof.commitmentId))
        writer.writeField(try lanePrivacyWitnessPayload(proof.witness))
        return writer.data
    }

    private static func lanePrivacyJSON(_ proof: LanePrivacyProof) -> [String: Any] {
        let witness: [String: Any]
        switch proof.witness {
        case .merkle(let merkle):
            witness = [
                "kind": "merkle",
                "payload": [
                    "leaf": merkle.leaf.map(Int.init),
                    "proof": [
                        "leaf_index": merkle.leafIndex,
                        "audit_path": merkle.auditPath.map { $0.map(Int.init) }
                    ]
                ]
            ]
        }
        return [
            "commitment_id": proof.commitmentId,
            "witness": witness
        ]
    }

    private static func lanePrivacyWitnessPayload(_ witness: LanePrivacyProof.Witness) throws -> Data {
        var writer = CanonicalNoritoWriter()
        switch witness {
        case .merkle(let witness):
            writer.writeUInt32LE(0)
            writer.writeField(try lanePrivacyMerkleWitnessPayload(witness))
        }
        return writer.data
    }

    private static func lanePrivacyMerkleWitnessPayload(
        _ witness: LanePrivacyProof.MerkleWitness
    ) throws -> Data {
        var writer = CanonicalNoritoWriter()
        writer.writeField(try fixedBytesPayload(
            witness.leaf,
            expectedLength: hashLength,
            makeError: ProofAttachmentError.invalidLanePrivacyHashLength
        ))

        var merkleProof = CanonicalNoritoWriter()
        merkleProof.writeField(CanonicalNorito.encodeUInt32(witness.leafIndex))
        merkleProof.writeField(try CanonicalNorito.encodeVec(witness.auditPath) { sibling in
            try CanonicalNorito.encodeOption(sibling) { hash in
                try ensureFixedLength(
                    hash,
                    expectedLength: hashLength,
                    makeError: ProofAttachmentError.invalidLanePrivacyHashLength
                )
                return hash
            }
        })
        writer.writeField(merkleProof.data)
        return writer.data
    }

    private static func verifyingKeyIdPayload(_ ref: VerifyingKeyReference) -> Data {
        var writer = CanonicalNoritoWriter()
        writer.writeField(CanonicalNorito.encodeString(ref.backend))
        writer.writeField(CanonicalNorito.encodeString(ref.name))
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
        var writer = CanonicalNoritoWriter()
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }
}

import Foundation
import CryptoKit

/// Refusal to construct or sign the sole direct X509 action shape.
public enum PrivacyX509TransactionErrorV1: Error, Equatable, Sendable {
    case invalidCanonicalInput(String)
    case unsupportedDirectAction
    case authorityMismatch
    case proofBounds
}

/// A prepared direct X509 transaction. All independent inputs are retained unchanged.
///
/// Give `materializedStatementNorito` to the holder prover, then attach its exact raw
/// proof with `sign`. This API admits one direct action with one Proof/ActionProof
/// occurrence. It does not construct contract or mixed-action authority bindings.
public struct PrivacyX509DirectTransactionV1: Sendable {
    public let materializedStatementNorito: Data
    public let orderedIntentDigest: Data
    public let actionStatementNorito: Data
    public let actionStatementDigest: Data
    private let plan: PrivacyX509WireV1.DirectPlan
    private let networkId: NetworkId
    private let authority: String
    private let requestArchive: Data
    private let admission: PrivacyExact12CapabilityTupleAdmissionV1

    /// Freeze one ordinary instruction transaction before proof generation.
    /// `requestNorito` is the canonical proof-free request with ZERO derived intent.
    public static func prepare(
        requestNorito: Data,
        admission: PrivacyExact12CapabilityTupleAdmissionV1,
        networkId: NetworkId,
        authority: String,
        creationTimeMs: UInt64,
        feePayment: FeePaymentIntent,
        ttlMs: UInt64 = 100_000,
        nonce: UInt32? = nil,
        metadata: [String: ToriiJSONValue] = [:]
    ) throws -> Self {
        let limits = try admission.requireX509ConstructionV1(
            requestNorito, expectedNetworkId: networkId
        )
        let frame = try TransactionInstructionFrame.privacyExact12SubmitProof(
            protocolId: .irohaZkX509StarkP256V1, framedPayload: requestNorito,
            admission: admission
        )
        let payload = try SingleInstructionSwiftNoritoEncoder.instructionsPayload(
            networkId: networkId, authority: authority, creationTimeMs: creationTimeMs,
            ttlMs: ttlMs, frames: [frame], feePayment: feePayment, nonce: nonce,
            metadata: metadata
        )
        let plan = try PrivacyX509WireV1.prepareDirect(requestArchive: requestNorito,
                                                     unsignedPayload: payload)
        try plan.requireStatementBounds(limits)
        return Self(materializedStatementNorito: plan.materializedStatement,
                    orderedIntentDigest: plan.orderedIntentDigest,
                    actionStatementNorito: plan.actionStatement,
                    actionStatementDigest: plan.actionDigest,
                    plan: plan, networkId: networkId, authority: authority,
                    requestArchive: requestNorito, admission: admission)
    }

    /// Recheck the original admission and sign the exact completed occurrence.
    /// Only a single Ed25519 controller matching the retained authority is admitted.
    /// Proof validity, current State and replay protection remain consensus checks.
    public func sign(rawProof: Data, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let limits = try admission.requireX509ConstructionV1(
            requestArchive, expectedNetworkId: networkId
        )
        return try plan.sign(rawProof: rawProof, limits: limits, authority: authority,
                             signingKey: signingKey)
    }
}

/// Canonical model projections shared by the direct producer and Exact12 fixture checks.
/// These routines establish byte bindings, never authenticated State or proof validity.
enum PrivacyX509WireV1 {
    static let wireId = "iroha.privacy.submit_zk_x509_certificate.v1"
    static let requestSchema = "iroha_data_model::isi::privacy::SubmitPrivacyZkX509CertificateV1"
    static let concreteStatementSchema = "iroha_data_model::privacy::IrohaZkX509StarkP256StatementV1"
    static let statementSchema = "iroha.privacy.statement.v1"
    static let actionSchema = "iroha_data_model::transaction::signed_attachments::ActionStatementV1"
    static let transactionSchema = "iroha_data_model::transaction::signed::model::TransactionPayload"
    static let statementLimit = 256 * 1024
    static let maximumProofBytes = 9 * 1024 * 1024
    static let zeroDigest = Data(repeating: 0, count: 32)
    // Same exact field/framing equation as the model's canonical_frame_byte_limit_v1.
    static var requestLimit: Int {
        statementLimit - frame(Data(), schema: statementSchema, alignment: 16).count - 4
            + fields([fields([u32(0), u32(0)])]).count
            + frame(Data(), schema: requestSchema, alignment: 8).count
    }

    private static func fieldLength(_ bytes: Int) -> Int {
        var remaining = bytes
        var prefix = 1
        while remaining >= 128 { remaining >>= 7; prefix += 1 }
        return prefix + bytes
    }
    private static var instructionTupleLimit: Int {
        fieldLength(string(wireId).count) + fieldLength(8 + requestLimit)
    }

    struct Request: Sendable {
        let statement: [Data]
        let context: [Data]
        let actionIndex: UInt32
        var statementPayload: Data { PrivacyX509WireV1.fields(statement) }
        var taggedStatement: Data { PrivacyX509WireV1.tagged(5, statementPayload) }
    }

    static func parseRequest(_ archive: Data) throws -> Request {
        let payload = try canonicalFrame(archive, schema: requestSchema,
                                         alignment: 8, maximum: requestLimit)
        let request = try split(payload, count: 2, maximum: statementLimit)
        let statement = try split(request[0], count: 20, maximum: statementLimit)
        let context = try split(statement[0], count: 8, maximum: statementLimit)
        guard context[0].count == 32, context[1].count == 4,
              try digest(context[2]) == zeroDigest else {
            throw invalid("X509 request context requires exact network, ordinal and ZERO intent")
        }
        for field in context.dropFirst(3) { _ = try digest(field, nonzero: true) }
        let reference = try split(request[1], count: 2, maximum: 4)
        guard try integer32(reference[1]) == 0 else { throw invalid("X509 proof slot must be zero") }
        let result = Request(statement: statement, context: context,
                             actionIndex: try integer32(reference[0]))
        guard frame(result.taggedStatement, schema: statementSchema, alignment: 16).count
                <= statementLimit else { throw invalid("X509 statement exceeds its canonical ceiling") }
        return result
    }

    struct DirectPlan: Sendable {
        let payloadFields: [Data]
        let actionIndependentFields: [Data]
        let orderedIntentDigest: Data
        let actionStatement: Data
        let actionDigest: Data
        let materializedStatement: Data
        let requestBytes: Int

        func sign(rawProof: Data, limits: PrivacyConsensusLimitsV1,
                  authority: String, signingKey: SigningKey) throws -> SignedTransactionEnvelope {
            let address = try AccountAddress.parseEncoded(authority)
            guard try CanonicalNorito.encodeCompactAccountId(authority) == payloadFields[1],
                  signingKey.algorithm == .ed25519,
                  let controller = address.singleControllerInfo(),
                  controller.algorithm == .ed25519,
                  controller.publicKey == (try signingKey.publicKey()) else {
                throw PrivacyX509TransactionErrorV1.authorityMismatch
            }
            let payload = try completedPayload(rawProof: rawProof, limits: limits)
            let message = IrohaHash.hash(payload)
            let signature = try signingKey.sign(message)
            guard let publicKey = try? Curve25519.Signing.PublicKey(rawRepresentation: controller.publicKey),
                  publicKey.isValidSignature(signature, for: message) else {
                throw PrivacyX509TransactionErrorV1.authorityMismatch
            }
            return SingleInstructionSwiftNoritoEncoder.signedEnvelope(
                transactionPayload: payload, signature: signature
            )
        }

        func requireStatementBounds(_ limits: PrivacyConsensusLimitsV1) throws {
            guard materializedStatement.count <= Int(limits.maxStatementAndEncryptedOutputBytesPerTransaction),
                  requestBytes <= Int(limits.maxActionBytes),
                  requestBytes <= Int(limits.maxPrivacyBytesPerTransaction),
                  limits.maxActionsPerTransaction >= 1 else {
                throw PrivacyX509TransactionErrorV1.proofBounds
            }
        }

        func completedPayload(rawProof: Data, limits: PrivacyConsensusLimitsV1) throws -> Data {
            try requireStatementBounds(limits)
            guard !rawProof.isEmpty,
                  rawProof.count <= PrivacyX509WireV1.maximumProofBytes,
                  rawProof.count <= Int(limits.maxProofBytesPerAction),
                  rawProof.contains(where: { $0 != 0 }),
                  requestBytes <= Int(limits.maxActionBytes) - rawProof.count,
                  requestBytes <= Int(limits.maxPrivacyBytesPerTransaction) - rawProof.count else {
                throw PrivacyX509TransactionErrorV1.proofBounds
            }
            let hash = try PrivacyX509WireV1.hash(rawProof)
            var complete = payloadFields
            complete[8] = PrivacyX509WireV1.attachments(actionIndependentFields, actionDigest: actionDigest,
                                      proof: rawProof, proofHash: hash)
            return PrivacyX509WireV1.fields(complete)
        }
    }

    static func prepareDirect(requestArchive: Data, unsignedPayload: Data) throws -> DirectPlan {
        let request = try parseRequest(requestArchive)
        var payload = try split(unsignedPayload, count: 9, maximum: 16 * 1024 * 1024)
        guard request.actionIndex == 0, try integer32(request.context[1]) == 0,
              payload[8] == Data([0]),
              payload[0] == tagged(0, request.context[0]),
              payload[1] == request.statement[17],
              try singleInstruction(payload[3]) == requestArchive else {
            throw PrivacyX509TransactionErrorV1.unsupportedDirectAction
        }
        let publicInputs = try domainHash("iroha.privacy.zk-x509-request-public-inputs.v1",
            frame(request.statementPayload, schema: concreteStatementSchema, alignment: 8))
        let action = [u32(0), tagged(0, string(wireId)), u32(0), u32(0),
                      tagged(1, u32(0)), wrappedDigest(publicInputs)]
        payload[8] = attachments(action, actionDigest: zeroDigest, proof: Data(), proofHash: zeroDigest)
        let normalized = frame(fields(payload), schema: transactionSchema, alignment: 8)
        let ordered = try domainHash("iroha.transaction.ordered-intent-digest.v1", normalized)
        let actionFrame = frame(fields([wrappedDigest(ordered)] + action),
                                schema: actionSchema, alignment: 8)
        let actionDigest = try domainHash("iroha.transaction.action-statement-digest.v1", actionFrame)
        var context = request.context
        context[2] = wrappedDigest(actionDigest)
        var statement = request.statement
        statement[0] = fields(context)
        return DirectPlan(payloadFields: payload, actionIndependentFields: action,
                          orderedIntentDigest: ordered, actionStatement: actionFrame,
                          actionDigest: actionDigest,
                          materializedStatement: frame(tagged(5, fields(statement)),
                                                       schema: statementSchema, alignment: 16),
                          requestBytes: requestArchive.count)
    }

    /// Verify exact direct fixture normalization against the same model construction owner.
    static func validateFixture(_ row: PrivacyExact12TypedFixtureRowV1) throws {
        var unsigned = try split(row.unsignedTransactionPayloadNorito, count: 9,
                                 maximum: PrivacyExact12FixtureCodecV1.maximumUnsignedTransactionBytes)
        let originalAttachments = unsigned[8]
        unsigned[8] = Data([0])
        let plan = try prepareDirect(requestArchive: row.submitProofInstructionNorito,
                                     unsignedPayload: fields(unsigned))
        let envelope = try canonicalFrame(row.envelopeNorito,
            schema: "iroha.privacy.proof-envelope.v1", alignment: 16,
            maximum: PrivacyExact12FixtureCodecV1.maximumEnvelopeBytes)
        let envelopeFields = try split(envelope, count: 13,
                                       maximum: PrivacyExact12FixtureCodecV1.maximumEnvelopeBytes)
        let taggedProof = envelopeFields[12]
        guard taggedProof.prefix(4) == u32(5) else { throw invalid("X509 proof protocol") }
        let proofWrapper = try split(Data(taggedProof.dropFirst(4)), count: 1,
                                     maximum: maximumProofBytes)[0]
        // PrivacyProofBytesV1 is a tuple struct around Vec<u8>.
        let proofVector = try split(proofWrapper, count: 1, maximum: maximumProofBytes)[0]
        let rawProof = try rawVector(proofVector, maximum: maximumProofBytes)
        guard !rawProof.isEmpty, rawProof.contains(where: { $0 != 0 }),
              originalAttachments == attachments(plan.actionIndependentFields,
                  actionDigest: plan.actionDigest, proof: rawProof, proofHash: try hash(rawProof)),
              row.statementNorito == plan.materializedStatement,
              row.transactionIntentProjectionNorito == plan.actionStatement,
              row.transactionIntentDigest == plan.actionDigest,
              try digest(envelopeFields[10]) == domainHash("iroha:privacy:statement:v1", row.statementNorito) else {
            throw invalid("X509 request, original occurrence, intent or materialized statement differs")
        }
    }

    private static func attachments(_ action: [Data], actionDigest: Data,
                                    proof: Data, proofHash: Data) -> Data {
        let reference = fields([u32(0), u32(0), u32(0), u64(UInt64(proof.count)), wrappedDigest(proofHash)])
        let actionPayload = fields(action + [wrappedDigest(actionDigest), sequence([reference])])
        let occurrence = fields([u32(0), u32(0), u64(UInt64(proof.count)), wrappedDigest(proofHash),
                                 u64(UInt64(proof.count)) + proof])
        return Data([1]) + fields([fields([sequence([actionPayload]), sequence([occurrence])])])
    }

    static func singleInstruction(_ bytes: Data) throws -> Data {
        guard bytes.prefix(4) == u32(0) else { throw invalid("X509 requires direct instructions") }
        let sequence = try split(Data(bytes.dropFirst(4)), count: 1, maximum: 8 + fieldLength(instructionTupleLimit))[0]
        guard sequence.prefix(8) == u64(1) else { throw invalid("X509 requires one direct instruction") }
        let boxed = try split(Data(sequence.dropFirst(8)), count: 1, maximum: instructionTupleLimit)[0]
        let pair = try split(boxed, count: 2, maximum: instructionTupleLimit)
        guard pair[0] == string(wireId) else { throw invalid("retired or substituted X509 instruction wire") }
        return try rawVector(pair[1], maximum: requestLimit)
    }

    static func canonicalFrame(_ archive: Data, schema: String, alignment: Int, maximum: Int) throws -> Data {
        guard archive.count <= maximum, let decoded = noritoDecodeFrame(archive),
              frame(decoded.payload, schema: schema, alignment: alignment) == archive else {
            throw invalid("invalid canonical \(schema) frame")
        }
        return decoded.payload
    }

    static func frame(_ bytes: Data, schema: String, alignment: Int) -> Data {
        noritoEncode(typeName: schema, payload: bytes, flags: NoritoHeader.compactLen,
                     payloadAlignment: alignment)
    }
    static func fields(_ values: [Data]) -> Data {
        var writer = CompactNoritoWriter()
        for value in values { writer.writeField(value) }
        return writer.data
    }
    static func sequence(_ values: [Data]) -> Data { u64(UInt64(values.count)) + fields(values) }
    static func tagged(_ tag: UInt32, _ payload: Data) -> Data { u32(tag) + fields([payload]) }
    static func u32(_ value: UInt32) -> Data { CompactNorito.encodeUInt32(value) }
    static func u64(_ value: UInt64) -> Data { CompactNorito.encodeUInt64(value) }
    static func string(_ value: String) -> Data { CompactNorito.encodeString(value) }
    static func wrappedDigest(_ bytes: Data) -> Data { fields([bytes]) }
    static func digest(_ bytes: Data, nonzero: Bool = false) throws -> Data {
        let value = try split(bytes, count: 1, maximum: 32)[0]
        guard value.count == 32, !nonzero || value.contains(where: { $0 != 0 }) else {
            throw invalid("invalid digest")
        }
        return value
    }
    static func integer32(_ bytes: Data) throws -> UInt32 {
        guard bytes.count == 4 else { throw invalid("invalid u32") }
        return bytes.enumerated().reduce(UInt32(0)) { $0 | (UInt32($1.element) << (8 * $1.offset)) }
    }
    static func hash(_ bytes: Data) throws -> Data {
        guard let result = Blake3.hash(bytes) else { throw invalid("BLAKE3 framing failure") }
        return result
    }
    static func domainHash(_ domain: String, _ bytes: Data) throws -> Data {
        try hash(Data(domain.utf8) + u64(UInt64(bytes.count)) + bytes)
    }
    static func rawVector(_ bytes: Data, maximum: Int) throws -> Data {
        guard bytes.count >= 8, bytes.count - 8 <= maximum,
              bytes.prefix(8) == u64(UInt64(bytes.count - 8)) else { throw invalid("invalid byte vector") }
        return Data(bytes.dropFirst(8))
    }
    static func split(_ bytes: Data, count: Int, maximum: Int) throws -> [Data] {
        var cursor = bytes.startIndex
        var result: [Data] = []
        for _ in 0..<count {
            var size: UInt64 = 0
            var shift = 0
            var prefix = Data()
            while true {
                guard cursor < bytes.endIndex, shift <= 63 else { throw invalid("truncated field length") }
                let byte = bytes[cursor]
                cursor = bytes.index(after: cursor)
                prefix.append(byte)
                guard shift != 63 || byte <= 1 else { throw invalid("overflowing field length") }
                size |= UInt64(byte & 0x7f) << shift
                if byte & 0x80 == 0 { break }
                shift += 7
            }
            guard size <= UInt64(maximum), size <= UInt64(bytes.distance(from: cursor, to: bytes.endIndex)) else {
                throw invalid("field exceeds byte ceiling")
            }
            let end = bytes.index(cursor, offsetBy: Int(size))
            let field = Data(bytes[cursor..<end])
            guard fields([field]) == prefix + field else { throw invalid("noncanonical field length") }
            result.append(field)
            cursor = end
        }
        guard cursor == bytes.endIndex else { throw invalid("trailing fields") }
        return result
    }
    static func invalid(_ reason: String) -> PrivacyX509TransactionErrorV1 { .invalidCanonicalInput(reason) }
}

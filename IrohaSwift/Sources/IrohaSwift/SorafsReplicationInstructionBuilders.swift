import Foundation

/// Maximum decoded `ReplicationOrderV1` payload accepted by the V1 builders.
public let sorafsReplicationOrderMaxPayloadBytesV1 = 1_048_576

/// Errors emitted by the canonical SoraFS replication-order instruction helpers.
public enum SorafsReplicationInstructionBuilderError: LocalizedError, Equatable {
    case invalidIdentifier(field: String)
    case invalidPayload(reason: String)
    case invalidEpochWindow
    case invalidInstruction(reason: String)

    public var errorDescription: String? {
        switch self {
        case let .invalidIdentifier(field):
            return "\(field) must be a non-zero, lowercase 64-character hexadecimal identifier"
        case let .invalidPayload(reason):
            return "order_payload is invalid: \(reason)"
        case .invalidEpochWindow:
            return "deadline_epoch must be greater than issued_epoch"
        case let .invalidInstruction(reason):
            return "replication-order instruction is invalid: \(reason)"
        }
    }
}

/// Policy-relevant fields decoded from a canonical `ReplicationOrderV1` archive.
public struct SorafsReplicationOrderPayloadSummary: Equatable, Sendable {
    public let orderId: String
    public let targetReplicas: UInt16
    public let providerIds: [String]
    public let issuedAt: UInt64
    public let deadlineAt: UInt64
}

private enum SorafsReplicationOrderV1 {
    static let typeName = "sorafs_manifest::capacity::ReplicationOrderV1"
    static let maximumAssignments = 1_024
    static let maximumPayloadBase64Characters =
        4 * ((sorafsReplicationOrderMaxPayloadBytesV1 + 2) / 3)

    static func canonicalIdentifier(_ value: String, field: String) throws -> String {
        let bytes = Array(value.utf8)
        guard bytes.count == 64,
              bytes.allSatisfy({
                  (0x30...0x39).contains($0) || (0x61...0x66).contains($0)
              }),
              bytes.contains(where: { $0 != 0x30 })
        else {
            throw SorafsReplicationInstructionBuilderError.invalidIdentifier(field: field)
        }
        return value
    }

    static func canonicalBase64(_ value: String, expectedOrderId: String) throws -> Data {
        guard !value.isEmpty,
              value.utf8.count <= maximumPayloadBase64Characters,
              value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              !value.unicodeScalars.contains(where: { CharacterSet.whitespacesAndNewlines.contains($0) }),
              let payload = Data(base64Encoded: value),
              !payload.isEmpty,
              payload.base64EncodedString() == value
        else {
            throw SorafsReplicationInstructionBuilderError.invalidPayload(
                reason: "expected non-empty canonical standard base64"
            )
        }
        _ = try validate(payload, expectedOrderId: expectedOrderId)
        return payload
    }

    static func validate(
        _ archive: Data,
        expectedOrderId: String? = nil
    ) throws -> SorafsReplicationOrderPayloadSummary {
        guard !archive.isEmpty, archive.count <= sorafsReplicationOrderMaxPayloadBytesV1 else {
            throw SorafsReplicationInstructionBuilderError.invalidPayload(
                reason: "decoded payload must contain 1...\(sorafsReplicationOrderMaxPayloadBytesV1) bytes"
            )
        }
        guard let frame = noritoDecodeFrame(archive),
              frame.header.schema == noritoSchemaHash(forTypeName: typeName),
              frame.header.compression == .none,
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == 0,
              noritoEncode(
                  typeName: typeName,
                  payload: frame.payload,
                  flags: NoritoHeader.compactLen
              ) == archive
        else {
            throw SorafsReplicationInstructionBuilderError.invalidPayload(
                reason: "expected a canonical, unpadded, compact-length ReplicationOrderV1 frame"
            )
        }

        var reader = CompactNoritoReader(frame.payload, context: "ReplicationOrderV1")
        let version = try reader.field("version")
        let orderIdBytes = try reader.field("order_id")
        _ = try reader.field("manifest_cid")
        _ = try reader.field("manifest_digest")
        _ = try reader.field("chunking_profile")
        let targetReplicasBytes = try reader.field("target_replicas")
        let assignmentsBytes = try reader.field("assignments")
        let issuedAtBytes = try reader.field("issued_at")
        let deadlineAtBytes = try reader.field("deadline_at")
        _ = try reader.field("sla")
        _ = try reader.field("metadata")
        try reader.requireEnd()

        guard version == Data([1]) else {
            throw invalidPayload("version must be 1")
        }
        guard orderIdBytes.count == 32, orderIdBytes.contains(where: { $0 != 0 }) else {
            throw invalidPayload("order_id must be a non-zero 32-byte value")
        }
        let orderId = orderIdBytes.hexString
        if let expectedOrderId {
            let expected = try canonicalIdentifier(
                expectedOrderId,
                field: "IssueReplicationOrder.order_id"
            )
            guard orderId == expected else {
                throw invalidPayload(
                    "IssueReplicationOrder.order_id must match ReplicationOrderV1.order_id"
                )
            }
        }

        guard targetReplicasBytes.count == 2 else {
            throw invalidPayload("target_replicas must be a u16")
        }
        let targetReplicas = targetReplicasBytes.littleEndianUInt16
        guard targetReplicas > 0 else {
            throw invalidPayload("target_replicas must be greater than zero")
        }

        var assignments = CompactNoritoReader(
            assignmentsBytes,
            context: "ReplicationOrderV1.assignments"
        )
        let assignmentCount = try assignments.fixedUInt64("count")
        guard assignmentCount > 0, assignmentCount <= UInt64(maximumAssignments) else {
            throw invalidPayload("assignments must contain 1...\(maximumAssignments) entries")
        }

        var providers = [Data]()
        providers.reserveCapacity(Int(assignmentCount))
        for index in 0..<Int(assignmentCount) {
            var assignment = CompactNoritoReader(
                try assignments.field("item[\(index)]"),
                context: "ReplicationOrderV1.assignments[\(index)]"
            )
            let provider = try assignment.field("provider_id")
            let sliceGiB = try assignment.field("slice_gib")
            _ = try assignment.field("lane")
            try assignment.requireEnd()
            guard provider.count == 32, provider.contains(where: { $0 != 0 }) else {
                throw invalidPayload(
                    "assignments[\(index)].provider_id must be a non-zero 32-byte value"
                )
            }
            guard try exactUInt64(sliceGiB, field: "assignments[\(index)].slice_gib") > 0 else {
                throw invalidPayload("assignments[\(index)].slice_gib must be positive")
            }
            if let previous = providers.last,
               !previous.lexicographicallyPrecedes(provider) {
                throw invalidPayload(
                    "assignments must use unique, strictly increasing provider_id values"
                )
            }
            providers.append(provider)
        }
        try assignments.requireEnd()
        guard Int(targetReplicas) <= providers.count else {
            throw invalidPayload("target_replicas must not exceed assignment count")
        }

        let issuedAt = try exactUInt64(issuedAtBytes, field: "issued_at")
        let deadlineAt = try exactUInt64(deadlineAtBytes, field: "deadline_at")
        guard deadlineAt > issuedAt else {
            throw invalidPayload("deadline_at must be greater than issued_at")
        }

        return SorafsReplicationOrderPayloadSummary(
            orderId: orderId,
            targetReplicas: targetReplicas,
            providerIds: providers.map(\.hexString),
            issuedAt: issuedAt,
            deadlineAt: deadlineAt
        )
    }

    private static func exactUInt64(_ data: Data, field: String) throws -> UInt64 {
        guard data.count == 8 else {
            throw invalidPayload("\(field) must contain exactly eight bytes")
        }
        return data.littleEndianUInt64
    }

    private static func invalidPayload(
        _ reason: String
    ) -> SorafsReplicationInstructionBuilderError {
        .invalidPayload(reason: reason)
    }
}

private struct CompactNoritoReader {
    private let data: Data
    private let context: String
    private var offset = 0

    init(_ data: Data, context: String) {
        self.data = data
        self.context = context
    }

    mutating func field(_ name: String) throws -> Data {
        try bytes(try compactLength(name), name)
    }

    mutating func fixedUInt64(_ name: String) throws -> UInt64 {
        try bytes(8, name).littleEndianUInt64
    }

    mutating func requireEnd() throws {
        guard offset == data.count else {
            throw SorafsReplicationInstructionBuilderError.invalidPayload(
                reason: "\(context) contains trailing bytes"
            )
        }
    }

    private mutating func compactLength(_ name: String) throws -> Int {
        var value: UInt64 = 0
        var shift = 0
        var consumed = 0
        while consumed < 10 {
            let byte = try bytes(1, "\(name).length")[0]
            consumed += 1
            let part = UInt64(byte & 0x7f)
            if shift == 63, part > 1 {
                throw malformedLength(name)
            }
            value |= part << UInt64(shift)
            if byte & 0x80 == 0 {
                if consumed > 1, part == 0 {
                    throw malformedLength(name)
                }
                guard value <= UInt64(Int.max) else {
                    throw malformedLength(name)
                }
                return Int(value)
            }
            shift += 7
        }
        throw malformedLength(name)
    }

    private mutating func bytes(_ count: Int, _ name: String) throws -> Data {
        guard count >= 0, offset <= data.count, count <= data.count - offset else {
            throw SorafsReplicationInstructionBuilderError.invalidPayload(
                reason: "\(context).\(name) overruns the Norito payload"
            )
        }
        defer { offset += count }
        return data.subdata(in: offset..<(offset + count))
    }

    private func malformedLength(
        _ name: String
    ) -> SorafsReplicationInstructionBuilderError {
        .invalidPayload(reason: "\(context).\(name) uses a noncanonical compact length")
    }
}

private extension Data {
    var littleEndianUInt16: UInt16 {
        UInt16(self[0]) | (UInt16(self[1]) << 8)
    }

    var littleEndianUInt64: UInt64 {
        enumerated().reduce(UInt64(0)) { value, entry in
            value | (UInt64(entry.element) << UInt64(entry.offset * 8))
        }
    }

    var hexString: String {
        map { String(format: "%02x", $0) }.joined()
    }
}

/// Canonical `IssueReplicationOrder` instruction fields.
public struct SorafsIssueReplicationOrderInstruction: Equatable, Sendable {
    public let orderId: String
    public let orderPayload: Data
    public let issuedEpoch: UInt64
    public let deadlineEpoch: UInt64

    public init(
        orderId: String,
        orderPayload: Data,
        issuedEpoch: UInt64,
        deadlineEpoch: UInt64
    ) throws {
        self.orderId = try SorafsReplicationOrderV1.canonicalIdentifier(
            orderId,
            field: "order_id"
        )
        guard deadlineEpoch > issuedEpoch else {
            throw SorafsReplicationInstructionBuilderError.invalidEpochWindow
        }
        _ = try SorafsReplicationOrderV1.validate(
            orderPayload,
            expectedOrderId: self.orderId
        )
        self.orderPayload = orderPayload
        self.issuedEpoch = issuedEpoch
        self.deadlineEpoch = deadlineEpoch
    }

    public init(
        orderId: String,
        orderPayloadBase64: String,
        issuedEpoch: UInt64,
        deadlineEpoch: UInt64
    ) throws {
        let canonicalOrderId = try SorafsReplicationOrderV1.canonicalIdentifier(
            orderId,
            field: "order_id"
        )
        let payload = try SorafsReplicationOrderV1.canonicalBase64(
            orderPayloadBase64,
            expectedOrderId: canonicalOrderId
        )
        try self.init(
            orderId: canonicalOrderId,
            orderPayload: payload,
            issuedEpoch: issuedEpoch,
            deadlineEpoch: deadlineEpoch
        )
    }

    public var orderPayloadBase64: String {
        orderPayload.base64EncodedString()
    }

    public func noritoJSON() throws -> NoritoJSON {
        try NoritoJSON.fromJSONObject([
            "IssueReplicationOrder": [
                "order_id": orderId,
                "order_payload": orderPayloadBase64,
                "issued_epoch": NSNumber(value: issuedEpoch),
                "deadline_epoch": NSNumber(value: deadlineEpoch),
            ],
        ])
    }
}

/// Exact governed signer-policy identity expected at completion commit.
public struct SorafsProviderIngestCompletionSignerPolicyV1: Equatable, Sendable {
    public let policyId: String
    public let revision: UInt64
    public let predecessorDigest: String?
    public let policyDigest: String

    public init(
        policyId: String,
        revision: UInt64,
        predecessorDigest: String?,
        policyDigest: String
    ) throws {
        guard revision > 0 else {
            throw SorafsReplicationInstructionBuilderError.invalidInstruction(
                reason: "signer_policy.revision must be greater than zero"
            )
        }
        if revision == 1, predecessorDigest != nil {
            throw SorafsReplicationInstructionBuilderError.invalidInstruction(
                reason: "signer_policy.predecessor_digest must be absent at revision one"
            )
        }
        if revision > 1, predecessorDigest == nil {
            throw SorafsReplicationInstructionBuilderError.invalidInstruction(
                reason: "signer_policy.predecessor_digest is required after revision one"
            )
        }
        self.policyId = try SorafsReplicationOrderV1.canonicalIdentifier(
            policyId,
            field: "signer_policy.policy_id"
        )
        self.revision = revision
        self.predecessorDigest = try predecessorDigest.map {
            try SorafsReplicationOrderV1.canonicalIdentifier(
                $0,
                field: "signer_policy.predecessor_digest"
            )
        }
        self.policyDigest = try SorafsReplicationOrderV1.canonicalIdentifier(
            policyDigest,
            field: "signer_policy.policy_digest"
        )
    }

    fileprivate var jsonObject: [String: Any] {
        [
            "policy_id": policyId,
            "revision": NSNumber(value: revision),
            "predecessor_digest": predecessorDigest.map { $0 as Any } ?? NSNull(),
            "policy_digest": policyDigest,
        ]
    }
}

/// Exact provider owner and signer policy expected at completion commit.
public struct SorafsProviderIngestCompletionAuthorityV1: Equatable, Sendable {
    public let providerOwner: String
    public let signerPolicy: SorafsProviderIngestCompletionSignerPolicyV1

    public init(
        providerOwner: String,
        signerPolicy: SorafsProviderIngestCompletionSignerPolicyV1
    ) throws {
        guard providerOwner == providerOwner.trimmingCharacters(in: .whitespacesAndNewlines),
              providerOwner.rangeOfCharacter(from: .whitespacesAndNewlines) == nil,
              !providerOwner.contains("@"),
              !providerOwner.contains("#"),
              !providerOwner.contains("$")
        else {
            throw SorafsReplicationInstructionBuilderError.invalidInstruction(
                reason: "provider_owner must be an exact canonical I105 account id"
            )
        }
        do {
            let prefix = try AccountAddress
                .inspectI105NetworkPrefix(providerOwner).chainDiscriminant
            let address = try AccountAddress.parseEncodedSwiftOnly(
                providerOwner,
                expectedPrefix: prefix
            )
            guard try address.toI105(networkPrefix: prefix) == providerOwner else {
                throw SorafsReplicationInstructionBuilderError.invalidInstruction(
                    reason: "provider_owner must be an exact canonical I105 account id"
                )
            }
        } catch {
            throw SorafsReplicationInstructionBuilderError.invalidInstruction(
                reason: "provider_owner must be an exact canonical I105 account id"
            )
        }
        self.providerOwner = providerOwner
        self.signerPolicy = signerPolicy
    }

    fileprivate var jsonObject: [String: Any] {
        [
            "provider_owner": providerOwner,
            "signer_policy": signerPolicy.jsonObject,
        ]
    }
}

/// Exact finalized committed-chain prefix used to prepare a completion.
public struct SorafsProviderIngestFinalizedAnchorV1: Equatable, Sendable {
    public let height: UInt64
    public let blockHash: String

    public init(height: UInt64, blockHash: String) throws {
        guard height > 0 else {
            throw SorafsReplicationInstructionBuilderError.invalidInstruction(
                reason: "finalized_anchor.height must be greater than zero"
            )
        }
        self.height = height
        self.blockHash = try SorafsReplicationOrderV1.canonicalIdentifier(
            blockHash,
            field: "finalized_anchor.block_hash"
        )
    }

    fileprivate var jsonObject: [String: Any] {
        [
            "height": NSNumber(value: height),
            "block_hash": blockHash,
        ]
    }
}

/// Canonical provider-specific `CompleteReplicationOrder` instruction fields.
public struct SorafsCompleteReplicationOrderInstruction: Equatable, Sendable {
    public let orderId: String
    public let providerId: String
    public let completionEpoch: UInt64
    public let expectedAuthority: SorafsProviderIngestCompletionAuthorityV1
    public let expectedAssignmentRevision: UInt64
    public let finalizedAnchor: SorafsProviderIngestFinalizedAnchorV1

    public init(
        orderId: String,
        providerId: String,
        completionEpoch: UInt64,
        expectedAuthority: SorafsProviderIngestCompletionAuthorityV1,
        expectedAssignmentRevision: UInt64,
        finalizedAnchor: SorafsProviderIngestFinalizedAnchorV1
    ) throws {
        self.orderId = try SorafsReplicationOrderV1.canonicalIdentifier(
            orderId,
            field: "order_id"
        )
        self.providerId = try SorafsReplicationOrderV1.canonicalIdentifier(
            providerId,
            field: "provider_id"
        )
        self.completionEpoch = completionEpoch
        guard expectedAssignmentRevision > 0 else {
            throw SorafsReplicationInstructionBuilderError.invalidInstruction(
                reason: "expected_assignment_revision must be greater than zero"
            )
        }
        self.expectedAuthority = expectedAuthority
        self.expectedAssignmentRevision = expectedAssignmentRevision
        self.finalizedAnchor = finalizedAnchor
    }

    public func noritoJSON() throws -> NoritoJSON {
        try NoritoJSON.fromJSONObject([
            "CompleteReplicationOrder": [
                "order_id": orderId,
                "provider_id": providerId,
                "completion_epoch": NSNumber(value: completionEpoch),
                "expected_authority": expectedAuthority.jsonObject,
                "expected_assignment_revision": NSNumber(
                    value: expectedAssignmentRevision
                ),
                "finalized_anchor": finalizedAnchor.jsonObject,
            ],
        ])
    }
}

/// Canonical `ExpireReplicationOrder` instruction fields.
public struct SorafsExpireReplicationOrderInstruction: Equatable, Sendable {
    public let orderId: String
    public let expirationEpoch: UInt64

    public init(orderId: String, expirationEpoch: UInt64) throws {
        self.orderId = try SorafsReplicationOrderV1.canonicalIdentifier(
            orderId,
            field: "order_id"
        )
        self.expirationEpoch = expirationEpoch
    }

    public func noritoJSON() throws -> NoritoJSON {
        try NoritoJSON.fromJSONObject([
            "ExpireReplicationOrder": [
                "order_id": orderId,
                "expiration_epoch": NSNumber(value: expirationEpoch),
            ],
        ])
    }
}

/// A schema-closed decoded SoraFS replication-order instruction.
public enum SorafsReplicationOrderInstruction: Equatable, Sendable {
    case issue(SorafsIssueReplicationOrderInstruction)
    case complete(SorafsCompleteReplicationOrderInstruction)
    case expire(SorafsExpireReplicationOrderInstruction)

    public func noritoJSON() throws -> NoritoJSON {
        switch self {
        case let .issue(value): return try value.noritoJSON()
        case let .complete(value): return try value.noritoJSON()
        case let .expire(value): return try value.noritoJSON()
        }
    }
}

/// Builders and schema-closed decoding for the three V1 replication-order ISIs.
public enum SorafsReplicationInstructionBuilders {
    public static func validateOrderPayloadV1(
        _ payload: Data,
        expectedOrderId: String? = nil
    ) throws -> SorafsReplicationOrderPayloadSummary {
        try SorafsReplicationOrderV1.validate(payload, expectedOrderId: expectedOrderId)
    }

    public static func issueReplicationOrder(
        orderId: String,
        orderPayload: Data,
        issuedEpoch: UInt64,
        deadlineEpoch: UInt64
    ) throws -> NoritoJSON {
        try SorafsIssueReplicationOrderInstruction(
            orderId: orderId,
            orderPayload: orderPayload,
            issuedEpoch: issuedEpoch,
            deadlineEpoch: deadlineEpoch
        ).noritoJSON()
    }

    public static func issueReplicationOrder(
        orderId: String,
        orderPayloadBase64: String,
        issuedEpoch: UInt64,
        deadlineEpoch: UInt64
    ) throws -> NoritoJSON {
        try SorafsIssueReplicationOrderInstruction(
            orderId: orderId,
            orderPayloadBase64: orderPayloadBase64,
            issuedEpoch: issuedEpoch,
            deadlineEpoch: deadlineEpoch
        ).noritoJSON()
    }

    public static func completeReplicationOrder(
        orderId: String,
        providerId: String,
        completionEpoch: UInt64,
        expectedAuthority: SorafsProviderIngestCompletionAuthorityV1,
        expectedAssignmentRevision: UInt64,
        finalizedAnchor: SorafsProviderIngestFinalizedAnchorV1
    ) throws -> NoritoJSON {
        try SorafsCompleteReplicationOrderInstruction(
            orderId: orderId,
            providerId: providerId,
            completionEpoch: completionEpoch,
            expectedAuthority: expectedAuthority,
            expectedAssignmentRevision: expectedAssignmentRevision,
            finalizedAnchor: finalizedAnchor
        ).noritoJSON()
    }

    public static func expireReplicationOrder(
        orderId: String,
        expirationEpoch: UInt64
    ) throws -> NoritoJSON {
        try SorafsExpireReplicationOrderInstruction(
            orderId: orderId,
            expirationEpoch: expirationEpoch
        ).noritoJSON()
    }

    public static func decode(
        _ instruction: NoritoJSON
    ) throws -> SorafsReplicationOrderInstruction {
        guard let outer = try JSONSerialization.jsonObject(
            with: instruction.data
        ) as? [String: Any],
              outer.count == 1,
              let variant = outer.keys.first
        else {
            throw invalidInstruction("expected exactly one instruction variant")
        }
        switch variant {
        case "IssueReplicationOrder":
            let body = try exactBody(
                outer[variant],
                fields: ["order_id", "order_payload", "issued_epoch", "deadline_epoch"],
                variant: variant
            )
            return .issue(try SorafsIssueReplicationOrderInstruction(
                orderId: try string(body["order_id"], field: "order_id"),
                orderPayloadBase64: try string(body["order_payload"], field: "order_payload"),
                issuedEpoch: try epoch(body["issued_epoch"], field: "issued_epoch"),
                deadlineEpoch: try epoch(body["deadline_epoch"], field: "deadline_epoch")
            ))
        case "CompleteReplicationOrder":
            let body = try exactBody(
                outer[variant],
                fields: [
                    "order_id",
                    "provider_id",
                    "completion_epoch",
                    "expected_authority",
                    "expected_assignment_revision",
                    "finalized_anchor",
                ],
                variant: variant
            )
            let authority = try exactBody(
                body["expected_authority"],
                fields: ["provider_owner", "signer_policy"],
                variant: "ProviderIngestCompletionAuthorityV1"
            )
            let signerPolicy = try exactBody(
                authority["signer_policy"],
                fields: [
                    "policy_id",
                    "revision",
                    "predecessor_digest",
                    "policy_digest",
                ],
                variant: "ProviderIngestCompletionSignerPolicyV1"
            )
            let anchor = try exactBody(
                body["finalized_anchor"],
                fields: ["height", "block_hash"],
                variant: "ProviderIngestFinalizedAnchorV1"
            )
            return .complete(try SorafsCompleteReplicationOrderInstruction(
                orderId: try string(body["order_id"], field: "order_id"),
                providerId: try string(body["provider_id"], field: "provider_id"),
                completionEpoch: try epoch(body["completion_epoch"], field: "completion_epoch"),
                expectedAuthority: try SorafsProviderIngestCompletionAuthorityV1(
                    providerOwner: try string(
                        authority["provider_owner"],
                        field: "provider_owner"
                    ),
                    signerPolicy: try SorafsProviderIngestCompletionSignerPolicyV1(
                        policyId: try string(
                            signerPolicy["policy_id"],
                            field: "policy_id"
                        ),
                        revision: try epoch(
                            signerPolicy["revision"],
                            field: "revision"
                        ),
                        predecessorDigest: try optionalString(
                            signerPolicy["predecessor_digest"],
                            field: "predecessor_digest"
                        ),
                        policyDigest: try string(
                            signerPolicy["policy_digest"],
                            field: "policy_digest"
                        )
                    )
                ),
                expectedAssignmentRevision: try epoch(
                    body["expected_assignment_revision"],
                    field: "expected_assignment_revision"
                ),
                finalizedAnchor: try SorafsProviderIngestFinalizedAnchorV1(
                    height: try epoch(anchor["height"], field: "height"),
                    blockHash: try string(anchor["block_hash"], field: "block_hash")
                )
            ))
        case "ExpireReplicationOrder":
            let body = try exactBody(
                outer[variant],
                fields: ["order_id", "expiration_epoch"],
                variant: variant
            )
            return .expire(try SorafsExpireReplicationOrderInstruction(
                orderId: try string(body["order_id"], field: "order_id"),
                expirationEpoch: try epoch(body["expiration_epoch"], field: "expiration_epoch")
            ))
        default:
            throw invalidInstruction("unsupported variant \(variant)")
        }
    }

    private static func exactBody(
        _ value: Any?,
        fields: Set<String>,
        variant: String
    ) throws -> [String: Any] {
        guard let body = value as? [String: Any], Set(body.keys) == fields else {
            throw invalidInstruction(
                "\(variant) must contain exactly \(fields.sorted().joined(separator: ", "))"
            )
        }
        return body
    }

    private static func string(_ value: Any?, field: String) throws -> String {
        guard let value = value as? String else {
            throw invalidInstruction("\(field) must be a string")
        }
        return value
    }

    private static func optionalString(
        _ value: Any?,
        field: String
    ) throws -> String? {
        if value is NSNull {
            return nil
        }
        return try string(value, field: field)
    }

    private static func epoch(_ value: Any?, field: String) throws -> UInt64 {
        guard !(value is Bool), let number = value as? NSNumber else {
            throw invalidInstruction("\(field) must be a non-negative u64")
        }
        let literal = number.stringValue
        guard !literal.isEmpty,
              literal.utf8.allSatisfy({ (0x30...0x39).contains($0) }),
              let epoch = UInt64(literal)
        else {
            throw invalidInstruction("\(field) must be a non-negative u64")
        }
        return epoch
    }

    private static func invalidInstruction(
        _ reason: String
    ) -> SorafsReplicationInstructionBuilderError {
        .invalidInstruction(reason: reason)
    }
}

public extension IrohaSDK {
    func buildIssueReplicationOrder(
        orderId: String,
        orderPayload: Data,
        issuedEpoch: UInt64,
        deadlineEpoch: UInt64
    ) throws -> NoritoJSON {
        try SorafsReplicationInstructionBuilders.issueReplicationOrder(
            orderId: orderId,
            orderPayload: orderPayload,
            issuedEpoch: issuedEpoch,
            deadlineEpoch: deadlineEpoch
        )
    }

    func buildIssueReplicationOrder(
        orderId: String,
        orderPayloadBase64: String,
        issuedEpoch: UInt64,
        deadlineEpoch: UInt64
    ) throws -> NoritoJSON {
        try SorafsReplicationInstructionBuilders.issueReplicationOrder(
            orderId: orderId,
            orderPayloadBase64: orderPayloadBase64,
            issuedEpoch: issuedEpoch,
            deadlineEpoch: deadlineEpoch
        )
    }

    func buildCompleteReplicationOrder(
        orderId: String,
        providerId: String,
        completionEpoch: UInt64,
        expectedAuthority: SorafsProviderIngestCompletionAuthorityV1,
        expectedAssignmentRevision: UInt64,
        finalizedAnchor: SorafsProviderIngestFinalizedAnchorV1
    ) throws -> NoritoJSON {
        try SorafsReplicationInstructionBuilders.completeReplicationOrder(
            orderId: orderId,
            providerId: providerId,
            completionEpoch: completionEpoch,
            expectedAuthority: expectedAuthority,
            expectedAssignmentRevision: expectedAssignmentRevision,
            finalizedAnchor: finalizedAnchor
        )
    }

    func buildExpireReplicationOrder(
        orderId: String,
        expirationEpoch: UInt64
    ) throws -> NoritoJSON {
        try SorafsReplicationInstructionBuilders.expireReplicationOrder(
            orderId: orderId,
            expirationEpoch: expirationEpoch
        )
    }
}

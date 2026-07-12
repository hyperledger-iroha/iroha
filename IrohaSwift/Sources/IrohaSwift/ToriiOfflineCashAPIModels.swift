import Foundation

/// Canonical first-release Torii Offline routes.
public enum OfflineAPI {
    public enum Endpoint: String, Sendable {
        case readiness = "/v1/offline/readiness"
        case topUp = "/v1/offline/top-up"
        case redeem = "/v1/offline/redeem"
        case operations = "/v1/offline/operations"

        public var path: String { rawValue }
    }

    public static func operationPath(_ operationId: String) throws -> String {
        "\(Endpoint.operations.path)/\(try OfflineOperationValidation.operationId(operationId))"
    }
}

public enum OfflineOperationKind: String, Codable, Equatable, Sendable {
    case topUp = "top_up"
    case redeem
}

public enum OfflineOperationState: String, Codable, Equatable, Sendable {
    case pending
}

public enum OfflineOperationError: Error, LocalizedError, Equatable, Sendable {
    case invalidField(String)
    case invalidNoritoArchive

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid Offline operation field: \(field)."
        case .invalidNoritoArchive:
            return "Offline operation request must be a canonical Norito archive."
        }
    }
}

/// A schema-bound Offline top-up command submitted directly to Torii.
public struct OfflineTopUpRequest: Equatable, Sendable {
    /// Lowercase hex derived from the archive's nonzero 32-byte operation ID.
    public let operationId: String
    private let archive: Data

    /// Validates and retains a canonical first-release Offline top-up request archive.
    public init(noritoArchive: Data) throws {
        let validated = try OfflineOperationValidation.requestArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            operationIdFieldIndex: 6,
            fieldCount: 8
        )
        self.operationId = validated.operationId
        self.archive = validated.archive
    }

    public func noritoArchive() -> Data { archive }
}

/// A schema-bound Offline redemption command submitted directly to Torii.
public struct OfflineRedeemRequest: Equatable, Sendable {
    /// Lowercase hex derived from the archive's nonzero 32-byte operation ID.
    public let operationId: String
    private let archive: Data

    /// Validates and retains a canonical first-release Offline redemption request archive.
    public init(noritoArchive: Data) throws {
        let validated = try OfflineOperationValidation.requestArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.redeemRequestWireName,
            operationIdFieldIndex: 9,
            fieldCount: 11
        )
        self.operationId = validated.operationId
        self.archive = validated.archive
    }

    public func noritoArchive() -> Data { archive }
}

public struct OfflineOperationReference: Codable, Equatable, Sendable {
    public let operationId: String
    public let kind: OfflineOperationKind
    public let state: OfflineOperationState
    public let transactionHash: String
    public let statusUri: String
    public let submittedAtMs: UInt64

    public init(
        operationId: String,
        kind: OfflineOperationKind,
        state: OfflineOperationState,
        transactionHash: String,
        statusUri: String,
        submittedAtMs: UInt64
    ) throws {
        let validatedOperationId = try OfflineOperationValidation.operationId(operationId)
        self.operationId = validatedOperationId
        self.kind = kind
        self.state = state
        self.transactionHash = try OfflineOperationValidation.transactionHash(
            transactionHash,
            field: "transaction_hash"
        )
        self.statusUri = try OfflineOperationValidation.statusUri(
            statusUri,
            operationId: validatedOperationId
        )
        self.submittedAtMs = submittedAtMs
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decode(String.self, forKey: .operationId),
            kind: container.decode(OfflineOperationKind.self, forKey: .kind),
            state: container.decode(OfflineOperationState.self, forKey: .state),
            transactionHash: container.decode(String.self, forKey: .transactionHash),
            statusUri: container.decode(String.self, forKey: .statusUri),
            submittedAtMs: container.decode(UInt64.self, forKey: .submittedAtMs)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case operationId = "operation_id"
        case kind
        case state
        case transactionHash = "transaction_hash"
        case statusUri = "status_uri"
        case submittedAtMs = "submitted_at_ms"
    }
}

/// Canonical finalized top-up anchor consumed by the Offline wallet prover.
///
/// The public API intentionally exposes the current semantic name while the
/// versioned consensus wire type remains an internal codec detail.
public struct OfflineTopUpAnchor: Equatable, Sendable {
    private let archive: Data
    private let anchorDigest: Data

    /// Validates and retains a canonical top-up anchor Norito archive.
    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityAnchorMaximumArchiveBytes else {
            throw OfflineOperationError.invalidNoritoArchive
        }
        let wireValue = try KagemushaRecursiveSpendCodecs.decodeTopUpAnchor(
            Data(noritoArchive)
        )
        self.archive = Data(wireValue.archive)
        self.anchorDigest = Data(wireValue.anchorDigest)
    }

    /// Returns a defensive copy of the canonical Norito archive.
    public func noritoArchive() -> Data {
        Data(archive)
    }

    /// Digest committed by the finalized anchor, returned as a defensive copy.
    public var digest: Data {
        Data(anchorDigest)
    }
}

/// Opaque typed consensus proof returned by the canonical Torii top-up
/// operation status resource.
public typealias OfflineTopUpFinalityProof = KagemushaTopUpFinalityProofArchive

public struct OfflineTopUpResult: Equatable, Sendable {
    public let transactionHash: String
    public let finalizedBlockHeight: UInt64
    public let serverTimeMs: UInt64
    public let anchor: OfflineTopUpAnchor
    public let finalityProof: OfflineTopUpFinalityProof

    public init(
        transactionHash: String,
        finalizedBlockHeight: UInt64,
        serverTimeMs: UInt64,
        anchor: OfflineTopUpAnchor,
        finalityProof: OfflineTopUpFinalityProof
    ) throws {
        self.transactionHash = try OfflineOperationValidation.transactionHash(
            transactionHash,
            field: "transaction_hash"
        )
        self.finalizedBlockHeight = try OfflineOperationValidation.positive(
            finalizedBlockHeight,
            field: "finalized_block_height"
        )
        self.serverTimeMs = try OfflineOperationValidation.positive(
            serverTimeMs,
            field: "server_time_ms"
        )
        self.anchor = anchor
        self.finalityProof = finalityProof
    }
}

public struct OfflineRedeemResult: Equatable, Sendable {
    public let transactionHash: String
    public let finalizedBlockHeight: UInt64
    public let serverTimeMs: UInt64

    public init(
        transactionHash: String,
        finalizedBlockHeight: UInt64,
        serverTimeMs: UInt64
    ) throws {
        self.transactionHash = try OfflineOperationValidation.transactionHash(
            transactionHash,
            field: "transaction_hash"
        )
        self.finalizedBlockHeight = try OfflineOperationValidation.positive(
            finalizedBlockHeight,
            field: "finalized_block_height"
        )
        self.serverTimeMs = try OfflineOperationValidation.positive(
            serverTimeMs,
            field: "server_time_ms"
        )
    }
}

public enum OfflineOperationResult: Equatable, Sendable {
    case topUp(OfflineTopUpResult)
    case redeem(OfflineRedeemResult)
}

public struct OfflineQueueErrorSnapshot: Equatable, Sendable {
    public let state: String
    public let queued: UInt64
    public let capacity: UInt64
    public let saturated: Bool

    public init(state: String, queued: UInt64, capacity: UInt64, saturated: Bool) throws {
        self.state = try OfflineOperationValidation.exactToken(
            state,
            field: "error.details.queue.state"
        )
        self.queued = queued
        self.capacity = capacity
        self.saturated = saturated
    }
}

public struct OfflineAxtErrorDetails: Equatable, Sendable {
    public let code: String?
    public let reason: String?
    public let snapshotVersion: UInt64?
    public let dataspace: UInt64?
    public let lane: UInt32?
    public let nextMinHandleEra: UInt64?
    public let nextMinSubNonce: UInt64?

    public init(
        code: String? = nil,
        reason: String? = nil,
        snapshotVersion: UInt64? = nil,
        dataspace: UInt64? = nil,
        lane: UInt32? = nil,
        nextMinHandleEra: UInt64? = nil,
        nextMinSubNonce: UInt64? = nil
    ) throws {
        self.code = try code.map {
            try OfflineOperationValidation.exactText($0, field: "error.details.axt.code")
        }
        self.reason = try reason.map {
            try OfflineOperationValidation.exactText($0, field: "error.details.axt.reason")
        }
        self.snapshotVersion = snapshotVersion
        self.dataspace = dataspace
        self.lane = lane
        self.nextMinHandleEra = nextMinHandleEra
        self.nextMinSubNonce = nextMinSubNonce
    }
}

public struct OfflineOperationErrorDetails: Equatable, Sendable {
    public let layer: String?
    public let rejectCode: String?
    public let queue: OfflineQueueErrorSnapshot?
    public let retryAfterSeconds: UInt64?
    public let endpoint: String?
    public let field: String?
    public let expected: String?
    public let actual: String?
    public let profile: String?
    public let chainDiscriminant: UInt16?
    public let transactionHash: String?
    public let lastStatus: String?
    public let hint: String?
    public let axt: OfflineAxtErrorDetails?

    public init(
        layer: String? = nil,
        rejectCode: String? = nil,
        queue: OfflineQueueErrorSnapshot? = nil,
        retryAfterSeconds: UInt64? = nil,
        endpoint: String? = nil,
        field: String? = nil,
        expected: String? = nil,
        actual: String? = nil,
        profile: String? = nil,
        chainDiscriminant: UInt16? = nil,
        transactionHash: String? = nil,
        lastStatus: String? = nil,
        hint: String? = nil,
        axt: OfflineAxtErrorDetails? = nil
    ) throws {
        self.layer = try Self.exactOptionalText(layer, field: "error.details.layer")
        self.rejectCode = try rejectCode.map {
            try OfflineOperationValidation.exactText($0, field: "error.details.reject_code")
        }
        self.queue = queue
        self.retryAfterSeconds = retryAfterSeconds
        self.endpoint = try Self.exactOptionalText(endpoint, field: "error.details.endpoint")
        self.field = try Self.exactOptionalText(field, field: "error.details.field")
        self.expected = try Self.exactOptionalText(expected, field: "error.details.expected")
        self.actual = try Self.exactOptionalText(actual, field: "error.details.actual")
        self.profile = try Self.exactOptionalText(profile, field: "error.details.profile")
        self.chainDiscriminant = chainDiscriminant
        self.transactionHash = try transactionHash.map {
            try OfflineOperationValidation.transactionHash(
                $0,
                field: "error.details.transaction_hash"
            )
        }
        self.lastStatus = try Self.exactOptionalText(
            lastStatus,
            field: "error.details.last_status"
        )
        self.hint = try Self.exactOptionalText(hint, field: "error.details.hint")
        self.axt = axt
    }

    private static func exactOptionalText(_ value: String?, field: String) throws -> String? {
        try value.map { try OfflineOperationValidation.exactText($0, field: field) }
    }
}

public struct OfflineOperationErrorEnvelope: Equatable, Sendable {
    public let code: String
    public let message: String
    public let details: OfflineOperationErrorDetails?

    public init(
        code: String,
        message: String,
        details: OfflineOperationErrorDetails? = nil
    ) throws {
        self.code = try OfflineOperationValidation.stableCode(code, field: "error.code")
        self.message = try OfflineOperationValidation.exactText(
            message,
            field: "error.message"
        )
        self.details = details
    }
}

/// Pollable state returned by `/v1/offline/operations/{operation_id}`.
public enum OfflineOperationStatus: Equatable, Sendable {
    /// Validated payload for a queued or not-yet-finalized operation.
    public struct Pending: Equatable, Sendable {
        public let operationId: String
        public let kind: OfflineOperationKind
        public let transactionHash: String
        public let submittedAtMs: UInt64

        public init(
            operationId: String,
            kind: OfflineOperationKind,
            transactionHash: String,
            submittedAtMs: UInt64
        ) throws {
            self.operationId = try OfflineOperationValidation.operationId(operationId)
            self.kind = kind
            self.transactionHash = try OfflineOperationValidation.transactionHash(
                transactionHash,
                field: "transaction_hash"
            )
            self.submittedAtMs = submittedAtMs
        }
    }

    /// Validated payload for a finalized operation.
    public struct Applied: Equatable, Sendable {
        public let operationId: String
        public let result: OfflineOperationResult

        public init(operationId: String, result: OfflineOperationResult) throws {
            self.operationId = try OfflineOperationValidation.operationId(operationId)
            self.result = result
        }
    }

    /// Validated payload for a terminally rejected operation.
    public struct Rejected: Equatable, Sendable {
        public let operationId: String
        public let kind: OfflineOperationKind
        public let transactionHash: String
        public let error: OfflineOperationErrorEnvelope

        public init(
            operationId: String,
            kind: OfflineOperationKind,
            transactionHash: String,
            error: OfflineOperationErrorEnvelope
        ) throws {
            self.operationId = try OfflineOperationValidation.operationId(operationId)
            self.kind = kind
            self.transactionHash = try OfflineOperationValidation.transactionHash(
                transactionHash,
                field: "transaction_hash"
            )
            self.error = error
        }
    }

    case pending(Pending)
    case applied(Applied)
    case rejected(Rejected)

    /// Canonical identifier shared by every tagged operation state.
    public var operationId: String {
        switch self {
        case let .pending(value): value.operationId
        case let .applied(value): value.operationId
        case let .rejected(value): value.operationId
        }
    }
}

public enum OfflineOperationCodec {
    private static let referenceSchema =
        "iroha_torii_shared::offline_api::OfflineOperationReference"
    private static let statusSchema =
        "iroha_torii_shared::offline_api::OfflineOperationStatus"

    public static func decodeReference(_ archive: Data) throws -> OfflineOperationReference {
        guard let frame = noritoDecodeFrame(archive),
              frame.header.compression == .none,
              frame.header.schema == noritoSchemaHash(forTypeName: referenceSchema),
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == 0 else {
            throw OfflineOperationError.invalidNoritoArchive
        }
        let compact = true
        var reader = OfflineNoritoReader(data: frame.payload)
        let operationId = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        let kindTag = try readField(&reader, compact: compact) { try $0.readUInt32LE() }
        let stateTag = try readField(&reader, compact: compact) { try $0.readUInt32LE() }
        let transactionHash = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        let statusUri = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        let submittedAtMs = try readField(&reader, compact: compact) { try $0.readUInt64LE() }
        guard reader.remaining() == 0 else {
            throw OfflineOperationError.invalidNoritoArchive
        }
        let kind: OfflineOperationKind
        switch kindTag {
        case 0: kind = .topUp
        case 1: kind = .redeem
        default: throw OfflineOperationError.invalidField("kind")
        }
        guard stateTag == 0 else {
            throw OfflineOperationError.invalidField("state")
        }
        return try OfflineOperationReference(
            operationId: operationId,
            kind: kind,
            state: .pending,
            transactionHash: transactionHash,
            statusUri: statusUri,
            submittedAtMs: submittedAtMs
        )
    }

    public static func encodeReference(_ reference: OfflineOperationReference) -> Data {
        var payload = OfflineCompactNoritoWriter()
        payload.writeField(OfflineCompactNorito.encodeString(reference.operationId))
        payload.writeField(OfflineCompactNorito.encodeUInt32(reference.kind == .topUp ? 0 : 1))
        payload.writeField(OfflineCompactNorito.encodeUInt32(0))
        payload.writeField(OfflineCompactNorito.encodeString(reference.transactionHash))
        payload.writeField(OfflineCompactNorito.encodeString(reference.statusUri))
        var submittedAt = OfflineCompactNoritoWriter()
        submittedAt.writeUInt64LE(reference.submittedAtMs)
        payload.writeField(submittedAt.data)
        return noritoEncode(
            typeName: referenceSchema,
            payload: payload.data,
            flags: NoritoHeader.compactLen
        )
    }

    public static func decodeStatus(_ archive: Data) throws -> OfflineOperationStatus {
        guard let frame = noritoDecodeFrame(archive),
              frame.header.compression == .none,
              frame.header.schema == noritoSchemaHash(forTypeName: statusSchema),
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == 8 else {
            throw OfflineOperationError.invalidNoritoArchive
        }
        var reader = OfflineNoritoReader(data: frame.payload)
        let variant = try reader.readUInt32LE()
        let status: OfflineOperationStatus
        switch variant {
        case 0:
            let operationId = try readOperationIdField(&reader, compact: true)
            let kind = try readKindField(&reader, compact: true)
            let transactionHash = try readExactTextField(
                &reader,
                compact: true,
                field: "transaction_hash"
            )
            let submittedAtMs = try readField(&reader, compact: true) {
                try $0.readUInt64LE()
            }
            status = .pending(try .init(
                operationId: operationId,
                kind: kind,
                transactionHash: transactionHash,
                submittedAtMs: submittedAtMs
            ))
        case 1:
            let operationId = try readOperationIdField(&reader, compact: true)
            let result = try readField(&reader, compact: true) {
                try decodeResult(&$0, compact: true)
            }
            status = .applied(try .init(operationId: operationId, result: result))
        case 2:
            let operationId = try readOperationIdField(&reader, compact: true)
            let kind = try readKindField(&reader, compact: true)
            let transactionHash = try readExactTextField(
                &reader,
                compact: true,
                field: "transaction_hash"
            )
            let error = try readField(&reader, compact: true) {
                try decodeErrorEnvelope(&$0, compact: true)
            }
            status = .rejected(try .init(
                operationId: operationId,
                kind: kind,
                transactionHash: transactionHash,
                error: error
            ))
        default:
            throw OfflineOperationError.invalidField("status")
        }
        guard reader.remaining() == 0 else {
            throw OfflineOperationError.invalidNoritoArchive
        }
        return status
    }

    private static func decodeResult(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> OfflineOperationResult {
        switch try reader.readUInt32LE() {
        case 0:
            return try readField(&reader, compact: compact) {
                .topUp(try decodeTopUpResult(&$0, compact: compact))
            }
        case 1:
            return try readField(&reader, compact: compact) {
                .redeem(try decodeRedeemResult(&$0, compact: compact))
            }
        default:
            throw OfflineOperationError.invalidField("result")
        }
    }

    private static func decodeTopUpResult(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> OfflineTopUpResult {
        let transactionHash = try readExactTextField(
            &reader,
            compact: compact,
            field: "transaction_hash"
        )
        let finalizedBlockHeight = try readField(&reader, compact: compact) {
            try $0.readUInt64LE()
        }
        let serverTimeMs = try readField(&reader, compact: compact) {
            try $0.readUInt64LE()
        }
        let anchorPayload = try readField(&reader, compact: compact) {
            try $0.readBytes($0.remaining())
        }
        let anchorArchive = noritoEncode(
            typeName: KagemushaRecursiveSpend.topUpAnchorWireName,
            payload: anchorPayload,
            flags: NoritoHeader.compactLen
        )
        let anchor = try OfflineTopUpAnchor(noritoArchive: anchorArchive)
        let finalityProofPayload = try readField(&reader, compact: compact) {
            try $0.readBytes($0.remaining())
        }
        let finalityProof = try OfflineTopUpFinalityProof(
            noritoArchive: noritoEncode(
                typeName: KagemushaRecursiveSpend.topUpFinalityProofWireName,
                payload: finalityProofPayload,
                flags: NoritoHeader.compactLen
            )
        )
        return try OfflineTopUpResult(
            transactionHash: transactionHash,
            finalizedBlockHeight: finalizedBlockHeight,
            serverTimeMs: serverTimeMs,
            anchor: anchor,
            finalityProof: finalityProof
        )
    }

    private static func decodeRedeemResult(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> OfflineRedeemResult {
        let transactionHash = try readExactTextField(
            &reader,
            compact: compact,
            field: "transaction_hash"
        )
        let finalizedBlockHeight = try readField(&reader, compact: compact) {
            try $0.readUInt64LE()
        }
        let serverTimeMs = try readField(&reader, compact: compact) {
            try $0.readUInt64LE()
        }
        return try OfflineRedeemResult(
            transactionHash: transactionHash,
            finalizedBlockHeight: finalizedBlockHeight,
            serverTimeMs: serverTimeMs
        )
    }

    private static func decodeErrorEnvelope(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> OfflineOperationErrorEnvelope {
        let code = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        let message = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        let details = try readField(&reader, compact: compact) {
            try decodeOption(&$0, compact: compact) {
                try decodeErrorDetails(&$0, compact: compact)
            }
        }
        return try OfflineOperationErrorEnvelope(code: code, message: message, details: details)
    }

    private static func decodeErrorDetails(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> OfflineOperationErrorDetails {
        let layer = try readOptionalStringField(&reader, compact: compact)
        let rejectCode = try readOptionalStringField(&reader, compact: compact)
        let queue = try readField(&reader, compact: compact) {
            try decodeOption(&$0, compact: compact) {
                try decodeQueueSnapshot(&$0, compact: compact)
            }
        }
        let retryAfterSeconds = try readOptionalScalarField(
            &reader,
            compact: compact,
            decode: { try $0.readUInt64LE() }
        )
        let endpoint = try readOptionalStringField(&reader, compact: compact)
        let field = try readOptionalStringField(&reader, compact: compact)
        let expected = try readOptionalStringField(&reader, compact: compact)
        let actual = try readOptionalStringField(&reader, compact: compact)
        let profile = try readOptionalStringField(&reader, compact: compact)
        let chainDiscriminant = try readOptionalScalarField(
            &reader,
            compact: compact,
            decode: { try $0.readUInt16LE() }
        )
        let transactionHash = try readOptionalStringField(&reader, compact: compact)
        let lastStatus = try readOptionalStringField(&reader, compact: compact)
        let hint = try readOptionalStringField(&reader, compact: compact)
        let axt = try readField(&reader, compact: compact) {
            try decodeOption(&$0, compact: compact) {
                try decodeAxtDetails(&$0, compact: compact)
            }
        }
        return try OfflineOperationErrorDetails(
            layer: layer,
            rejectCode: rejectCode,
            queue: queue,
            retryAfterSeconds: retryAfterSeconds,
            endpoint: endpoint,
            field: field,
            expected: expected,
            actual: actual,
            profile: profile,
            chainDiscriminant: chainDiscriminant,
            transactionHash: transactionHash,
            lastStatus: lastStatus,
            hint: hint,
            axt: axt
        )
    }

    private static func decodeQueueSnapshot(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> OfflineQueueErrorSnapshot {
        let state = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        let queued = try readField(&reader, compact: compact) { try $0.readUInt64LE() }
        let capacity = try readField(&reader, compact: compact) { try $0.readUInt64LE() }
        let saturated = try readField(&reader, compact: compact) {
            switch try $0.readUInt8() {
            case 0: return false
            case 1: return true
            default: throw OfflineOperationError.invalidField("queue.saturated")
            }
        }
        return try OfflineQueueErrorSnapshot(
            state: state,
            queued: queued,
            capacity: capacity,
            saturated: saturated
        )
    }

    private static func decodeAxtDetails(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> OfflineAxtErrorDetails {
        let code = try readOptionalStringField(&reader, compact: compact)
        let reason = try readOptionalStringField(&reader, compact: compact)
        let snapshotVersion = try readOptionalScalarField(
            &reader,
            compact: compact,
            decode: { try $0.readUInt64LE() }
        )
        let dataspace = try readOptionalScalarField(
            &reader,
            compact: compact,
            decode: { try $0.readUInt64LE() }
        )
        let lane = try readOptionalScalarField(
            &reader,
            compact: compact,
            decode: { try $0.readUInt32LE() }
        )
        let nextMinHandleEra = try readOptionalScalarField(
            &reader,
            compact: compact,
            decode: { try $0.readUInt64LE() }
        )
        let nextMinSubNonce = try readOptionalScalarField(
            &reader,
            compact: compact,
            decode: { try $0.readUInt64LE() }
        )
        return try OfflineAxtErrorDetails(
            code: code,
            reason: reason,
            snapshotVersion: snapshotVersion,
            dataspace: dataspace,
            lane: lane,
            nextMinHandleEra: nextMinHandleEra,
            nextMinSubNonce: nextMinSubNonce
        )
    }

    private static func readOperationIdField(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> String {
        let value = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        return try OfflineOperationValidation.operationId(value)
    }

    private static func readKindField(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> OfflineOperationKind {
        let tag = try readField(&reader, compact: compact) { try $0.readUInt32LE() }
        switch tag {
        case 0: return .topUp
        case 1: return .redeem
        default: throw OfflineOperationError.invalidField("kind")
        }
    }

    private static func readExactTextField(
        _ reader: inout OfflineNoritoReader,
        compact: Bool,
        field: String
    ) throws -> String {
        let value = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        return try OfflineOperationValidation.exactText(value, field: field)
    }

    private static func readOptionalStringField(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> String? {
        try readField(&reader, compact: compact) {
            try decodeOption(&$0, compact: compact) {
                try readString(&$0, compact: compact)
            }
        }
    }

    private static func readOptionalScalarField<T>(
        _ reader: inout OfflineNoritoReader,
        compact: Bool,
        decode: (inout OfflineNoritoReader) throws -> T
    ) throws -> T? {
        try readField(&reader, compact: compact) {
            try decodeOption(&$0, compact: compact, decode: decode)
        }
    }

    private static func decodeOption<T>(
        _ reader: inout OfflineNoritoReader,
        compact: Bool,
        decode: (inout OfflineNoritoReader) throws -> T
    ) throws -> T? {
        switch try reader.readUInt8() {
        case 0:
            guard reader.remaining() == 0 else {
                throw OfflineOperationError.invalidNoritoArchive
            }
            return nil
        case 1:
            let payload = compact ? try reader.readCompactField() : try reader.readField()
            var child = OfflineNoritoReader(data: payload)
            let value = try decode(&child)
            guard child.remaining() == 0, reader.remaining() == 0 else {
                throw OfflineOperationError.invalidNoritoArchive
            }
            return value
        default:
            throw OfflineOperationError.invalidField("option")
        }
    }

    private static func readField<T>(
        _ reader: inout OfflineNoritoReader,
        compact: Bool,
        _ decode: (inout OfflineNoritoReader) throws -> T
    ) throws -> T {
        let bytes = compact ? try reader.readCompactField() : try reader.readField()
        var child = OfflineNoritoReader(data: bytes)
        let value = try decode(&child)
        guard child.remaining() == 0 else {
            throw OfflineOperationError.invalidNoritoArchive
        }
        return value
    }

    private static func readString(
        _ reader: inout OfflineNoritoReader,
        compact: Bool
    ) throws -> String {
        let length = compact ? try reader.readVarint() : try reader.readUInt64LE()
        guard length <= UInt64(Int.max),
              let value = String(
                data: try reader.readBytes(Int(length)),
                encoding: .utf8
              ) else {
            throw OfflineOperationError.invalidField("string")
        }
        return value
    }
}

private enum OfflineOperationValidation {
    static func positive(_ value: UInt64, field: String) throws -> UInt64 {
        guard value > 0 else {
            throw OfflineOperationError.invalidField(field)
        }
        return value
    }

    static func operationId(_ value: String) throws -> String {
        let bytes = Array(value.utf8)
        guard bytes.count == 64,
              bytes.contains(where: { $0 != UInt8(ascii: "0") }),
              bytes.allSatisfy({
                  ($0 >= UInt8(ascii: "0") && $0 <= UInt8(ascii: "9"))
                      || ($0 >= UInt8(ascii: "a") && $0 <= UInt8(ascii: "f"))
              }) else {
            throw OfflineOperationError.invalidField("operation_id")
        }
        return value
    }

    static func transactionHash(_ value: String, field: String) throws -> String {
        let bytes = Array(value.utf8)
        guard bytes.count == 64,
              bytes.allSatisfy({
                  ($0 >= UInt8(ascii: "0") && $0 <= UInt8(ascii: "9"))
                      || ($0 >= UInt8(ascii: "a") && $0 <= UInt8(ascii: "f"))
              }) else {
            throw OfflineOperationError.invalidField(field)
        }
        return value
    }

    static func statusUri(_ value: String, operationId: String) throws -> String {
        let expected = "\(OfflineAPI.Endpoint.operations.path)/\(operationId)"
        guard value == expected else {
            throw OfflineOperationError.invalidField("status_uri")
        }
        return value
    }

    static func stableCode(_ value: String, field: String) throws -> String {
        let bytes = Array(value.utf8)
        guard (1...64).contains(bytes.count),
              let first = bytes.first,
              isLowercaseLetter(first) || isDigit(first),
              bytes.allSatisfy({
                  isLowercaseLetter($0) || isDigit($0) || $0 == UInt8(ascii: "_")
              }) else {
            throw OfflineOperationError.invalidField(field)
        }
        return value
    }

    static func exactText(_ value: String, field: String) throws -> String {
        guard !value.isEmpty,
              value.trimmingCharacters(in: .whitespacesAndNewlines) == value,
              !value.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains)
        else {
            throw OfflineOperationError.invalidField(field)
        }
        return value
    }

    static func exactToken(_ value: String, field: String) throws -> String {
        let exact = try exactText(value, field: field)
        guard !exact.unicodeScalars.contains(where: CharacterSet.whitespacesAndNewlines.contains)
        else {
            throw OfflineOperationError.invalidField(field)
        }
        return exact
    }

    private static func isDigit(_ byte: UInt8) -> Bool {
        byte >= UInt8(ascii: "0") && byte <= UInt8(ascii: "9")
    }

    private static func isLowercaseLetter(_ byte: UInt8) -> Bool {
        byte >= UInt8(ascii: "a") && byte <= UInt8(ascii: "z")
    }

    static func requestArchive(
        _ value: Data,
        schema: String,
        operationIdFieldIndex: Int,
        fieldCount: Int
    ) throws -> (archive: Data, operationId: String) {
        guard !value.isEmpty,
              value.count <= KagemushaRecursiveSpend.artifactMaximumFileBytes,
              let frame = noritoDecodeFrame(value),
              frame.header.schema == noritoSchemaHash(forTypeName: schema),
              frame.header.compression == .none,
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == 0,
              !frame.payload.isEmpty,
              operationIdFieldIndex >= 0,
              operationIdFieldIndex < fieldCount else {
            throw OfflineOperationError.invalidNoritoArchive
        }

        var reader = OfflineNoritoReader(data: frame.payload)
        var fields = [Data]()
        fields.reserveCapacity(fieldCount)
        do {
            for _ in 0..<fieldCount {
                fields.append(try reader.readCompactField())
            }
        } catch {
            throw OfflineOperationError.invalidNoritoArchive
        }
        guard reader.remaining() == 0 else {
            throw OfflineOperationError.invalidNoritoArchive
        }

        var canonicalPayload = OfflineCompactNoritoWriter()
        for field in fields {
            canonicalPayload.writeField(field)
        }
        guard noritoEncode(
            typeName: schema,
            payload: canonicalPayload.data,
            flags: NoritoHeader.compactLen
        ) == value else {
            throw OfflineOperationError.invalidNoritoArchive
        }

        let operationId = fields[operationIdFieldIndex]
        guard operationId.count == 32,
              operationId.contains(where: { $0 != 0 }) else {
            throw OfflineOperationError.invalidField("operation_id")
        }
        return (Data(value), operationId.hexEncodedString())
    }
}

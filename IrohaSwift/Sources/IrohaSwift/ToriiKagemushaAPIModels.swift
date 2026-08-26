import Foundation

/// Canonical first-release Kagemusha routes on Torii's `/v1/offline` wire namespace.
package enum KagemushaToriiAPI {
    public enum Endpoint: String, Sendable {
        case readiness = "/v1/offline/readiness"
        case topUp = "/v1/offline/top-up"
        case redeem = "/v1/offline/redeem"
        case operations = "/v1/offline/operations"

        public var path: String { rawValue }
    }

    public static func operationPath(_ operationId: String) throws -> String {
        "\(Endpoint.operations.path)/\(try KagemushaOperationValidation.operationId(operationId))"
    }
}

/// Native-verified receiver registration lineage carried inside a portable receive offer.
///
/// Construction is restricted to native receive-offer verification of the request digest,
/// registration tuple/lifetime, admitting transaction and Merkle paths, policy, header, and
/// historical V2 finality certificate.
package struct KagemushaRecipientRegistrationLineage: Equatable, Sendable {
    public static let maximumArchiveBytes = 4 * 1024 * 1024
    public let noritoArchive: Data

    init(verifiedArchive: Data) throws {
        guard !verifiedArchive.isEmpty,
              verifiedArchive.count <= Self.maximumArchiveBytes else {
            throw ToriiClientError.invalidPayload(
                "Kagemusha receiver lineage exceeds its canonical response bound"
            )
        }
        self.noritoArchive = Data(verifiedArchive)
    }
}

/// App-owned durable checkpoint supplied to native receive-offer verification.
/// `contextID` is the exact marked Iroha HeightContextId.
package struct KagemushaFinalityCheckpointV2: Equatable, Sendable {
    public let height: UInt64
    public let contextID: Data

    public init(height: UInt64, contextID: Data) throws {
        guard (1...UInt64(Int64.max)).contains(height),
              contextID.count == 32,
              contextID.contains(where: { $0 != 0 }),
              contextID.last.map({ ($0 & 1) == 1 }) == true else {
            throw KagemushaRecursiveSpendError.invalidField("finalityCheckpoint")
        }
        self.height = height
        self.contextID = Data(contextID)
    }

    init(promotedCheckpointBytes: Data) throws {
        guard promotedCheckpointBytes.count == 40 else {
            throw KagemushaRecursiveSpendError.invalidArchive("promotedCheckpoint")
        }
        let height = promotedCheckpointBytes.prefix(8).reduce(UInt64(0)) {
            ($0 << 8) | UInt64($1)
        }
        try self.init(height: height, contextID: Data(promotedCheckpointBytes.suffix(32)))
    }
}

package struct KagemushaProjectedRecipientReceiveOfferV2: Equatable, Sendable {
    public let request: KagemushaRecipientPaymentRequest
    public let lineageArchive: Data
    public let publisherCheckpointEnvelope: Data
}

package struct KagemushaVerifiedRecipientReceiveOfferV2: Equatable, Sendable {
    public let request: KagemushaRecipientPaymentRequest
    public let lineage: KagemushaRecipientRegistrationLineage
    public let publisherCheckpointEnvelope: Data
    public let promotedCheckpoint: KagemushaFinalityCheckpointV2
}

/// Canonical portable receive offer carried as the Kagemusha IPM1
/// RECEIVE_REQUEST body. Native projection is the only decoder, so Swift never
/// guesses the Rust `Archived<T>` alignment or header padding.
package struct KagemushaRecipientReceiveOfferV2: Equatable, Sendable {
    public static let maximumArchiveBytes = 24_576
    public static let maximumPublisherEnvelopeBytes = 2_048
    public let noritoArchive: Data
    private let chainDiscriminant: UInt16

    public init(
        request: KagemushaRecipientPaymentRequest,
        lineageArchive: Data,
        publisherCheckpointEnvelope: Data,
        chainDiscriminant: UInt16
    ) throws {
        _ = try KagemushaRecursiveSpend.canonicalAccountAddress(
            request.payload.recipient,
            field: "recipientReceiveOffer.request.recipient",
            expectedChainDiscriminant: chainDiscriminant
        )
        guard !lineageArchive.isEmpty,
              lineageArchive.count <= KagemushaRecipientRegistrationLineage.maximumArchiveBytes,
              (1...Self.maximumPublisherEnvelopeBytes)
                .contains(publisherCheckpointEnvelope.count),
              let archive = try NoritoNativeBridge.shared
                .kagemushaRecipientReceiveOfferCreateV2(
                    requestArchive: request.archive,
                    lineageArchive: lineageArchive,
                    publisherCheckpointEnvelope: publisherCheckpointEnvelope
                ) else {
            if lineageArchive.isEmpty
                || lineageArchive.count > KagemushaRecipientRegistrationLineage.maximumArchiveBytes
                || !(1...Self.maximumPublisherEnvelopeBytes)
                    .contains(publisherCheckpointEnvelope.count) {
                throw KagemushaRecursiveSpendError.invalidField("recipientReceiveOffer")
            }
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        try self.init(
            validatingNativeArchive: archive,
            chainDiscriminant: chainDiscriminant
        )
    }

    public init(
        noritoArchive: Data,
        chainDiscriminant: UInt16
    ) throws {
        try self.init(
            validatingNativeArchive: noritoArchive,
            chainDiscriminant: chainDiscriminant
        )
    }

    public func project(
        chainDiscriminant: UInt16
    ) throws -> KagemushaProjectedRecipientReceiveOfferV2 {
        guard chainDiscriminant == self.chainDiscriminant else {
            throw KagemushaRecursiveSpendError.invalidField(
                "recipientReceiveOffer.chainDiscriminant"
            )
        }
        guard let result = try NoritoNativeBridge.shared
            .kagemushaRecipientReceiveOfferProjectV2(offerArchive: noritoArchive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return KagemushaProjectedRecipientReceiveOfferV2(
            request: try KagemushaRecursiveSpendCodecs.decodeRecipientRequest(
                result.request,
                chainDiscriminant: chainDiscriminant
            ),
            lineageArchive: result.lineage,
            publisherCheckpointEnvelope: result.publisherEnvelope
        )
    }

    /// Verify the exact offer after app policy authenticated the projected
    /// publisher envelope and selected the corresponding durable checkpoint.
    public func verify(
        chainDiscriminant: UInt16,
        atMilliseconds: UInt64,
        trustedCheckpoint: KagemushaFinalityCheckpointV2,
        authenticatedPublisherCheckpointEnvelope: Data
    ) throws -> KagemushaVerifiedRecipientReceiveOfferV2 {
        guard (1...UInt64(Int64.max)).contains(atMilliseconds),
              (1...Self.maximumPublisherEnvelopeBytes)
                .contains(authenticatedPublisherCheckpointEnvelope.count) else {
            throw KagemushaRecursiveSpendError.invalidField("recipientReceiveOffer.verify")
        }
        let projected = try project(chainDiscriminant: chainDiscriminant)
        guard projected.publisherCheckpointEnvelope
                == authenticatedPublisherCheckpointEnvelope else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "recipientReceiveOffer.publisherEnvelope"
            )
        }
        guard let result = try NoritoNativeBridge.shared
            .kagemushaRecipientReceiveOfferVerifyV2(
                offerArchive: noritoArchive,
                verifiedAtMilliseconds: atMilliseconds,
                trustedCheckpointHeight: trustedCheckpoint.height,
                trustedCheckpointContextID: trustedCheckpoint.contextID
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        guard result.request == projected.request.archive,
              result.lineage == projected.lineageArchive,
              result.publisherEnvelope == authenticatedPublisherCheckpointEnvelope else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "recipientReceiveOffer.nativeProjection"
            )
        }
        return KagemushaVerifiedRecipientReceiveOfferV2(
            request: projected.request,
            lineage: try KagemushaRecipientRegistrationLineage(
                verifiedArchive: result.lineage
            ),
            publisherCheckpointEnvelope: result.publisherEnvelope,
            promotedCheckpoint: try KagemushaFinalityCheckpointV2(
                promotedCheckpointBytes: result.promotedCheckpoint
            )
        )
    }

    private init(
        validatingNativeArchive archive: Data,
        chainDiscriminant: UInt16
    ) throws {
        guard !archive.isEmpty, archive.count <= Self.maximumArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("recipientReceiveOffer")
        }
        guard let result = try NoritoNativeBridge.shared
            .kagemushaRecipientReceiveOfferProjectV2(offerArchive: archive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        let request = try KagemushaRecursiveSpendCodecs.decodeRecipientRequest(
            result.request,
            chainDiscriminant: chainDiscriminant
        )
        guard request.archive == result.request,
              !result.lineage.isEmpty,
              result.lineage.count
                <= KagemushaRecipientRegistrationLineage.maximumArchiveBytes,
              (1...Self.maximumPublisherEnvelopeBytes)
                .contains(result.publisherEnvelope.count) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "recipientReceiveOffer.nativeProjection"
            )
        }
        self.noritoArchive = Data(archive)
        self.chainDiscriminant = chainDiscriminant
    }
}

package enum KagemushaOperationKind: String, Codable, Equatable, Sendable {
    case topUp = "top_up"
    case redeem
}

package enum KagemushaOperationState: String, Codable, Equatable, Sendable {
    case pending
}

package enum KagemushaOperationError: Error, LocalizedError, Equatable, Sendable {
    case invalidField(String)
    case invalidNoritoArchive

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid Kagemusha operation field: \(field)."
        case .invalidNoritoArchive:
            return "Kagemusha operation request must be a canonical Norito archive."
        }
    }
}

/// A schema-bound Kagemusha top-up command submitted directly to Torii.
package struct KagemushaTopUpRequest: Equatable, Sendable {
    /// Exact bridge ABI-22 / Kagemusha V4 top-up archive ceiling enforced by Torii.
    public static let maximumArchiveBytes = 512 * 1_024
    /// Lowercase hex derived from the archive's nonzero 32-byte operation ID.
    public let operationId: String
    private let archive: Data

    /// Validates and retains a canonical first-release Kagemusha top-up request archive.
    public init(noritoArchive: Data) throws {
        let validated = try KagemushaOperationValidation.requestArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            operationIdFieldIndex: 6,
            fieldCount: 8,
            expectedWireVersion: KagemushaRecursiveSpend.wireVersionV4,
            maximumArchiveBytes: Self.maximumArchiveBytes
        )
        self.operationId = validated.operationId
        self.archive = validated.archive
    }

    public func noritoArchive() -> Data { archive }
}

/// A schema-bound Kagemusha redemption command submitted directly to Torii.
package struct KagemushaRedeemRequest: Equatable, Sendable {
    /// Exact bridge ABI-22 / Kagemusha V4 redemption archive ceiling enforced by Torii.
    public static let maximumArchiveBytes = 48 * 1_024 * 1_024
    /// Lowercase hex derived from the archive's nonzero 32-byte operation ID.
    public let operationId: String
    private let archive: Data

    /// Validates and retains a canonical first-release Kagemusha redemption request archive.
    public init(noritoArchive: Data) throws {
        let validated = try KagemushaOperationValidation.requestArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.redeemRequestWireName,
            operationIdFieldIndex: 8,
            fieldCount: 10,
            expectedWireVersion: KagemushaRecursiveSpend.wireVersionV4,
            maximumArchiveBytes: Self.maximumArchiveBytes
        )
        self.operationId = validated.operationId
        self.archive = validated.archive
    }

    public func noritoArchive() -> Data { archive }
}

package struct KagemushaOperationReference: Codable, Equatable, Sendable {
    public let operationId: String
    public let kind: KagemushaOperationKind
    public let state: KagemushaOperationState
    public let transactionHash: String
    public let statusUri: String
    public let submittedAtMs: UInt64

    public init(
        operationId: String,
        kind: KagemushaOperationKind,
        state: KagemushaOperationState,
        transactionHash: String,
        statusUri: String,
        submittedAtMs: UInt64
    ) throws {
        let validatedOperationId = try KagemushaOperationValidation.operationId(operationId)
        self.operationId = validatedOperationId
        self.kind = kind
        self.state = state
        self.transactionHash = try KagemushaOperationValidation.transactionHash(
            transactionHash,
            field: "transaction_hash"
        )
        self.statusUri = try KagemushaOperationValidation.statusUri(
            statusUri,
            operationId: validatedOperationId
        )
        self.submittedAtMs = try KagemushaOperationValidation.positive(
            submittedAtMs,
            field: "submitted_at_ms"
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationId: container.decode(String.self, forKey: .operationId),
            kind: container.decode(KagemushaOperationKind.self, forKey: .kind),
            state: container.decode(KagemushaOperationState.self, forKey: .state),
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

/// Canonical finalized top-up anchor consumed by the Kagemusha wallet prover.
///
/// The public API intentionally exposes the current semantic name while the
/// versioned consensus wire type remains an internal codec detail.
package struct KagemushaTopUpAnchor: Equatable, Sendable {
    private let archive: Data
    private let anchorDigest: Data
    /// Exact genesis-derived network identity authenticated by the anchor.
    public let networkId: NetworkId
    /// Operation identity authenticated by the finalized anchor.
    public let operationId: String
    /// Transaction hash authenticated by the finalized anchor.
    public let finalizedTransactionHash: String
    /// Block height authenticated by the finalized anchor.
    public let finalizedBlockHeight: UInt64

    /// Validates and retains a canonical top-up anchor Norito archive.
    public init(
        noritoArchive: Data,
        chainDiscriminant: UInt16
    ) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityAnchorMaximumArchiveBytes else {
            throw KagemushaOperationError.invalidNoritoArchive
        }
        let wireValue = try KagemushaRecursiveSpendCodecs.decodeTopUpAnchorV4(
            Data(noritoArchive),
            chainDiscriminant: chainDiscriminant
        )
        self.archive = Data(wireValue.noritoArchive)
        self.anchorDigest = Data(wireValue.anchorDigest)
        self.networkId = wireValue.networkID
        self.operationId = wireValue.topUpOperationID.hexLowercased()
        self.finalizedTransactionHash = wireValue.finalizedTransactionHash.hexLowercased()
        self.finalizedBlockHeight = wireValue.finalizedHeight
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
package typealias KagemushaTopUpFinalityProof = KagemushaTopUpFinalityProofArchive

package struct KagemushaTopUpResult: Equatable, Sendable {
    public let transactionHash: String
    public let finalizedBlockHeight: UInt64
    public let serverTimeMs: UInt64
    public let anchor: KagemushaTopUpAnchor
    public let finalityProof: KagemushaTopUpFinalityProof

    public init(
        transactionHash: String,
        finalizedBlockHeight: UInt64,
        serverTimeMs: UInt64,
        anchor: KagemushaTopUpAnchor,
        finalityProof: KagemushaTopUpFinalityProof
    ) throws {
        self.transactionHash = try KagemushaOperationValidation.transactionHash(
            transactionHash,
            field: "transaction_hash"
        )
        self.finalizedBlockHeight = try KagemushaOperationValidation.positive(
            finalizedBlockHeight,
            field: "finalized_block_height"
        )
        self.serverTimeMs = try KagemushaOperationValidation.positive(
            serverTimeMs,
            field: "server_time_ms"
        )
        guard self.transactionHash == anchor.finalizedTransactionHash,
              self.finalizedBlockHeight == anchor.finalizedBlockHeight else {
            throw KagemushaOperationError.invalidField("top_up_result.anchor_binding")
        }
        self.anchor = anchor
        self.finalityProof = finalityProof
    }
}

package struct KagemushaRedeemResult: Equatable, Sendable {
    public let transactionHash: String
    public let finalizedBlockHeight: UInt64
    public let serverTimeMs: UInt64

    public init(
        transactionHash: String,
        finalizedBlockHeight: UInt64,
        serverTimeMs: UInt64
    ) throws {
        self.transactionHash = try KagemushaOperationValidation.transactionHash(
            transactionHash,
            field: "transaction_hash"
        )
        self.finalizedBlockHeight = try KagemushaOperationValidation.positive(
            finalizedBlockHeight,
            field: "finalized_block_height"
        )
        self.serverTimeMs = try KagemushaOperationValidation.positive(
            serverTimeMs,
            field: "server_time_ms"
        )
    }
}

package enum KagemushaOperationResult: Equatable, Sendable {
    case topUp(KagemushaTopUpResult)
    case redeem(KagemushaRedeemResult)
}

/// Exact terminal rejection carried by the first-release offline operation status.
///
/// Torii's generic error envelope may expose structured details on unrelated
/// routes. The Offline Cash V1 status contract deliberately narrows that wire
/// type to one stable code, one bounded message, and an absent details field.
package struct KagemushaOperationErrorEnvelope: Equatable, Sendable {
    public static let rejectionCode = "offline_operation_rejected"
    public static let maximumMessageUnicodeScalars = 1_024

    public let code: String
    public let message: String

    public init(code: String, message: String) throws {
        guard code == Self.rejectionCode else {
            throw KagemushaOperationError.invalidField("error.code")
        }
        self.code = code
        self.message = try KagemushaOperationValidation.rejectionMessage(
            message,
            field: "error.message"
        )
    }
}

/// Pollable state returned by `/v1/offline/operations/{operation_id}`.
package enum KagemushaOperationStatus: Equatable, Sendable {
    /// Validated payload for a queued or not-yet-finalized operation.
    public struct Pending: Equatable, Sendable {
        public let operationId: String
        public let kind: KagemushaOperationKind
        public let transactionHash: String
        public let submittedAtMs: UInt64

        public init(
            operationId: String,
            kind: KagemushaOperationKind,
            transactionHash: String,
            submittedAtMs: UInt64
        ) throws {
            self.operationId = try KagemushaOperationValidation.operationId(operationId)
            self.kind = kind
            self.transactionHash = try KagemushaOperationValidation.transactionHash(
                transactionHash,
                field: "transaction_hash"
            )
            self.submittedAtMs = try KagemushaOperationValidation.positive(
                submittedAtMs,
                field: "submitted_at_ms"
            )
        }
    }

    /// Validated payload for a finalized operation.
    public struct Applied: Equatable, Sendable {
        public let operationId: String
        public let result: KagemushaOperationResult

        public init(operationId: String, result: KagemushaOperationResult) throws {
            self.operationId = try KagemushaOperationValidation.operationId(operationId)
            if case .topUp(let topUp) = result,
               self.operationId != topUp.anchor.operationId {
                throw KagemushaOperationError.invalidField("operation_id.anchor_binding")
            }
            self.result = result
        }
    }

    /// Validated payload for a terminally rejected operation.
    public struct Rejected: Equatable, Sendable {
        public let operationId: String
        public let kind: KagemushaOperationKind
        public let transactionHash: String
        public let error: KagemushaOperationErrorEnvelope

        public init(
            operationId: String,
            kind: KagemushaOperationKind,
            transactionHash: String,
            error: KagemushaOperationErrorEnvelope
        ) throws {
            self.operationId = try KagemushaOperationValidation.operationId(operationId)
            self.kind = kind
            self.transactionHash = try KagemushaOperationValidation.transactionHash(
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

package enum KagemushaOperationCodec {
    /// A reference contains only bounded identifiers, tags, a status URI, and
    /// a timestamp. Reject oversized input before Norito frame parsing.
    public static let referenceMaximumArchiveBytes = 4 * 1_024
    /// Applied top-up status may contain the 2 MiB finality proof plus its
    /// anchor and framing. Three MiB is a tight first-release wire ceiling.
    public static let statusMaximumArchiveBytes = 3 * 1_024 * 1_024
    /// Upper bound for every individual textual field decoded by this codec.
    public static let maximumTextFieldUTF8Bytes = 64 * 1_024

    private static let referenceSchema =
        "iroha_torii_shared::offline_api::OfflineOperationReference"
    private static let statusSchema =
        "iroha_torii_shared::offline_api::OfflineOperationStatus"

    public static func decodeReference(_ archive: Data) throws -> KagemushaOperationReference {
        guard !archive.isEmpty,
              archive.count <= referenceMaximumArchiveBytes,
              let frame = noritoDecodeFrame(archive),
              frame.header.compression == .none,
              frame.header.schema == noritoSchemaHash(forTypeName: referenceSchema),
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == 0 else {
            throw KagemushaOperationError.invalidNoritoArchive
        }
        let compact = true
        var reader = CanonicalNoritoReader(data: frame.payload)
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
            throw KagemushaOperationError.invalidNoritoArchive
        }
        let kind: KagemushaOperationKind
        switch kindTag {
        case 0: kind = .topUp
        case 1: kind = .redeem
        default: throw KagemushaOperationError.invalidField("kind")
        }
        guard stateTag == 0 else {
            throw KagemushaOperationError.invalidField("state")
        }
        return try KagemushaOperationReference(
            operationId: operationId,
            kind: kind,
            state: .pending,
            transactionHash: transactionHash,
            statusUri: statusUri,
            submittedAtMs: submittedAtMs
        )
    }

    public static func encodeReference(_ reference: KagemushaOperationReference) -> Data {
        var payload = CompactNoritoWriter()
        payload.writeField(CompactNorito.encodeString(reference.operationId))
        payload.writeField(CompactNorito.encodeUInt32(reference.kind == .topUp ? 0 : 1))
        payload.writeField(CompactNorito.encodeUInt32(0))
        payload.writeField(CompactNorito.encodeString(reference.transactionHash))
        payload.writeField(CompactNorito.encodeString(reference.statusUri))
        var submittedAt = CompactNoritoWriter()
        submittedAt.writeUInt64LE(reference.submittedAtMs)
        payload.writeField(submittedAt.data)
        return noritoEncode(
            typeName: referenceSchema,
            payload: payload.data,
            flags: NoritoHeader.compactLen
        )
    }

    public static func decodeStatus(
        _ archive: Data,
        chainDiscriminant: UInt16
    ) throws -> KagemushaOperationStatus {
        guard !archive.isEmpty,
              archive.count <= statusMaximumArchiveBytes,
              let frame = noritoDecodeFrame(archive),
              frame.header.compression == .none,
              frame.header.schema == noritoSchemaHash(forTypeName: statusSchema),
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == 8 else {
            throw KagemushaOperationError.invalidNoritoArchive
        }
        var reader = CanonicalNoritoReader(data: frame.payload)
        let variant = try reader.readUInt32LE()
        let status: KagemushaOperationStatus
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
                try decodeResult(
                    &$0,
                    compact: true,
                    chainDiscriminant: chainDiscriminant
                )
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
            throw KagemushaOperationError.invalidField("status")
        }
        guard reader.remaining() == 0 else {
            throw KagemushaOperationError.invalidNoritoArchive
        }
        return status
    }

    private static func decodeResult(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool,
        chainDiscriminant: UInt16
    ) throws -> KagemushaOperationResult {
        switch try reader.readUInt32LE() {
        case 0:
            return try readField(&reader, compact: compact) {
                .topUp(try decodeTopUpResult(
                    &$0,
                    compact: compact,
                    chainDiscriminant: chainDiscriminant
                ))
            }
        case 1:
            return try readField(&reader, compact: compact) {
                .redeem(try decodeRedeemResult(&$0, compact: compact))
            }
        default:
            throw KagemushaOperationError.invalidField("result")
        }
    }

    private static func decodeTopUpResult(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool,
        chainDiscriminant: UInt16
    ) throws -> KagemushaTopUpResult {
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
        let anchorArchive = KagemushaRecursiveSpend.frameArchive(
            schema: KagemushaRecursiveSpend.topUpAnchorWireNameV4,
            payload: anchorPayload
        )
        let anchor = try KagemushaTopUpAnchor(
            noritoArchive: anchorArchive,
            chainDiscriminant: chainDiscriminant
        )
        let finalityProofPayload = try readField(&reader, compact: compact) {
            try $0.readBytes($0.remaining())
        }
        let finalityProof = try KagemushaTopUpFinalityProof(
            noritoArchive: KagemushaRecursiveSpend.frameArchive(
                schema: KagemushaRecursiveSpend.topUpFinalityProofWireName,
                payload: finalityProofPayload
            )
        )
        return try KagemushaTopUpResult(
            transactionHash: transactionHash,
            finalizedBlockHeight: finalizedBlockHeight,
            serverTimeMs: serverTimeMs,
            anchor: anchor,
            finalityProof: finalityProof
        )
    }

    private static func decodeRedeemResult(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> KagemushaRedeemResult {
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
        return try KagemushaRedeemResult(
            transactionHash: transactionHash,
            finalizedBlockHeight: finalizedBlockHeight,
            serverTimeMs: serverTimeMs
        )
    }

    private static func decodeErrorEnvelope(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> KagemushaOperationErrorEnvelope {
        let code = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        let message = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        return try KagemushaOperationErrorEnvelope(code: code, message: message)
    }

    private static func readOperationIdField(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> String {
        let value = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        return try KagemushaOperationValidation.operationId(value)
    }

    private static func readKindField(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> KagemushaOperationKind {
        let tag = try readField(&reader, compact: compact) { try $0.readUInt32LE() }
        switch tag {
        case 0: return .topUp
        case 1: return .redeem
        default: throw KagemushaOperationError.invalidField("kind")
        }
    }

    private static func readExactTextField(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool,
        field: String
    ) throws -> String {
        let value = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        return try KagemushaOperationValidation.exactText(value, field: field)
    }

    private static func readField<T>(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool,
        _ decode: (inout CanonicalNoritoReader) throws -> T
    ) throws -> T {
        let bytes = compact ? try reader.readCompactField() : try reader.readField()
        var child = CanonicalNoritoReader(data: bytes)
        let value = try decode(&child)
        guard child.remaining() == 0 else {
            throw KagemushaOperationError.invalidNoritoArchive
        }
        return value
    }

    private static func readString(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> String {
        let length = compact ? try reader.readVarint() : try reader.readUInt64LE()
        guard length <= UInt64(maximumTextFieldUTF8Bytes),
              let value = String(
                data: try reader.readBytes(Int(length)),
                encoding: .utf8
              ) else {
            throw KagemushaOperationError.invalidField("string")
        }
        return value
    }
}

private enum KagemushaOperationValidation {
    static func positive(_ value: UInt64, field: String) throws -> UInt64 {
        guard value > 0 else {
            throw KagemushaOperationError.invalidField(field)
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
            throw KagemushaOperationError.invalidField("operation_id")
        }
        return value
    }

    static func transactionHash(_ value: String, field: String) throws -> String {
        guard CanonicalIrohaHashText.decode(value) != nil else {
            throw KagemushaOperationError.invalidField(field)
        }
        return value
    }

    static func statusUri(_ value: String, operationId: String) throws -> String {
        let expected = "\(KagemushaToriiAPI.Endpoint.operations.path)/\(operationId)"
        guard value == expected else {
            throw KagemushaOperationError.invalidField("status_uri")
        }
        return value
    }

    static func exactText(_ value: String, field: String) throws -> String {
        guard !value.isEmpty,
              value.utf8.count
                <= KagemushaOperationCodec.maximumTextFieldUTF8Bytes,
              value.trimmingCharacters(in: .whitespacesAndNewlines) == value,
              !value.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains)
        else {
            throw KagemushaOperationError.invalidField(field)
        }
        return value
    }

    static func rejectionMessage(_ value: String, field: String) throws -> String {
        let utf8 = Data(value.utf8)
        guard !value.isEmpty,
              value.unicodeScalars.count
                <= KagemushaOperationErrorEnvelope.maximumMessageUnicodeScalars,
              value.trimmingCharacters(in: .whitespacesAndNewlines) == value,
              !value.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains),
              String(data: utf8, encoding: .utf8) == value else {
            throw KagemushaOperationError.invalidField(field)
        }
        return value
    }

    static func requestArchive(
        _ value: Data,
        schema: String,
        operationIdFieldIndex: Int,
        fieldCount: Int,
        expectedWireVersion: UInt16,
        maximumArchiveBytes: Int
    ) throws -> (archive: Data, operationId: String) {
        guard let requiredPaddingLength = KagemushaRecursiveSpend
            .requiredHeaderPaddingLength(forWireName: schema),
              !value.isEmpty,
              value.count <= maximumArchiveBytes,
              value.count <= KagemushaRecursiveSpend.artifactMaximumInMemoryArchiveBytes,
              let frame = noritoDecodeFrame(value),
              frame.header.schema == noritoSchemaHash(forTypeName: schema),
              frame.header.compression == .none,
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == requiredPaddingLength,
              !frame.payload.isEmpty,
              operationIdFieldIndex >= 0,
              operationIdFieldIndex < fieldCount else {
            throw KagemushaOperationError.invalidNoritoArchive
        }

        var reader = CanonicalNoritoReader(data: frame.payload)
        var fields = [Data]()
        fields.reserveCapacity(fieldCount)
        do {
            for _ in 0..<fieldCount {
                fields.append(try reader.readCompactField())
            }
        } catch {
            throw KagemushaOperationError.invalidNoritoArchive
        }
        guard reader.remaining() == 0 else {
            throw KagemushaOperationError.invalidNoritoArchive
        }

        guard fields[0].count == MemoryLayout<UInt16>.size else {
            throw KagemushaOperationError.invalidNoritoArchive
        }
        var decodedVersion: UInt16 = 0
        fields[0].withUnsafeBytes { buffer in
            if let baseAddress = buffer.baseAddress {
                memcpy(&decodedVersion, baseAddress, MemoryLayout<UInt16>.size)
            }
        }
        guard UInt16(littleEndian: decodedVersion) == expectedWireVersion else {
            throw KagemushaOperationError.invalidNoritoArchive
        }

        var canonicalPayload = CompactNoritoWriter()
        for field in fields {
            canonicalPayload.writeField(field)
        }
        guard KagemushaRecursiveSpend.frameArchive(
            schema: schema,
            payload: canonicalPayload.data
        ) == value else {
            throw KagemushaOperationError.invalidNoritoArchive
        }

        let operationId = fields[operationIdFieldIndex]
        guard operationId.count == 32,
              operationId.contains(where: { $0 != 0 }) else {
            throw KagemushaOperationError.invalidField("operation_id")
        }
        return (Data(value), operationId.hexEncodedString())
    }
}

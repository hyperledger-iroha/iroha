import Foundation

/// Canonical first-release Kagemusha routes on Torii's `/v1/offline` wire namespace.
public enum KagemushaToriiAPI {
    public enum Endpoint: String, Sendable {
        case capability = "/v1/offline/readiness"
        case topUp = "/v1/offline/top-up"
        case redeem = "/v1/offline/redeem"
        case operations = "/v1/offline/operations"
        case receiverLineage = "/v1/offline/receiver-lineage"

        public var path: String { rawValue }
    }

    public static func operationPath(_ operationId: String) throws -> String {
        "\(Endpoint.operations.path)/\(try KagemushaOperationValidation.operationId(operationId))"
    }
}

/// Native-verified proof that a signed payment request's exact receiver registration is finalized.
///
/// Construction is restricted to the Torii client after native verification of the request
/// digest, registration tuple/lifetime, admitting transaction and Merkle paths, policy, header,
/// and historical V2 finality certificate.
public struct KagemushaRecipientRegistrationLineage: Equatable, Sendable {
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

/// App-owned durable checkpoint supplied to native receiver-lineage
/// verification. `contextID` is the exact marked Iroha HeightContextId.
public struct KagemushaFinalityCheckpointV2: Equatable, Sendable {
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

/// Canonical reusable Torii query for one receiver registration tuple.
public struct KagemushaRecipientLineageQueryV2: Equatable, Sendable {
    public static let maximumArchiveBytes = 32 * 1_024
    public let noritoArchive: Data
    public let trustedCheckpointHeight: UInt64

    public init(
        networkID: NetworkId,
        recipient: String,
        chainDiscriminant: UInt16,
        receiverDeviceID: String,
        assetDefinitionID: String,
        trustedCheckpointHeight: UInt64
    ) throws {
        _ = try KagemushaRecursiveSpend.canonicalAccountAddress(
            recipient,
            field: "lineageQuery.recipient",
            expectedChainDiscriminant: chainDiscriminant
        )
        try KagemushaRecursiveSpend.requirePortableText(
            receiverDeviceID,
            field: "lineageQuery.receiverDeviceID"
        )
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil,
              (1...UInt64(Int64.max)).contains(trustedCheckpointHeight),
              let archive = try NoritoNativeBridge.shared
                  .kagemushaRecipientLineageQueryCreateV2(
                      chainDiscriminant: chainDiscriminant,
                      networkID: networkID,
                      recipient: Data(recipient.utf8),
                      receiverDeviceID: Data(receiverDeviceID.utf8),
                      assetDefinitionID: Data(assetDefinitionID.utf8),
                      trustedCheckpointHeight: trustedCheckpointHeight
                  ) else {
            if AssetDefinitionAddress.decode(assetDefinitionID) == nil
                || !(1...UInt64(Int64.max)).contains(trustedCheckpointHeight) {
                throw KagemushaRecursiveSpendError.invalidField("lineageQuery")
            }
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        guard !archive.isEmpty, archive.count <= Self.maximumArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("lineageQuery")
        }
        self.noritoArchive = archive
        self.trustedCheckpointHeight = trustedCheckpointHeight
    }
}

public struct KagemushaVerifiedRecipientRegistrationLineageV2: Equatable, Sendable {
    public let lineage: KagemushaRecipientRegistrationLineage
    public let promotedCheckpoint: KagemushaFinalityCheckpointV2
}

public extension KagemushaRecipientRegistrationLineage {
    static func verifyV2(
        request: KagemushaRecipientPaymentRequest,
        lineageArchive: Data,
        verifiedAtMilliseconds: UInt64,
        trustedCheckpoint: KagemushaFinalityCheckpointV2
    ) throws -> KagemushaVerifiedRecipientRegistrationLineageV2 {
        guard (1...UInt64(Int64.max)).contains(verifiedAtMilliseconds),
              !lineageArchive.isEmpty,
              lineageArchive.count <= Self.maximumArchiveBytes,
              let result = try NoritoNativeBridge.shared
                .kagemushaRecipientRegistrationLineageVerifyV2(
                    requestArchive: request.archive,
                    lineageArchive: lineageArchive,
                    verifiedAtMilliseconds: verifiedAtMilliseconds,
                    trustedCheckpointHeight: trustedCheckpoint.height,
                    trustedCheckpointContextID: trustedCheckpoint.contextID
                ) else {
            if verifiedAtMilliseconds == 0 || lineageArchive.isEmpty
                || lineageArchive.count > Self.maximumArchiveBytes {
                throw KagemushaRecursiveSpendError.invalidField("receiverLineage")
            }
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        guard result.lineage == lineageArchive else {
            throw KagemushaRecursiveSpendError.invalidArchive("receiverLineage.canonical")
        }
        return KagemushaVerifiedRecipientRegistrationLineageV2(
            lineage: try KagemushaRecipientRegistrationLineage(
                verifiedArchive: result.lineage
            ),
            promotedCheckpoint: try KagemushaFinalityCheckpointV2(
                promotedCheckpointBytes: result.promotedCheckpoint
            )
        )
    }
}

public struct KagemushaProjectedRecipientReceiveOfferV2: Equatable, Sendable {
    public let request: KagemushaRecipientPaymentRequest
    public let lineageArchive: Data
    public let publisherCheckpointEnvelope: Data
}

public struct KagemushaVerifiedRecipientReceiveOfferV2: Equatable, Sendable {
    public let request: KagemushaRecipientPaymentRequest
    public let lineage: KagemushaRecipientRegistrationLineage
    public let publisherCheckpointEnvelope: Data
    public let promotedCheckpoint: KagemushaFinalityCheckpointV2
}

/// Canonical portable receive offer carried as the Kagemusha IPM1
/// RECEIVE_REQUEST body. Native projection is the only decoder, so Swift never
/// guesses the Rust `Archived<T>` alignment or header padding.
public struct KagemushaRecipientReceiveOfferV2: Equatable, Sendable {
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

public enum KagemushaOperationKind: String, Codable, Equatable, Sendable {
    case topUp = "top_up"
    case redeem
}

public enum KagemushaOperationState: String, Codable, Equatable, Sendable {
    case pending
}

public enum KagemushaOperationError: Error, LocalizedError, Equatable, Sendable {
    case invalidField(String)
    case invalidNoritoArchive
    case nativeValidationUnavailable

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid Kagemusha operation field: \(field)."
        case .invalidNoritoArchive:
            return "Kagemusha operation request must be a canonical Norito archive."
        case .nativeValidationUnavailable:
            return "Kagemusha operation status validation requires the native Norito bridge."
        }
    }
}

/// Immutable identity authenticated by one complete Kagemusha chain request.
///
/// Every lifecycle response carries this value as one nested object. The
/// active transaction hash remains outside the identity because a retry may
/// replace a pending carrier without changing the authorized request.
public struct KagemushaOperationIdentity: Codable, Equatable, Sendable {
    public let operationID: String
    public let requestAuthorityDigest: String
    public let canonicalRequestDigest: String
    public let kind: KagemushaOperationKind
    public let issuedAtMs: UInt64
    public let expiresAtMs: UInt64

    public init(
        operationID: String,
        requestAuthorityDigest: String,
        canonicalRequestDigest: String,
        kind: KagemushaOperationKind,
        issuedAtMs: UInt64,
        expiresAtMs: UInt64
    ) throws {
        self.operationID = try KagemushaOperationValidation.markedDigest(
            operationID,
            field: "identity.operation_id"
        )
        self.requestAuthorityDigest = try KagemushaOperationValidation.markedDigest(
            requestAuthorityDigest,
            field: "identity.request_authority_digest"
        )
        self.canonicalRequestDigest = try KagemushaOperationValidation.markedDigest(
            canonicalRequestDigest,
            field: "identity.canonical_request_digest"
        )
        self.kind = kind
        self.issuedAtMs = try KagemushaOperationValidation.positive(
            issuedAtMs,
            field: "identity.issued_at_ms"
        )
        guard expiresAtMs > issuedAtMs,
              expiresAtMs - issuedAtMs
                <= KagemushaRecursiveSpend.maximumAuthorizationTTLMilliseconds else {
            throw KagemushaOperationError.invalidField("identity.expires_at_ms")
        }
        self.expiresAtMs = expiresAtMs
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            operationID: container.decode(String.self, forKey: .operationID),
            requestAuthorityDigest: container.decode(
                String.self,
                forKey: .requestAuthorityDigest
            ),
            canonicalRequestDigest: container.decode(
                String.self,
                forKey: .canonicalRequestDigest
            ),
            kind: container.decode(KagemushaOperationKind.self, forKey: .kind),
            issuedAtMs: container.decode(UInt64.self, forKey: .issuedAtMs),
            expiresAtMs: container.decode(UInt64.self, forKey: .expiresAtMs)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case operationID = "operation_id"
        case requestAuthorityDigest = "request_authority_digest"
        case canonicalRequestDigest = "canonical_request_digest"
        case kind
        case issuedAtMs = "issued_at_ms"
        case expiresAtMs = "expires_at_ms"
    }
}

/// A schema-bound Kagemusha top-up command submitted directly to Torii.
public struct KagemushaTopUpRequest: Equatable, Sendable {
    /// Exact ABI-21/V4 top-up archive ceiling enforced by Torii.
    public static let maximumArchiveBytes = 512 * 1_024
    /// Complete immutable identity derived from the final authorized request.
    public let identity: KagemushaOperationIdentity
    private let archive: Data

    /// Validates and retains a canonical first-release Kagemusha top-up request archive.
    public init(noritoArchive: Data) throws {
        let validated = try KagemushaOperationValidation.requestArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            operationIdFieldIndex: 6,
            fieldCount: 8,
            kind: .topUp,
            expectedWireVersion: KagemushaRecursiveSpend.wireVersionV4,
            maximumArchiveBytes: Self.maximumArchiveBytes
        )
        self.identity = validated.identity
        self.archive = validated.archive
    }

    public func noritoArchive() -> Data { archive }
}

/// A schema-bound Kagemusha redemption command submitted directly to Torii.
public struct KagemushaRedeemRequest: Equatable, Sendable {
    /// Exact ABI-21/V4 redemption archive ceiling enforced by Torii.
    public static let maximumArchiveBytes = 48 * 1_024 * 1_024
    /// Complete immutable identity derived from the final authorized request.
    public let identity: KagemushaOperationIdentity
    private let archive: Data

    /// Validates and retains a canonical first-release Kagemusha redemption request archive.
    public init(noritoArchive: Data) throws {
        let validated = try KagemushaOperationValidation.requestArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.redeemRequestWireName,
            operationIdFieldIndex: 8,
            fieldCount: 10,
            kind: .redeem,
            expectedWireVersion: KagemushaRecursiveSpend.wireVersionV4,
            maximumArchiveBytes: Self.maximumArchiveBytes
        )
        self.identity = validated.identity
        self.archive = validated.archive
    }

    public func noritoArchive() -> Data { archive }
}

public struct KagemushaOperationReference: Codable, Equatable, Sendable {
    public let identity: KagemushaOperationIdentity
    public let state: KagemushaOperationState
    public let transactionHash: String
    public let statusUri: String

    public init(
        identity: KagemushaOperationIdentity,
        state: KagemushaOperationState,
        transactionHash: String,
        statusUri: String
    ) throws {
        self.identity = identity
        self.state = state
        self.transactionHash = try KagemushaOperationValidation.transactionHash(
            transactionHash,
            field: "transaction_hash"
        )
        self.statusUri = try KagemushaOperationValidation.statusUri(
            statusUri,
            operationId: identity.operationID
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            identity: container.decode(KagemushaOperationIdentity.self, forKey: .identity),
            state: container.decode(KagemushaOperationState.self, forKey: .state),
            transactionHash: container.decode(String.self, forKey: .transactionHash),
            statusUri: container.decode(String.self, forKey: .statusUri)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case identity
        case state
        case transactionHash = "transaction_hash"
        case statusUri = "status_uri"
    }
}

/// Canonical finalized top-up anchor consumed by the Kagemusha wallet prover.
///
/// The public API intentionally exposes the current semantic name while the
/// versioned consensus wire type remains an internal codec detail. Parsing an
/// anchor alone does not authenticate finality; the operation-status decoder
/// validates inclusion natively and the prover later authenticates the QC and
/// its release-pinned validator roster.
public struct KagemushaTopUpAnchor: Equatable, Sendable {
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

public struct KagemushaTopUpResult: Equatable, Sendable {
    public let transactionHash: String
    public let finalizedBlockHeight: UInt64
    public let anchor: KagemushaTopUpAnchor
    public let finalityProof: KagemushaTopUpFinalityProofArchive

    /// Constructed only after the complete enclosing status passed native
    /// canonical, Merkle-inclusion, and mutual-binding validation.
    init(
        transactionHash: String,
        finalizedBlockHeight: UInt64,
        anchor: KagemushaTopUpAnchor,
        finalityProof: KagemushaTopUpFinalityProofArchive
    ) throws {
        self.transactionHash = try KagemushaOperationValidation.transactionHash(
            transactionHash,
            field: "transaction_hash"
        )
        self.finalizedBlockHeight = try KagemushaOperationValidation.positive(
            finalizedBlockHeight,
            field: "finalized_block_height"
        )
        guard self.transactionHash == anchor.finalizedTransactionHash,
              self.finalizedBlockHeight == anchor.finalizedBlockHeight else {
            throw KagemushaOperationError.invalidField("top_up_result.anchor_binding")
        }
        self.anchor = anchor
        self.finalityProof = finalityProof
    }
}

public struct KagemushaRedeemResult: Equatable, Sendable {
    public let transactionHash: String
    public let finalizedBlockHeight: UInt64

    public init(
        transactionHash: String,
        finalizedBlockHeight: UInt64
    ) throws {
        self.transactionHash = try KagemushaOperationValidation.transactionHash(
            transactionHash,
            field: "transaction_hash"
        )
        self.finalizedBlockHeight = try KagemushaOperationValidation.positive(
            finalizedBlockHeight,
            field: "finalized_block_height"
        )
    }
}

public enum KagemushaOperationResult: Equatable, Sendable {
    case topUp(KagemushaTopUpResult)
    case redeem(KagemushaRedeemResult)
}

public struct KagemushaQueueErrorSnapshot: Equatable, Sendable {
    public let state: String
    public let queued: UInt64
    public let capacity: UInt64
    public let saturated: Bool

    public init(state: String, queued: UInt64, capacity: UInt64, saturated: Bool) throws {
        self.state = try KagemushaOperationValidation.exactToken(
            state,
            field: "error.details.queue.state"
        )
        self.queued = queued
        self.capacity = capacity
        self.saturated = saturated
    }
}

public struct KagemushaAxtErrorDetails: Equatable, Sendable {
    public let code: String?
    public let reason: String?
    public let snapshotVersion: UInt64?
    public let dataspace: UInt64?
    public let lane: UInt32?
    public let activeHandleEra: UInt64?
    public let nextHandleCounter: UInt64?

    public init(
        code: String? = nil,
        reason: String? = nil,
        snapshotVersion: UInt64? = nil,
        dataspace: UInt64? = nil,
        lane: UInt32? = nil,
        activeHandleEra: UInt64? = nil,
        nextHandleCounter: UInt64? = nil
    ) throws {
        self.code = try code.map {
            try KagemushaOperationValidation.exactText($0, field: "error.details.axt.code")
        }
        self.reason = try reason.map {
            try KagemushaOperationValidation.exactText($0, field: "error.details.axt.reason")
        }
        self.snapshotVersion = snapshotVersion
        self.dataspace = dataspace
        self.lane = lane
        self.activeHandleEra = activeHandleEra
        self.nextHandleCounter = nextHandleCounter
    }
}

public struct KagemushaOperationErrorDetails: Equatable, Sendable {
    public let layer: String?
    public let rejectCode: String?
    public let queue: KagemushaQueueErrorSnapshot?
    public let retryAfterSeconds: UInt64?
    public let endpoint: String?
    public let field: String?
    public let expected: String?
    public let actual: String?
    public let profile: String?
    public let chainDiscriminant: UInt16?
    public let entrypointHash: String?
    public let transactionHash: String?
    public let lastStatus: String?
    public let hint: String?
    public let axt: KagemushaAxtErrorDetails?

    public init(
        layer: String? = nil,
        rejectCode: String? = nil,
        queue: KagemushaQueueErrorSnapshot? = nil,
        retryAfterSeconds: UInt64? = nil,
        endpoint: String? = nil,
        field: String? = nil,
        expected: String? = nil,
        actual: String? = nil,
        profile: String? = nil,
        chainDiscriminant: UInt16? = nil,
        entrypointHash: String? = nil,
        transactionHash: String? = nil,
        lastStatus: String? = nil,
        hint: String? = nil,
        axt: KagemushaAxtErrorDetails? = nil
    ) throws {
        self.layer = try Self.exactOptionalText(layer, field: "error.details.layer")
        self.rejectCode = try rejectCode.map {
            try KagemushaOperationValidation.exactText($0, field: "error.details.reject_code")
        }
        self.queue = queue
        self.retryAfterSeconds = retryAfterSeconds
        self.endpoint = try Self.exactOptionalText(endpoint, field: "error.details.endpoint")
        self.field = try Self.exactOptionalText(field, field: "error.details.field")
        self.expected = try Self.exactOptionalText(expected, field: "error.details.expected")
        self.actual = try Self.exactOptionalText(actual, field: "error.details.actual")
        self.profile = try Self.exactOptionalText(profile, field: "error.details.profile")
        self.chainDiscriminant = chainDiscriminant
        self.entrypointHash = try entrypointHash.map {
            try KagemushaOperationValidation.transactionHash(
                $0,
                field: "error.details.entrypoint_hash"
            )
        }
        self.transactionHash = try transactionHash.map {
            try KagemushaOperationValidation.transactionHash(
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
        try value.map { try KagemushaOperationValidation.exactText($0, field: field) }
    }
}

public struct KagemushaOperationErrorEnvelope: Equatable, Sendable {
    public let code: String
    public let message: String
    public let details: KagemushaOperationErrorDetails?

    public init(
        code: String,
        message: String,
        details: KagemushaOperationErrorDetails? = nil
    ) throws {
        self.code = try KagemushaOperationValidation.stableCode(code, field: "error.code")
        self.message = try KagemushaOperationValidation.exactText(
            message,
            field: "error.message"
        )
        self.details = details
    }
}

/// Pollable state returned by `/v1/offline/operations/{operation_id}`.
public enum KagemushaOperationStatus: Equatable, Sendable {
    /// Validated payload for a queued or not-yet-finalized operation.
    public struct Pending: Equatable, Sendable {
        public let identity: KagemushaOperationIdentity
        public let transactionHash: String

        public init(
            identity: KagemushaOperationIdentity,
            transactionHash: String
        ) throws {
            self.identity = identity
            self.transactionHash = try KagemushaOperationValidation.transactionHash(
                transactionHash,
                field: "transaction_hash"
            )
        }
    }

    /// Validated payload for a finalized operation.
    public struct Applied: Equatable, Sendable {
        public let identity: KagemushaOperationIdentity
        public let result: KagemushaOperationResult

        /// Constructed only by the native-validated status decoder.
        init(identity: KagemushaOperationIdentity, result: KagemushaOperationResult) throws {
            switch (identity.kind, result) {
            case let (.topUp, .topUp(topUp)):
                guard identity.operationID == topUp.anchor.operationId else {
                    throw KagemushaOperationError.invalidField(
                        "identity.operation_id.anchor_binding"
                    )
                }
            case (.redeem, .redeem):
                break
            default:
                throw KagemushaOperationError.invalidField("identity.kind")
            }
            self.identity = identity
            self.result = result
        }
    }

    /// Validated payload for one rejected, retryable carrier attempt.
    public struct Rejected: Equatable, Sendable {
        public let identity: KagemushaOperationIdentity
        public let transactionHash: String
        public let error: KagemushaOperationErrorEnvelope

        public init(
            identity: KagemushaOperationIdentity,
            transactionHash: String,
            error: KagemushaOperationErrorEnvelope
        ) throws {
            self.identity = identity
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

    /// Complete immutable identity shared by every tagged operation state.
    public var identity: KagemushaOperationIdentity {
        switch self {
        case let .pending(value): value.identity
        case let .applied(value): value.identity
        case let .rejected(value): value.identity
        }
    }
}

public enum KagemushaOperationCodec {
    /// A reference contains only bounded identifiers, tags, a status URI, and
    /// a timestamp. Reject oversized input before Norito frame parsing.
    public static let referenceMaximumArchiveBytes = 4 * 1_024
    /// Exact shared native/canonical ceiling for an operation status. Applied
    /// top-up status may contain the bounded finality proof plus framing.
    public static let statusMaximumArchiveBytes = 4 * 1_024 * 1_024
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
        let identity = try readField(&reader, compact: compact) {
            try decodeIdentity(&$0, compact: compact)
        }
        let stateTag = try readField(&reader, compact: compact) { try $0.readUInt32LE() }
        let transactionHash = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        let statusUri = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        guard reader.remaining() == 0 else {
            throw KagemushaOperationError.invalidNoritoArchive
        }
        guard stateTag == 0 else {
            throw KagemushaOperationError.invalidField("state")
        }
        return try KagemushaOperationReference(
            identity: identity,
            state: .pending,
            transactionHash: transactionHash,
            statusUri: statusUri
        )
    }

    public static func encodeReference(_ reference: KagemushaOperationReference) -> Data {
        var payload = CompactNoritoWriter()
        payload.writeField(encodeIdentity(reference.identity))
        payload.writeField(CompactNorito.encodeUInt32(0))
        payload.writeField(CompactNorito.encodeString(reference.transactionHash))
        payload.writeField(CompactNorito.encodeString(reference.statusUri))
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
        let nativeValidated: Bool?
        do {
            nativeValidated = try NoritoNativeBridge.shared
                .kagemushaOfflineOperationStatusValidateV2(
                    statusArchive: archive
                )
        } catch {
            throw KagemushaOperationError.invalidNoritoArchive
        }
        guard nativeValidated == true else {
            throw KagemushaOperationError.nativeValidationUnavailable
        }
        var reader = CanonicalNoritoReader(data: frame.payload)
        let variant = try reader.readUInt32LE()
        let status: KagemushaOperationStatus
        switch variant {
        case 0:
            let identity = try readField(&reader, compact: true) {
                try decodeIdentity(&$0, compact: true)
            }
            let transactionHash = try readExactTextField(
                &reader,
                compact: true,
                field: "transaction_hash"
            )
            status = .pending(try .init(
                identity: identity,
                transactionHash: transactionHash
            ))
        case 1:
            let identity = try readField(&reader, compact: true) {
                try decodeIdentity(&$0, compact: true)
            }
            let result = try readField(&reader, compact: true) {
                try decodeResult(
                    &$0,
                    compact: true,
                    chainDiscriminant: chainDiscriminant
                )
            }
            status = .applied(try .init(identity: identity, result: result))
        case 2:
            let identity = try readField(&reader, compact: true) {
                try decodeIdentity(&$0, compact: true)
            }
            let transactionHash = try readExactTextField(
                &reader,
                compact: true,
                field: "transaction_hash"
            )
            let error = try readField(&reader, compact: true) {
                try decodeErrorEnvelope(&$0, compact: true)
            }
            status = .rejected(try .init(
                identity: identity,
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

    private static func decodeIdentity(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> KagemushaOperationIdentity {
        let operationID = try readOperationIdField(&reader, compact: compact)
        let requestAuthorityDigest = try readMarkedDigestField(
            &reader,
            compact: compact,
            field: "identity.request_authority_digest"
        )
        let canonicalRequestDigest = try readMarkedDigestField(
            &reader,
            compact: compact,
            field: "identity.canonical_request_digest"
        )
        let kind = try readKindField(&reader, compact: compact)
        let issuedAtMs = try readField(&reader, compact: compact) {
            try $0.readUInt64LE()
        }
        let expiresAtMs = try readField(&reader, compact: compact) {
            try $0.readUInt64LE()
        }
        return try KagemushaOperationIdentity(
            operationID: operationID,
            requestAuthorityDigest: requestAuthorityDigest,
            canonicalRequestDigest: canonicalRequestDigest,
            kind: kind,
            issuedAtMs: issuedAtMs,
            expiresAtMs: expiresAtMs
        )
    }

    private static func encodeIdentity(_ identity: KagemushaOperationIdentity) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeString(identity.operationID))
        writer.writeField(CompactNorito.encodeString(identity.requestAuthorityDigest))
        writer.writeField(CompactNorito.encodeString(identity.canonicalRequestDigest))
        writer.writeField(CompactNorito.encodeUInt32(identity.kind == .topUp ? 0 : 1))
        writer.writeField(CompactNorito.encodeUInt64(identity.issuedAtMs))
        writer.writeField(CompactNorito.encodeUInt64(identity.expiresAtMs))
        return writer.data
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
        let finalityProof = try KagemushaTopUpFinalityProofArchive(
            noritoArchive: KagemushaRecursiveSpend.frameArchive(
                schema: KagemushaRecursiveSpend.topUpFinalityProofWireName,
                payload: finalityProofPayload
            )
        )
        return try KagemushaTopUpResult(
            transactionHash: transactionHash,
            finalizedBlockHeight: finalizedBlockHeight,
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
        return try KagemushaRedeemResult(
            transactionHash: transactionHash,
            finalizedBlockHeight: finalizedBlockHeight
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
        let details = try readField(&reader, compact: compact) {
            try decodeOption(&$0, compact: compact) {
                try decodeErrorDetails(&$0, compact: compact)
            }
        }
        return try KagemushaOperationErrorEnvelope(code: code, message: message, details: details)
    }

    private static func decodeErrorDetails(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> KagemushaOperationErrorDetails {
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
        let entrypointHash = try readOptionalStringField(&reader, compact: compact)
        let transactionHash = try readOptionalStringField(&reader, compact: compact)
        let lastStatus = try readOptionalStringField(&reader, compact: compact)
        let hint = try readOptionalStringField(&reader, compact: compact)
        let axt = try readField(&reader, compact: compact) {
            try decodeOption(&$0, compact: compact) {
                try decodeAxtDetails(&$0, compact: compact)
            }
        }
        return try KagemushaOperationErrorDetails(
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
            entrypointHash: entrypointHash,
            transactionHash: transactionHash,
            lastStatus: lastStatus,
            hint: hint,
            axt: axt
        )
    }

    private static func decodeQueueSnapshot(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> KagemushaQueueErrorSnapshot {
        let state = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        let queued = try readField(&reader, compact: compact) { try $0.readUInt64LE() }
        let capacity = try readField(&reader, compact: compact) { try $0.readUInt64LE() }
        let saturated = try readField(&reader, compact: compact) {
            switch try $0.readUInt8() {
            case 0: return false
            case 1: return true
            default: throw KagemushaOperationError.invalidField("queue.saturated")
            }
        }
        return try KagemushaQueueErrorSnapshot(
            state: state,
            queued: queued,
            capacity: capacity,
            saturated: saturated
        )
    }

    private static func decodeAxtDetails(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> KagemushaAxtErrorDetails {
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
        let activeHandleEra = try readOptionalScalarField(
            &reader,
            compact: compact,
            decode: { try $0.readUInt64LE() }
        )
        let nextHandleCounter = try readOptionalScalarField(
            &reader,
            compact: compact,
            decode: { try $0.readUInt64LE() }
        )
        return try KagemushaAxtErrorDetails(
            code: code,
            reason: reason,
            snapshotVersion: snapshotVersion,
            dataspace: dataspace,
            lane: lane,
            activeHandleEra: activeHandleEra,
            nextHandleCounter: nextHandleCounter
        )
    }

    private static func readOperationIdField(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> String {
        let value = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        return try KagemushaOperationValidation.markedDigest(
            value,
            field: "identity.operation_id"
        )
    }

    private static func readMarkedDigestField(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool,
        field: String
    ) throws -> String {
        let value = try readField(&reader, compact: compact) {
            try readString(&$0, compact: compact)
        }
        return try KagemushaOperationValidation.markedDigest(value, field: field)
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

    private static func readOptionalStringField(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> String? {
        try readField(&reader, compact: compact) {
            try decodeOption(&$0, compact: compact) {
                try readString(&$0, compact: compact)
            }
        }
    }

    private static func readOptionalScalarField<T>(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool,
        decode: (inout CanonicalNoritoReader) throws -> T
    ) throws -> T? {
        try readField(&reader, compact: compact) {
            try decodeOption(&$0, compact: compact, decode: decode)
        }
    }

    private static func decodeOption<T>(
        _ reader: inout CanonicalNoritoReader,
        compact: Bool,
        decode: (inout CanonicalNoritoReader) throws -> T
    ) throws -> T? {
        switch try reader.readUInt8() {
        case 0:
            guard reader.remaining() == 0 else {
                throw KagemushaOperationError.invalidNoritoArchive
            }
            return nil
        case 1:
            let payload = compact ? try reader.readCompactField() : try reader.readField()
            var child = CanonicalNoritoReader(data: payload)
            let value = try decode(&child)
            guard child.remaining() == 0, reader.remaining() == 0 else {
                throw KagemushaOperationError.invalidNoritoArchive
            }
            return value
        default:
            throw KagemushaOperationError.invalidField("option")
        }
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

enum KagemushaOperationIdentityDerivation {
    private static let operationIDDomain = Data(
        "iroha:offline:kagemusha:operation-id:v4\0".utf8
    )
    private static let authorityDigestDomain = Data(
        "iroha:offline:kagemusha:operation-outcome-authority:v4\0".utf8
    )
    private static let requestDigestDomain = Data(
        "iroha:offline:kagemusha:operation-request:v4\0".utf8
    )
    private static let accountIDSchema =
        "iroha_data_model::account::model::AccountId"

    static func standaloneAccountIDArchive(
        compactControllerPayload: Data
    ) throws -> Data {
        guard AccountAddress.isCanonicalCompactNoritoAccountControllerPayload(
            compactControllerPayload
        ) else {
            throw KagemushaOperationError.invalidField("authorization.authority")
        }
        let archive = noritoEncode(
            typeName: accountIDSchema,
            payload: compactControllerPayload,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 8
        )
        guard let frame = noritoDecodeFrame(archive),
              frame.header.schema == [
                  0x60, 0xe8, 0x14, 0x73, 0xae, 0xd0, 0xa1, 0x27,
                  0x6f, 0x1c, 0x57, 0x76, 0xd0, 0xf6, 0x9c, 0x38,
              ],
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == 0,
              frame.payload == compactControllerPayload else {
            throw KagemushaOperationError.invalidField("authorization.authority")
        }
        return archive
    }

    static func operationID(
        compactAuthorityPayload: Data,
        nonce: Data
    ) throws -> String {
        guard nonce.count == 32, nonce.contains(where: { $0 != 0 }) else {
            throw KagemushaOperationError.invalidField("authorization.nonce")
        }
        let authorityArchive = try standaloneAccountIDArchive(
            compactControllerPayload: compactAuthorityPayload
        )
        var preimage = operationIDDomain
        preimage.append(littleEndianUInt64(authorityArchive.count))
        preimage.append(authorityArchive)
        preimage.append(nonce)
        return markedHash(preimage).hexEncodedString()
    }

    static func requestAuthorityDigest(
        compactAuthorityPayload: Data
    ) throws -> String {
        let authorityArchive = try standaloneAccountIDArchive(
            compactControllerPayload: compactAuthorityPayload
        )
        var preimage = authorityDigestDomain
        preimage.append(littleEndianUInt64(authorityArchive.count))
        preimage.append(authorityArchive)
        return markedHash(preimage).hexEncodedString()
    }

    static func canonicalRequestDigest(
        requestArchive: Data,
        kind: KagemushaOperationKind
    ) -> String {
        var preimage = requestDigestDomain
        preimage.append(Data(kind == .topUp ? "top_up".utf8 : "redeem".utf8))
        preimage.append(littleEndianUInt64(requestArchive.count))
        preimage.append(requestArchive)
        return markedHash(preimage).hexEncodedString()
    }

    private static func littleEndianUInt64(_ value: Int) -> Data {
        var littleEndian = UInt64(value).littleEndian
        return withUnsafeBytes(of: &littleEndian) { Data($0) }
    }

    private static func markedHash(_ preimage: Data) -> Data {
        var digest = Blake2b.hash256(preimage)
        digest[digest.index(before: digest.endIndex)] |= 1
        return digest
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
        try markedDigest(value, field: "operation_id")
    }

    static func markedDigest(_ value: String, field: String) throws -> String {
        let bytes = Array(value.utf8)
        guard bytes.count == 64,
              bytes.contains(where: { $0 != UInt8(ascii: "0") }),
              bytes.last.map({ "13579bdf".utf8.contains($0) }) == true,
              bytes.allSatisfy({
                  ($0 >= UInt8(ascii: "0") && $0 <= UInt8(ascii: "9"))
                      || ($0 >= UInt8(ascii: "a") && $0 <= UInt8(ascii: "f"))
              }) else {
            throw KagemushaOperationError.invalidField(field)
        }
        return value
    }

    static func transactionHash(_ value: String, field: String) throws -> String {
        let bytes = Array(value.utf8)
        guard bytes.count == 64,
              bytes.contains(where: { $0 != UInt8(ascii: "0") }),
              let marker = bytes.last,
              "13579bdf".utf8.contains(marker),
              bytes.allSatisfy({
                  ($0 >= UInt8(ascii: "0") && $0 <= UInt8(ascii: "9"))
                      || ($0 >= UInt8(ascii: "a") && $0 <= UInt8(ascii: "f"))
              }) else {
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

    static func stableCode(_ value: String, field: String) throws -> String {
        let bytes = Array(value.utf8)
        guard (1...64).contains(bytes.count),
              let first = bytes.first,
              isLowercaseLetter(first) || isDigit(first),
              bytes.allSatisfy({
                  isLowercaseLetter($0) || isDigit($0) || $0 == UInt8(ascii: "_")
              }) else {
            throw KagemushaOperationError.invalidField(field)
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

    static func exactToken(_ value: String, field: String) throws -> String {
        let exact = try exactText(value, field: field)
        guard !exact.unicodeScalars.contains(where: CharacterSet.whitespacesAndNewlines.contains)
        else {
            throw KagemushaOperationError.invalidField(field)
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
        fieldCount: Int,
        kind: KagemushaOperationKind,
        expectedWireVersion: UInt16,
        maximumArchiveBytes: Int
    ) throws -> (archive: Data, identity: KagemushaOperationIdentity) {
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

        let outerOperationID = fields[operationIdFieldIndex]
        guard outerOperationID.count == 32,
              outerOperationID.contains(where: { $0 != 0 }),
              outerOperationID.last.map({ ($0 & 1) == 1 }) == true else {
            throw KagemushaOperationError.invalidField("operation_id")
        }
        let authorization = try requestAuthorizationIdentityFields(
            fields[fieldCount - 1],
            outerOperationID: outerOperationID
        )
        let derivedOperationID = try KagemushaOperationIdentityDerivation.operationID(
            compactAuthorityPayload: authorization.authority,
            nonce: authorization.nonce
        )
        guard derivedOperationID == outerOperationID.hexEncodedString() else {
            throw KagemushaOperationError.invalidField("authorization.operation_id")
        }
        let identity = try KagemushaOperationIdentity(
            operationID: derivedOperationID,
            requestAuthorityDigest: try KagemushaOperationIdentityDerivation
                .requestAuthorityDigest(
                    compactAuthorityPayload: authorization.authority
                ),
            canonicalRequestDigest: KagemushaOperationIdentityDerivation
                .canonicalRequestDigest(requestArchive: value, kind: kind),
            kind: kind,
            issuedAtMs: authorization.issuedAtMs,
            expiresAtMs: authorization.expiresAtMs
        )
        return (Data(value), identity)
    }

    private static func requestAuthorizationIdentityFields(
        _ authorization: Data,
        outerOperationID: Data
    ) throws -> (authority: Data, nonce: Data, issuedAtMs: UInt64, expiresAtMs: UInt64) {
        let fieldCount = 10
        let operationIdFieldIndex = 3
        let issuedAtMsFieldIndex = 4
        let expiresAtMsFieldIndex = 5
        let nonceFieldIndex = 6
        let payloadDigestFieldIndex = 7
        let registrationHashFieldIndex = 8
        var reader = CanonicalNoritoReader(data: authorization)
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

        var canonicalAuthorization = CompactNoritoWriter()
        for field in fields {
            canonicalAuthorization.writeField(field)
        }
        guard canonicalAuthorization.data == authorization else {
            throw KagemushaOperationError.invalidNoritoArchive
        }
        guard fields[operationIdFieldIndex] == outerOperationID else {
            throw KagemushaOperationError.invalidField("authorization.operation_id")
        }

        _ = try KagemushaOperationIdentityDerivation.standaloneAccountIDArchive(
            compactControllerPayload: fields[0]
        )
        var deviceID = CanonicalNoritoReader(data: fields[1])
        let deviceIDLength = try deviceID.readVarint()
        guard deviceIDLength > 0,
              deviceIDLength <= 128,
              let deviceIDValue = String(
                  data: try deviceID.readBytes(Int(deviceIDLength)),
                  encoding: .utf8
              ),
              deviceID.remaining() == 0,
              deviceIDValue.trimmingCharacters(in: .whitespacesAndNewlines) == deviceIDValue,
              !deviceIDValue.unicodeScalars.contains(
                  where: CharacterSet.controlCharacters.contains
              ),
              !fields[2].isEmpty,
              !fields[9].isEmpty else {
            throw KagemushaOperationError.invalidField("authorization")
        }
        guard fields[nonceFieldIndex].count == 32,
              fields[nonceFieldIndex].contains(where: { $0 != 0 }) else {
            throw KagemushaOperationError.invalidField("authorization.nonce")
        }
        guard fields[payloadDigestFieldIndex].count == 32,
              fields[payloadDigestFieldIndex].last.map({ ($0 & 1) == 1 }) == true,
              fields[registrationHashFieldIndex].count == 32,
              fields[registrationHashFieldIndex].last.map({ ($0 & 1) == 1 }) == true else {
            throw KagemushaOperationError.invalidField("authorization.digest")
        }

        let issuedAtField = fields[issuedAtMsFieldIndex]
        guard issuedAtField.count == MemoryLayout<UInt64>.size else {
            throw KagemushaOperationError.invalidField("authorization.issued_at_ms")
        }
        var decodedIssuedAtMs: UInt64 = 0
        issuedAtField.withUnsafeBytes { buffer in
            if let baseAddress = buffer.baseAddress {
                memcpy(&decodedIssuedAtMs, baseAddress, MemoryLayout<UInt64>.size)
            }
        }
        let issuedAtMs = try positive(
            UInt64(littleEndian: decodedIssuedAtMs),
            field: "authorization.issued_at_ms"
        )
        let expiresAtField = fields[expiresAtMsFieldIndex]
        guard expiresAtField.count == MemoryLayout<UInt64>.size else {
            throw KagemushaOperationError.invalidField("authorization.expires_at_ms")
        }
        var decodedExpiresAtMs: UInt64 = 0
        expiresAtField.withUnsafeBytes { buffer in
            if let baseAddress = buffer.baseAddress {
                memcpy(&decodedExpiresAtMs, baseAddress, MemoryLayout<UInt64>.size)
            }
        }
        let expiresAtMs = UInt64(littleEndian: decodedExpiresAtMs)
        guard expiresAtMs > issuedAtMs,
              expiresAtMs - issuedAtMs
                <= KagemushaRecursiveSpend.maximumAuthorizationTTLMilliseconds else {
            throw KagemushaOperationError.invalidField("authorization.expires_at_ms")
        }
        return (
            fields[0],
            fields[nonceFieldIndex],
            issuedAtMs,
            expiresAtMs
        )
    }
}

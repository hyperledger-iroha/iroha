import Foundation

/// Fail-closed errors specific to the public Offline Cash V1 Torii facade.
public enum OfflineCashToriiV1Error: Error, Equatable, Sendable {
    case nativeBridgeABI22Unavailable
    case invalidCanonicalRequest
    case invalidCanonicalReference
    case invalidCanonicalStatus
    case localSigningContextRequired
    case requestNetworkMismatch
}

/// Stable Offline Cash V1 operation kind.
public enum OfflineCashOperationKindV1: String, Equatable, Sendable {
    case topUp = "top_up"
    case redeem
}

/// Stable Offline Cash V1 operation state.
public enum OfflineCashOperationStateV1: String, Equatable, Sendable {
    case pending
    case applied
    case rejected
}

/// One validated universal Offline Cash V1 readiness blocker.
public struct OfflineCashReadinessBlockerV1: Equatable, Sendable {
    public let code: String
    public let message: String

    fileprivate init(_ blocker: ToriiKagemushaReadinessBlocker) {
        code = blocker.code
        message = blocker.message
    }
}

/// Asset-neutral readiness advertised by the canonical Offline Cash V1 route.
public struct OfflineCashReadinessV1: Equatable, Sendable {
    public let mandatory: Bool
    public let cashHandoffCapability: String
    public let requiredBridgeABIVersion: UInt32
    public let maximumHops: UInt32
    public let ready: Bool
    public let blockers: [OfflineCashReadinessBlockerV1]

    /// The universal capability response deliberately selects no asset.
    public var assets: [String] { [] }

    fileprivate init(_ status: ToriiOfflineStatus) {
        mandatory = status.mandatory
        cashHandoffCapability = status.cashHandoffCapability
        requiredBridgeABIVersion = status.requiredBridgeAbiVersion
        maximumHops = status.maxHops
        ready = status.ready
        blockers = status.blockers.map(OfflineCashReadinessBlockerV1.init)
    }
}

/// Opaque canonical signed Offline Cash V1 top-up request.
public struct OfflineCashTopUpRequestV1: Equatable, Sendable {
    public static let maximumCanonicalBytes = 512 * 1_024

    private let canonical: Data
    package let substrate: KagemushaTopUpRequest
    package let nativeProjection: KagemushaSubmissionRequestProjectionV4

    /// Operation id derived by native ABI22 validation of the signed request.
    public var operationId: String { nativeProjection.operationId.hexEncodedString() }
    /// Authorization issuance time derived by native ABI22 validation.
    public var submittedAtMilliseconds: UInt64 {
        nativeProjection.submittedAtMilliseconds
    }

    public init(canonicalNorito: Data) throws {
        guard !canonicalNorito.isEmpty,
              canonicalNorito.count <= Self.maximumCanonicalBytes else {
            throw OfflineCashToriiV1Error.invalidCanonicalRequest
        }
        let substrate: KagemushaTopUpRequest
        do {
            substrate = try KagemushaTopUpRequest(noritoArchive: canonicalNorito)
        } catch {
            throw OfflineCashToriiV1Error.invalidCanonicalRequest
        }
        let projection: KagemushaSubmissionRequestProjectionV4
        do {
            guard let decoded = try NoritoNativeBridge.shared
                .kagemushaTopUpSubmissionRequestProjectV4(requestArchive: canonicalNorito) else {
                throw OfflineCashToriiV1Error.nativeBridgeABI22Unavailable
            }
            projection = decoded
        } catch let error as OfflineCashToriiV1Error {
            throw error
        } catch {
            throw OfflineCashToriiV1Error.invalidCanonicalRequest
        }
        guard substrate.operationId == projection.operationId.hexEncodedString() else {
            throw OfflineCashToriiV1Error.invalidCanonicalRequest
        }
        canonical = Data(canonicalNorito)
        self.substrate = substrate
        nativeProjection = projection
    }

    public static func decodeCanonical(_ canonicalNorito: Data) throws -> Self {
        try Self(canonicalNorito: canonicalNorito)
    }

    /// Defensive copy retained for byte-identical idempotent retries.
    public func encodeCanonical() -> Data { Data(canonical) }
}

/// Opaque canonical signed Offline Cash V1 redemption request.
public struct OfflineCashRedeemRequestV1: Equatable, Sendable {
    public static let maximumCanonicalBytes = 48 * 1_024 * 1_024

    private let canonical: Data
    package let substrate: KagemushaRedeemRequest
    package let nativeProjection: KagemushaSubmissionRequestProjectionV4

    /// Operation id derived by native ABI22 validation of the signed request.
    public var operationId: String { nativeProjection.operationId.hexEncodedString() }
    /// Authorization issuance time derived by native ABI22 validation.
    public var submittedAtMilliseconds: UInt64 {
        nativeProjection.submittedAtMilliseconds
    }

    public init(canonicalNorito: Data) throws {
        guard !canonicalNorito.isEmpty,
              canonicalNorito.count <= Self.maximumCanonicalBytes else {
            throw OfflineCashToriiV1Error.invalidCanonicalRequest
        }
        let substrate: KagemushaRedeemRequest
        do {
            substrate = try KagemushaRedeemRequest(noritoArchive: canonicalNorito)
        } catch {
            throw OfflineCashToriiV1Error.invalidCanonicalRequest
        }
        let projection: KagemushaSubmissionRequestProjectionV4
        do {
            guard let decoded = try NoritoNativeBridge.shared
                .kagemushaRedeemSubmissionRequestProjectV4(requestArchive: canonicalNorito) else {
                throw OfflineCashToriiV1Error.nativeBridgeABI22Unavailable
            }
            projection = decoded
        } catch let error as OfflineCashToriiV1Error {
            throw error
        } catch {
            throw OfflineCashToriiV1Error.invalidCanonicalRequest
        }
        guard substrate.operationId == projection.operationId.hexEncodedString() else {
            throw OfflineCashToriiV1Error.invalidCanonicalRequest
        }
        canonical = Data(canonicalNorito)
        self.substrate = substrate
        nativeProjection = projection
    }

    public static func decodeCanonical(_ canonicalNorito: Data) throws -> Self {
        try Self(canonicalNorito: canonicalNorito)
    }

    /// Defensive copy retained for byte-identical idempotent retries.
    public func encodeCanonical() -> Data { Data(canonical) }
}

/// Safe projection of an accepted Offline Cash V1 operation reference.
public struct OfflineCashOperationReferenceProjectionV1: Equatable, Sendable {
    public let operationId: String
    public let kind: OfflineCashOperationKindV1
    public let state: OfflineCashOperationStateV1
    public let transactionHash: String
    public let statusURI: String
    public let submittedAtMilliseconds: UInt64
}

/// Opaque canonical accepted-operation reference.
public struct OfflineCashOperationReferenceV1: Equatable, Sendable {
    public static let maximumCanonicalBytes = 4 * 1_024

    private let canonical: Data
    private let decoded: KagemushaOperationReference

    public init(canonicalNorito: Data) throws {
        guard !canonicalNorito.isEmpty,
              canonicalNorito.count <= Self.maximumCanonicalBytes else {
            throw OfflineCashToriiV1Error.invalidCanonicalReference
        }
        let decoded: KagemushaOperationReference
        do {
            decoded = try KagemushaOperationCodec.decodeReference(canonicalNorito)
        } catch {
            throw OfflineCashToriiV1Error.invalidCanonicalReference
        }
        guard KagemushaOperationCodec.encodeReference(decoded) == canonicalNorito else {
            throw OfflineCashToriiV1Error.invalidCanonicalReference
        }
        canonical = Data(canonicalNorito)
        self.decoded = decoded
    }

    public static func decodeCanonical(_ canonicalNorito: Data) throws -> Self {
        try Self(canonicalNorito: canonicalNorito)
    }

    public func encodeCanonical() -> Data { Data(canonical) }

    public func project() -> OfflineCashOperationReferenceProjectionV1 {
        OfflineCashOperationReferenceProjectionV1(
            operationId: decoded.operationId,
            kind: Self.kind(decoded.kind),
            state: .pending,
            transactionHash: decoded.transactionHash,
            statusURI: decoded.statusUri,
            submittedAtMilliseconds: decoded.submittedAtMs
        )
    }

    private static func kind(_ value: KagemushaOperationKind) -> OfflineCashOperationKindV1 {
        switch value {
        case .topUp: .topUp
        case .redeem: .redeem
        }
    }
}

/// Stable terminal rejection returned by an Offline Cash V1 operation.
public struct OfflineCashOperationRejectionV1: Equatable, Sendable {
    public let code: String
    public let message: String
}

/// Opaque finalized top-up evidence with safe finality metadata.
public struct OfflineCashFinalizedTopUpV1: Equatable, Sendable {
    private let anchor: Data
    private let finalityProof: Data
    public let finalizedBlockHeight: UInt64
    public let serverTimeMilliseconds: UInt64

    fileprivate init(_ result: KagemushaTopUpResult) {
        anchor = result.anchor.noritoArchive()
        finalityProof = Data(result.finalityProof.noritoArchive)
        finalizedBlockHeight = result.finalizedBlockHeight
        serverTimeMilliseconds = result.serverTimeMs
    }

    public func encodeAnchorCanonical() -> Data { Data(anchor) }
    public func encodeFinalityProofCanonical() -> Data { Data(finalityProof) }
}

/// Strict projection of one decoded Offline Cash V1 operation status.
public struct OfflineCashOperationStatusProjectionV1: Equatable, Sendable {
    public let state: OfflineCashOperationStateV1
    public let kind: OfflineCashOperationKindV1
    public let operationId: String
    public let transactionHash: String
    public let submittedAtMilliseconds: UInt64?
    public let finalizedBlockHeight: UInt64?
    public let serverTimeMilliseconds: UInt64?
    public let finalizedTopUp: OfflineCashFinalizedTopUpV1?
    public let rejection: OfflineCashOperationRejectionV1?
}

/// Opaque canonical poll response with an explicit safe projection.
public struct OfflineCashOperationStatusV1: Equatable, Sendable {
    public static let maximumCanonicalBytes = 3 * 1_024 * 1_024

    private let canonical: Data
    private let decoded: KagemushaOperationStatus

    public init(canonicalNorito: Data, chainDiscriminant: UInt16) throws {
        guard !canonicalNorito.isEmpty,
              canonicalNorito.count <= Self.maximumCanonicalBytes else {
            throw OfflineCashToriiV1Error.invalidCanonicalStatus
        }
        do {
            guard let validated = try NoritoNativeBridge.shared
                .kagemushaOperationStatusValidateV4(statusArchive: canonicalNorito) else {
                throw OfflineCashToriiV1Error.nativeBridgeABI22Unavailable
            }
            guard validated == canonicalNorito else {
                throw OfflineCashToriiV1Error.invalidCanonicalStatus
            }
        } catch let error as OfflineCashToriiV1Error {
            throw error
        } catch {
            throw OfflineCashToriiV1Error.invalidCanonicalStatus
        }
        do {
            decoded = try KagemushaOperationCodec.decodeStatus(
                canonicalNorito,
                chainDiscriminant: chainDiscriminant
            )
        } catch {
            throw OfflineCashToriiV1Error.invalidCanonicalStatus
        }
        canonical = Data(canonicalNorito)
    }

    public static func decodeCanonical(
        _ canonicalNorito: Data,
        chainDiscriminant: UInt16
    ) throws -> Self {
        try Self(canonicalNorito: canonicalNorito, chainDiscriminant: chainDiscriminant)
    }

    public func encodeCanonical() -> Data { Data(canonical) }

    public func project() -> OfflineCashOperationStatusProjectionV1 {
        switch decoded {
        case let .pending(value):
            return OfflineCashOperationStatusProjectionV1(
                state: .pending,
                kind: Self.kind(value.kind),
                operationId: value.operationId,
                transactionHash: value.transactionHash,
                submittedAtMilliseconds: value.submittedAtMs,
                finalizedBlockHeight: nil,
                serverTimeMilliseconds: nil,
                finalizedTopUp: nil,
                rejection: nil
            )
        case let .applied(value):
            switch value.result {
            case let .topUp(result):
                return OfflineCashOperationStatusProjectionV1(
                    state: .applied,
                    kind: .topUp,
                    operationId: value.operationId,
                    transactionHash: result.transactionHash,
                    submittedAtMilliseconds: nil,
                    finalizedBlockHeight: result.finalizedBlockHeight,
                    serverTimeMilliseconds: result.serverTimeMs,
                    finalizedTopUp: OfflineCashFinalizedTopUpV1(result),
                    rejection: nil
                )
            case let .redeem(result):
                return OfflineCashOperationStatusProjectionV1(
                    state: .applied,
                    kind: .redeem,
                    operationId: value.operationId,
                    transactionHash: result.transactionHash,
                    submittedAtMilliseconds: nil,
                    finalizedBlockHeight: result.finalizedBlockHeight,
                    serverTimeMilliseconds: result.serverTimeMs,
                    finalizedTopUp: nil,
                    rejection: nil
                )
            }
        case let .rejected(value):
            return OfflineCashOperationStatusProjectionV1(
                state: .rejected,
                kind: Self.kind(value.kind),
                operationId: value.operationId,
                transactionHash: value.transactionHash,
                submittedAtMilliseconds: nil,
                finalizedBlockHeight: nil,
                serverTimeMilliseconds: nil,
                finalizedTopUp: nil,
                rejection: OfflineCashOperationRejectionV1(
                    code: value.error.code,
                    message: value.error.message
                )
            )
        }
    }

    package var finalizedTopUpNetworkId: NetworkId? {
        guard case let .applied(value) = decoded,
              case let .topUp(result) = value.result else {
            return nil
        }
        return result.anchor.networkId
    }

    private static func kind(_ value: KagemushaOperationKind) -> OfflineCashOperationKindV1 {
        switch value {
        case .topUp: .topUp
        case .redeem: .redeem
        }
    }
}

/// Public client for exactly the four first-release Offline Cash V1 Torii routes.
public final class OfflineCashToriiClientV1: @unchecked Sendable {
    public static let readinessPath = "/v1/offline/readiness"
    public static let topUpPath = "/v1/offline/top-up"
    public static let redeemPath = "/v1/offline/redeem"
    public static let operationsPath = "/v1/offline/operations"
    public static let jsonMediaType = "application/json"
    public static let noritoMediaType = "application/x-norito"

    public let localSigningContext: ToriiLocalSigningContext
    public let chainDiscriminant: UInt16
    private let client: ToriiClient

    /// Binds the facade to an immutable genesis identity and I105 discriminant.
    public init(client: ToriiClient, chainDiscriminant: UInt16) throws {
        guard let localSigningContext = client.localSigningContext else {
            throw OfflineCashToriiV1Error.localSigningContextRequired
        }
        self.client = client
        self.localSigningContext = localSigningContext
        self.chainDiscriminant = chainDiscriminant
    }

    public func getReadiness() async throws -> OfflineCashReadinessV1 {
        OfflineCashReadinessV1(try await client.getOfflineCapability())
    }

    public func submitTopUp(
        _ request: OfflineCashTopUpRequestV1
    ) async throws -> OfflineCashOperationReferenceV1 {
        try requireLocalNetwork(request.nativeProjection.networkId)
        let reference = try await client.submitKagemushaTopUp(
            request.substrate,
            expectedSubmittedAtMilliseconds: request.submittedAtMilliseconds
        )
        return try OfflineCashOperationReferenceV1(
            canonicalNorito: KagemushaOperationCodec.encodeReference(reference)
        )
    }

    public func submitRedeem(
        _ request: OfflineCashRedeemRequestV1
    ) async throws -> OfflineCashOperationReferenceV1 {
        try requireLocalNetwork(request.nativeProjection.networkId)
        let reference = try await client.submitKagemushaRedeem(
            request.substrate,
            expectedSubmittedAtMilliseconds: request.submittedAtMilliseconds
        )
        return try OfflineCashOperationReferenceV1(
            canonicalNorito: KagemushaOperationCodec.encodeReference(reference)
        )
    }

    public func getOperation(
        operationId: String
    ) async throws -> OfflineCashOperationStatusV1 {
        let response = try await client.getKagemushaOperationStatusArchive(
            operationId: operationId,
            chainDiscriminant: chainDiscriminant,
            expectedNetworkId: localSigningContext.networkId
        )
        let status = try OfflineCashOperationStatusV1(
            canonicalNorito: response.archive,
            chainDiscriminant: chainDiscriminant
        )
        if let finalizedNetwork = status.finalizedTopUpNetworkId {
            try requireLocalNetwork(finalizedNetwork)
        }
        return status
    }

    private func requireLocalNetwork(_ networkId: NetworkId) throws {
        guard networkId == localSigningContext.networkId else {
            throw OfflineCashToriiV1Error.requestNetworkMismatch
        }
    }
}

import Foundation

#if canImport(Darwin)
import Darwin
#endif

/// Fail-closed errors from the authoritative private-settlement response verifier.
public enum AtomicPrivateSettlementNativeVerifierErrorV1: Error, Equatable, Sendable {
    /// The exact native bridge symbols required by this SDK are unavailable.
    case bridgeUnavailable
    /// A byte bound, network identity, digest, or signing-key input is malformed.
    case invalidInput
    /// Typed Norito or cryptographic verification rejected the response.
    case nativeRejected(Int32)
}

/// Injectable boundary for full private-settlement response authentication.
///
/// Production callers use ``AtomicPrivateSettlementNativeResponseVerifierV1``.
/// The protocol exists so deterministic transport tests can substitute an
/// isolated verifier without weakening the production default.
public protocol AtomicPrivateSettlementResponseVerifyingV1: Sendable {
    /// Fail unless every native verifier required by the restricted routes is callable now.
    func requireAvailable() throws

    func verifyCommitteeProof(
        responseJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data
    ) throws

    func verifyAuditorCapsule(
        responseJSON: Data,
        requestJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data,
        auditorSigningKey: String
    ) throws

    func verifyAuditApproval(
        responseJSON: Data,
        requestJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data,
        auditorSigningKey: String
    ) throws
}

/// Native verifier backed by the same typed rules as the Rust Torii client.
public struct AtomicPrivateSettlementNativeResponseVerifierV1:
    AtomicPrivateSettlementResponseVerifyingV1, Sendable
{
    public static let maximumResponseBytes = 32 * 1024 * 1024
    public static let maximumApprovalRequestBytes = 1024 * 1024
    private static let maximumPublicKeyBytes = 1024
    private static let rejectedStatus: Int32 = -507

    public init() {}

    public func requireAvailable() throws {
        #if canImport(Darwin)
        guard let committeeProof = NoritoNativeBridge.shared.resolveNativeSymbol(
            "connect_norito_private_settlement_committee_proof_response_verify_v1",
            as: CommitteeProofVerifierFn.self
        ), let auditorCapsule = NoritoNativeBridge.shared.resolveNativeSymbol(
            "connect_norito_private_settlement_auditor_capsule_response_verify_with_request_v1",
            as: AuditorCapsuleVerifierFn.self
        ), let auditApproval = NoritoNativeBridge.shared.resolveNativeSymbol(
            "connect_norito_private_settlement_audit_approval_response_verify_v1",
            as: AuditApprovalVerifierFn.self
        ) else {
            throw AtomicPrivateSettlementNativeVerifierErrorV1.bridgeUnavailable
        }
        let linkageProbeStatuses = [
            committeeProof(nil, 0, nil, 0, nil, 0),
            auditorCapsule(nil, 0, nil, 0, nil, 0, nil, 0, nil, 0),
            auditApproval(nil, 0, nil, 0, nil, 0, nil, 0, nil, 0),
        ]
        guard linkageProbeStatuses.allSatisfy({ $0 == Self.rejectedStatus }) else {
            throw AtomicPrivateSettlementNativeVerifierErrorV1.bridgeUnavailable
        }
        #else
        throw AtomicPrivateSettlementNativeVerifierErrorV1.bridgeUnavailable
        #endif
    }

    public func verifyCommitteeProof(
        responseJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data
    ) throws {
        try Self.validateCommon(
            responseJSON: responseJSON,
            expectedNetworkID: expectedNetworkID,
            requestedPayloadDigest: requestedPayloadDigest
        )
        try requireAvailable()
        #if canImport(Darwin)
        guard let function = NoritoNativeBridge.shared.resolveNativeSymbol(
            "connect_norito_private_settlement_committee_proof_response_verify_v1",
            as: CommitteeProofVerifierFn.self
        ) else {
            throw AtomicPrivateSettlementNativeVerifierErrorV1.bridgeUnavailable
        }
        let status = responseJSON.withUnsafeBytes { response in
            expectedNetworkID.withUnsafeBytes { network in
                requestedPayloadDigest.withUnsafeBytes { payload in
                    function(
                        response.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(response.count),
                        network.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(network.count),
                        payload.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(payload.count)
                    )
                }
            }
        }
        try Self.requireSuccess(status)
        #else
        throw AtomicPrivateSettlementNativeVerifierErrorV1.bridgeUnavailable
        #endif
    }

    public func verifyAuditorCapsule(
        responseJSON: Data,
        requestJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data,
        auditorSigningKey: String
    ) throws {
        try Self.validateCommon(
            responseJSON: responseJSON,
            expectedNetworkID: expectedNetworkID,
            requestedPayloadDigest: requestedPayloadDigest
        )
        guard !requestJSON.isEmpty,
              requestJSON.count <= Self.maximumApprovalRequestBytes else {
            throw AtomicPrivateSettlementNativeVerifierErrorV1.invalidInput
        }
        let key = try Self.validatedPublicKeyBytes(auditorSigningKey)
        try requireAvailable()
        #if canImport(Darwin)
        guard let function = NoritoNativeBridge.shared.resolveNativeSymbol(
            "connect_norito_private_settlement_auditor_capsule_response_verify_with_request_v1",
            as: AuditorCapsuleVerifierFn.self
        ) else {
            throw AtomicPrivateSettlementNativeVerifierErrorV1.bridgeUnavailable
        }
        let status = responseJSON.withUnsafeBytes { response in
            requestJSON.withUnsafeBytes { request in
                expectedNetworkID.withUnsafeBytes { network in
                    requestedPayloadDigest.withUnsafeBytes { payload in
                        key.withUnsafeBytes { signingKey in
                            function(
                                response.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(response.count),
                                request.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(request.count),
                                network.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(network.count),
                                payload.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(payload.count),
                                signingKey.bindMemory(to: CChar.self).baseAddress,
                                CUnsignedLong(signingKey.count)
                            )
                        }
                    }
                }
            }
        }
        try Self.requireSuccess(status)
        #else
        throw AtomicPrivateSettlementNativeVerifierErrorV1.bridgeUnavailable
        #endif
    }

    public func verifyAuditApproval(
        responseJSON: Data,
        requestJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data,
        auditorSigningKey: String
    ) throws {
        try Self.validateCommon(
            responseJSON: responseJSON,
            expectedNetworkID: expectedNetworkID,
            requestedPayloadDigest: requestedPayloadDigest
        )
        guard !requestJSON.isEmpty,
              requestJSON.count <= Self.maximumApprovalRequestBytes else {
            throw AtomicPrivateSettlementNativeVerifierErrorV1.invalidInput
        }
        let key = try Self.validatedPublicKeyBytes(auditorSigningKey)
        try requireAvailable()
        #if canImport(Darwin)
        guard let function = NoritoNativeBridge.shared.resolveNativeSymbol(
            "connect_norito_private_settlement_audit_approval_response_verify_v1",
            as: AuditApprovalVerifierFn.self
        ) else {
            throw AtomicPrivateSettlementNativeVerifierErrorV1.bridgeUnavailable
        }
        let status = responseJSON.withUnsafeBytes { response in
            requestJSON.withUnsafeBytes { request in
                expectedNetworkID.withUnsafeBytes { network in
                    requestedPayloadDigest.withUnsafeBytes { payload in
                        key.withUnsafeBytes { signingKey in
                            function(
                                response.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(response.count),
                                request.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(request.count),
                                network.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(network.count),
                                payload.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(payload.count),
                                signingKey.bindMemory(to: CChar.self).baseAddress,
                                CUnsignedLong(signingKey.count)
                            )
                        }
                    }
                }
            }
        }
        try Self.requireSuccess(status)
        #else
        throw AtomicPrivateSettlementNativeVerifierErrorV1.bridgeUnavailable
        #endif
    }

    private static func validateCommon(
        responseJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data
    ) throws {
        guard !responseJSON.isEmpty,
              responseJSON.count <= Self.maximumResponseBytes,
              expectedNetworkID.count == 32,
              requestedPayloadDigest.count == 32 else {
            throw AtomicPrivateSettlementNativeVerifierErrorV1.invalidInput
        }
    }

    private static func validatedPublicKeyBytes(_ literal: String) throws -> Data {
        let bytes = Data(literal.utf8)
        guard literal == literal.trimmingCharacters(in: .whitespacesAndNewlines),
              !literal.isEmpty,
              !literal.utf8.contains(0),
              bytes.count <= Self.maximumPublicKeyBytes else {
            throw AtomicPrivateSettlementNativeVerifierErrorV1.invalidInput
        }
        return bytes
    }

    private static func requireSuccess(_ status: Int32) throws {
        guard status == 0 else {
            throw AtomicPrivateSettlementNativeVerifierErrorV1.nativeRejected(status)
        }
    }

    #if canImport(Darwin)
    private typealias CommitteeProofVerifierFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong
    ) -> Int32

    private typealias AuditorCapsuleVerifierFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<CChar>?, CUnsignedLong
    ) -> Int32

    private typealias AuditApprovalVerifierFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<CChar>?, CUnsignedLong
    ) -> Int32
    #endif
}

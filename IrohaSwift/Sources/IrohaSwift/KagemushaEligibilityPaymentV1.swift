import Foundation

/// Caller-owned trust needed to authenticate the BridgeFinalityProof embedded
/// in a finalized device-attestation policy view.
public struct OfflineDeviceFinalityTrustAnchorV1: Equatable, Sendable {
    public let networkId: NetworkId
    public let trustedHeightContextId: Data

    public init(networkId: NetworkId, trustedHeightContextId: Data) throws {
        guard trustedHeightContextId.count == 32,
              trustedHeightContextId[31] & 1 == 1 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDeviceFinalityTrustAnchorV1.contextId"
            )
        }
        self.networkId = networkId
        self.trustedHeightContextId = Data(trustedHeightContextId)
    }
}

/// Nominal ABI-22 input. Native preparation performs exact credential decoding
/// and issuer-signature validation before any payload is returned.
public struct KagemushaEligibilityCredentialV1: Equatable, Sendable {
    public let noritoArchive: Data
    fileprivate let verificationTrustAnchor: OfflineDeviceFinalityTrustAnchorV1?

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.maximumEligibilityCredentialArchiveBytesV1 else {
            throw KagemushaRecursiveSpendError.invalidArchive("eligibilityCredentialV1.size")
        }
        self.noritoArchive = Data(noritoArchive)
        verificationTrustAnchor = nil
    }

    init(
        validatedArchive: Data,
        trustAnchor: OfflineDeviceFinalityTrustAnchorV1
    ) throws {
        guard !validatedArchive.isEmpty,
              validatedArchive.count
                <= KagemushaRecursiveSpend.maximumEligibilityCredentialArchiveBytesV1 else {
            throw KagemushaRecursiveSpendError.invalidArchive("eligibilityCredentialV1.size")
        }
        noritoArchive = Data(validatedArchive)
        verificationTrustAnchor = trustAnchor
    }
}

/// Canonical native `PublicKey` archive naming the governed credential issuer.
public struct KagemushaEligibilityIssuerPublicKeyV1: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.maximumEligibilityIssuerArchiveBytesV1 else {
            throw KagemushaRecursiveSpendError.invalidArchive("eligibilityIssuerV1.size")
        }
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Finalized, freshness-bounded device-attestation policy view consumed by
/// first-delivery admission. Native code verifies its exact typed archive.
public struct KagemushaDeviceAttestationPolicyViewV1: Equatable, Sendable {
    public let noritoArchive: Data
    fileprivate let verificationTrustAnchor: OfflineDeviceFinalityTrustAnchorV1?

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.maximumDeviceAttestationPolicyViewArchiveBytesV1 else {
            throw KagemushaRecursiveSpendError.invalidArchive("policyViewV1.size")
        }
        self.noritoArchive = Data(noritoArchive)
        verificationTrustAnchor = nil
    }

    init(
        validatedArchive: Data,
        trustAnchor: OfflineDeviceFinalityTrustAnchorV1
    ) throws {
        guard !validatedArchive.isEmpty,
              validatedArchive.count
                <= KagemushaRecursiveSpend.maximumDeviceAttestationPolicyViewArchiveBytesV1 else {
            throw KagemushaRecursiveSpendError.invalidArchive("policyViewV1.size")
        }
        noritoArchive = Data(validatedArchive)
        verificationTrustAnchor = trustAnchor
    }
}

/// Canonical finalized policy bytes together with the native-authenticated
/// epoch, freshness, and Sumeragi finality binding used to trust them.
public struct OfflineDeviceFinalizedPolicyViewV1: Equatable, Sendable {
    public let policyView: KagemushaDeviceAttestationPolicyViewV1
    public let claims: OfflineDeviceEligibilityPolicyClaimsV1

    static let claimsProjectionMagic = Data([0x49, 0x44, 0x50, 0x56, 0x43, 0x4c, 0x31, 0])
    static let claimsProjectionBytes = 136

    init(
        policyView: KagemushaDeviceAttestationPolicyViewV1,
        nativeClaims: Data
    ) throws {
        guard nativeClaims.count == Self.claimsProjectionBytes,
              nativeClaims.prefix(8) == Self.claimsProjectionMagic else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDeviceFinalizedPolicyViewV1.frame"
            )
        }
        let policyEpoch = Self.readUInt64(nativeClaims, at: 8)
        let policyHash = Data(nativeClaims[16..<48])
        let freshnessDeadline = Self.readUInt64(nativeClaims, at: 48)
        let finalizedBlockHeight = Self.readUInt64(nativeClaims, at: 56)
        let finalizedBlockHash = Data(nativeClaims[64..<96])
        let finalizedBlockTimestamp = Self.readUInt64(nativeClaims, at: 96)
        let finalityEvidenceHash = Data(nativeClaims[104..<136])
        guard policyEpoch > 0,
              policyHash.contains(where: { $0 != 0 }),
              freshnessDeadline > finalizedBlockTimestamp,
              finalizedBlockHeight > 0,
              finalizedBlockHash.contains(where: { $0 != 0 }),
              finalizedBlockTimestamp > 0,
              finalityEvidenceHash.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDeviceFinalizedPolicyViewV1.claims"
            )
        }
        self.policyView = policyView
        claims = OfflineDeviceEligibilityPolicyClaimsV1(
            policyEpoch: policyEpoch,
            policyHash: policyHash,
            freshnessDeadlineMilliseconds: freshnessDeadline,
            finality: OfflineDevicePolicyFinalityClaimsV1(
                finalizedBlockHeight: finalizedBlockHeight,
                finalizedBlockHash: finalizedBlockHash,
                finalizedBlockTimestampMilliseconds: finalizedBlockTimestamp,
                finalityEvidenceHash: finalityEvidenceHash
            )
        )
    }

    private static func readUInt64(_ data: Data, at offset: Int) -> UInt64 {
        data[offset..<(offset + 8)].reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
    }
}

/// Unsigned ABI-22 eligibility wrapper around the unchanged ABI-21/V4 payment.
public struct KagemushaEligibilityPaymentPayloadV1: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        try Self.requireFraming(noritoArchive)
        guard let signingBytes = try NoritoNativeBridge.shared
            .kagemushaEligibilityPaymentSigningBytesV1(payloadArchive: noritoArchive),
              !signingBytes.isEmpty,
              signingBytes.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV2 else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        self.noritoArchive = Data(noritoArchive)
    }

    init(validatedArchive: Data) throws {
        try Self.requireFraming(validatedArchive)
        noritoArchive = Data(validatedArchive)
    }

    private static func requireFraming(_ archive: Data) throws {
        guard !archive.isEmpty,
              archive.count
                <= KagemushaRecursiveSpend.maximumEligibilityPaymentEnvelopeArchiveBytesV1 else {
            throw KagemushaRecursiveSpendError.invalidArchive("eligibilityPayloadV1.size")
        }
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.eligibilityPaymentPayloadWireNameV1,
            field: "eligibilityPayloadV1"
        )
    }
}

/// Statically validated ABI-22 eligibility payment. First delivery still
/// requires the signed receiver request, governed issuer, and current policy.
public struct KagemushaEligibilityPaymentEnvelopeV1: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        let validated = try KagemushaRecursiveSpend.validateEligibilityPaymentStaticV1(
            archive: noritoArchive
        )
        self = validated
    }

    init(validatedArchive: Data) throws {
        guard !validatedArchive.isEmpty,
              validatedArchive.count
                <= KagemushaRecursiveSpend.maximumEligibilityPaymentEnvelopeArchiveBytesV1 else {
            throw KagemushaRecursiveSpendError.invalidArchive("eligibilityEnvelopeV1.size")
        }
        try KagemushaRecursiveSpend.requireArchive(
            validatedArchive,
            schema: KagemushaRecursiveSpend.eligibilityPaymentEnvelopeWireNameV1,
            field: "eligibilityEnvelopeV1"
        )
        noritoArchive = Data(validatedArchive)
    }
}

public extension KagemushaRecursiveSpend {
    static func verifyFinalizedDeviceAttestationPolicyViewV1(
        archive: Data,
        trustAnchor: OfflineDeviceFinalityTrustAnchorV1,
        evaluationTimeMilliseconds: UInt64
    ) throws -> OfflineDeviceFinalizedPolicyViewV1 {
        let policyView = try verifyDeviceAttestationPolicyViewV1(
            archive: archive,
            trustAnchor: trustAnchor,
            evaluationTimeMilliseconds: evaluationTimeMilliseconds
        )
        guard let nativeClaims = try NoritoNativeBridge.shared
            .offlineDeviceAttestationPolicyViewClaimsV1(
                policyViewArchive: archive,
                expectedNetworkId: trustAnchor.networkId.bytes,
                trustedContextId: trustAnchor.trustedHeightContextId,
                evaluationTimeMilliseconds: evaluationTimeMilliseconds
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try OfflineDeviceFinalizedPolicyViewV1(
            policyView: policyView,
            nativeClaims: nativeClaims
        )
    }

    static func verifyDeviceAttestationPolicyViewV1(
        archive: Data,
        trustAnchor: OfflineDeviceFinalityTrustAnchorV1,
        evaluationTimeMilliseconds: UInt64
    ) throws -> KagemushaDeviceAttestationPolicyViewV1 {
        guard evaluationTimeMilliseconds > 0,
              let canonical = try NoritoNativeBridge.shared
                .offlineDeviceAttestationPolicyViewVerifyV1(
                    policyViewArchive: archive,
                    expectedNetworkId: trustAnchor.networkId.bytes,
                    trustedContextId: trustAnchor.trustedHeightContextId,
                    evaluationTimeMilliseconds: evaluationTimeMilliseconds
                ), canonical == archive else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaDeviceAttestationPolicyViewV1(
            validatedArchive: canonical,
            trustAnchor: trustAnchor
        )
    }

    static func verifyEligibilityCredentialV1(
        archive: Data,
        expectedIssuer: KagemushaEligibilityIssuerPublicKeyV1,
        currentPolicyView: KagemushaDeviceAttestationPolicyViewV1,
        evaluationTimeMilliseconds: UInt64
    ) throws -> KagemushaEligibilityCredentialV1 {
        guard evaluationTimeMilliseconds > 0,
              let trustAnchor = currentPolicyView.verificationTrustAnchor,
              let canonical = try NoritoNativeBridge.shared
                .offlineDeviceEligibilityCredentialVerifyV1(
                    credentialArchive: archive,
                    expectedIssuerArchive: expectedIssuer.noritoArchive,
                    policyViewArchive: currentPolicyView.noritoArchive,
                    expectedNetworkId: trustAnchor.networkId.bytes,
                    trustedContextId: trustAnchor.trustedHeightContextId,
                    evaluationTimeMilliseconds: evaluationTimeMilliseconds
                ), canonical == archive else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaEligibilityCredentialV1(
            validatedArchive: canonical,
            trustAnchor: trustAnchor
        )
    }

    /// Authenticate an IPN1 peer certificate through the governed eligibility
    /// issuer and current finalized policy, then return the exact device key
    /// authorized to sign that peer transcript.
    static func verifyEligibilityPeerCertificateV1(
        archive: Data,
        expectedIssuer: KagemushaEligibilityIssuerPublicKeyV1,
        currentPolicyView: KagemushaDeviceAttestationPolicyViewV1,
        evaluationTimeMilliseconds: UInt64
    ) throws -> KagemushaDevicePublicKeyV2 {
        guard evaluationTimeMilliseconds > 0,
              let trustAnchor = currentPolicyView.verificationTrustAnchor,
              let publicKey = try NoritoNativeBridge.shared
                .offlineDeviceEligibilityPeerCertificateVerifyV1(
                    credentialArchive: archive,
                    expectedIssuerArchive: expectedIssuer.noritoArchive,
                    policyViewArchive: currentPolicyView.noritoArchive,
                    expectedNetworkId: trustAnchor.networkId.bytes,
                    trustedContextId: trustAnchor.trustedHeightContextId,
                    evaluationTimeMilliseconds: evaluationTimeMilliseconds
                ), publicKey.count == KagemushaDevicePublicKeyV2.sec1ByteCount else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaDevicePublicKeyV2(sec1Bytes: publicKey)
    }

    static func prepareEligibilityPaymentV1(
        payment: KagemushaRecursiveSpendPeerPaymentV4,
        credential: KagemushaEligibilityCredentialV1,
        request: KagemushaRecipientPaymentRequest
    ) throws -> KagemushaEligibilityPaymentPayloadV1 {
        guard credential.verificationTrustAnchor != nil,
              let archive = try NoritoNativeBridge.shared.kagemushaEligibilityPaymentPrepareV1(
            paymentArchive: payment.noritoArchive,
            credentialArchive: credential.noritoArchive,
            requestArchive: request.archive
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaEligibilityPaymentPayloadV1(validatedArchive: archive)
    }

    static func eligibilityPaymentSigningBytesV1(
        payload: KagemushaEligibilityPaymentPayloadV1
    ) throws -> Data {
        guard let bytes = try NoritoNativeBridge.shared
            .kagemushaEligibilityPaymentSigningBytesV1(
                payloadArchive: payload.noritoArchive
            ), !bytes.isEmpty, bytes.count <= maximumPeerArchiveBytesV2 else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return bytes
    }

    static func finalizeEligibilityPaymentV1(
        payload: KagemushaEligibilityPaymentPayloadV1,
        signature: KagemushaDeviceSignatureV2
    ) throws -> KagemushaEligibilityPaymentEnvelopeV1 {
        guard let archive = try NoritoNativeBridge.shared
            .kagemushaEligibilityPaymentFinalizeV1(
                payloadArchive: payload.noritoArchive,
                signature: signature.rawBytes
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaEligibilityPaymentEnvelopeV1(validatedArchive: archive)
    }

    static func validateEligibilityPaymentStaticV1(
        _ envelope: KagemushaEligibilityPaymentEnvelopeV1
    ) throws -> KagemushaEligibilityPaymentEnvelopeV1 {
        try validateEligibilityPaymentStaticV1(archive: envelope.noritoArchive)
    }

    static func validateEligibilityPaymentFirstDeliveryV1(
        envelope: KagemushaEligibilityPaymentEnvelopeV1,
        request: KagemushaRecipientPaymentRequest,
        expectedIssuer: KagemushaEligibilityIssuerPublicKeyV1,
        currentPolicyView: KagemushaDeviceAttestationPolicyViewV1,
        receivedAtMilliseconds: UInt64
    ) throws -> KagemushaRecursiveSpendPeerPaymentV4 {
        guard receivedAtMilliseconds > 0,
              let trustAnchor = currentPolicyView.verificationTrustAnchor,
              let payment = try NoritoNativeBridge.shared
                .kagemushaEligibilityPaymentValidateFirstDeliveryFinalizedV1(
                    envelopeArchive: envelope.noritoArchive,
                    requestArchive: request.archive,
                    expectedIssuerArchive: expectedIssuer.noritoArchive,
                    policyViewArchive: currentPolicyView.noritoArchive,
                    expectedNetworkId: trustAnchor.networkId.bytes,
                    trustedContextId: trustAnchor.trustedHeightContextId,
                    receivedAtMilliseconds: receivedAtMilliseconds
                ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendPeerPaymentV4(noritoArchive: payment)
    }

    fileprivate static func validateEligibilityPaymentStaticV1(
        archive: Data
    ) throws -> KagemushaEligibilityPaymentEnvelopeV1 {
        guard !archive.isEmpty,
              archive.count <= maximumEligibilityPaymentEnvelopeArchiveBytesV1,
              let canonical = try NoritoNativeBridge.shared
                .kagemushaEligibilityPaymentValidateStaticV1(envelopeArchive: archive),
              canonical == archive else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaEligibilityPaymentEnvelopeV1(validatedArchive: canonical)
    }
}

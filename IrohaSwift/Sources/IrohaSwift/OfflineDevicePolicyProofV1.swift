import Foundation

/// Caller-owned durable checkpoint for paged device-policy finality sync.
///
/// Persist the complete value atomically after native verification and before
/// requesting the next page. Neither Torii nor the SDK supplies its own trust
/// root or storage location.
public struct OfflineDevicePolicyCheckpointV1: Equatable, Sendable {
    public let networkId: NetworkId
    public let height: UInt64
    public let heightContextId: Data

    public init(networkId: NetworkId, height: UInt64, heightContextId: Data) throws {
        guard height > 0,
              heightContextId.count == 32,
              heightContextId[31] & 1 == 1 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDevicePolicyCheckpointV1"
            )
        }
        self.networkId = networkId
        self.height = height
        self.heightContextId = Data(heightContextId)
    }
}

/// One natively verified page and the exact checkpoint eligible for durable promotion.
public struct OfflineDevicePolicyVerifiedPageV1: Equatable, Sendable {
    public let evaluatedCheckpoint: OfflineDevicePolicyCheckpointV1
    public let moreAvailable: Bool
    public let terminalPolicyView: KagemushaDeviceAttestationPolicyViewV1?

    static let projectionMagic = Data([0x49, 0x44, 0x50, 0x50, 0x56, 0x31, 0, 0])
    static let fixedProjectionBytes = 56

    init(nativeProjection: Data, expectedNetworkId: NetworkId) throws {
        guard nativeProjection.count >= Self.fixedProjectionBytes,
              nativeProjection.count
                <= KagemushaRecursiveSpend.maximumDevicePolicyVerifiedPageBytesV1,
              nativeProjection.prefix(8) == Self.projectionMagic else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDevicePolicyVerifiedPageV1.frame"
            )
        }
        let height = nativeProjection[8..<16].reduce(UInt64(0)) {
            ($0 << 8) | UInt64($1)
        }
        let context = Data(nativeProjection[16..<48])
        let moreByte = nativeProjection[48]
        guard moreByte <= 1,
              nativeProjection[49..<52].allSatisfy({ $0 == 0 }) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDevicePolicyVerifiedPageV1.flags"
            )
        }
        let policyLength = nativeProjection[52..<56].reduce(UInt32(0)) {
            ($0 << 8) | UInt32($1)
        }
        guard let expectedLength = Int(exactly: policyLength).flatMap({ length in
            Self.fixedProjectionBytes.addingReportingOverflow(length).overflow
                ? nil : Self.fixedProjectionBytes + length
        }), expectedLength == nativeProjection.count else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDevicePolicyVerifiedPageV1.length"
            )
        }
        let moreAvailable = moreByte == 1
        guard moreAvailable == (policyLength == 0) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDevicePolicyVerifiedPageV1.pagination"
            )
        }
        let checkpoint = try OfflineDevicePolicyCheckpointV1(
            networkId: expectedNetworkId,
            height: height,
            heightContextId: context
        )
        let policy: KagemushaDeviceAttestationPolicyViewV1?
        if policyLength == 0 {
            policy = nil
        } else {
            let trust = try OfflineDeviceFinalityTrustAnchorV1(
                networkId: expectedNetworkId,
                trustedHeightContextId: context
            )
            policy = try KagemushaDeviceAttestationPolicyViewV1(
                validatedArchive: Data(nativeProjection[56...]),
                trustAnchor: trust
            )
        }
        evaluatedCheckpoint = checkpoint
        self.moreAvailable = moreAvailable
        terminalPolicyView = policy
    }
}

public extension KagemushaRecursiveSpend {
    /// Encode the exact typed body for one policy-proof page request.
    static func makeOfflineDevicePolicyProofRequestV1(
        checkpoint: OfflineDevicePolicyCheckpointV1
    ) throws -> Data {
        guard let request = try NoritoNativeBridge.shared.offlineDevicePolicyProofRequestV1(
            trustedCheckpointHeight: checkpoint.height,
            trustedCheckpointContextId: checkpoint.heightContextId
        ), !request.isEmpty else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return request
    }

    /// Verify one canonical proof page from caller-retained trust.
    ///
    /// The caller must atomically persist `evaluatedCheckpoint` before using it
    /// to request another page. This primitive intentionally owns no storage.
    static func verifyOfflineDevicePolicyProofPageV1(
        archive: Data,
        checkpoint: OfflineDevicePolicyCheckpointV1,
        evaluationTimeMilliseconds: UInt64
    ) throws -> OfflineDevicePolicyVerifiedPageV1 {
        guard evaluationTimeMilliseconds > 0,
              !archive.isEmpty,
              archive.count <= maximumDevicePolicyProofPageArchiveBytesV1,
              let projection = try NoritoNativeBridge.shared.offlineDevicePolicyProofVerifyV1(
                  proofPageArchive: archive,
                  expectedNetworkId: checkpoint.networkId.bytes,
                  trustedCheckpointHeight: checkpoint.height,
                  trustedCheckpointContextId: checkpoint.heightContextId,
                  evaluationTimeMilliseconds: evaluationTimeMilliseconds
              ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try OfflineDevicePolicyVerifiedPageV1(
            nativeProjection: projection,
            expectedNetworkId: checkpoint.networkId
        )
    }
}

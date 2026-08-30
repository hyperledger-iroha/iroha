import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineDevicePolicyFinalityV1Tests: XCTestCase {
    private func projection(
        height: UInt64 = 17,
        context: Data = Data(repeating: 0x23, count: 32),
        moreAvailable: Bool,
        policy: Data = Data()
    ) -> Data {
        var result = OfflineDevicePolicyVerifiedPageV1.projectionMagic
        result.append(contentsOf: withUnsafeBytes(of: height.bigEndian, Array.init))
        result.append(context)
        result.append(moreAvailable ? 1 : 0)
        result.append(contentsOf: [0, 0, 0])
        result.append(contentsOf: withUnsafeBytes(of: UInt32(policy.count).bigEndian, Array.init))
        result.append(policy)
        return result
    }

    private func claimsProjection(
        epoch: UInt64 = 7,
        freshnessDeadline: UInt64 = 2_000_000,
        finalizedHeight: UInt64 = 44,
        finalizedTimestamp: UInt64 = 1_000_000
    ) -> Data {
        var result = OfflineDeviceFinalizedPolicyViewV1.claimsProjectionMagic
        result.append(contentsOf: withUnsafeBytes(of: epoch.bigEndian, Array.init))
        result.append(Data(repeating: 0x47, count: 32))
        result.append(contentsOf: withUnsafeBytes(of: freshnessDeadline.bigEndian, Array.init))
        result.append(contentsOf: withUnsafeBytes(of: finalizedHeight.bigEndian, Array.init))
        result.append(Data(repeating: 0x49, count: 32))
        result.append(contentsOf: withUnsafeBytes(of: finalizedTimestamp.bigEndian, Array.init))
        result.append(Data(repeating: 0x4b, count: 32))
        return result
    }

    func testTrustAnchorRequiresExactMarkedContextAndOwnsItsBytes() throws {
        let network = try NetworkId(bytes: Data(repeating: 0x11, count: 32))
        var context = Data(repeating: 0x23, count: 32)
        let anchor = try OfflineDeviceFinalityTrustAnchorV1(
            networkId: network,
            trustedHeightContextId: context
        )
        context[0] = 0
        XCTAssertEqual(anchor.trustedHeightContextId, Data(repeating: 0x23, count: 32))

        XCTAssertThrowsError(try OfflineDeviceFinalityTrustAnchorV1(
            networkId: network,
            trustedHeightContextId: Data(repeating: 0x23, count: 31)
        ))
        XCTAssertThrowsError(try OfflineDeviceFinalityTrustAnchorV1(
            networkId: network,
            trustedHeightContextId: Data(repeating: 0x22, count: 32)
        ))
    }

    func testDurableCheckpointAndVerifiedProjectionFailClosed() throws {
        let network = try NetworkId(bytes: Data(repeating: 0x11, count: 32))
        let checkpoint = try OfflineDevicePolicyCheckpointV1(
            networkId: network,
            height: 17,
            heightContextId: Data(repeating: 0x23, count: 32)
        )
        XCTAssertThrowsError(try OfflineDevicePolicyCheckpointV1(
            networkId: network,
            height: 0,
            heightContextId: Data(repeating: 0x23, count: 32)
        ))

        let intermediate = try OfflineDevicePolicyVerifiedPageV1(
            nativeProjection: projection(moreAvailable: true),
            expectedNetworkId: network
        )
        XCTAssertEqual(intermediate.evaluatedCheckpoint, checkpoint)
        XCTAssertTrue(intermediate.moreAvailable)
        XCTAssertNil(intermediate.terminalPolicyView)

        let policy = Data([0x91, 0x92, 0x93])
        let terminal = try OfflineDevicePolicyVerifiedPageV1(
            nativeProjection: projection(moreAvailable: false, policy: policy),
            expectedNetworkId: network
        )
        XCTAssertFalse(terminal.moreAvailable)
        XCTAssertEqual(terminal.terminalPolicyView?.noritoArchive, policy)

        var badMagic = projection(moreAvailable: true)
        badMagic[0] ^= 1
        XCTAssertThrowsError(try OfflineDevicePolicyVerifiedPageV1(
            nativeProjection: badMagic,
            expectedNetworkId: network
        ))
        var badReserved = projection(moreAvailable: true)
        badReserved[49] = 1
        XCTAssertThrowsError(try OfflineDevicePolicyVerifiedPageV1(
            nativeProjection: badReserved,
            expectedNetworkId: network
        ))
        XCTAssertThrowsError(try OfflineDevicePolicyVerifiedPageV1(
            nativeProjection: projection(moreAvailable: false),
            expectedNetworkId: network
        ))
        var badLength = projection(moreAvailable: false, policy: policy)
        badLength[55] = 4
        XCTAssertThrowsError(try OfflineDevicePolicyVerifiedPageV1(
            nativeProjection: badLength,
            expectedNetworkId: network
        ))
    }

    func testFinalizedPolicyClaimsProjectionIsTypedAndFailClosed() throws {
        let network = try NetworkId(bytes: Data(repeating: 0x11, count: 32))
        let trust = try OfflineDeviceFinalityTrustAnchorV1(
            networkId: network,
            trustedHeightContextId: Data(repeating: 0x23, count: 32)
        )
        let policy = try KagemushaDeviceAttestationPolicyViewV1(
            validatedArchive: Data([0x91, 0x92]),
            trustAnchor: trust
        )
        let finalized = try OfflineDeviceFinalizedPolicyViewV1(
            policyView: policy,
            nativeClaims: claimsProjection()
        )
        XCTAssertEqual(finalized.policyView.noritoArchive, Data([0x91, 0x92]))
        XCTAssertEqual(finalized.claims.policyEpoch, 7)
        XCTAssertEqual(finalized.claims.finality.finalizedBlockHeight, 44)

        var badMagic = claimsProjection()
        badMagic[0] ^= 1
        XCTAssertThrowsError(try OfflineDeviceFinalizedPolicyViewV1(
            policyView: policy,
            nativeClaims: badMagic
        ))
        XCTAssertThrowsError(try OfflineDeviceFinalizedPolicyViewV1(
            policyView: policy,
            nativeClaims: claimsProjection(freshnessDeadline: 1_000_000)
        ))
        XCTAssertThrowsError(try OfflineDeviceFinalizedPolicyViewV1(
            policyView: policy,
            nativeClaims: claimsProjection(finalizedHeight: 0)
        ))
    }

    func testProductionSurfaceUsesAuthenticatedQueryAndFinalizedNativeGate() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let torii = try String(
            contentsOf: root.appendingPathComponent("Sources/IrohaSwift/ToriiClient.swift"),
            encoding: .utf8
        )
        let eligibility = try String(
            contentsOf: root.appendingPathComponent(
                "Sources/IrohaSwift/KagemushaEligibilityPaymentV1.swift"
            ),
            encoding: .utf8
        )
        let native = try String(
            contentsOf: root.appendingPathComponent(
                "Sources/IrohaSwift/KagemushaRecursiveSpendV2Native.swift"
            ),
            encoding: .utf8
        )
        XCTAssertTrue(torii.contains("/v1/offline/device-attestation-policy"))
        XCTAssertTrue(torii.contains("/v1/offline/device-attestation-policy/proof"))
        XCTAssertTrue(torii.contains("makeCanonicalAccountRequest("))
        XCTAssertTrue(torii.contains("method: .post"))
        XCTAssertTrue(torii.contains("maximumDevicePolicyProofPageArchiveBytesV1"))
        XCTAssertTrue(torii.contains("verifyOfflineDevicePolicyProofPageV1("))
        XCTAssertTrue(torii.contains("verifyDeviceAttestationPolicyViewV1("))
        XCTAssertTrue(torii.contains("getOfflineDeviceAttestationFinalizedPolicyViewV1("))
        XCTAssertTrue(torii.contains("verifyFinalizedDeviceAttestationPolicyViewV1("))
        XCTAssertTrue(eligibility.contains(
            "kagemushaEligibilityPaymentValidateFirstDeliveryFinalizedV1("
        ))
        XCTAssertTrue(eligibility.contains("verificationTrustAnchor"))
        XCTAssertTrue(native.contains(
            "connect_norito_offline_device_attestation_policy_view_claims_v1"
        ))
    }
}

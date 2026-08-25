import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineCashV1Tests: XCTestCase {
    func testReleaseProbeRequiresBothAuthenticatedIdentities() {
        let status = OfflineCashReleaseStatusV1.installed()
        XCTAssertFalse(status.available)
        XCTAssertNil(status.installedReleaseId)
        XCTAssertNil(status.installedArtifactManifestSHA256)
        XCTAssertTrue(status.blocker?.hasPrefix("offline-cash-v1-") == true)
    }

    func testExactFirstReleaseCapsAndPeerPrefix() {
        XCTAssertEqual(OfflineCashReleaseStatusV1.requiredNativeBridgeABIVersion, 22)
        XCTAssertEqual(OfflineCashPaymentRequestV1.maximumCanonicalBytes, 768)
        XCTAssertEqual(OfflineCashPaymentV1.maximumCanonicalBytes, 7_936)
        XCTAssertEqual(OfflineCashAcknowledgementV1.maximumCanonicalBytes, 256)
        XCTAssertEqual(OfflineCashPeerAdapterV1.textPrefix, "kgm2:")
        XCTAssertEqual(OfflineCashPeerAdapterV1.maximumTextSessionBytes, 12_288)
        XCTAssertEqual(OfflineCashArtifactSetInstallerV1.requiredArtifactCount, 34)
        let expectedArtifactRoles: [OfflineCashArtifactRoleV1] = [
            .paramsEq,
            .paramsEp,
            .statePkEq,
            .stateVkEq,
            .statePkEp,
            .stateVkEp,
            .guardUsePkEq,
            .guardUseVkEq,
            .guardUsePkEp,
            .guardUseVkEp,
            .platformBindPkEq,
            .platformBindVkEq,
            .platformBindPkEp,
            .platformBindVkEp,
            .androidKeyCertPkEq,
            .androidKeyCertVkEq,
            .androidKeyCertPkEp,
            .androidKeyCertVkEp,
            .guardBundlePkEq,
            .guardBundleVkEq,
            .guardBundlePkEp,
            .guardBundleVkEp,
            .p256V3PkEq,
            .p256V3VkEq,
            .p256V3PkEp,
            .p256V3VkEp,
            .stateLeafPkEq,
            .stateLeafVkEq,
            .stateLeafPkEp,
            .stateLeafVkEp,
            .guardBundleLeafPkEq,
            .guardBundleLeafVkEq,
            .guardBundleLeafPkEp,
            .guardBundleLeafVkEp,
        ]
        XCTAssertEqual(OfflineCashArtifactRoleV1.allCases, expectedArtifactRoles)
        XCTAssertEqual(
            expectedArtifactRoles.map(\.rawValue),
            (0..<OfflineCashArtifactSetInstallerV1.requiredArtifactCount).map(UInt8.init)
        )
    }

    func testWalletSessionVocabularyDoesNotClaimDeviceCommit() {
        XCTAssertEqual(
            [
                OfflineCashWalletSessionStateV1.unavailable,
                .receiveRequestReady,
                .paymentVerified,
                .acknowledgementVerified,
            ].map(\.rawValue),
            [
                "unavailable",
                "receiveRequestReady",
                "paymentVerified",
                "acknowledgementVerified",
            ]
        )
        XCTAssertEqual(
            [
                OfflineCashWalletSessionEventV1.paymentVerified,
                .paymentVerificationReplay,
                .acknowledgementVerified,
                .acknowledgementVerificationReplay,
            ].map(\.rawValue),
            [
                "paymentVerified",
                "paymentVerificationReplay",
                "acknowledgementVerified",
                "acknowledgementVerificationReplay",
            ]
        )
    }

    func testWalletSessionConstructionRequiresTypedNetworkAndAssetContext() {
        let initializer: (
            OfflineCashPaymentRequestV1,
            Data,
            Data,
            NetworkId,
            String
        ) throws -> OfflineCashWalletSessionV1 = OfflineCashWalletSessionV1.init(
            request:expectedReleaseId:expectedArtifactManifestSHA256:
                expectedNetworkID:expectedAssetDefinitionID:
        )

        _ = initializer
    }
}

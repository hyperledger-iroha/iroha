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

    func testVerificationSessionVocabularyDoesNotClaimDeviceCommit() {
        XCTAssertEqual(
            [
                OfflineCashVerificationSessionStateV1.unavailable,
                .requestVerified,
                .paymentVerified,
                .acknowledgementVerified,
            ].map(\.rawValue),
            [
                "unavailable",
                "requestVerified",
                "paymentVerified",
                "acknowledgementVerified",
            ]
        )
        XCTAssertEqual(
            [
                OfflineCashVerificationSessionEventV1.paymentVerified,
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

    func testVerificationSessionConstructionRequiresTypedNetworkAndAssetContext() {
        let initializer: (
            OfflineCashPaymentRequestV1,
            Data,
            Data,
            NetworkId,
            String
        ) throws -> OfflineCashVerificationSessionV1 = OfflineCashVerificationSessionV1.init(
            request:expectedReleaseId:expectedArtifactManifestSHA256:
                expectedNetworkID:expectedAssetDefinitionID:
        )

        _ = initializer
    }

    func testWalletFacadeHasExactStableStatesAndAlwaysFailsClosed() throws {
        XCTAssertEqual(
            OfflineCashWalletSessionStateV1.allCases.map(\.rawValue),
            Array(UInt8(0)...UInt8(12))
        )
        XCTAssertEqual(
            OfflineCashWalletSessionStateV1.allCases,
            [
                .unavailable,
                .setupRequired,
                .empty,
                .topUpPending,
                .available,
                .receiveRequestReady,
                .sendPreparing,
                .paymentCommitted,
                .awaitingAcknowledgement,
                .received,
                .redeemPending,
                .recoveryRequired,
                .error,
            ]
        )
        XCTAssertEqual(OfflineCashWalletSessionStatusV1.unavailable.rawValue, 0)
        XCTAssertEqual(
            OfflineCashWalletSessionActionV1.allCases.map(\.rawValue),
            Array(UInt8(0)...UInt8(8))
        )
        XCTAssertEqual(
            OfflineCashWalletSessionActionV1.allCases,
            [
                .setUp,
                .topUp,
                .createReceiveRequest,
                .prepareSend,
                .commitPayment,
                .recordAcknowledgementEvidence,
                .receivePayment,
                .redeem,
                .recover,
            ]
        )
        XCTAssertThrowsError(try OfflineCashWalletSessionV1.open()) { error in
            XCTAssertEqual(error as? OfflineCashWalletSessionErrorV1, .unavailable)
        }

        let session = OfflineCashWalletSessionV1.unavailable()
        XCTAssertEqual(session.status, .unavailable)
        XCTAssertEqual(session.state, .unavailable)
        for action in OfflineCashWalletSessionActionV1.allCases {
            XCTAssertThrowsError(try session.attempt(action)) { error in
                XCTAssertEqual(error as? OfflineCashWalletSessionErrorV1, .unavailable)
            }
            XCTAssertEqual(session.state, .unavailable)
        }
    }
}

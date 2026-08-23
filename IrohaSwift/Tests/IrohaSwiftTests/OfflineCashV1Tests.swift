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
    }
}

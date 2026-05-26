import XCTest
@testable import IrohaSwift

final class OfflineRevocationBundleTests: XCTestCase {
    func testPolicyLookupsNormalizeAccountsVerdictsAndAssets() throws {
        let limit = try ToriiOfflineAssetSendLimit(
            assetDefinitionId: "pkr#offline",
            dailySendLimit: "10.00",
            monthlySendLimit: "100.00"
        )
        let bundle = ToriiOfflineRevocationBundle(
            issuedAtMs: 1_000,
            expiresAtMs: 2_000,
            verdictIds: [" Verdict-1 "],
            blacklistedAccountIds: ["i105blacklisted"],
            assetSendLimits: [limit],
            issuerSignatureBase64: Data([1]).base64EncodedString()
        )

        XCTAssertTrue(bundle.blacklistsAccount(" i105blacklisted "))
        XCTAssertTrue(bundle.revokesVerdict("verdict-1"))
        XCTAssertEqual(bundle.sendLimit(assetDefinitionId: "PKR#offline")?.dailySendLimit, "10.00")
        XCTAssertFalse(bundle.blacklistsAccount("i105allowed"))
        XCTAssertFalse(bundle.revokesVerdict(nil))
    }
}

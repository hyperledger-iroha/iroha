import XCTest
@testable import IrohaSwift

final class OfflineRevocationBundleTests: XCTestCase {
    func testPolicyLookupsUseExactAccountsVerdictsAndAssets() throws {
        let limit = try ToriiOfflineAssetSendLimit(
            assetDefinitionId: "pkr#offline",
            dailySendLimit: "10.00",
            monthlySendLimit: "100.00"
        )
        let bundle = try ToriiOfflineRevocationBundle(
            issuedAtMs: 1_000,
            expiresAtMs: 2_000,
            verdictIds: ["verdict-1"],
            blacklistedAccountIds: ["i105blacklisted"],
            assetSendLimits: [limit],
            issuerSignatureBase64: Self.signatureBase64()
        )

        XCTAssertTrue(bundle.blacklistsAccount("i105blacklisted"))
        XCTAssertTrue(bundle.revokesVerdict("verdict-1"))
        XCTAssertEqual(bundle.sendLimit(assetDefinitionId: "pkr#offline")?.dailySendLimit, "10.00")
        XCTAssertFalse(bundle.blacklistsAccount(" i105blacklisted "))
        XCTAssertFalse(bundle.blacklistsAccount("I105blacklisted"))
        XCTAssertFalse(bundle.revokesVerdict(" verdict-1 "))
        XCTAssertFalse(bundle.revokesVerdict("Verdict-1"))
        XCTAssertNil(bundle.sendLimit(assetDefinitionId: " pkr#offline "))
        XCTAssertNil(bundle.sendLimit(assetDefinitionId: "PKR#offline"))
        XCTAssertFalse(bundle.blacklistsAccount("i105allowed"))
        XCTAssertFalse(bundle.revokesVerdict(nil))
    }

    func testRevocationBundleRejectsMalformedPolicyFields() throws {
        func bundle(
            verdictIds: [String] = ["verdict-1"],
            blacklistedAccountIds: [String] = ["i105blacklisted"],
            assetDefinitionId: String = "pkr#offline",
            dailySendLimit: String = "10.00",
            monthlySendLimit: String = "100.00",
            issuedAtMs: UInt64 = 1_000,
            expiresAtMs: UInt64 = 2_000,
            issuerSignatureBase64: String = Self.signatureBase64()
        ) throws -> ToriiOfflineRevocationBundle {
            let limit = try ToriiOfflineAssetSendLimit(
                assetDefinitionId: assetDefinitionId,
                dailySendLimit: dailySendLimit,
                monthlySendLimit: monthlySendLimit
            )
            return try ToriiOfflineRevocationBundle(
                issuedAtMs: issuedAtMs,
                expiresAtMs: expiresAtMs,
                verdictIds: verdictIds,
                blacklistedAccountIds: blacklistedAccountIds,
                assetSendLimits: [limit],
                issuerSignatureBase64: issuerSignatureBase64
            )
        }

        XCTAssertNoThrow(try ToriiOfflineCashCodec.revocationBundleUnsignedPayload(try bundle()))

        for invalidBundle in [
            { try bundle(verdictIds: [" verdict-1"]) },
            { try bundle(verdictIds: ["verdict-1\n"]) },
            { try bundle(blacklistedAccountIds: [" i105blacklisted"]) },
            { try bundle(blacklistedAccountIds: ["i105blacklisted "]) },
            { try bundle(assetDefinitionId: " pkr#offline") },
            { try bundle(assetDefinitionId: "pkr#offline ") },
            { try bundle(dailySendLimit: "-1.00") },
            { try bundle(monthlySendLimit: "-1.00") },
            { try bundle(expiresAtMs: 1_000) },
            { try bundle(issuerSignatureBase64: Data([1]).base64EncodedString()) },
        ] {
            XCTAssertThrowsError(try invalidBundle())
        }

        let json = """
        {
          "issued_at_ms": 1000,
          "expires_at_ms": 2000,
          "verdict_ids": [" verdict-1"],
          "blacklisted_account_ids": ["i105blacklisted"],
          "asset_send_limits": [{
            "asset_definition_id": "pkr#offline",
            "daily_send_limit": "10.00",
            "monthly_send_limit": "100.00"
          }],
          "issuer_signature_base64": "\(Self.signatureBase64())"
        }
        """.data(using: .utf8)!
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiOfflineRevocationBundle.self, from: json))
    }

    private static func signatureBase64() -> String {
        Data(repeating: 1, count: 64).base64EncodedString()
    }
}

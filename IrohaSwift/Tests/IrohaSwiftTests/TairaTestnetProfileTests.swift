import XCTest
@testable import IrohaSwift

final class TairaTestnetProfileTests: XCTestCase {
    private let networkIdLiteral =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"

    func testExactPublicMetadata() {
        XCTAssertEqual(TairaTestnetProfile.toriiBaseURL.absoluteString, "https://taira.sora.org")
        XCTAssertEqual(TairaTestnetProfile.chainId, "fc56984b-2be7-431d-840e-21514d1883f0")
        XCTAssertEqual(TairaTestnetProfile.i105Discriminant, 369)
        XCTAssertEqual(TairaTestnetProfile.kagemushaAssetDefinitionId, "7ZepsJTHCVLKsrFFNZGSRGZgvBhv")
        XCTAssertEqual(TairaTestnetProfile.kagemushaAssetAlias, "ds#boi.is")
        XCTAssertEqual(TairaTestnetProfile.kagemushaAssetScale, 2)
        XCTAssertEqual(TairaTestnetProfile.xorAssetDefinitionId, "6TEAJqbb8oEPmLncoNiMRbLEK6tw")
        XCTAssertEqual(TairaTestnetProfile.xorAssetAlias, "xor#universal")
        XCTAssertEqual(TairaTestnetProfile.xorAssetScale, 9)
    }

    func testClientRequiresCallerSuppliedDeployedNetworkId() throws {
        let networkId = try NetworkId(literal: networkIdLiteral)
        let client = TairaTestnetProfile.makeClient(deployedNetworkId: networkId)

        XCTAssertEqual(client.baseURL.absoluteString, "https://taira.sora.org/")
        XCTAssertEqual(client.localSigningContext?.networkId, networkId)
    }
}

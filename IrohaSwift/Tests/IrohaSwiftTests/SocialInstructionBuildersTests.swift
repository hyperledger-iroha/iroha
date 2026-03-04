import XCTest
@testable import IrohaSwift

final class SocialInstructionBuildersTests: XCTestCase {
    private let fixturePepper = "pepper-social-v1"
    private let fixtureDigestHex = "E8A8D90BF72F280BBB4AB6D1F759521D29A08DA83CCFBB3E2EE0EDE22606FB9B"
    private let fixtureDigestLiteral = "hash:E8A8D90BF72F280BBB4AB6D1F759521D29A08DA83CCFBB3E2EE0EDE22606FB9B#E0A4"
    private let fixtureAmount = "1.0000000000"

    func testSocialKeyedHashCanonicalizesHexToLiteral() throws {
        let binding = try SocialKeyedHash(pepperId: fixturePepper, digest: fixtureDigestHex)
        XCTAssertEqual(binding.pepperId, fixturePepper)
        XCTAssertEqual(binding.digest, fixtureDigestLiteral)
    }

    func testSocialKeyedHashAcceptsCanonicalLiteral() throws {
        let binding = try SocialKeyedHash(pepperId: fixturePepper, digest: fixtureDigestLiteral)
        XCTAssertEqual(binding.pepperId, fixturePepper)
        XCTAssertEqual(binding.digest, fixtureDigestLiteral)
    }

    func testClaimTwitterFollowRewardInstructionShape() throws {
        let binding = try SocialKeyedHash(pepperId: fixturePepper, digest: fixtureDigestHex)
        let json = try SocialInstructionBuilders.claimTwitterFollowReward(binding: binding)
        let object = try JSONSerialization.jsonObject(with: json.data, options: []) as? [String: Any]

        guard
            let root = object,
            let inner = root["ClaimTwitterFollowReward"] as? [String: Any],
            let bindingHash = inner["binding_hash"] as? [String: Any]
        else {
            return XCTFail("Unexpected JSON structure: \(String(describing: object))")
        }

        XCTAssertEqual(bindingHash["pepper_id"] as? String, fixturePepper)
        XCTAssertEqual(bindingHash["digest"] as? String, fixtureDigestLiteral)
    }

    func testSendToTwitterInstructionShape() throws {
        let binding = try SocialKeyedHash(pepperId: fixturePepper, digest: fixtureDigestHex)
        let json = try SocialInstructionBuilders.sendToTwitter(binding: binding, amount: fixtureAmount)
        let object = try JSONSerialization.jsonObject(with: json.data, options: []) as? [String: Any]

        guard
            let root = object,
            let inner = root["SendToTwitter"] as? [String: Any],
            let bindingHash = inner["binding_hash"] as? [String: Any]
        else {
            return XCTFail("Unexpected JSON structure: \(String(describing: object))")
        }

        XCTAssertEqual(inner["amount"] as? String, fixtureAmount)
        XCTAssertEqual(bindingHash["pepper_id"] as? String, fixturePepper)
        XCTAssertEqual(bindingHash["digest"] as? String, fixtureDigestLiteral)
    }

    func testCancelTwitterEscrowInstructionShape() throws {
        let binding = try SocialKeyedHash(pepperId: fixturePepper, digest: fixtureDigestHex)
        let json = try SocialInstructionBuilders.cancelTwitterEscrow(binding: binding)
        let object = try JSONSerialization.jsonObject(with: json.data, options: []) as? [String: Any]

        guard
            let root = object,
            let inner = root["CancelTwitterEscrow"] as? [String: Any],
            let bindingHash = inner["binding_hash"] as? [String: Any]
        else {
            return XCTFail("Unexpected JSON structure: \(String(describing: object))")
        }

        XCTAssertEqual(bindingHash["pepper_id"] as? String, fixturePepper)
        XCTAssertEqual(bindingHash["digest"] as? String, fixtureDigestLiteral)
    }

    func testRejectsInvalidDigest() {
        XCTAssertThrowsError(try SocialKeyedHash(pepperId: fixturePepper, digest: "deadbeef")) { error in
            guard case SocialInstructionBuilderError.invalidDigest = error else {
                return XCTFail("Expected invalidDigest error")
            }
        }
    }

    func testRejectsEmptyAmount() throws {
        let binding = try SocialKeyedHash(pepperId: fixturePepper, digest: fixtureDigestHex)
        XCTAssertThrowsError(try SocialInstructionBuilders.sendToTwitter(binding: binding, amount: "   ")) { error in
            guard case SocialInstructionBuilderError.invalidAmount = error else {
            return XCTFail("Expected invalidAmount error")
            }
        }
    }

    func testIrohaSDKConvenienceHelpersProduceSamePayloads() throws {
        let sdk = IrohaSDK(baseURL: URL(string: "https://example.test")!)
        let binding = try SocialKeyedHash(pepperId: fixturePepper, digest: fixtureDigestHex)

        let claimSdk = try sdk.buildClaimTwitterFollowReward(binding: binding)
        let claimDirect = try SocialInstructionBuilders.claimTwitterFollowReward(binding: binding)
        XCTAssertEqual(claimSdk.data, claimDirect.data)

        let sendSdk = try sdk.buildSendToTwitter(binding: binding, amount: fixtureAmount)
        let sendDirect = try SocialInstructionBuilders.sendToTwitter(binding: binding, amount: fixtureAmount)
        XCTAssertEqual(sendSdk.data, sendDirect.data)

        let cancelSdk = try sdk.buildCancelTwitterEscrow(binding: binding)
        let cancelDirect = try SocialInstructionBuilders.cancelTwitterEscrow(binding: binding)
        XCTAssertEqual(cancelSdk.data, cancelDirect.data)
    }
}

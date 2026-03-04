import XCTest
@testable import IrohaSwift

@available(macOS 10.15, iOS 13.0, *)
final class AccountIdTests: XCTestCase {

    func testDefaultNetworkPrefix() {
        XCTAssertEqual(AccountId.defaultNetworkPrefix, 0x02F1)
    }

    func testMakeProducesEd0120Format() {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let accountId = AccountId.make(publicKey: publicKey, domain: "wonderland")

        XCTAssertTrue(accountId.hasPrefix("ed0120"))
        XCTAssertTrue(accountId.hasSuffix("@wonderland"))
        XCTAssertEqual(
            accountId,
            "ed0120ABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABABAB@wonderland"
        )
    }

    func testMakeIH58ProducesValidFormat() throws {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let accountId = try AccountId.makeIH58(publicKey: publicKey, domain: "wonderland")

        // IH58 format should NOT start with ed0120
        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))

        // Should be parseable back
        // IH58 addresses are base58 encoded, typically 40-50 chars
        XCTAssertGreaterThan(accountId.count, 30)
    }

    func testMakeIH58WithCustomNetworkPrefix() throws {
        let publicKey = Data(repeating: 0xCD, count: 32)
        let customPrefix: UInt16 = 753

        let accountId = try AccountId.makeIH58(
            publicKey: publicKey,
            domain: "testdomain",
            networkPrefix: customPrefix
        )

        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))
    }

    func testMakeIH58ThrowsForInvalidPublicKey() {
        let invalidPublicKey = Data(repeating: 0x00, count: 16) // Too short

        XCTAssertThrowsError(
            try AccountId.makeIH58(publicKey: invalidPublicKey, domain: "wonderland")
        )
    }

    func testKeypairAccountIdMethod() throws {
        let keypair = try Keypair.generate()
        let accountId = try keypair.accountId(domain: "wonderland")

        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))

        // Verify consistency - calling twice should produce same result
        let accountId2 = try keypair.accountId(domain: "wonderland")
        XCTAssertEqual(accountId, accountId2)
    }

    func testKeypairAccountIdWithCustomPrefix() throws {
        let keypair = try Keypair.generate()
        let customPrefix: UInt16 = 753

        let accountId = try keypair.accountId(domain: "test", networkPrefix: customPrefix)

        XCTAssertFalse(accountId.contains("@"))
    }

    func testMakeAndMakeIH58ProduceDifferentFormats() throws {
        let publicKey = Data(repeating: 0xEF, count: 32)
        let domain = "wonderland"

        let ed0120AccountId = AccountId.make(publicKey: publicKey, domain: domain)
        let ih58AccountId = try AccountId.makeIH58(publicKey: publicKey, domain: domain)

        // They should be different formats
        XCTAssertNotEqual(ed0120AccountId, ih58AccountId)

        // ed0120 format
        XCTAssertTrue(ed0120AccountId.hasPrefix("ed0120"))

        // IH58 format
        XCTAssertFalse(ih58AccountId.hasPrefix("ed0120"))
    }

    func testNormalizeForComparisonStripsDomainSuffixForIh58() throws {
        let publicKey = Data(repeating: 0x11, count: 32)
        let ih58 = try AccountId.makeIH58(publicKey: publicKey, domain: "default")

        XCTAssertEqual(AccountId.normalizeForComparison(ih58), ih58)
        XCTAssertEqual(AccountId.normalizeForComparison("\(ih58)@default"), ih58)
        XCTAssertEqual(AccountId.normalizeForComparison("\(ih58)@WONDERLAND"), ih58)

        XCTAssertTrue(AccountId.matchesForComparison(ih58, "\(ih58)@default"))
        XCTAssertTrue(AccountId.matchesForComparison("\(ih58)@default", "\(ih58)@wonderland"))
    }

    func testNormalizeForComparisonCanonicalizesDomainForNonIh58Literals() {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let rawUpper = AccountId.make(publicKey: publicKey, domain: "WONDERLAND")
        let rawLower = AccountId.make(publicKey: publicKey, domain: "wonderland")

        XCTAssertEqual(AccountId.normalizeForComparison(rawUpper), rawLower)
        XCTAssertFalse(AccountId.matchesForComparison(rawUpper, AccountId.make(publicKey: publicKey, domain: "otherland")))
    }
}

import XCTest
@testable import IrohaSwift

@available(macOS 10.15, iOS 13.0, *)
final class AccountIdTests: XCTestCase {

    func testDefaultNetworkPrefix() {
        XCTAssertEqual(AccountId.defaultNetworkPrefix, 0x02F1)
    }

    func testMakeProducesI105Format() {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let accountId = AccountId.make(publicKey: publicKey)

        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))
        XCTAssertGreaterThan(accountId.count, 30)
    }

    func testMakeI105ProducesValidFormat() throws {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let accountId = try AccountId.makeI105(publicKey: publicKey)

        // i105 format should NOT start with ed0120
        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))

        // Should be parseable back
        // I105 addresses are compact encoded literals, typically 40-50 chars
        XCTAssertGreaterThan(accountId.count, 30)
    }

    func testMakeI105WithCustomNetworkPrefix() throws {
        let publicKey = Data(repeating: 0xCD, count: 32)
        let customPrefix: UInt16 = 753

        let accountId = try AccountId.makeI105(publicKey: publicKey, networkPrefix: customPrefix
        )

        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))
    }

    func testMakeI105ThrowsForInvalidPublicKey() {
        let invalidPublicKey = Data(repeating: 0x00, count: 16) // Too short

        XCTAssertThrowsError(
            try AccountId.makeI105(publicKey: invalidPublicKey)
        )
    }

    func testKeypairAccountIdMethod() throws {
        let keypair = try Keypair.generate()
        let accountId = try keypair.accountId()

        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))

        // Verify consistency - calling twice should produce same result
        let accountId2 = try keypair.accountId()
        XCTAssertEqual(accountId, accountId2)
    }

    func testKeypairAccountIdWithCustomPrefix() throws {
        let keypair = try Keypair.generate()
        let customPrefix: UInt16 = 753

        let accountId = try keypair.accountId(networkPrefix: customPrefix)

        XCTAssertFalse(accountId.contains("@"))
    }

    func testMakeAndMakeI105ProduceSameFormat() throws {
        let publicKey = Data(repeating: 0xEF, count: 32)

        let accountId = AccountId.make(publicKey: publicKey)
        let i105AccountId = try AccountId.makeI105(publicKey: publicKey)

        XCTAssertEqual(accountId, i105AccountId)
        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))
    }

    func testNormalizeForComparisonLeavesAccountAliasesUntouched() throws {
        let publicKey = Data(repeating: 0x11, count: 32)
        let i105 = try AccountId.makeI105(publicKey: publicKey)
        let alias = "alice@dataspace"
        let scopedAlias = "alice@banka.dataspace"

        XCTAssertEqual(AccountId.normalizeForComparison(i105), i105)
        XCTAssertEqual(AccountId.normalizeForComparison(alias), alias)
        XCTAssertEqual(AccountId.normalizeForComparison(scopedAlias), scopedAlias)

        XCTAssertFalse(AccountId.matchesForComparison(i105, alias))
        XCTAssertFalse(AccountId.matchesForComparison(alias, scopedAlias))
    }

    func testNormalizeForComparisonDoesNotCanonicalizeLegacyLiterals() {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let rawUpper = "ed0120\(publicKey.map { String(format: "%02X", $0) }.joined())@BANKA.SBP"
        XCTAssertEqual(AccountId.normalizeForComparison(rawUpper), rawUpper)
        XCTAssertFalse(AccountId.matchesForComparison(rawUpper, rawUpper.lowercased()))
    }
}

import XCTest
@testable import IrohaSwift

@available(macOS 10.15, iOS 13.0, *)
final class AccountIdTests: XCTestCase {

    func testDefaultNetworkPrefix() {
        XCTAssertEqual(AccountId.defaultNetworkPrefix, 0x02F1)
    }

    func testMakeProducesIH58Format() {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let accountId = AccountId.make(publicKey: publicKey)

        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))
        XCTAssertGreaterThan(accountId.count, 30)
    }

    func testMakeIH58ProducesValidFormat() throws {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let accountId = try AccountId.makeIH58(publicKey: publicKey)

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

        let accountId = try AccountId.makeIH58(publicKey: publicKey, networkPrefix: customPrefix
        )

        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))
    }

    func testMakeIH58ThrowsForInvalidPublicKey() {
        let invalidPublicKey = Data(repeating: 0x00, count: 16) // Too short

        XCTAssertThrowsError(
            try AccountId.makeIH58(publicKey: invalidPublicKey)
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

    func testMakeAndMakeIH58ProduceSameFormat() throws {
        let publicKey = Data(repeating: 0xEF, count: 32)
        let domain = "wonderland"

        let accountId = AccountId.make(publicKey: publicKey)
        let ih58AccountId = try AccountId.makeIH58(publicKey: publicKey)

        XCTAssertEqual(accountId, ih58AccountId)
        XCTAssertFalse(accountId.hasPrefix("ed0120"))
        XCTAssertFalse(accountId.contains("@"))
    }

    func testNormalizeForComparisonLeavesDomainSuffixedLiteralsUntouched() throws {
        let publicKey = Data(repeating: 0x11, count: 32)
        let ih58 = try AccountId.makeIH58(publicKey: publicKey)

        XCTAssertEqual(AccountId.normalizeForComparison(ih58), ih58)
        XCTAssertEqual(AccountId.normalizeForComparison("\(ih58)@default"), "\(ih58)@default")
        XCTAssertEqual(AccountId.normalizeForComparison("\(ih58)@WONDERLAND"), "\(ih58)@WONDERLAND")

        XCTAssertFalse(AccountId.matchesForComparison(ih58, "\(ih58)@default"))
        XCTAssertFalse(AccountId.matchesForComparison("\(ih58)@default", "\(ih58)@wonderland"))
    }

    func testNormalizeForComparisonDoesNotCanonicalizeLegacyLiterals() {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let rawUpper = "ed0120\(publicKey.map { String(format: "%02X", $0) }.joined())@WONDERLAND"
        XCTAssertEqual(AccountId.normalizeForComparison(rawUpper), rawUpper)
        XCTAssertFalse(AccountId.matchesForComparison(rawUpper, rawUpper.lowercased()))
    }
}

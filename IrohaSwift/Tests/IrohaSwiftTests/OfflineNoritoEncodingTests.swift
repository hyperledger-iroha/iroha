import XCTest
@testable import IrohaSwift

final class OfflineNoritoEncodingTests: XCTestCase {
    func testAssetIdParseRejectsShorthandDefinition() {
        assertInvalidAssetId("xor##alice@wonderland")
    }

    func testAssetIdParseAcceptsExplicitDefinition() throws {
        let parts = try OfflineAssetIdParts.parse("rose#wonderland#alice@wonderland")
        XCTAssertEqual(parts, OfflineAssetIdParts(accountId: "alice@wonderland",
                                                  definitionName: "rose",
                                                  definitionDomain: "wonderland"))
    }

    func testAssetIdParseAcceptsExplicitDefinitionWithIh58Account() throws {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 1, count: 32))
        let address = try AccountAddress.fromAccount(domain: "wonderland", publicKey: keypair.publicKey)
        let ih58 = try address.toIH58(networkPrefix: 0x02F1)
        let parts = try OfflineAssetIdParts.parse("rose#wonderland#\(ih58)")
        XCTAssertEqual(parts, OfflineAssetIdParts(accountId: ih58,
                                                  definitionName: "rose",
                                                  definitionDomain: "wonderland"))
    }

    func testAssetIdParseRejectsShorthandWithIh58Account() throws {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 2, count: 32))
        let address = try AccountAddress.fromAccount(domain: "wonderland", publicKey: keypair.publicKey)
        let ih58 = try address.toIH58(networkPrefix: 0x02F1)
        assertInvalidAssetId("xor##\(ih58)")
    }

    func testAssetIdParseRejectsEmptyAccountName() {
        assertInvalidAccountId("rose#wonderland#@wonderland", expected: "@wonderland")
    }

    func testAssetIdParseRejectsExtraAtSign() {
        assertInvalidAccountId("rose#wonderland#alice@wonderland@extra",
                               expected: "alice@wonderland@extra")
    }

    func testAssetIdParseRejectsReservedCharacterInDefinition() {
        assertInvalidAssetId("rose$#wonderland#alice@wonderland")
    }

    func testAssetIdParseRejectsExtraDefinitionSeparator() {
        assertInvalidAssetId("rose#won#der#alice@wonderland")
    }

    func testAssetIdParseRejectsReservedCharacterInAccountName() {
        assertInvalidAccountId("rose#wonderland#alice$@wonderland", expected: "alice$@wonderland")
    }

    private func assertInvalidAssetId(_ value: String) {
        XCTAssertThrowsError(try OfflineAssetIdParts.parse(value)) { error in
            guard case let OfflineNoritoError.invalidAssetId(raw) = error else {
                return XCTFail("Expected invalidAssetId error, got \(error)")
            }
            XCTAssertEqual(raw, value)
        }
    }

    private func assertInvalidAccountId(_ value: String, expected: String) {
        XCTAssertThrowsError(try OfflineAssetIdParts.parse(value)) { error in
            guard case let OfflineNoritoError.invalidAccountId(actual) = error else {
                return XCTFail("Expected invalidAccountId error, got \(error)")
            }
            XCTAssertEqual(actual, expected)
        }
    }
}

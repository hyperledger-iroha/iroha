import XCTest
@testable import IrohaSwift

final class OfflineNoritoEncodingTests: XCTestCase {
    func testAssetIdParseAcceptsShorthandDefinition() throws {
        let parts = try OfflineAssetIdParts.parse("xor##alice@wonderland")
        XCTAssertEqual(parts, OfflineAssetIdParts(accountId: "alice@wonderland",
                                                  definitionName: "xor",
                                                  definitionDomain: "wonderland"))
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

    func testAssetIdParseAcceptsShorthandWithDefaultDomainIh58Account() throws {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 2, count: 32))
        let address = try AccountAddress.fromAccount(domain: AccountAddress.defaultDomainName,
                                                     publicKey: keypair.publicKey)
        let ih58 = try address.toIH58(networkPrefix: 0x02F1)
        let parts = try OfflineAssetIdParts.parse("xor##\(ih58)")
        XCTAssertEqual(parts, OfflineAssetIdParts(accountId: ih58,
                                                  definitionName: "xor",
                                                  definitionDomain: AccountAddress.defaultDomainName))
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

    func testAccountIdResolutionViaUaidResolver() throws {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 3, count: 32))
        let address = try AccountAddress.fromAccount(domain: "wonderland", publicKey: keypair.publicKey)
        let ih58 = try address.toIH58(networkPrefix: 0x02F1)
        let uaid = "uaid:" + String(repeating: "0", count: 63) + "1"
        OfflineAccountResolvers.setAccountResolver { kind, value in
            switch kind {
            case .uaid:
                XCTAssertEqual(value, uaid)
                return ih58
            case .opaque:
                return nil
            }
        }
        defer { OfflineAccountResolvers.setAccountResolver(nil) }

        let resolved = try OfflineNorito.encodeAccountId(uaid)
        let expected = try OfflineNorito.encodeAccountId(ih58)
        XCTAssertEqual(resolved, expected)
    }

    func testAccountIdResolutionViaOpaqueResolver() throws {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 4, count: 32))
        let address = try AccountAddress.fromAccount(domain: "wonderland", publicKey: keypair.publicKey)
        let ih58 = try address.toIH58(networkPrefix: 0x02F1)
        let opaque = "opaque:" + String(repeating: "0", count: 64)
        OfflineAccountResolvers.setAccountResolver { kind, value in
            switch kind {
            case .uaid:
                return nil
            case .opaque:
                XCTAssertEqual(value, opaque)
                return ih58
            }
        }
        defer { OfflineAccountResolvers.setAccountResolver(nil) }

        let resolved = try OfflineNorito.encodeAccountId(opaque)
        let expected = try OfflineNorito.encodeAccountId(ih58)
        XCTAssertEqual(resolved, expected)
    }

    func testEncodeAccountIdRebasesDefaultDomainIh58WhenExplicitDomainProvided() throws {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: 5, count: 32))
        let defaultAddress = try AccountAddress.fromAccount(
            domain: AccountAddress.defaultDomainName,
            publicKey: keypair.publicKey
        )
        let defaultIh58 = try defaultAddress.toIH58(networkPrefix: 0x02F1)
        let providedLiteral = "\(defaultIh58)@bank1"

        let expectedAddress = try defaultAddress.rebasedFromDefaultDomain(to: "bank1")
        let expectedIh58 = try expectedAddress.toIH58(networkPrefix: 0x02F1)
        let expected = try OfflineNorito.encodeAccountId("\(expectedIh58)@bank1")

        let encoded = try OfflineNorito.encodeAccountId(providedLiteral)
        XCTAssertEqual(encoded, expected)
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

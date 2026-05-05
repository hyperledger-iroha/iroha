import XCTest
@testable import IrohaSwift

final class OfflineNoritoEncodingTests: XCTestCase {
    private func makeAddress(seed: UInt8) throws -> AccountAddress {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: seed, count: 32))
        return try AccountAddress.fromAccount(publicKey: keypair.publicKey)
    }

    private func makeI105(seed: UInt8) throws -> String {
        let address = try makeAddress(seed: seed)
        return try address.toI105(networkPrefix: 0x02F1)
    }

    func testEncodeAssetIdAcceptsCanonicalPublicLiteral() throws {
        let assetId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#\(try makeI105(seed: 1))"
        let encoded = try OfflineNorito.encodeAssetId(assetId)
        XCTAssertFalse(encoded.isEmpty)
    }

    func testEncodeAssetIdRejectsTextualForms() {
        assertInvalidAssetId("62Fk4FPcMuLvW5QjDGNF2a4jAmjM#alice@banka.dataspace")
        assertInvalidAssetId("xor##alice@banka.dataspace")
        assertInvalidAssetId("rose##alice@banka.dataspace")
    }

    func testEncodeAssetIdRejectsMalformedPublicLiterals() throws {
        assertInvalidAssetId("not:an-asset")
        assertInvalidAssetId("62Fk4FPcMuLvW5QjDGNF2a4jAmjM#")
        assertInvalidAssetId("62Fk4FPcMuLvW5QjDGNF2a4jAmjM#\(try makeI105(seed: 9))#dataspace:")
    }

    func testEncodeAccountIdAcceptsI105() throws {
        let address = try makeAddress(seed: 1)
        let i105 = try address.toI105(networkPrefix: 0x02F1)
        let encodedFromI105 = try OfflineNorito.encodeAccountId(i105)
        XCTAssertFalse(encodedFromI105.isEmpty)
    }

    func testEncodeAccountIdRejectsAliasLiteral() {
        let literal = "alice@banka.dataspace"
        assertInvalidAccountId(literal, expected: literal)
    }

    func testEncodeAccountIdRejectsI105WithDomainSuffix() throws {
        let i105 = try makeI105(seed: 2)
        let providedLiteral = "\(i105)@banka"
        assertInvalidAccountId(providedLiteral, expected: providedLiteral)
    }

    func testEncodeAccountIdRejectsUaidLiteral() {
        let uaid = "uaid:" + String(repeating: "0", count: 63) + "1"
        assertInvalidAccountId(uaid, expected: uaid)
    }

    func testEncodeAccountIdRejectsOpaqueLiteral() {
        let opaque = "opaque:" + String(repeating: "0", count: 64)
        assertInvalidAccountId(opaque, expected: opaque)
    }

    func testEncodeAccountIdRejectsCanonicalHexLiteral() throws {
        let address = try makeAddress(seed: 3)
        let canonical = try address.canonicalHex()
        assertInvalidAccountId(canonical, expected: canonical)
    }

    private func assertInvalidAssetId(_ value: String) {
        XCTAssertThrowsError(try OfflineNorito.encodeAssetId(value)) { error in
            guard case let OfflineNoritoError.invalidAssetId(raw) = error else {
                return XCTFail("Expected invalidAssetId error, got \(error)")
            }
            XCTAssertEqual(raw, value)
        }
    }

    private func assertInvalidAccountId(_ value: String, expected: String) {
        XCTAssertThrowsError(try OfflineNorito.encodeAccountId(value)) { error in
            guard case let OfflineNoritoError.invalidAccountId(actual) = error else {
                return XCTFail("Expected invalidAccountId error, got \(error)")
            }
            XCTAssertEqual(actual, expected)
        }
    }
}

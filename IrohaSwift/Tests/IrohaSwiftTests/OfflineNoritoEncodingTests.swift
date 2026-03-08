import XCTest
@testable import IrohaSwift

final class OfflineNoritoEncodingTests: XCTestCase {
    private func makeAddress(seed: UInt8,
                             domain: String = AccountAddress.defaultDomainName) throws -> AccountAddress {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: seed, count: 32))
        return try AccountAddress.fromAccount(publicKey: keypair.publicKey)
    }

    private func makeIH58(seed: UInt8,
                          domain: String = AccountAddress.defaultDomainName) throws -> String {
        let address = try makeAddress(seed: seed, domain: domain)
        return try address.toIH58(networkPrefix: 0x02F1)
    }

    func testEncodeAssetIdAcceptsNoritoHexLiteral() throws {
        let encoded = try OfflineNorito.encodeAssetId("norito:0A0B")
        XCTAssertEqual(encoded, Data([0x0A, 0x0B]))
    }

    func testEncodeAssetIdRejectsTextualForms() {
        assertInvalidAssetId("rose#wonderland#alice@wonderland")
        assertInvalidAssetId("xor##alice@wonderland")
        assertInvalidAssetId("rose##alice@wonderland")
    }

    func testEncodeAssetIdRejectsInvalidHexPayload() {
        assertInvalidAssetId("norito:GG")
        assertInvalidAssetId("norito:abc")
        assertInvalidAssetId("norito:")
    }

    func testEncodeAccountIdAcceptsIh58AndCompressedForms() throws {
        let address = try makeAddress(seed: 1)
        let ih58 = try address.toIH58(networkPrefix: 0x02F1)
        let compressed = try address.toCompressedSora()
        let encodedFromIh58 = try OfflineNorito.encodeAccountId(ih58)
        let encodedFromCompressed = try OfflineNorito.encodeAccountId(compressed)
        XCTAssertEqual(encodedFromCompressed, encodedFromIh58)
    }

    func testEncodeAccountIdRejectsAliasLiteral() {
        let literal = "alice@wonderland"
        assertInvalidAccountId(literal, expected: literal)
    }

    func testEncodeAccountIdRejectsIh58WithDomainSuffix() throws {
        let ih58 = try makeIH58(seed: 2)
        let providedLiteral = "\(ih58)@hbl"
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
        let address = try makeAddress(seed: 3, domain: "wonderland")
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

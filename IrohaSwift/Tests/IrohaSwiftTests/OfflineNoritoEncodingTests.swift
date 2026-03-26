import XCTest
@testable import IrohaSwift

final class OfflineNoritoEncodingTests: XCTestCase {
    private func makeAddress(seed: UInt8,
                             domain: String = AccountAddress.defaultDomainName) throws -> AccountAddress {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: seed, count: 32))
        return try AccountAddress.fromAccount(publicKey: keypair.publicKey)
    }

    private func makeI105(seed: UInt8,
                          domain: String = AccountAddress.defaultDomainName) throws -> String {
        let address = try makeAddress(seed: seed, domain: domain)
        return try address.toI105(networkPrefix: 0x02F1)
    }

    func testEncodeAssetIdAcceptsCanonicalPublicLiteral() throws {
        let assetId =
            "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"
        let encoded = try OfflineNorito.encodeAssetId(assetId)
        XCTAssertFalse(encoded.isEmpty)
    }

    func testEncodeAssetIdRejectsTextualForms() {
        assertInvalidAssetId("62Fk4FPcMuLvW5QjDGNF2a4jAmjM#alice@hbl.sbp")
        assertInvalidAssetId("xor##alice@hbl.sbp")
        assertInvalidAssetId("rose##alice@hbl.sbp")
    }

    func testEncodeAssetIdRejectsMalformedPublicLiterals() {
        assertInvalidAssetId("not:an-asset")
        assertInvalidAssetId("62Fk4FPcMuLvW5QjDGNF2a4jAmjM#")
        assertInvalidAssetId("62Fk4FPcMuLvW5QjDGNF2a4jAmjM#6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn#dataspace:")
    }

    func testEncodeAccountIdAcceptsI105AndI105DefaultForms() throws {
        let address = try makeAddress(seed: 1)
        let i105 = try address.toI105(networkPrefix: 0x02F1)
        let i105Default = try address.toI105Default()
        let encodedFromI105 = try OfflineNorito.encodeAccountId(i105)
        let encodedFromI105Default = try OfflineNorito.encodeAccountId(i105Default)
        XCTAssertEqual(encodedFromI105Default, encodedFromI105)
    }

    func testEncodeAccountIdRejectsAliasLiteral() {
        let literal = "alice@hbl.sbp"
        assertInvalidAccountId(literal, expected: literal)
    }

    func testEncodeAccountIdRejectsI105WithDomainSuffix() throws {
        let i105 = try makeI105(seed: 2)
        let providedLiteral = "\(i105)@hbl"
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

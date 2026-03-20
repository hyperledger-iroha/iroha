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

    func testEncodeAssetIdFromComponentsMatchesLiteralEncoding() throws {
        guard NoritoNativeBridge.shared.canEncodeAssetIdLiteral else {
            throw XCTSkip("connect_norito_encode_asset_id_literal is unavailable in this runtime")
        }
        let accountId = try makeI105(seed: 7)
        let literal = try OfflineNorito.assetIdLiteral(assetDefinitionId: "usd#wonderland", accountId: accountId)
        let encodedFromLiteral = try OfflineNorito.encodeAssetId(literal)
        let encodedFromParts = try OfflineNorito.encodeAssetId(
            assetDefinitionId: "usd#wonderland",
            accountId: accountId
        )
        XCTAssertEqual(literal.hasPrefix("norito:"), true)
        XCTAssertEqual(encodedFromParts, encodedFromLiteral)
    }

    func testAssetIdLiteralBuilderRejectsAliasWithoutOnlineResolution() throws {
        let accountId = try makeI105(seed: 9)
        XCTAssertThrowsError(
            try OfflineNorito.assetIdLiteral(
                assetDefinitionId: "usd#issuer@main",
                accountId: accountId
            )
        ) { error in
            guard case let OfflineNoritoError.invalidAssetId(raw) = error else {
                return XCTFail("Expected invalidAssetId error, got \(error)")
            }
            XCTAssertEqual(raw, "usd#issuer@main")
        }
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
        let literal = "alice@wonderland"
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

    // MARK: - assetDefinitionIdFromAlias

    func testAssetDefinitionIdFromAliasProducesExpectedAid() throws {
        guard NoritoNativeBridge.shared.blake3Hash(data: Data()) != nil else {
            throw XCTSkip("NoritoBridge blake3 hashing unavailable")
        }
        let aid = try OfflineNorito.assetDefinitionIdFromAlias("usd#wonderland")
        XCTAssertEqual(aid, "aid:bef53c1ccd1749e180dfbad6519bfd66")
    }

    func testAssetDefinitionIdFromAliasPublicAPI() throws {
        guard NoritoNativeBridge.shared.blake3Hash(data: Data()) != nil else {
            throw XCTSkip("NoritoBridge blake3 hashing unavailable")
        }
        let aid = try ToriiClient.assetDefinitionId(fromAlias: "usd#wonderland")
        XCTAssertEqual(aid, "aid:bef53c1ccd1749e180dfbad6519bfd66")
    }

    func testAssetDefinitionIdFromAliasRejectsInvalidInput() throws {
        guard NoritoNativeBridge.shared.blake3Hash(data: Data()) != nil else {
            throw XCTSkip("NoritoBridge blake3 hashing unavailable")
        }
        XCTAssertThrowsError(try OfflineNorito.assetDefinitionIdFromAlias(""))
        XCTAssertThrowsError(try OfflineNorito.assetDefinitionIdFromAlias("usd"))
    }

    func testAssetDefinitionIdFromAliasHasUuidV4Bits() throws {
        guard NoritoNativeBridge.shared.blake3Hash(data: Data()) != nil else {
            throw XCTSkip("NoritoBridge blake3 hashing unavailable")
        }
        let aid = try OfflineNorito.assetDefinitionIdFromAlias("usd#wonderland")
        // aid:XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX — strip prefix and parse hex
        let hex = String(aid.dropFirst(4))
        let bytes = stride(from: 0, to: hex.count, by: 2).map { i -> UInt8 in
            let start = hex.index(hex.startIndex, offsetBy: i)
            let end = hex.index(start, offsetBy: 2)
            return UInt8(hex[start..<end], radix: 16) ?? 0
        }
        // UUIDv4: bytes[6] high nibble = 0x4, bytes[8] top 2 bits = 10
        XCTAssertEqual(bytes[6] & 0xf0, 0x40, "Version nibble should be 4")
        XCTAssertEqual(bytes[8] & 0xc0, 0x80, "Variant bits should be 10xx")
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

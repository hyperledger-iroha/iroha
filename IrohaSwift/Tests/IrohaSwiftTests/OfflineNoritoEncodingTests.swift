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
        let account = try makeI105(seed: 1)
        let assetId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#\(account)"
        XCTAssertFalse(try OfflineNorito.encodeAssetId(assetId).isEmpty)
        XCTAssertFalse(try OfflineNorito.encodeAssetId("\(assetId)#dataspace:0").isEmpty)
        XCTAssertFalse(try OfflineNorito.encodeAssetId("\(assetId)#dataspace:1").isEmpty)
    }

    func testEncodeAssetIdRejectsTextualForms() {
        assertInvalidAssetId("62Fk4FPcMuLvW5QjDGNF2a4jAmjM#alice@banka.dataspace")
        assertInvalidAssetId("xor##alice@banka.dataspace")
        assertInvalidAssetId("rose##alice@banka.dataspace")
    }

    func testEncodeAssetIdRejectsMalformedPublicLiterals() throws {
        assertInvalidAssetId("not:an-asset")
        assertInvalidAssetId("62Fk4FPcMuLvW5QjDGNF2a4jAmjM#")
        let assetId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#\(try makeI105(seed: 9))"
        assertInvalidAssetId("\(assetId)#dataspace:")
        assertInvalidAssetId("\(assetId)#dataspace:+1")
        assertInvalidAssetId("\(assetId)#dataspace:01")
        assertInvalidAssetId("\(assetId)#dataspace:-1")
        assertInvalidAssetId("\(assetId)#DATASPACE:1")
        assertInvalidAssetId("\(assetId)#dataspace:18446744073709551616")
    }

    func testEncodeAccountIdAcceptsI105() throws {
        let address = try makeAddress(seed: 1)
        let i105 = try address.toI105(networkPrefix: 0x02F1)
        let encodedFromI105 = try OfflineNorito.encodeAccountId(i105)
        XCTAssertFalse(encodedFromI105.isEmpty)
    }

    func testDecodeCertificatePayloadPreservesMultisigAccountClassByte() throws {
        let memberPublicKey = Data((0..<32).map { UInt8(0x80 + $0) })
        var canonical = Data([0x0A, 0x01, 0x01])
        appendUInt16BE(1, to: &canonical)
        appendUInt16BE(1, to: &canonical)
        canonical.append(0x01)
        appendUInt16BE(1, to: &canonical)
        appendUInt16BE(UInt16(memberPublicKey.count), to: &canonical)
        canonical.append(memberPublicKey)

        let accountId = try AccountAddress
            .fromCanonicalBytes(canonical)
            .toI105(networkPrefix: 0x02F1)
        let singleKeyAccountId = try AccountAddress
            .fromAccount(publicKey: memberPublicKey)
            .toI105(networkPrefix: 0x02F1)
        XCTAssertNotEqual(accountId, singleKeyAccountId)

        let decoded = try OfflineNoteV2Decoding.decodeCertificatePayload(
            try keyCertificatePayloadFrame(
                accountIdPayload: try compactMultisigAccountId(memberPublicKey: memberPublicKey)
            )
        )
        XCTAssertEqual(decoded.accountId, accountId)
        XCTAssertNotEqual(decoded.accountId, singleKeyAccountId)
        XCTAssertEqual(
            try AccountAddress.fromI105(decoded.accountId, expectedPrefix: 0x02F1).canonicalBytes(),
            canonical
        )
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

    private func appendUInt16BE(_ value: UInt16, to data: inout Data) {
        data.append(UInt8((value >> 8) & 0xff))
        data.append(UInt8(value & 0xff))
    }

    private func keyCertificatePayloadFrame(accountIdPayload: Data) throws -> Data {
        var payload = Data()
        payload.append(compactField(OfflineCompactNorito.encodeString(OfflineNoteV2Constants.keyCertificatePayloadDomain)))
        payload.append(compactField(OfflineCompactNorito.encodeUInt16(OfflineNoteV2Constants.keyCertificateVersion)))
        payload.append(compactField(OfflineCompactNorito.encodeString(OfflineNoteV2Constants.iosAppAttestPlatform)))
        payload.append(compactField(OfflineCompactNorito.encodeString("sdk-multisig-key")))
        payload.append(compactField(OfflineCompactNorito.encodeString("sdk-device")))
        payload.append(compactField(accountIdPayload))
        payload.append(compactField(OfflineNorito.encodeBytesVec(Data(repeating: 0x11, count: 32))))
        payload.append(compactField(OfflineCompactNorito.encodeString(OfflineNoteV2Constants.iosAppAttestAssertionScheme)))
        payload.append(compactField(OfflineCompactNorito.encodeString(OfflineNoteV2Constants.iosAppAttestAssertionKeyAlgorithm)))
        payload.append(compactField(OfflineNorito.encodeBytesVec(p256AssertionPublicKey())))
        payload.append(compactField(
            try OfflineCompactNorito.encodeOption(Optional<UInt32>.none, encode: OfflineCompactNorito.encodeUInt32)
        ))
        payload.append(compactField(OfflineNorito.encodeBool(true)))
        return noritoEncode(
            typeName: OfflineNoteV2TypeNames.keyCertificatePayload,
            payload: payload,
            flags: 2
        )
    }

    private func compactMultisigAccountId(memberPublicKey: Data) throws -> Data {
        guard memberPublicKey.count == 32 else {
            throw AccountAddressError.invalidPublicKey
        }
        var controllerPublicKey = Data([SigningAlgorithm.ed25519.noritoDiscriminant])
        controllerPublicKey.append(memberPublicKey)

        var memberPayload = Data()
        memberPayload.append(compactField(compactConstVec(controllerPublicKey)))
        memberPayload.append(compactField(OfflineCompactNorito.encodeUInt16(1)))

        var membersPayload = Data()
        membersPayload.append(uint64LE(1))
        membersPayload.append(compactField(memberPayload))

        var policyPayload = Data()
        policyPayload.append(compactField(Data([1])))
        policyPayload.append(compactField(OfflineCompactNorito.encodeUInt16(1)))
        policyPayload.append(compactField(membersPayload))

        var account = Data()
        account.append(uint32LE(1))
        account.append(compactField(policyPayload))
        return account
    }

    private func p256AssertionPublicKey() -> Data {
        Data([
            0x04,
            0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47,
            0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40, 0xf2,
            0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0,
            0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2, 0x96,
            0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b,
            0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16,
            0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce,
            0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5
        ])
    }

    private func compactConstVec(_ bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(bytes.count))
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }

    private func compactField(_ payload: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(payload)
        return writer.data
    }

    private func uint32LE(_ value: UInt32) -> Data {
        var copy = value.littleEndian
        return Data(bytes: &copy, count: MemoryLayout<UInt32>.size)
    }

    private func uint64LE(_ value: UInt64) -> Data {
        var copy = value.littleEndian
        return Data(bytes: &copy, count: MemoryLayout<UInt64>.size)
    }
}

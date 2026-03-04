import CryptoKit
import XCTest
@testable import IrohaSwift

final class ConnectKeyStoreTests: XCTestCase {
    private func temporaryDirectory() throws -> URL {
        let url = URL(fileURLWithPath: NSTemporaryDirectory(), isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    func testGeneratePersistsKeypairAndAttestation() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCryptoAvailable,
                      "NoritoBridge connect crypto symbols not linked")
        let dir = try temporaryDirectory()
        let store = ConnectKeyStore(directory: dir, configuration: .init(preferKeychain: false))

        let (firstPair, firstAttestation) = try store.generateOrLoad(label: "wallet")
        XCTAssertEqual(firstAttestation.publicKeyDigest.count, 32)

        let (secondPair, secondAttestation) = try store.generateOrLoad(label: "wallet")
        XCTAssertEqual(firstPair.publicKey, secondPair.publicKey)
        XCTAssertEqual(firstPair.privateKey, secondPair.privateKey)
        XCTAssertEqual(firstAttestation, secondAttestation)
    }

    func testRejectsInvalidLabel() throws {
        let dir = try temporaryDirectory()
        let store = ConnectKeyStore(directory: dir)
        XCTAssertThrowsError(try store.generateOrLoad(label: "bad/label")) { error in
            guard case ConnectKeyStoreError.invalidLabel = error else {
                XCTFail("expected invalidLabel error, got \(error)")
                return
            }
        }
    }

    func testRejectsTraversalLabel() throws {
        let dir = try temporaryDirectory()
        let store = ConnectKeyStore(directory: dir, configuration: .init(preferKeychain: false))
        XCTAssertThrowsError(try store.delete(label: "../wallet")) { error in
            guard case ConnectKeyStoreError.invalidLabel = error else {
                XCTFail("expected invalidLabel error for traversal, got \(error)")
                return
            }
        }
    }

    func testSanitizedFilenameUsed() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCryptoAvailable,
                      "NoritoBridge connect crypto symbols not linked")
        let dir = try temporaryDirectory()
        let store = ConnectKeyStore(directory: dir, configuration: .init(preferKeychain: false))

        _ = try store.generateOrLoad(label: "  wallet ")
        let contents = try FileManager.default.contentsOfDirectory(atPath: dir.path)
        XCTAssertTrue(contents.contains("wallet.json"))
    }

    func testDeleteRemovesEntry() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCryptoAvailable,
                      "NoritoBridge connect crypto symbols not linked")
        let dir = try temporaryDirectory()
        let store = ConnectKeyStore(directory: dir)

        _ = try store.generateOrLoad(label: "ephemeral")
        try store.delete(label: "ephemeral")

        let (pair, attestation) = try store.generateOrLoad(label: "ephemeral")
        XCTAssertEqual(attestation.publicKeyDigest.count, 32)
        XCTAssertEqual(pair.publicKey.count, 32)
        XCTAssertEqual(pair.privateKey.count, 32)
    }

    func testTamperDetectionRejectsModifiedRecord() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCryptoAvailable,
                      "NoritoBridge connect crypto symbols not linked")
        let dir = try temporaryDirectory()
        let store = ConnectKeyStore(directory: dir, configuration: .init(preferKeychain: false))
        let label = "secure-wallet"

        _ = try store.generateOrLoad(label: label)
        let path = dir.appendingPathComponent("\(label).json")
        var data = try Data(contentsOf: path)
        if !data.isEmpty {
            data[0] = data[0] ^ 0xFF
        }
        try data.write(to: path)

        XCTAssertThrowsError(try store.generateOrLoad(label: label)) { error in
            guard let storeError = error as? ConnectKeyStoreError else {
                XCTFail("expected ConnectKeyStoreError, got \(error)")
                return
            }
            switch storeError {
            case .corrupt, .integrityMismatch:
                return
            default:
                XCTFail("expected corruption error, got \(storeError)")
            }
        }
    }

    func testLegacyHmacOrderingAccepted() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCryptoAvailable,
                      "NoritoBridge connect crypto symbols not linked")
        let dir = try temporaryDirectory()
        let store = ConnectKeyStore(directory: dir, configuration: .init(preferKeychain: false))
        let label = "wallet"

        _ = try store.generateOrLoad(label: label)

        let path = dir.appendingPathComponent("\(label).json")
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let envelope = try decoder.decode(TestEnvelope.self, from: Data(contentsOf: path))

        let integrityKey = try Data(contentsOf: dir.appendingPathComponent(".integrity.key"))
        let legacyHmac = try makeLegacyHmac(label: label, key: integrityKey, payload: envelope.payload)
        let rewritten = TestEnvelope(version: envelope.version, payload: envelope.payload, hmac: legacyHmac)

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        try encoder.encode(rewritten).write(to: path)

        let (pair, attestation) = try store.generateOrLoad(label: label)
        XCTAssertEqual(pair.publicKey, envelope.payload.keyPair.publicKey)
        XCTAssertEqual(pair.privateKey, envelope.payload.keyPair.privateKey)
        XCTAssertEqual(attestation, envelope.payload.attestation)
    }

}

private struct TestEnvelope: Codable {
    let version: Int
    let payload: TestStoredKey
    let hmac: String
}

private struct TestStoredKey: Codable {
    let keyPair: ConnectKeyPair
    let attestation: ConnectKeyStore.Attestation
}

private func makeLegacyHmac(label: String, key: Data, payload: TestStoredKey) throws -> String {
    let publicKey = try encodeJSONString(payload.keyPair.publicKey.base64EncodedString())
    let privateKey = try encodeJSONString(payload.keyPair.privateKey.base64EncodedString())
    let publicKeyDigest = try encodeJSONString(payload.attestation.publicKeyDigest.base64EncodedString())
    let deviceLabel = try encodeJSONString(payload.attestation.deviceLabel)
    let createdAt = try encodeDateJSONString(payload.attestation.createdAt)

    let keyPair = jsonObject(keys: ["publicKey", "privateKey"], values: [
        "publicKey": publicKey,
        "privateKey": privateKey,
    ])
    let attestation = jsonObject(keys: ["deviceLabel", "createdAt", "publicKeyDigest"], values: [
        "deviceLabel": deviceLabel,
        "createdAt": createdAt,
        "publicKeyDigest": publicKeyDigest,
    ])
    let payloadJSON = jsonObject(keys: ["keyPair", "attestation"], values: [
        "keyPair": keyPair,
        "attestation": attestation,
    ])

    let symmetricKey = SymmetricKey(data: key)
    var hmac = HMAC<SHA256>(key: symmetricKey)
    hmac.update(data: Data(label.utf8))
    hmac.update(data: Data(payloadJSON.utf8))
    return Data(hmac.finalize()).base64EncodedString()
}

private func jsonObject(keys: [String], values: [String: String]) -> String {
    let parts = keys.map { key in
        "\"\(key)\":\(values[key] ?? "null")"
    }
    return "{\(parts.joined(separator: ","))}"
}

private func encodeJSONString(_ value: String) throws -> String {
    let encoder = JSONEncoder()
    let data = try encoder.encode(value)
    guard let string = String(data: data, encoding: .utf8) else {
        throw CocoaError(.fileWriteUnknown)
    }
    return string
}

private func encodeDateJSONString(_ date: Date) throws -> String {
    let encoder = JSONEncoder()
    encoder.dateEncodingStrategy = .iso8601
    let data = try encoder.encode(date)
    guard let string = String(data: data, encoding: .utf8) else {
        throw CocoaError(.fileWriteUnknown)
    }
    return string
}

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
        let store = ConnectKeyStore(directory: dir)

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

    func testMigratesLegacyPlaintextRecord() throws {
        let dir = try temporaryDirectory()
        let configuration = ConnectKeyStore.Configuration(preferKeychain: false)
        let store = ConnectKeyStore(directory: dir, configuration: configuration)

        let publicKey = Data(repeating: 0x11, count: 32)
        let privateKey = Data(repeating: 0x22, count: 32)
        let attestation = ConnectKeyStore.Attestation(
            publicKeyDigest: Data(SHA256.hash(data: publicKey)),
            deviceLabel: "legacy-device",
            createdAt: Date(timeIntervalSince1970: 1_700_000_000)
        )
        let legacy = ConnectKeyStore.StoredKey(
            keyPair: ConnectKeyPair(publicKey: publicKey, privateKey: privateKey),
            attestation: attestation
        )

        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let legacyData = try encoder.encode(legacy)
        let legacyPath = dir.appendingPathComponent("legacy.json")
        try legacyData.write(to: legacyPath)

        let (loadedPair, loadedAttestation) = try store.generateOrLoad(label: "legacy")
        XCTAssertEqual(loadedPair, legacy.keyPair)
        XCTAssertEqual(loadedAttestation, attestation)

        let migratedData = try Data(contentsOf: legacyPath)
        struct Envelope: Decodable {
            let hmac: String
        }
        let decoded = try JSONDecoder().decode(Envelope.self, from: migratedData)
        XCTAssertFalse(decoded.hmac.isEmpty, "migration should wrap payload with HMAC")
    }
}

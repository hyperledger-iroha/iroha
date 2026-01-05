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

}

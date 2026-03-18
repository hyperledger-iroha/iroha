import XCTest
@testable import IrohaSwift

final class ConnectCryptoTests: XCTestCase {
    private func requireBridge() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCryptoAvailable,
                      "NoritoBridge connect crypto symbols not linked")
    }

    func testGenerateKeyPairProducesDeterministicLengths() throws {
        try requireBridge()
        let pair = try ConnectCrypto.generateKeyPair()
        XCTAssertEqual(pair.publicKey.count, 32)
        XCTAssertEqual(pair.privateKey.count, 32)

        let derivedPublic = try ConnectCrypto.publicKey(fromPrivateKey: pair.privateKey)
        XCTAssertEqual(derivedPublic, pair.publicKey)
    }

    func testDeriveDirectionKeysUsesBridge() throws {
        try requireBridge()
        let pair = try ConnectCrypto.generateKeyPair()
        let sessionID = Data(repeating: 0xAB, count: 32)

        let keys = try ConnectCrypto.deriveDirectionKeys(localPrivateKey: pair.privateKey,
                                                         peerPublicKey: pair.publicKey,
                                                         sessionID: sessionID)
        XCTAssertEqual(keys.appToWallet.count, 32)
        XCTAssertEqual(keys.walletToApp.count, 32)
        XCTAssertNotEqual(keys.appToWallet, keys.walletToApp, "directional keys should differ")
    }

    func testDeriveDirectionKeysRejectsInvalidLengths() throws {
        try requireBridge()
        let validKey = Data(repeating: 0x01, count: 32)
        let sessionID = Data(repeating: 0x02, count: 32)

        XCTAssertThrowsError(
            try ConnectCrypto.deriveDirectionKeys(localPrivateKey: Data(),
                                                  peerPublicKey: validKey,
                                                  sessionID: sessionID)
        ) { error in
            guard case ConnectCryptoError.invalidPrivateKeyLength = error else {
                return XCTFail("expected invalidPrivateKeyLength")
            }
        }

        XCTAssertThrowsError(
            try ConnectCrypto.deriveDirectionKeys(localPrivateKey: validKey,
                                                  peerPublicKey: Data(),
                                                  sessionID: sessionID)
        ) { error in
            guard case ConnectCryptoError.invalidPublicKeyLength = error else {
                return XCTFail("expected invalidPublicKeyLength")
            }
        }

        XCTAssertThrowsError(
            try ConnectCrypto.deriveDirectionKeys(localPrivateKey: validKey,
                                                  peerPublicKey: validKey,
                                                  sessionID: Data())
        ) { error in
            guard case ConnectCryptoError.invalidSessionIdentifierLength = error else {
                return XCTFail("expected invalidSessionIdentifierLength")
            }
        }
    }
}

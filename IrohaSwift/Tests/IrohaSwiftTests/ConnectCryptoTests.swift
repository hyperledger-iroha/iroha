import XCTest
import CryptoKit
@testable import IrohaSwift

final class ConnectCryptoTests: XCTestCase {
    private func requireBridge() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isConnectCryptoAvailable,
            "NoritoBridge connect crypto symbols not linked"
        )
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

    func testRelayAuthHashUsesConnectDomain() throws {
        let sessionID = Data((0..<32).map(UInt8.init))
        var expectedInput = Data("iroha-connect|relay-auth|v1".utf8)
        expectedInput.append(sessionID)
        expectedInput.append(contentsOf: "relay-token".utf8)
        let expected = Data(SHA256.hash(data: expectedInput))

        XCTAssertEqual(try ConnectCrypto.relayAuthHash(sessionID: sessionID, relayToken: "relay-token"), expected)
    }

    func testRelayAuthHashMatchesSharedFixture() throws {
        let sessionID = try XCTUnwrap(Data(hexString: "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"))
        XCTAssertEqual(
            try ConnectCrypto.relayAuthHash(sessionID: sessionID, relayToken: "relay-token-vector").hexEncodedString(),
            "65de07a9c6110f16b6b7c64e63c71437d88d122344e1a67d2c932a16187cce2f"
        )
    }

    func testSessionIDBindsExactNetworkAppKeyAndNonce() throws {
        let appPublicKey = Data((1...32).map(UInt8.init))
        let nonce = Data((65...80).map(UInt8.init))
        let sid = try ConnectCrypto.deriveSessionID(
            networkID: TestNetworkIds.canonical,
            appPublicKey: appPublicKey,
            nonce: nonce
        )

        XCTAssertNotEqual(
            sid,
            try ConnectCrypto.deriveSessionID(
                networkID: TestNetworkIds.other,
                appPublicKey: appPublicKey,
                nonce: nonce
            )
        )
        var otherAppKey = appPublicKey
        otherAppKey[0] ^= 1
        XCTAssertNotEqual(
            sid,
            try ConnectCrypto.deriveSessionID(
                networkID: TestNetworkIds.canonical,
                appPublicKey: otherAppKey,
                nonce: nonce
            )
        )
        XCTAssertThrowsError(
            try ConnectCrypto.deriveSessionID(
                networkID: TestNetworkIds.canonical,
                appPublicKey: Data(repeating: 0, count: 32),
                nonce: nonce
            )
        )
        XCTAssertThrowsError(
            try ConnectCrypto.deriveSessionID(
                networkID: TestNetworkIds.canonical,
                appPublicKey: appPublicKey,
                nonce: Data(repeating: 0, count: 16)
            )
        )
    }

    func testApprovalSignatureRejectsNetworkAccountRelayAndSignatureSubstitution() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let accountID = try AccountId.makeI105(publicKey: signingKey.publicKey())
        let sessionID = Data((1...32).map(UInt8.init))
        let appPublicKey = Data((33...64).map(UInt8.init))
        let walletPublicKey = Data((65...96).map(UInt8.init))
        let relayAuth = try ConnectCrypto.relayAuthHash(
            sessionID: sessionID,
            relayToken: "relay-token"
        )
        let preimage = try ConnectCrypto.buildApprovalPreimage(
            networkID: TestNetworkIds.canonical,
            sessionID: sessionID,
            appPublicKey: appPublicKey,
            walletPublicKey: walletPublicKey,
            accountID: accountID,
            permissions: nil,
            proof: nil,
            relayAuthHash: relayAuth
        )
        let signature = try signingKey.sign(preimage)
        let walletSignature = ConnectWalletSignature(
            algorithm: "ed25519",
            signature: signature
        )

        XCTAssertNoThrow(try ConnectCrypto.verifyApprovalSignature(
            networkID: TestNetworkIds.canonical,
            sessionID: sessionID,
            appPublicKey: appPublicKey,
            walletPublicKey: walletPublicKey,
            accountID: accountID,
            permissions: nil,
            proof: nil,
            relayAuthHash: relayAuth,
            walletSignature: walletSignature
        ))

        func verify(networkID: NetworkId = TestNetworkIds.canonical,
                    accountID: String,
                    relayAuth: Data,
                    signature: Data,
                    algorithm: String = "ed25519") throws {
            try ConnectCrypto.verifyApprovalSignature(
                networkID: networkID,
                sessionID: sessionID,
                appPublicKey: appPublicKey,
                walletPublicKey: walletPublicKey,
                accountID: accountID,
                permissions: nil,
                proof: nil,
                relayAuthHash: relayAuth,
                walletSignature: ConnectWalletSignature(
                    algorithm: algorithm,
                    signature: signature
                )
            )
        }

        XCTAssertThrowsError(try verify(
            networkID: TestNetworkIds.other,
            accountID: accountID,
            relayAuth: relayAuth,
            signature: signature
        ))
        let otherKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x43, count: 32))
        XCTAssertThrowsError(try verify(
            accountID: AccountId.makeI105(publicKey: otherKey.publicKey()),
            relayAuth: relayAuth,
            signature: signature
        ))
        XCTAssertThrowsError(try verify(
            accountID: accountID,
            relayAuth: ConnectCrypto.relayAuthHash(
                sessionID: sessionID,
                relayToken: "other-relay"
            ),
            signature: signature
        ))
        var forged = signature
        forged[0] ^= 1
        XCTAssertThrowsError(try verify(
            accountID: accountID,
            relayAuth: relayAuth,
            signature: forged
        ))
        XCTAssertThrowsError(try verify(
            accountID: accountID,
            relayAuth: relayAuth,
            signature: signature,
            algorithm: "Ed25519"
        ))
    }
}

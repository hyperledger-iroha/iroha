import CryptoKit
import XCTest
@testable import IrohaSwift

final class SigningKeyTests: XCTestCase {
    func testEd25519SigningProducesEnvelope() throws {
        guard #available(macOS 10.15, iOS 13.0, *) else {
            throw XCTSkip("Curve25519 requires macOS 10.15 / iOS 13")
        }
        let key = Curve25519.Signing.PrivateKey()
        let signingKey = try SigningKey.ed25519(privateKey: key.rawRepresentation,
                                                metadata: SigningMetadata(label: "unit-test"))
        let message = Data("swift-signing".utf8)
        let envelope = try signingKey.makeEnvelope(message: message)

        XCTAssertEqual(envelope.algorithm, .ed25519)
        XCTAssertEqual(try signingKey.publicKey(), key.publicKey.rawRepresentation)
        let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: envelope.publicKey)
        XCTAssertTrue(publicKey.isValidSignature(envelope.signature, for: message))
    }

    func testSm2SigningKeyPreservesMetadata() throws {
        let privateKey = Data(repeating: 0xAB, count: Sm2Keypair.privateKeyLength)
        let publicKey = Data(repeating: 0xCD, count: Sm2Keypair.publicKeyLength)
        let pair = try Sm2Keypair(distid: "dist", privateKey: privateKey, publicKey: publicKey)
        let signingKey = SigningKey.sm2(pair)

        XCTAssertEqual(signingKey.algorithm, .sm2)
        XCTAssertEqual(signingKey.metadata.distid, pair.distid)
        XCTAssertEqual(signingKey.metadata.storage, .bridge)
        XCTAssertEqual(try signingKey.publicKey(), publicKey)
    }

    func testSecp256k1SigningRoundTrip() throws {
        guard NoritoNativeBridge.shared.secp256k1Supported else {
            throw XCTSkip("Secp256k1 bridge functions are unavailable on this platform.")
        }
        let privateKey = Data(repeating: 0x01, count: Secp256k1Keypair.privateKeyLength)
        let keypair = try Secp256k1Keypair(privateKey: privateKey)
        let signingKey = SigningKey.secp256k1(keypair, metadata: SigningMetadata(label: "secp"))
        let message = Data("swift-secp256k1-signing".utf8)
        let signature = try signingKey.sign(message)
        XCTAssertEqual(signature.count, Secp256k1Keypair.signatureLength)
        XCTAssertTrue(try keypair.verify(message: message, signature: signature))

        let envelope = try signingKey.makeEnvelope(message: message)
        XCTAssertEqual(envelope.algorithm, .secp256k1)
        XCTAssertEqual(envelope.publicKey, keypair.publicKey)
        XCTAssertEqual(envelope.signature.count, Secp256k1Keypair.signatureLength)

        let verified = NoritoNativeBridge.shared.secp256k1Verify(
            publicKey: envelope.publicKey,
            message: message,
            signature: envelope.signature
        )
        XCTAssertEqual(verified, true)
    }

    func testMlDsaSigningRoundTripWhenBridgeAvailable() throws {
        guard NoritoNativeBridge.shared.mldsaSupported else {
            throw XCTSkip("ML-DSA bridge is unavailable in this environment.")
        }
        let keypair = try MlDsaKeypair.generate(suite: .mlDsa65)
        let signingKey = try SigningKey.mlDsa(privateKey: keypair.secretKey,
                                              metadata: SigningMetadata(label: "mldsa"))
        let message = Data("swift-ml-dsa".utf8)
        let signature = try signingKey.sign(message)
        let expectedLength = try keypair.suite.parameters().signatureLength
        XCTAssertEqual(signature.count, expectedLength)

        guard let verified = NoritoNativeBridge.shared.verifyDetached(
            algorithm: .mlDsa,
            publicKey: try signingKey.publicKey(),
            message: message,
            signature: signature
        ) else {
            throw XCTSkip("ML-DSA verify bridge is unavailable in this environment.")
        }
        XCTAssertTrue(verified)
    }

    func testMultihashPrivateKeyMatchesFixtureAuthority() throws {
        guard #available(macOS 10.15, iOS 13.0, *) else {
            throw XCTSkip("Curve25519 requires macOS 10.15 / iOS 13")
        }
        let authorityId = try loadWalletFixtureAuthority()
        guard let privateKeyBytes = Data(hexString: "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53") else {
            return XCTFail("invalid multihash private key hex")
        }
        let signingKey = try SigningKey.fromMultihashPrivateKey(privateKeyBytes)
        XCTAssertEqual(signingKey.algorithm, .ed25519)

        let publicKey = try signingKey.publicKey()
        let (address, format) = try AccountAddress.parseEncoded(authorityId, expectedPrefix: 753)
        XCTAssertEqual(format, .ih58)
        guard let controller = address.singleControllerInfo() else {
            return XCTFail("expected single-key controller in authority_id")
        }
        XCTAssertEqual(controller.algorithm, .ed25519)
        XCTAssertEqual(controller.publicKey, publicKey)
    }

    private func loadWalletFixtureAuthority() throws -> String {
        struct WalletFixtureAuthority: Decodable {
            let authorityId: String

            enum CodingKeys: String, CodingKey {
                case authorityId = "authority_id"
            }
        }

        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // SigningKeyTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift
            .appendingPathComponent("fixtures/confidential/wallet_flows_v1.json")
        let data = try Data(contentsOf: url)
        let decoder = JSONDecoder()
        let fixture = try decoder.decode(WalletFixtureAuthority.self, from: data)
        return fixture.authorityId
    }
}

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

    func testEd25519SignatureAdmissionRejectsInertAndMalformedR() throws {
        guard #available(macOS 10.15, iOS 13.0, *) else {
            throw XCTSkip("Curve25519 requires macOS 10.15 / iOS 13")
        }
        let key = Curve25519.Signing.PrivateKey()
        let message = Data("swift-ed25519-signature-admission".utf8)
        let signature = try key.signature(for: message)
        XCTAssertTrue(Ed25519SignatureAdmission.isValidSignature(signature))

        let smallOrderR = Data([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])
        let noncanonicalR = Data([
            0xEE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F,
        ])

        XCTAssertFalse(Ed25519SignatureAdmission.isValidSignature(Data(repeating: 0, count: 64)))
        XCTAssertFalse(Ed25519SignatureAdmission.isValidSignature(Data(signature.dropLast())))
        for replacementR in [smallOrderR, noncanonicalR] {
            var malformed = signature
            malformed.replaceSubrange(0..<replacementR.count, with: replacementR)
            XCTAssertFalse(Ed25519SignatureAdmission.isValidSignature(malformed))
        }
    }

    func testEd25519PublicKeyAdmissionRejectsWeakOrNoncanonicalMaterial() throws {
        guard #available(macOS 10.15, iOS 13.0, *) else {
            throw XCTSkip("Curve25519 requires macOS 10.15 / iOS 13")
        }
        let key = Curve25519.Signing.PrivateKey()
        XCTAssertTrue(Ed25519PublicKeyAdmission.isValidPublicKey(key.publicKey.rawRepresentation))

        let smallOrderKey = Data([
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])
        let noncanonicalKey = Data([
            0xEE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F,
        ])

        XCTAssertFalse(Ed25519PublicKeyAdmission.isValidPublicKey(Data(repeating: 0, count: 32)))
        XCTAssertFalse(Ed25519PublicKeyAdmission.isValidPublicKey(Data(repeating: 0x42, count: 31)))
        XCTAssertFalse(Ed25519PublicKeyAdmission.isValidPublicKey(smallOrderKey))
        XCTAssertFalse(Ed25519PublicKeyAdmission.isValidPublicKey(noncanonicalKey))
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
        let expectedLength = keypair.suite.parameters().signatureLength
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

    func testMlDsaSuiteParametersAreAvailableBeforeBridge() {
        let expected: [(MlDsaSuite, Int, Int, Int)] = [
            (.mlDsa44, 1_312, 2_560, 2_420),
            (.mlDsa65, 1_952, 4_032, 3_309),
            (.mlDsa87, 2_592, 4_896, 4_627),
        ]
        for (suite, publicKeyLength, secretKeyLength, signatureLength) in expected {
            let parameters = suite.parameters()
            XCTAssertEqual(parameters.publicKeyLength, publicKeyLength)
            XCTAssertEqual(parameters.secretKeyLength, secretKeyLength)
            XCTAssertEqual(parameters.signatureLength, signatureLength)
        }
    }

    func testMlDsaKeypairRejectsMalformedKeyLengthsBeforeBridge() {
        let parameters = MlDsaSuite.mlDsa65.parameters()
        let publicKey = Data(repeating: 0xA5, count: parameters.publicKeyLength)
        let secretKey = Data(repeating: 0x5A, count: parameters.secretKeyLength)
        XCTAssertNoThrow(try MlDsaKeypair(suite: .mlDsa65,
                                          publicKey: publicKey,
                                          secretKey: secretKey))

        XCTAssertThrowsError(try MlDsaKeypair(suite: .mlDsa65,
                                              publicKey: Data(publicKey.dropLast()),
                                              secretKey: secretKey)) { error in
            guard case MlDsaError.invalidKeyLength = error else {
                return XCTFail("short ML-DSA public key failed with unexpected error: \(error)")
            }
        }

        var overlongSecretKey = secretKey
        overlongSecretKey.append(0x42)
        XCTAssertThrowsError(try MlDsaKeypair(suite: .mlDsa65,
                                              publicKey: publicKey,
                                              secretKey: overlongSecretKey)) { error in
            guard case MlDsaError.invalidKeyLength = error else {
                return XCTFail("overlong ML-DSA secret key failed with unexpected error: \(error)")
            }
        }
    }

    func testMlDsaVerifyRejectsMalformedSignatureLengthsBeforeBridge() throws {
        let parameters = MlDsaSuite.mlDsa65.parameters()
        let keypair = try MlDsaKeypair(suite: .mlDsa65,
                                       publicKey: Data(repeating: 0xA5, count: parameters.publicKeyLength),
                                       secretKey: Data(repeating: 0x5A, count: parameters.secretKeyLength))
        let message = Data("swift-ml-dsa-length-admission".utf8)

        XCTAssertThrowsError(try keypair.verify(message: message,
                                                signature: Data(repeating: 0x11,
                                                                count: parameters.signatureLength - 1))) { error in
            guard case MlDsaError.invalidSignatureLength = error else {
                return XCTFail("short ML-DSA signature failed with unexpected error: \(error)")
            }
        }

        XCTAssertThrowsError(try keypair.verify(message: message,
                                                signature: Data(repeating: 0x22,
                                                                count: parameters.signatureLength + 1))) { error in
            guard case MlDsaError.invalidSignatureLength = error else {
                return XCTFail("overlong ML-DSA signature failed with unexpected error: \(error)")
            }
        }
    }

    func testMlDsaExactLengthVerifyReportsUnavailableBridge() throws {
        guard !NoritoNativeBridge.shared.mldsaSupported else {
            throw XCTSkip("ML-DSA bridge is available in this environment.")
        }
        let parameters = MlDsaSuite.mlDsa65.parameters()
        let keypair = try MlDsaKeypair(suite: .mlDsa65,
                                       publicKey: Data(repeating: 0xA5, count: parameters.publicKeyLength),
                                       secretKey: Data(repeating: 0x5A, count: parameters.secretKeyLength))

        XCTAssertThrowsError(try keypair.verify(message: Data("swift-ml-dsa-bridge-unavailable".utf8),
                                                signature: Data(repeating: 0x33,
                                                                count: parameters.signatureLength))) { error in
            guard case MlDsaError.bridgeUnavailable = error else {
                return XCTFail("exact-length ML-DSA verification failed with unexpected error: \(error)")
            }
        }
    }

    func testMlDsaVerifyRejectsMalformedSignaturesWhenBridgeAvailable() throws {
        guard NoritoNativeBridge.shared.mldsaSupported else {
            throw XCTSkip("ML-DSA bridge is unavailable in this environment.")
        }
        let keypair = try MlDsaKeypair.generate(suite: .mlDsa65)
        let message = Data("swift-ml-dsa-signature-admission".utf8)
        let signature = try keypair.sign(message: message)

        XCTAssertTrue(try keypair.verify(message: message, signature: signature))

        var shortSignature = signature
        shortSignature.removeLast()
        XCTAssertThrowsError(try keypair.verify(message: message, signature: shortSignature)) { error in
            guard case MlDsaError.invalidSignatureLength = error else {
                return XCTFail("short ML-DSA signature failed with unexpected error: \(error)")
            }
        }

        var overlongSignature = signature
        overlongSignature.append(0x42)
        XCTAssertThrowsError(try keypair.verify(message: message, signature: overlongSignature)) { error in
            guard case MlDsaError.invalidSignatureLength = error else {
                return XCTFail("overlong ML-DSA signature failed with unexpected error: \(error)")
            }
        }

        let allZeroSignature = Data(repeating: 0, count: signature.count)
        XCTAssertEqual(NoritoNativeBridge.shared.mldsaVerify(
            suiteId: keypair.suite.rawValue,
            publicKey: keypair.publicKey,
            message: message,
            signature: allZeroSignature
        ), false)
        XCTAssertFalse(try keypair.verify(message: message, signature: allZeroSignature))
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
        let address = try AccountAddress.parseEncoded(authorityId, expectedPrefix: 753)
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

import XCTest
@testable import IrohaSwift

final class Secp256k1SigningTests: XCTestCase {
    private let privateKeyHex = "e4f21b38e005d4f895a29e84948d7cc83eac79041aeb644ee4fab8d9da42f713"
    private let publicKeyHex = "0242c1e1f775237a26da4fd51b8d75ee2709711f6e90303e511169a324ef0789c0"
    private let signatureHex = "0aab347be3530a3fd7d91c354956561101e6f273b8a1ea3d414f82fbd5939db34b99c54c16c45bf4cde8193b58d718e7efa8c055e7add7d9c9cbe8935e849200"
    private let message = Data("This is a dummy message for use with tests".utf8)

    func testSecp256k1KeypairSignsAndVerifies() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .secp256k1),
                      "secp256k1 bridge functions are unavailable on this platform.")
        guard let privateKey = Data(hexString: privateKeyHex) else {
            return XCTFail("invalid private key hex")
        }
        guard let expectedPublic = Data(hexString: publicKeyHex) else {
            return XCTFail("invalid public key hex")
        }
        guard let expectedSignature = Data(hexString: signatureHex) else {
            return XCTFail("invalid signature hex")
        }

        let keypair = try Secp256k1Keypair(privateKey: privateKey)
        XCTAssertEqual(keypair.publicKey, expectedPublic)

        let signature = try keypair.sign(message: message)
        XCTAssertEqual(signature, expectedSignature)
        XCTAssertTrue(try keypair.verify(message: message, signature: signature))
    }

    func testSecp256k1SigningKeyEnvelope() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .secp256k1),
                      "secp256k1 bridge functions are unavailable on this platform.")
        guard let privateKey = Data(hexString: privateKeyHex),
              let expectedPublic = Data(hexString: publicKeyHex),
              let expectedSignature = Data(hexString: signatureHex) else {
            return XCTFail("failed to build fixtures")
        }

        let signingKey = try SigningKey.secp256k1(privateKey: privateKey)
        let envelope = try signingKey.makeEnvelope(message: message)

        XCTAssertEqual(envelope.algorithm, .secp256k1)
        XCTAssertEqual(envelope.publicKey, expectedPublic)
        XCTAssertEqual(envelope.signature, expectedSignature)
    }

    func testSecp256k1BridgeDetachedHelpers() throws {
        guard NoritoNativeBridge.shared.secp256k1Supported else {
            throw XCTSkip("secp256k1 bridge functions are unavailable on this platform.")
        }
        guard let privateKey = Data(hexString: privateKeyHex),
              let expectedPublic = Data(hexString: publicKeyHex) else {
            return XCTFail("failed to build fixtures")
        }

        guard let derivedPublic = NoritoNativeBridge.shared.publicKeyFromPrivate(
            algorithm: .secp256k1,
            privateKey: privateKey
        ) else {
            return XCTFail("bridge could not derive public key")
        }
        XCTAssertEqual(derivedPublic, expectedPublic)

        guard let signature = NoritoNativeBridge.shared.signDetached(
            algorithm: .secp256k1,
            privateKey: privateKey,
            message: message
        ) else {
            return XCTFail("bridge could not sign message")
        }
        XCTAssertEqual(signature.count, Secp256k1Keypair.signatureLength)
        if let expectedSignature = Data(hexString: signatureHex) {
            XCTAssertEqual(signature, expectedSignature)
        }

        guard let verified = NoritoNativeBridge.shared.verifyDetached(
            algorithm: .secp256k1,
            publicKey: expectedPublic,
            message: message,
            signature: signature
        ) else {
            return XCTFail("bridge could not verify signature")
        }
        XCTAssertTrue(verified)
    }
}

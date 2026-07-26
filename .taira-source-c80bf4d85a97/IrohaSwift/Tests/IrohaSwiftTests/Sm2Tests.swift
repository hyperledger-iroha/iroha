import Foundation
import XCTest
@testable import IrohaSwift

final class Sm2Tests: XCTestCase {
    private struct Fixture: Decodable {
        struct Vector: Decodable {
            let caseId: String
            let note: String?
            let distid: String
            let seedHex: String?
            let messageHex: String
            let privateKeyHex: String?
            let publicKeySec1Hex: String
            let publicKeyMultihash: String?
            let publicKeyPrefixed: String?
            let za: String?
            let signature: String
            let r: String?
            let s: String?

            private enum CodingKeys: String, CodingKey {
                case caseId = "case_id"
                case note
                case distid
                case seedHex = "seed_hex"
                case messageHex = "message_hex"
                case privateKeyHex = "private_key_hex"
                case publicKeySec1Hex = "public_key_sec1_hex"
                case publicKeyMultihash = "public_key_multihash"
                case publicKeyPrefixed = "public_key_prefixed"
                case za
                case signature
                case r
                case s
            }
        }

        let distid: String
        let seedHex: String
        let messageHex: String
        let privateKeyHex: String
        let publicKeySec1Hex: String
        let publicKeyMultihash: String
        let publicKeyPrefixed: String
        let za: String
        let signature: String
        let r: String
        let s: String
        let vectors: [Vector]

        private enum CodingKeys: String, CodingKey {
            case distid
            case seedHex = "seed_hex"
            case messageHex = "message_hex"
            case privateKeyHex = "private_key_hex"
            case publicKeySec1Hex = "public_key_sec1_hex"
            case publicKeyMultihash = "public_key_multihash"
            case publicKeyPrefixed = "public_key_prefixed"
            case za
            case signature
            case r
            case s
            case vectors
        }
    }

    func testCanonicalFixtureMatchesBridge() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isSm2Available, "SM2 helpers unavailable")

        let fixture = try loadFixture()
        guard
            let seed = Data(hexString: fixture.seedHex),
            let message = Data(hexString: fixture.messageHex),
            let expectedSignature = Data(hexString: fixture.signature),
            let expectedZa = Data(hexString: fixture.za)
        else {
            XCTFail("Failed to decode canonical fixture hex values")
            return
        }

        XCTAssertEqual(Sm2Keypair.defaultDistid(), fixture.distid)

        let keypair = try Sm2Keypair.deriveFromSeed(distid: fixture.distid, seed: seed)
        XCTAssertEqual(hexUppercase(keypair.privateKey), fixture.privateKeyHex)
        XCTAssertEqual(hexUppercase(keypair.publicKey), fixture.publicKeySec1Hex)
        XCTAssertEqual(try keypair.publicKeyPrefixed(), fixture.publicKeyPrefixed)
        XCTAssertEqual(try keypair.publicKeyMultihash(), fixture.publicKeyMultihash)
        XCTAssertEqual(hexUppercase(try keypair.computeZA()), hexUppercase(expectedZa))

        let signature = try keypair.sign(message: message)
        XCTAssertEqual(hexUppercase(signature), fixture.signature)
        XCTAssertTrue(try keypair.verify(message: message, signature: signature))
        XCTAssertTrue(try keypair.verify(message: message, signature: expectedSignature))

        let mutatedMessage = Data(message.reversed())
        XCTAssertFalse(try keypair.verify(message: mutatedMessage, signature: signature))
    }

    func testRustSdkVectorParity() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isSm2Available, "SM2 helpers unavailable")

        let fixture = try loadFixture()
        guard let vector = fixture.vectors.first(where: { $0.caseId == "sm2-rust-sdk-fixture-v1" }) else {
            XCTFail("Missing sm2-rust-sdk-fixture-v1 entry")
            return
        }
        guard
            let seedHex = vector.seedHex,
            let seed = Data(hexString: seedHex),
            let message = Data(hexString: vector.messageHex),
            let expectedSignature = Data(hexString: vector.signature),
            let expectedMultihash = vector.publicKeyMultihash,
            let expectedPrefixed = vector.publicKeyPrefixed
        else {
            XCTFail("Fixture vector missing required fields")
            return
        }

        let expectedZa = vector.za.flatMap(Data.init(hexString:))

        let keypair = try Sm2Keypair.deriveFromSeed(distid: vector.distid, seed: seed)
        if let pkHex = vector.privateKeyHex {
            XCTAssertEqual(hexUppercase(keypair.privateKey), pkHex)
        }
        XCTAssertEqual(hexUppercase(keypair.publicKey), vector.publicKeySec1Hex)
        XCTAssertEqual(try keypair.publicKeyMultihash(), expectedMultihash)
        XCTAssertEqual(try keypair.publicKeyPrefixed(), expectedPrefixed)
        if let expectedZa = expectedZa {
            XCTAssertEqual(hexUppercase(try keypair.computeZA()), hexUppercase(expectedZa))
        }

        let signature = try keypair.sign(message: message)
        XCTAssertEqual(signature, expectedSignature)
        XCTAssertTrue(try keypair.verify(message: message, signature: signature))

        var tamperedSignature = signature
        tamperedSignature[0] ^= 0xFF
        XCTAssertFalse(try keypair.verify(message: message, signature: tamperedSignature))
    }

    // MARK: - Helpers

    private func loadFixture() throws -> Fixture {
        let data = try Data(contentsOf: fixtureURL())
        let decoder = JSONDecoder()
        return try decoder.decode(Fixture.self, from: data)
    }

    private func fixtureURL() -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/sm/sm2_fixture.json")
    }

    private func hexUppercase(_ data: Data) -> String {
        data.map { String(format: "%02X", $0) }.joined()
    }
}

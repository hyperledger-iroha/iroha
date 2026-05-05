import XCTest
@testable import IrohaSwift

final class IrohaSDKSigningAlgorithmTests: XCTestCase {
    private let baseURL = URL(string: "https://example.com")!

    func testDefaultSigningAlgorithmRemainsEd25519() throws {
        let sdk = IrohaSDK(baseURL: baseURL)
        XCTAssertEqual(sdk.defaultSigningAlgorithm, .ed25519)

        let signingKey = try sdk.generateSigningKey()
        XCTAssertEqual(signingKey.algorithm, .ed25519)
    }

    func testSigningAlgorithmsMatchRustBridgeDiscriminants() {
        let expected: [(SigningAlgorithm, UInt8, String)] = [
            (.ed25519, 0, "ed25519"),
            (.secp256k1, 1, "secp256k1"),
            (.blsNormal, 2, "bls_normal"),
            (.blsSmall, 3, "bls_small"),
            (.mlDsa, 4, "ml-dsa"),
            (.gost2012_256A, 5, "gost3410-2012-256-paramset-a"),
            (.gost2012_256B, 6, "gost3410-2012-256-paramset-b"),
            (.gost2012_256C, 7, "gost3410-2012-256-paramset-c"),
            (.gost2012_512A, 8, "gost3410-2012-512-paramset-a"),
            (.gost2012_512B, 9, "gost3410-2012-512-paramset-b"),
            (.sm2, 10, "sm2")
        ]

        XCTAssertEqual(SigningAlgorithm.allCases.count, expected.count)
        for (algorithm, discriminant, wireName) in expected {
            XCTAssertEqual(algorithm.noritoDiscriminant, discriminant)
            XCTAssertEqual(SigningAlgorithm(noritoDiscriminant: discriminant), algorithm)
            XCTAssertEqual(algorithm.wireName, wireName)
        }
    }

    func testSigningKeyFromSeedUsesConfiguredMlDsa() throws {
        let sdk = IrohaSDK(baseURL: baseURL, defaultSigningAlgorithm: .mlDsa)
        let seed = Data("iroha-swift-ml-dsa-seed".utf8)

        guard NoritoNativeBridge.shared.keypairFromSeed(algorithm: .mlDsa, seed: seed) != nil else {
            throw XCTSkip("NoritoBridge ML-DSA seed derivation unavailable")
        }

        let signingKey = try sdk.signingKey(fromSeed: seed)
        XCTAssertEqual(signingKey.algorithm, .mlDsa)

        let second = try sdk.signingKey(fromSeed: seed)
        XCTAssertEqual(try signingKey.publicKey(), try second.publicKey())
    }

    func testKeypairSeedMaterialUsesEd25519() throws {
        let keypair = try Keypair.generate()
        let sdk = IrohaSDK(baseURL: baseURL)

        let signingKey = try sdk.signingKey(fromSeed: keypair.privateKeyBytes)
        XCTAssertEqual(signingKey.algorithm, .ed25519)
        XCTAssertEqual(try signingKey.publicKey(), keypair.publicKey)
    }
}

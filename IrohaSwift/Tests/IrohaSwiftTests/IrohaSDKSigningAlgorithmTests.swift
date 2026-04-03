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

    func testLegacyKeypairRemainsEd25519Compatible() throws {
        let keypair = try Keypair.generate()
        let sdk = IrohaSDK(baseURL: baseURL)

        let signingKey = try sdk.signingKey(fromSeed: keypair.privateKeyBytes)
        XCTAssertEqual(signingKey.algorithm, .ed25519)
        XCTAssertEqual(try signingKey.publicKey(), keypair.publicKey)
    }
}

import XCTest
@testable import IrohaSwift

final class MultisigPolicyBuilderTests: XCTestCase {
    func testBuilderProducesDigestAndCtap2Payload() throws {
        let builder = MultisigPolicyBuilder()
            .setVersion(1)
            .setThreshold(2)
            .addMember(algorithm: .ed25519,
                       weight: 1,
                       publicKey: Data(repeating: 0x11, count: 32))
            .addMember(algorithm: .ed25519,
                       weight: 1,
                       publicKey: Data(repeating: 0x22, count: 32))

        let policy = try builder.build()

        XCTAssertEqual(policy.members.count, 2)
        XCTAssertEqual(policy.threshold, 2)
        XCTAssertFalse(policy.ctap2Cbor.isEmpty)
        XCTAssertEqual(policy.digestBlake2b256.count, 32)
    }

    func testMissingThresholdThrows() {
        let builder = MultisigPolicyBuilder()
            .addMember(algorithm: .ed25519,
                       weight: 1,
                       publicKey: Data(repeating: 0x33, count: 32))

        XCTAssertThrowsError(try builder.build()) { error in
            guard case MultisigBuilderError.thresholdNotSet = error else {
                return XCTFail("Expected thresholdNotSet, got \(error)")
            }
        }
    }

    func testBuilderSupportsSecp256k1Members() throws {
        let policy = try MultisigPolicyBuilder()
            .setThreshold(1)
            .addMember(algorithm: .secp256k1,
                       weight: 1,
                       publicKey: Data(repeating: 0xAA, count: Secp256k1Keypair.publicKeyLength))
            .build()

        XCTAssertEqual(policy.members.first?.algorithm, .secp256k1)
        XCTAssertEqual(policy.members.first?.weight, 1)
        XCTAssertFalse(policy.ctap2Cbor.isEmpty)
    }
}

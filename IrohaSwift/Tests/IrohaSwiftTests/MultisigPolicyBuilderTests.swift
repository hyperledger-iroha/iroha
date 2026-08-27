import XCTest
@testable import IrohaSwift

final class MultisigPolicyBuilderTests: XCTestCase {
    func testBuilderProducesDigestAndCtap2Payload() throws {
        let firstPublicKey = try Keypair(
            privateKeyBytes: Data(repeating: 0x11, count: 32)
        ).publicKey
        let secondPublicKey = try Keypair(
            privateKeyBytes: Data(repeating: 0x22, count: 32)
        ).publicKey
        let builder = MultisigPolicyBuilder()
            .setVersion(1)
            .setThreshold(2)
            .addMember(algorithm: .ed25519,
                       weight: 1,
                       publicKey: firstPublicKey)
            .addMember(algorithm: .ed25519,
                       weight: 1,
                       publicKey: secondPublicKey)

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

    #if IROHASWIFT_ENABLE_MLDSA
    func testBuilderRequiresProtocolMlDsa65MemberKeys() throws {
        let publicKey = Data(repeating: 0xA5, count: 1_952)
        let policy = try MultisigPolicyBuilder()
            .setThreshold(1)
            .addMember(algorithm: .mlDsa, weight: 1, publicKey: publicKey)
            .build()
        XCTAssertEqual(policy.members.first?.publicKey, publicKey)

        for malformedKey in [
            Data(),
            Data(repeating: 0x20, count: 32),
            Data(repeating: 0x44, count: 1_312),
            Data(repeating: 0x65, count: 1_951),
            Data(repeating: 0x65, count: 1_953),
            Data(repeating: 0x87, count: 2_592),
            Data(repeating: 0, count: 1_952),
        ] {
            XCTAssertThrowsError(
                try MultisigPolicyBuilder()
                    .setThreshold(1)
                    .addMember(algorithm: .mlDsa, weight: 1, publicKey: malformedKey)
                    .build()
            ) { error in
                guard case AccountAddressError.invalidPublicKey = error else {
                    return XCTFail("unexpected error: \(error)")
                }
            }
        }
    }
    #endif
}

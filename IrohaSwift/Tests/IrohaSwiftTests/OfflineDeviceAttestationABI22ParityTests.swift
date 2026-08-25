import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Exact Rust/Swift golden coverage for the bridge ABI22 device-registration model.
final class OfflineDeviceAttestationABI22ParityTests: XCTestCase {
    func testRegistrationAndChallengeMatchRustCurrentModel() throws {
        let rust = try rustFixture()
        XCTAssertEqual(rust.fixture, "offline_device_attestation_abi22")
        XCTAssertEqual(rust.generatedBy, "kotlin-fixture-gen offline-device-attestation")
        let assertionPublicKey = try XCTUnwrap(Data(hexString: Self.p256Generator))
        let devicePublicKey = try KagemushaDevicePublicKeyV2(
            sec1Bytes: assertionPublicKey
        )
        let signingCertificate = Data(
            SHA256.hash(data: Data("abi22-unit-test-signing-certificate".utf8))
        )
        let registration = try KagemushaDeviceAttestationRegistration(
            version: 1,
            platform: KagemushaDeviceAttestation.androidKeyMintPlatform,
            keyId: Data(SHA256.hash(data: assertionPublicKey)).hexLowercased(),
            deviceId: "abi22-android-unit-test-device",
            accountId: rust.accountId,
            assetDefinitionId: nil,
            iosTeamId: nil,
            iosBundleId: nil,
            iosEnvironment: nil,
            androidPackageName: "org.hyperledger.iroha.abi22.fixture",
            androidSigningCertificateSha256: signingCertificate,
            publicKey: devicePublicKey,
            assertionScheme: KagemushaDeviceAttestation.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm:
                KagemushaDeviceAttestation.androidKeyMintAssertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: 1,
            oneUse: true,
            attestationReport:
                Data("abi22-unit-test-not-physical-attestation-evidence".utf8),
            recentBlockHeight: 42,
            recentBlockHash: IrohaHash.hash(Data("abi22-unit-test-block".utf8)),
            expiresAtMs: 2_000_000_000_000
        )

        XCTAssertEqual(
            try registration.noritoEncoded(),
            try XCTUnwrap(Data(hexString: rust.registrationHex))
        )
        XCTAssertEqual(
            registration.challengeHash,
            try XCTUnwrap(Data(hexString: rust.challengeHashHex))
        )
        XCTAssertEqual(
            registration.canonicalRegistrationId,
            try XCTUnwrap(Data(hexString: rust.registrationIdHex))
        )
        XCTAssertEqual(
            registration.canonicalRegistrationId,
            IrohaHash.hash(try registration.noritoEncoded())
        )
    }

    private func rustFixture() throws -> RustFixture {
        let url = try XCTUnwrap(
            Bundle.module.url(
                forResource: "offline_device_attestation_abi22",
                withExtension: "json"
            ),
            "The checked-in Rust-authored bridge ABI22 fixture is required."
        )
        return try JSONDecoder().decode(RustFixture.self, from: Data(contentsOf: url))
    }

    private struct RustFixture: Decodable {
        let fixture: String
        let generatedBy: String
        let registrationHex: String
        let challengeHashHex: String
        let accountId: String
        let registrationIdHex: String

        private enum CodingKeys: String, CodingKey {
            case fixture
            case generatedBy = "generated_by"
            case registrationHex = "registration_hex"
            case challengeHashHex = "challenge_hash_hex"
            case accountId = "account_id"
            case registrationIdHex = "registration_id_hex"
        }
    }

    private static let p256Generator =
        "04"
        + "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
        + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
}

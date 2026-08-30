import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Exact Rust/Swift golden coverage for ABI-22 device-registration V2.
final class OfflineDeviceAttestationV2ParityTests: XCTestCase {
    func testRegistrationAndChallengeMatchRustCurrentModel() throws {
        let rust = try rustFixture()
        XCTAssertEqual(rust.fixture, "offline_device_attestation_abi21")
        XCTAssertEqual(rust.generatedBy, "kotlin-fixture-gen offline-device-attestation")
        let assertionPublicKey = try XCTUnwrap(Data(hexString: Self.p256Generator))
        let devicePublicKey = try KagemushaDevicePublicKeyV2(
            sec1Bytes: assertionPublicKey
        )
        let signingCertificate = Data(
            SHA256.hash(data: Data("abi22-v2-unit-test-signing-certificate".utf8))
        )
        let registration = try KagemushaDeviceAttestationRegistration(
            version: KagemushaDeviceAttestation.registrationVersion,
            platform: KagemushaDeviceAttestation.androidKeyMintPlatform,
            keyId: Data(SHA256.hash(data: assertionPublicKey)).hexLowercased(),
            deviceId: "abi22-v2-android-unit-test-device",
            accountId: rust.accountId,
            assetDefinitionId: nil,
            iosTeamId: nil,
            iosBundleId: nil,
            iosEnvironment: nil,
            androidPackageName: "org.hyperledger.iroha.abi22.v2.fixture",
            androidSigningCertificateSha256: signingCertificate,
            androidAttestedDeviceProperties: try Self.androidProperties(),
            publicKey: devicePublicKey,
            assertionScheme: KagemushaDeviceAttestation.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm:
                KagemushaDeviceAttestation.androidKeyMintAssertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: 1,
            oneUse: true,
            attestationReport:
                Data("abi22-v2-unit-test-not-physical-attestation-evidence".utf8),
            recentBlockHeight: 42,
            recentBlockHash: IrohaHash.hash(Data("abi22-v2-unit-test-block".utf8)),
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

    private static func androidProperties() throws
        -> OfflineAndroidAttestedDevicePropertiesV2 {
        try OfflineAndroidAttestedDevicePropertiesV2(
            attestationVersion: 300,
            keymintVersion: 300,
            securityLevel: .strongBox,
            brand: "google",
            device: "husky",
            product: "husky",
            manufacturer: "Google",
            model: "Pixel 8 Pro",
            osVersion: 140_000,
            osPatchLevel: 202_608,
            vendorPatchLevel: 20_260_805,
            bootPatchLevel: 20_260_801,
            verifiedBootKey: Data(repeating: 0x42, count: 32),
            verifiedBootHash: Data(repeating: 0x24, count: 32)
        )
    }

    private func rustFixture() throws -> RustFixture {
        let url = try XCTUnwrap(
            Bundle.module.url(
                forResource: "offline_device_attestation_abi21",
                withExtension: "json"
            ),
            "The checked-in Rust-authored ABI-21 fixture is required."
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

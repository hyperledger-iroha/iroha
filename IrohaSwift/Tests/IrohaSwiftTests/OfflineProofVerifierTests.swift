import CryptoKit
import Foundation
@testable import IrohaSwift
import XCTest

final class OfflineProofVerifierTests: XCTestCase {
    func testErrorDescriptionReturnsVerifierMessage() {
        XCTAssertEqual(
            OfflineProofVerifierError.invalidBinding("binding-invalid").errorDescription,
            "binding-invalid"
        )
        XCTAssertEqual(
            OfflineProofVerifierError.invalidProof("proof-invalid").errorDescription,
            "proof-invalid"
        )
    }

    func testEd25519VerifierSupportRejectsWeakOrNoncanonicalPublicKeysBeforeCryptoKit() throws {
        guard #available(macOS 10.15, iOS 13.0, *) else {
            throw XCTSkip("Curve25519 requires macOS 10.15 / iOS 13")
        }
        let key = Curve25519.Signing.PrivateKey()
        let payload = Data("swift-offline-ed25519-public-key-admission".utf8)
        let signature = try key.signature(for: payload)
        XCTAssertTrue(try OfflineProofVerifierSupportTestHooks.verifyEd25519Signature(
            payload: payload,
            signature: signature,
            rawPublicKey: key.publicKey.rawRepresentation
        ))

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

        for rawPublicKey in [
            Data(repeating: 0, count: 32),
            smallOrderKey,
            noncanonicalKey,
        ] {
            XCTAssertFalse(try OfflineProofVerifierSupportTestHooks.verifyEd25519Signature(
                payload: payload,
                signature: signature,
                rawPublicKey: rawPublicKey
            ))
        }
    }

    func testDefaultIosRootCertificateIsBase64Encoded() {
        let encoded = IosOfflineProofVerifier.defaultTrustedRootCertificateBase64

        XCTAssertFalse(encoded.isEmpty)
        XCTAssertNotNil(Data(base64Encoded: encoded))
    }

    func testCounterpartyVerifierRequiresIosChallengeHash() throws {
        let binding = try ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "attestation-key",
            deviceId: "ios-device",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: "not-empty"
        )

        XCTAssertThrowsError(
            try CounterpartyOfflineProofVerifier().verifyDeviceBinding(
                accountId: "account",
                binding: binding,
                expectedChallengeHashHex: nil
            )
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Missing offline device binding challenge hash."
            )
        }
    }

    func testCounterpartyVerifierRoutesIosAppAttestThroughIosVerifier() throws {
        let binding = try ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "attestation-key",
            deviceId: "ios-device",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: ""
        )

        XCTAssertThrowsError(
            try CounterpartyOfflineProofVerifier().verifyDeviceBinding(
                accountId: "account",
                binding: binding,
                expectedChallengeHashHex: Self.hexLowercased(Self.challengeBytes())
            )
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Offline device binding iOS metadata is incomplete."
            )
        }
    }

    func testCounterpartyVerifierRoutesIosProofThroughIosVerifier() throws {
        let keyId = Data(repeating: 0x01, count: 32).base64EncodedString()
        let binding = try ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: keyId,
            deviceId: "ios-device",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: "not-empty",
            iosTeamId: "TEAMID1234",
            iosBundleId: "jp.co.soramitsu.iroha.offline",
            iosEnvironment: "production"
        )
        let proof = try ToriiOfflineDeviceProof(
            platform: "ios",
            attestationKeyId: keyId,
            challengeHashHex: Self.hexLowercased(Self.challengeBytes()),
            assertionBase64: Data("assertion".utf8).base64EncodedString()
        )

        XCTAssertThrowsError(
            try CounterpartyOfflineProofVerifier().verifyDeviceProof(binding: binding, proof: proof)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "iOS offline proofs must include a counter."
            )
        }
    }

    func testCounterpartyVerifierRejectsRemovedPlatformAliasesBeforeDispatch() throws {
        for platform in ["ios-appattest"] {
            XCTAssertThrowsError(try ToriiOfflineDeviceBinding(
                platform: platform,
                attestationKeyId: "attestation-key",
                deviceId: "ios-alias-device",
                offlinePublicKey: "offline-public-key",
                attestationReportBase64: ""
            )) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("platform"))
            }
            let binding = try ToriiOfflineDeviceBinding(
                platform: "ios",
                attestationKeyId: "attestation-key",
                deviceId: "ios-device",
                offlinePublicKey: "offline-public-key",
                attestationReportBase64: ""
            )
            XCTAssertThrowsError(
                try CounterpartyOfflineProofVerifier().verifyDeviceBinding(
                    accountId: "account",
                    binding: binding,
                    expectedChallengeHashHex: Self.hexLowercased(Self.challengeBytes())
                )
            ) { error in
                XCTAssertEqual(
                    (error as? OfflineProofVerifierError)?.errorDescription,
                    "Offline device binding iOS metadata is incomplete."
                )
            }
            let proof = try ToriiOfflineDeviceProof(
                platform: platform,
                attestationKeyId: "attestation-key",
                challengeHashHex: Self.hexLowercased(Self.challengeBytes()),
                assertionBase64: Data("assertion".utf8).base64EncodedString(),
                counter: nil
            )
            XCTAssertThrowsError(
                try CounterpartyOfflineProofVerifier().verifyDeviceProof(binding: binding, proof: proof)
            ) { error in
                XCTAssertEqual(
                    (error as? OfflineProofVerifierError)?.errorDescription,
                    "Unsupported offline device proof platform."
                )
            }
        }

        for platform in ["ios-app-attest", "android-keymint"] {
            XCTAssertThrowsError(try ToriiOfflineDeviceBinding(
                platform: platform,
                attestationKeyId: "attestation-key",
                deviceId: "retired-device",
                offlinePublicKey: "offline-public-key",
                attestationReportBase64: ""
            )) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("platform"))
            }
        }
        XCTAssertThrowsError(try ToriiOfflineDeviceProof(
            platform: "android-keymint",
            attestationKeyId: "attestation-key",
            challengeHashHex: Self.hexLowercased(Self.challengeBytes()),
            assertionBase64: Data("assertion".utf8).base64EncodedString(),
            counter: nil
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("platform"))
        }
    }

    func testCounterpartyVerifierDispatchesAndroidPlatform() throws {
        let androidBinding = try ToriiOfflineDeviceBinding(
            platform: "android",
            attestationKeyId: "attestation-key",
            deviceId: "android-device",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: ""
        )
        XCTAssertThrowsError(
            try CounterpartyOfflineProofVerifier().verifyDeviceBinding(
                accountId: "account",
                binding: androidBinding,
                expectedChallengeHashHex: nil
            )
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Offline device binding is incomplete."
            )
        }
    }

    func testCounterpartyVerifierRejectsPaddedPlatformBeforeDispatch() {
        XCTAssertThrowsError(try ToriiOfflineDeviceBinding(
            platform: " ios",
            attestationKeyId: "attestation-key",
            deviceId: "ios-device",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: ""
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("platform"))
        }
    }

    func testCounterpartyVerifierRejectsCaseChangedPlatformBeforeDispatch() {
        XCTAssertThrowsError(try ToriiOfflineDeviceBinding(
            platform: "IOS",
            attestationKeyId: "attestation-key",
            deviceId: "ios-device",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: ""
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("platform"))
        }

        XCTAssertThrowsError(try ToriiOfflineDeviceProof(
            platform: "IOS",
            attestationKeyId: "attestation-key",
            challengeHashHex: Self.hexLowercased(Self.challengeBytes()),
            assertionBase64: Data("assertion".utf8).base64EncodedString(),
            counter: nil
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("platform"))
        }

        XCTAssertThrowsError(try ToriiOfflineDeviceBinding(
            platform: "Android",
            attestationKeyId: "attestation-key",
            deviceId: "android-device",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: ""
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("platform"))
        }

        XCTAssertThrowsError(try ToriiOfflineDeviceProof(
            platform: "Android",
            attestationKeyId: "attestation-key",
            challengeHashHex: Self.hexLowercased(Self.challengeBytes()),
            assertionBase64: Data("assertion".utf8).base64EncodedString(),
            counter: nil
        )) { error in
            XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("platform"))
        }
    }

    func testIosVerifierRejectsWrongProofPlatformBeforeDispatch() throws {
        let binding = try ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "attestation-key",
            deviceId: "ios-device",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: "not-used-for-platform-validation"
        )
        let proof = try ToriiOfflineDeviceProof(
            platform: "android",
            attestationKeyId: "attestation-key",
            challengeHashHex: Self.hexLowercased(Self.challengeBytes()),
            assertionBase64: Data("assertion".utf8).base64EncodedString(),
            counter: nil
        )

        XCTAssertThrowsError(
            try IosOfflineProofVerifier().verifyDeviceProof(binding: binding, proof: proof)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Unsupported offline device proof platform."
            )
        }
    }

    func testAndroidVerifierRejectsWrongBindingPlatformBeforeDispatch() throws {
        let binding = try ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "attestation-key",
            deviceId: "ios-device",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: "not-used-for-platform-validation"
        )

        XCTAssertThrowsError(
            try AndroidOfflineProofVerifier().verifyDeviceBinding(binding)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Unsupported offline device binding platform."
            )
        }
    }

    func testIosVerifierRejectsNonExactMetadataBeforeDispatch() {
        func binding(
            teamId: String = "TEAMID1234",
            bundleId: String = "jp.co.soramitsu.iroha.offline",
            environment: String = "production"
        ) throws -> ToriiOfflineDeviceBinding {
            try ToriiOfflineDeviceBinding(
                platform: "ios",
                attestationKeyId: "attestation-key",
                deviceId: "ios-device",
                offlinePublicKey: "offline-public-key",
                attestationReportBase64: "not-used-for-metadata-validation",
                iosTeamId: teamId,
                iosBundleId: bundleId,
                iosEnvironment: environment
            )
        }

        for (invalidBinding, expectedField) in [
            ({ try binding(teamId: " TEAMID1234") }, "ios_team_id"),
            ({ try binding(bundleId: "jp.co.soramitsu.iroha.offline\n") }, "ios_bundle_id"),
            ({ try binding(environment: "Production") }, "ios_environment"),
            ({ try binding(environment: " production") }, "ios_environment"),
        ] {
            XCTAssertThrowsError(try invalidBinding()) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField(expectedField))
            }
        }
    }

    func testIosVerifierAcceptsIosBindingWithIosProofPlatformBeforeAssertionParsing() throws {
        let keyId = Data(repeating: 0x02, count: 32).base64EncodedString()
        let binding = try ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: keyId,
            deviceId: "ios-device",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: "",
            iosTeamId: "TEAMID1234",
            iosBundleId: "jp.co.soramitsu.iroha.offline",
            iosEnvironment: "production"
        )
        let proof = try ToriiOfflineDeviceProof(
            platform: "ios",
            attestationKeyId: keyId,
            challengeHashHex: Self.hexLowercased(Self.challengeBytes()),
            assertionBase64: Data("assertion".utf8).base64EncodedString(),
            counter: 1
        )

        XCTAssertThrowsError(
            try IosOfflineProofVerifier().verifyDeviceProof(binding: binding, proof: proof)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Offline device proof assertion is invalid."
            )
        }
    }

    func testAndroidVerifierRejectsIncompleteBinding() {
        for (makeBinding, expectedField) in [
            ({
                try ToriiOfflineDeviceBinding(
                    platform: "android",
                    attestationKeyId: "",
                    deviceId: "android-device",
                    offlinePublicKey: "offline-public-key",
                    attestationReportBase64: ""
                )
            }, "attestation_key_id"),
            ({
                try ToriiOfflineDeviceBinding(
                    platform: "android",
                    attestationKeyId: "attestation-key",
                    deviceId: "",
                    offlinePublicKey: "offline-public-key",
                    attestationReportBase64: ""
                )
            }, "device_id"),
            ({
                try ToriiOfflineDeviceBinding(
                    platform: "android",
                    attestationKeyId: "attestation-key",
                    deviceId: "android-device",
                    offlinePublicKey: "",
                    attestationReportBase64: ""
                )
            }, "offline_public_key"),
        ] {
            XCTAssertThrowsError(try makeBinding()) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField(expectedField))
            }
        }
    }

    func testAndroidVerifierRejectsRetiredEd25519Proof() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        let publicKey = privateKey.publicKey.rawRepresentation
        let keyId = Self.sha256Hex(publicKey)
        let challenge = Self.challengeBytes()
        let signature = try privateKey.signature(for: challenge)
        let binding = try ToriiOfflineDeviceBinding(
            platform: "android",
            attestationKeyId: keyId,
            deviceId: "android-device",
            offlinePublicKey: publicKey.base64EncodedString(),
            assertionPublicKey: publicKey.base64EncodedString(),
            attestationReportBase64: "not-used-for-proof"
        )
        let proof = try ToriiOfflineDeviceProof(
            platform: "android",
            attestationKeyId: keyId,
            challengeHashHex: Self.hexLowercased(challenge),
            assertionBase64: signature.base64EncodedString()
        )

        XCTAssertThrowsError(
            try AndroidOfflineProofVerifier().verifyDeviceProof(binding: binding, proof: proof)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Offline device binding assertion public key is invalid."
            )
        }
    }

    func testAndroidVerifierRejectsNonCanonicalChallengeHashHex() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        let publicKey = privateKey.publicKey.rawRepresentation
        let keyId = Self.sha256Hex(publicKey)
        let challenge = Self.challengeBytes()
        let signature = try privateKey.signature(for: challenge)
        let canonicalChallenge = Self.hexLowercased(challenge)
        for nonCanonicalChallenge in [
            canonicalChallenge.uppercased(),
            " \(canonicalChallenge)",
            "\(canonicalChallenge) ",
        ] {
            XCTAssertThrowsError(try ToriiOfflineDeviceProof(
                platform: "android",
                attestationKeyId: keyId,
                challengeHashHex: nonCanonicalChallenge,
                assertionBase64: signature.base64EncodedString()
            )) { error in
                XCTAssertEqual(error as? OfflineNotePayloadError, .invalidField("challenge_hash_hex"))
            }
        }
    }

    func testAndroidVerifierRejectsNonCanonicalAttestationKeyId() throws {
        let offlineKey = Curve25519.Signing.PrivateKey()
        let assertionKey = P256.Signing.PrivateKey()
        let assertionPublicKey = assertionKey.publicKey.x963Representation
        let keyId = Self.sha256Hex(assertionPublicKey)
        let challenge = Self.challengeBytes()
        let signature = try assertionKey.signature(for: challenge).derRepresentation
        let challengeHash = Self.hexLowercased(challenge)

        for (bindingKeyId, proofKeyId) in [
            (keyId.uppercased(), keyId.uppercased()),
            (keyId, keyId.uppercased()),
        ] {
            let binding = try ToriiOfflineDeviceBinding(
                platform: "android",
                attestationKeyId: bindingKeyId,
                deviceId: "android-device",
                offlinePublicKey: offlineKey.publicKey.rawRepresentation.base64EncodedString(),
                assertionPublicKey: assertionPublicKey.base64EncodedString(),
                attestationReportBase64: "not-used-for-proof"
            )
            let proof = try ToriiOfflineDeviceProof(
                platform: "android",
                attestationKeyId: proofKeyId,
                challengeHashHex: challengeHash,
                assertionBase64: signature.base64EncodedString()
            )

            XCTAssertThrowsError(
                try AndroidOfflineProofVerifier().verifyDeviceProof(binding: binding, proof: proof)
            ) { error in
                XCTAssertEqual(
                    (error as? OfflineProofVerifierError)?.errorDescription,
                    "Offline device proof does not match the device binding."
                )
            }
        }
    }

    func testAndroidKeyMintProofUsesAssertionPublicKey() throws {
        let offlineKey = Curve25519.Signing.PrivateKey()
        let assertionKey = P256.Signing.PrivateKey()
        let assertionPublicKey = assertionKey.publicKey.x963Representation
        let keyId = Self.sha256Hex(assertionPublicKey)
        let challenge = Self.challengeBytes()
        let signature = try assertionKey.signature(for: challenge).derRepresentation
        let binding = try ToriiOfflineDeviceBinding(
            platform: "android",
            attestationKeyId: keyId,
            deviceId: "android-keymint-device",
            offlinePublicKey: offlineKey.publicKey.rawRepresentation.base64EncodedString(),
            assertionPublicKey: assertionPublicKey.base64EncodedString(),
            attestationReportBase64: "not-used-for-proof"
        )
        let proof = try ToriiOfflineDeviceProof(
            platform: "android",
            attestationKeyId: keyId,
            challengeHashHex: Self.hexLowercased(challenge),
            assertionBase64: signature.base64EncodedString()
        )

        try AndroidOfflineProofVerifier().verifyDeviceProof(binding: binding, proof: proof)
    }

    func testAndroidKeyMintProofRejectsMissingAssertionPublicKey() throws {
        let offlineKey = Curve25519.Signing.PrivateKey()
        let assertionKey = P256.Signing.PrivateKey()
        let assertionPublicKey = assertionKey.publicKey.x963Representation
        let keyId = Self.sha256Hex(assertionPublicKey)
        let challenge = Self.challengeBytes()
        let signature = try assertionKey.signature(for: challenge).derRepresentation
        let binding = try ToriiOfflineDeviceBinding(
            platform: "android",
            attestationKeyId: keyId,
            deviceId: "android-keymint-device",
            offlinePublicKey: offlineKey.publicKey.rawRepresentation.base64EncodedString(),
            assertionPublicKey: nil,
            attestationReportBase64: "not-used-for-proof"
        )
        let proof = try ToriiOfflineDeviceProof(
            platform: "android",
            attestationKeyId: keyId,
            challengeHashHex: Self.hexLowercased(challenge),
            assertionBase64: signature.base64EncodedString()
        )

        XCTAssertThrowsError(
            try AndroidOfflineProofVerifier().verifyDeviceProof(binding: binding, proof: proof)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Offline device binding assertion public key is required for Android KeyMint."
            )
        }
    }

    func testAndroidKeyMintProofRejectsWrongAssertionPublicKey() throws {
        let offlineKey = Curve25519.Signing.PrivateKey()
        let signingKey = P256.Signing.PrivateKey()
        let verifierKey = P256.Signing.PrivateKey()
        let verifierPublicKey = verifierKey.publicKey.x963Representation
        let keyId = Self.sha256Hex(verifierPublicKey)
        let challenge = Self.challengeBytes()
        let signature = try signingKey.signature(for: challenge).derRepresentation
        let binding = try ToriiOfflineDeviceBinding(
            platform: "android",
            attestationKeyId: keyId,
            deviceId: "android-keymint-device",
            offlinePublicKey: offlineKey.publicKey.rawRepresentation.base64EncodedString(),
            assertionPublicKey: verifierPublicKey.base64EncodedString(),
            attestationReportBase64: "not-used-for-proof"
        )
        let proof = try ToriiOfflineDeviceProof(
            platform: "android",
            attestationKeyId: keyId,
            challengeHashHex: Self.hexLowercased(challenge),
            assertionBase64: signature.base64EncodedString()
        )

        XCTAssertThrowsError(
            try AndroidOfflineProofVerifier().verifyDeviceProof(binding: binding, proof: proof)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Offline device proof assertion is invalid."
            )
        }
    }

    private static func challengeBytes() -> Data {
        Data((0 ..< 32).map { UInt8($0) })
    }

    private static func sha256Hex(_ data: Data) -> String {
        hexLowercased(Data(SHA256.hash(data: data)))
    }

    private static func hexLowercased(_ data: Data) -> String {
        data.map { String(format: "%02x", $0) }.joined()
    }
}

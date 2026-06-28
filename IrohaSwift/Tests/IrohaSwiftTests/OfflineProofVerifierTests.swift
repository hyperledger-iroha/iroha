import CryptoKit
import Foundation
import IrohaSwift
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

    func testDefaultIosRootCertificateIsBase64Encoded() {
        let encoded = IosOfflineProofVerifier.defaultTrustedRootCertificateBase64

        XCTAssertFalse(encoded.isEmpty)
        XCTAssertNotNil(Data(base64Encoded: encoded))
    }

    func testCounterpartyVerifierRequiresIosChallengeHash() {
        let binding = ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "",
            deviceId: "ios-device",
            offlinePublicKey: "",
            attestationReportBase64: ""
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

    func testCounterpartyVerifierDispatchesCanonicalAbi7PlatformAliases() {
        for platform in ["ios-appattest", "ios-app-attest"] {
            let binding = ToriiOfflineDeviceBinding(
                platform: platform,
                attestationKeyId: "",
                deviceId: "ios-device",
                offlinePublicKey: "",
                attestationReportBase64: ""
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

        let androidBinding = ToriiOfflineDeviceBinding(
            platform: "android-keymint",
            attestationKeyId: "",
            deviceId: "android-device",
            offlinePublicKey: "",
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
        let binding = ToriiOfflineDeviceBinding(
            platform: " ios",
            attestationKeyId: "",
            deviceId: "ios-device",
            offlinePublicKey: "",
            attestationReportBase64: ""
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
                "Unsupported offline device binding platform."
            )
        }

        let proof = ToriiOfflineDeviceProof(
            platform: "ios",
            attestationKeyId: "",
            challengeHashHex: "",
            assertionBase64: "",
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

    func testCounterpartyVerifierRejectsCaseChangedPlatformBeforeDispatch() {
        let binding = ToriiOfflineDeviceBinding(
            platform: "IOS",
            attestationKeyId: "",
            deviceId: "ios-device",
            offlinePublicKey: "",
            attestationReportBase64: ""
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
                "Unsupported offline device binding platform."
            )
        }

        let iosProofBinding = ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "",
            deviceId: "ios-device",
            offlinePublicKey: "",
            attestationReportBase64: "not-empty"
        )
        let iosProof = ToriiOfflineDeviceProof(
            platform: "IOS",
            attestationKeyId: "",
            challengeHashHex: "",
            assertionBase64: "",
            counter: nil
        )
        XCTAssertThrowsError(
            try IosOfflineProofVerifier().verifyDeviceProof(binding: iosProofBinding, proof: iosProof)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Unsupported offline device proof platform."
            )
        }

        let androidBinding = ToriiOfflineDeviceBinding(
            platform: "Android",
            attestationKeyId: "",
            deviceId: "",
            offlinePublicKey: "",
            attestationReportBase64: ""
        )
        XCTAssertThrowsError(
            try AndroidOfflineProofVerifier().verifyDeviceBinding(androidBinding)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Unsupported offline device binding platform."
            )
        }

        let androidProof = ToriiOfflineDeviceProof(
            platform: "Android",
            attestationKeyId: "",
            challengeHashHex: "",
            assertionBase64: "",
            counter: nil
        )
        let exactAndroidBinding = ToriiOfflineDeviceBinding(
            platform: "android",
            attestationKeyId: "",
            deviceId: "",
            offlinePublicKey: "",
            attestationReportBase64: ""
        )
        XCTAssertThrowsError(
            try AndroidOfflineProofVerifier().verifyDeviceProof(binding: exactAndroidBinding, proof: androidProof)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Unsupported offline device proof platform."
            )
        }
    }

    func testAndroidVerifierRejectsIncompleteBinding() {
        let binding = ToriiOfflineDeviceBinding(
            platform: "android",
            attestationKeyId: "",
            deviceId: "",
            offlinePublicKey: "",
            attestationReportBase64: ""
        )

        XCTAssertThrowsError(
            try AndroidOfflineProofVerifier().verifyDeviceBinding(binding)
        ) { error in
            XCTAssertEqual(
                (error as? OfflineProofVerifierError)?.errorDescription,
                "Offline device binding is incomplete."
            )
        }
    }

    func testAndroidVerifierAcceptsLegacyEd25519Proof() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        let publicKey = privateKey.publicKey.rawRepresentation
        let keyId = Self.sha256Hex(publicKey)
        let challenge = Self.challengeBytes()
        let signature = try privateKey.signature(for: challenge)
        let binding = ToriiOfflineDeviceBinding(
            platform: "android",
            attestationKeyId: keyId,
            deviceId: "android-device",
            offlinePublicKey: publicKey.base64EncodedString(),
            attestationReportBase64: "not-used-for-proof"
        )
        let proof = ToriiOfflineDeviceProof(
            platform: "android",
            attestationKeyId: keyId,
            challengeHashHex: Self.hexLowercased(challenge),
            assertionBase64: signature.base64EncodedString()
        )

        try AndroidOfflineProofVerifier().verifyDeviceProof(binding: binding, proof: proof)
    }

    func testAndroidKeyMintProofUsesAssertionPublicKey() throws {
        let offlineKey = Curve25519.Signing.PrivateKey()
        let assertionKey = P256.Signing.PrivateKey()
        let assertionPublicKey = assertionKey.publicKey.x963Representation
        let keyId = Self.sha256Hex(assertionPublicKey)
        let challenge = Self.challengeBytes()
        let signature = try assertionKey.signature(for: challenge).derRepresentation
        let binding = ToriiOfflineDeviceBinding(
            platform: "android-keymint",
            attestationKeyId: keyId,
            deviceId: "android-keymint-device",
            offlinePublicKey: offlineKey.publicKey.rawRepresentation.base64EncodedString(),
            assertionPublicKey: assertionPublicKey.base64EncodedString(),
            attestationReportBase64: "not-used-for-proof"
        )
        let proof = ToriiOfflineDeviceProof(
            platform: "android-keymint",
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
        let binding = ToriiOfflineDeviceBinding(
            platform: "android-keymint",
            attestationKeyId: keyId,
            deviceId: "android-keymint-device",
            offlinePublicKey: offlineKey.publicKey.rawRepresentation.base64EncodedString(),
            assertionPublicKey: nil,
            attestationReportBase64: "not-used-for-proof"
        )
        let proof = ToriiOfflineDeviceProof(
            platform: "android-keymint",
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
        let binding = ToriiOfflineDeviceBinding(
            platform: "android-keymint",
            attestationKeyId: keyId,
            deviceId: "android-keymint-device",
            offlinePublicKey: offlineKey.publicKey.rawRepresentation.base64EncodedString(),
            assertionPublicKey: verifierPublicKey.base64EncodedString(),
            attestationReportBase64: "not-used-for-proof"
        )
        let proof = ToriiOfflineDeviceProof(
            platform: "android-keymint",
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

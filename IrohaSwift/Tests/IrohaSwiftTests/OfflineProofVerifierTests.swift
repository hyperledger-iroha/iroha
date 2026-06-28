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

    func testCounterpartyVerifierRejectsRemovedPlatformAliasesBeforeDispatch() {
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
                    "Unsupported offline device binding platform."
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
                "Unsupported offline device binding platform."
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
}

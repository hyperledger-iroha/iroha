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

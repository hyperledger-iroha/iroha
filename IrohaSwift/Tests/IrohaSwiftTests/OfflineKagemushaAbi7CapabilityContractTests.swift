import XCTest
@testable import IrohaSwift

final class OfflineKagemushaAbi7CapabilityContractTests: XCTestCase {
    func testCanonicalCapabilityIsSupported() throws {
        XCTAssertEqual(OfflineKagemushaAbi7CapabilityContract.mode, "recursive_compact_v1")
        XCTAssertEqual(OfflineKagemushaAbi7CapabilityContract.nativeBridgeAbiVersion, 7)
        XCTAssertEqual(
            OfflineKagemushaAbi7CapabilityContract.circuitId,
            KagemushaRecursiveCompactPaymentTokenProver.recursiveCompactCircuitIdV1
        )

        XCTAssertTrue(
            OfflineKagemushaAbi7CapabilityContract.isSupported(
                offlinePayments: true,
                lifecycleEnabled: true,
                mode: OfflineKagemushaAbi7CapabilityContract.mode,
                nativeBridgeAbiVersion: OfflineKagemushaAbi7CapabilityContract.nativeBridgeAbiVersion,
                circuitId: OfflineKagemushaAbi7CapabilityContract.circuitId,
                artifactSetId: "production-abi7-2026-06",
                artifactsAvailable: true
            )
        )

        XCTAssertNoThrow(
            try OfflineKagemushaAbi7CapabilityContract.validateArtifactMetadata(
                mode: OfflineKagemushaAbi7CapabilityContract.mode,
                nativeBridgeAbiVersion: OfflineKagemushaAbi7CapabilityContract.nativeBridgeAbiVersion,
                circuitId: OfflineKagemushaAbi7CapabilityContract.circuitId,
                artifactSetId: "production-abi7-2026-06"
            )
        )
    }

    func testUnsupportedCapabilityVariantsFailClosedWithSpecificReasons() {
        let cases: [(String, () throws -> Void, OfflineKagemushaAbi7CapabilityError)] = [
            (
                "offline payments disabled",
                {
                    try Self.validate(offlinePayments: false)
                },
                .offlinePaymentsDisabled
            ),
            (
                "lifecycle disabled",
                {
                    try Self.validate(lifecycleEnabled: false)
                },
                .lifecycleDisabled
            ),
            (
                "missing mode",
                {
                    try Self.validate(mode: nil)
                },
                .unsupportedMode(expected: OfflineKagemushaAbi7CapabilityContract.mode, actual: nil)
            ),
            (
                "mode with adversarial whitespace",
                {
                    try Self.validate(mode: " \(OfflineKagemushaAbi7CapabilityContract.mode) ")
                },
                .unsupportedMode(
                    expected: OfflineKagemushaAbi7CapabilityContract.mode,
                    actual: " \(OfflineKagemushaAbi7CapabilityContract.mode) "
                )
            ),
            (
                "missing native bridge ABI",
                {
                    try Self.validate(nativeBridgeAbiVersion: nil)
                },
                .unsupportedNativeBridgeAbi(
                    expected: OfflineKagemushaAbi7CapabilityContract.nativeBridgeAbiVersion,
                    actual: nil
                )
            ),
            (
                "older native bridge ABI",
                {
                    try Self.validate(nativeBridgeAbiVersion: 6)
                },
                .unsupportedNativeBridgeAbi(
                    expected: OfflineKagemushaAbi7CapabilityContract.nativeBridgeAbiVersion,
                    actual: 6
                )
            ),
            (
                "unexpected newer native bridge ABI",
                {
                    try Self.validate(nativeBridgeAbiVersion: 8)
                },
                .unsupportedNativeBridgeAbi(
                    expected: OfflineKagemushaAbi7CapabilityContract.nativeBridgeAbiVersion,
                    actual: 8
                )
            ),
            (
                "wrong circuit",
                {
                    try Self.validate(circuitId: "kagemusha-recursive-spend-v1")
                },
                .unsupportedCircuitId(
                    expected: OfflineKagemushaAbi7CapabilityContract.circuitId,
                    actual: "kagemusha-recursive-spend-v1"
                )
            ),
            (
                "blank artifact set",
                {
                    try Self.validate(artifactSetId: " \n\t ")
                },
                .missingArtifactSetId
            ),
            (
                "artifacts unavailable",
                {
                    try Self.validate(artifactsAvailable: false)
                },
                .artifactsUnavailable
            )
        ]

        for (label, operation, expectedError) in cases {
            XCTAssertThrowsError(try operation(), label) { error in
                XCTAssertEqual(error as? OfflineKagemushaAbi7CapabilityError, expectedError, label)
            }
        }
    }

    func testArtifactMetadataRejectsUnexpectedContractValues() {
        XCTAssertThrowsError(
            try OfflineKagemushaAbi7CapabilityContract.validateArtifactMetadata(
                mode: "recursive_spend_v1",
                nativeBridgeAbiVersion: OfflineKagemushaAbi7CapabilityContract.nativeBridgeAbiVersion,
                circuitId: OfflineKagemushaAbi7CapabilityContract.circuitId,
                artifactSetId: "production-abi7-2026-06"
            )
        ) { error in
            XCTAssertEqual(
                error as? OfflineKagemushaAbi7CapabilityError,
                .unsupportedMode(
                    expected: OfflineKagemushaAbi7CapabilityContract.mode,
                    actual: "recursive_spend_v1"
                )
            )
        }

        XCTAssertThrowsError(
            try OfflineKagemushaAbi7CapabilityContract.validateArtifactMetadata(
                mode: OfflineKagemushaAbi7CapabilityContract.mode,
                nativeBridgeAbiVersion: OfflineKagemushaAbi7CapabilityContract.nativeBridgeAbiVersion,
                circuitId: nil,
                artifactSetId: "production-abi7-2026-06"
            )
        ) { error in
            XCTAssertEqual(
                error as? OfflineKagemushaAbi7CapabilityError,
                .unsupportedCircuitId(expected: OfflineKagemushaAbi7CapabilityContract.circuitId, actual: nil)
            )
        }
    }

    private static func validate(
        offlinePayments: Bool = true,
        lifecycleEnabled: Bool = true,
        mode: String? = OfflineKagemushaAbi7CapabilityContract.mode,
        nativeBridgeAbiVersion: UInt32? = OfflineKagemushaAbi7CapabilityContract.nativeBridgeAbiVersion,
        circuitId: String? = OfflineKagemushaAbi7CapabilityContract.circuitId,
        artifactSetId: String? = "production-abi7-2026-06",
        artifactsAvailable: Bool = true
    ) throws {
        try OfflineKagemushaAbi7CapabilityContract.validate(
            offlinePayments: offlinePayments,
            lifecycleEnabled: lifecycleEnabled,
            mode: mode,
            nativeBridgeAbiVersion: nativeBridgeAbiVersion,
            circuitId: circuitId,
            artifactSetId: artifactSetId,
            artifactsAvailable: artifactsAvailable
        )
    }
}

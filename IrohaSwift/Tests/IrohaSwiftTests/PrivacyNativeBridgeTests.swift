import Foundation
import XCTest
@testable import IrohaSwift

final class PrivacyNativeBridgeTests: XCTestCase {
    func testExportsStableFfiVersion() {
        XCTAssertEqual(PrivacyNativeBridge.ffiVersionV1, 1)
        XCTAssertEqual(PrivacyNativeBridge.requiredBridgeAbiVersion, 7)
        XCTAssertEqual(PrivacyNativeBridge.privacyNativeArchiveMaxBytes, 64 * 1024 * 1024)
        XCTAssertEqual(PrivacyNativeBridge.ffiStatusError, 1)
        XCTAssertEqual(PrivacyNativeBridge.ffiErrorNullPointer, 1)
        XCTAssertEqual(PrivacyNativeBridge.ffiErrorMalformedNorito, 2)
        XCTAssertEqual(PrivacyNativeBridge.ffiErrorUnsupportedAlgorithm, 3)
        XCTAssertEqual(PrivacyNativeBridge.ffiErrorProductionDisabled, 4)
        XCTAssertEqual(PrivacyNativeBridge.ffiErrorInvalidRequest, 5)
    }

    func testPrivacyNativeAvailabilityProbeArchiveIsStableAndNonempty() {
        #if canImport(Darwin)
        let probeArchive = NoritoNativeBridge.privacyNativeAvailabilityProbeArchive

        XCTAssertEqual(probeArchive, privacyNoritoFrame(0x52))
        XCTAssertTrue(NoritoNativeBridge.isValidPrivacyNoritoArchive(probeArchive))
        XCTAssertNotEqual(
            probeArchive,
            Data("iroha-privacy-native-availability-probe-v1".utf8)
        )
        #endif
    }

    func testPrivacyNativeProbeResultRequiresSuccessfulNonemptyArchive() {
        #if canImport(Darwin)
        withPrivacyOutputPointer(privacyNoritoFrame(0x50)) { pointer, length in
            XCTAssertFalse(
                NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                    status: 0,
                    outPtr: pointer,
                    outLen: length,
                    expectedSchemaByte: 0x50
                )
            )
        }
        withPrivacyOutputPointer(privacyNoritoFrameWithPayload(0x51)) { pointer, length in
            XCTAssertFalse(
                NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                    status: 0,
                    outPtr: pointer,
                    outLen: length,
                    expectedSchemaByte: 0x50
                )
            )
        }
        withPrivacyOutputPointer(privacyNoritoFrameWithPadding(0x50, paddingLength: 64)) { pointer, length in
            XCTAssertTrue(
                NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                    status: 0,
                    outPtr: pointer,
                    outLen: length,
                    expectedSchemaByte: 0x50
                )
            )
        }
        withPrivacyOutputPointer(privacyNoritoFrameWithPadding(0x52, paddingLength: 64)) { pointer, length in
            XCTAssertTrue(
                NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                    status: 0,
                    outPtr: pointer,
                    outLen: length,
                    expectedSchemaByte: 0x52
                )
            )
        }
        withPrivacyOutputPointer(privacyNoritoFrameWithPayload(0x50)) { pointer, length in
            XCTAssertTrue(
                NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                    status: 0,
                    outPtr: pointer,
                    outLen: length,
                    expectedSchemaByte: 0x50
                )
            )
        }
        withPrivacyOutputPointer(privacyNoritoFrameWithFlags(0x42, flags: 0x26)) { pointer, length in
            XCTAssertTrue(
                NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                    status: 0,
                    outPtr: pointer,
                    outLen: length,
                    expectedSchemaByte: 0x42
                )
            )
        }
        withPrivacyOutputPointer(privacyNoritoFrameWithPayload(0x42)) { pointer, length in
            XCTAssertFalse(
                NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                    status: 0,
                    outPtr: pointer,
                    outLen: length,
                    expectedSchemaByte: 0x50
                )
            )
        }
        withPrivacyOutputPointer(privacyNoritoFrameWithPayload(0x56)) { pointer, length in
            XCTAssertFalse(
                NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                    status: 0,
                    outPtr: pointer,
                    outLen: length,
                    expectedSchemaByte: 0x42
                )
            )
        }
        withPrivacyOutputPointer(Data([0x01])) { pointer, length in
            XCTAssertFalse(
                NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                    status: 0,
                    outPtr: pointer,
                    outLen: length,
                    expectedSchemaByte: 0x50
                )
            )
        }
        for invalidArchive in [
            invalidPrivacyNoritoFrame(offset: 0, value: 0x58),
            invalidPrivacyNoritoFrame(offset: 4, value: 1),
            invalidPrivacyNoritoFrame(offset: 5, value: 1),
            invalidPrivacyNoritoFrame(offset: 22, value: 1),
            invalidPrivacyNoritoDeclaredPayloadLength(),
            invalidPrivacyNoritoOversizedPayloadLength(),
            invalidPrivacyNoritoFrame(offset: 39, value: 0x40),
            invalidPrivacyNoritoFrame(offset: 39, value: 0x20),
            invalidPrivacyNoritoWithNonzeroPadding(),
            invalidPrivacyNoritoWithExcessivePadding(),
            invalidPrivacyNoritoFrame(offset: 31, value: 1),
            invalidPrivacyNoritoPayloadTamper()
        ] {
            withPrivacyOutputPointer(invalidArchive) { pointer, length in
                XCTAssertFalse(
                    NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                        status: 0,
                        outPtr: pointer,
                        outLen: length,
                        expectedSchemaByte: 0x50
                    )
                )
            }
        }
        XCTAssertFalse(
            NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                status: -311,
                outPtr: nil,
                outLen: 0,
                expectedSchemaByte: 0x50
            )
        )
        XCTAssertFalse(
            NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                status: 0,
                outPtr: nil,
                outLen: 1,
                expectedSchemaByte: 0x50
            )
        )
        XCTAssertFalse(
            NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                status: 0,
                outPtr: nil,
                outLen: 0,
                expectedSchemaByte: 0x50
            )
        )
        XCTAssertFalse(
            NoritoNativeBridge.isValidPrivacyNativeProbeResult(
                status: 0,
                outPtr: nil,
                outLen: CUnsignedLong(PrivacyNativeBridge.privacyNativeArchiveMaxBytes + 1),
                expectedSchemaByte: 0x50
            )
        )
        #endif
    }

    func testPrivacyCapabilitiesFailClosedWithMalformedNativeArchive() {
        let capabilities = PrivacyNativeBridge.privacyCapabilities(
            fromArchive: privacyNoritoFrameWithPayload(0x42),
            bridgeAvailable: true
        )

        XCTAssertTrue(capabilities.swiftSdkAvailable)
        XCTAssertTrue(capabilities.bridgeAvailable)
        assertFailClosedProductionGate(capabilities)
    }

    func testPrivacyCapabilitiesRemainFailClosedWithBridgeUnavailable() {
        let capabilities = PrivacyNativeBridge.privacyCapabilities(bridgeAvailable: false)

        XCTAssertTrue(capabilities.swiftSdkAvailable)
        XCTAssertFalse(capabilities.bridgeAvailable)
        assertFailClosedProductionGate(capabilities)
    }

    func testPrivacyCapabilitiesReturnValueCopies() {
        let capabilities = PrivacyNativeBridge.privacyCapabilities(
            fromArchive: privacyNoritoFrameWithPayload(0x42),
            bridgeAvailable: true
        )
        var requiredGates = capabilities.productionGate.requiredGates
        requiredGates.append("tampered")
        var missing = capabilities.productionGate.missing
        missing.append("tampered")
        var auditReferences = capabilities.productionGate.auditReferences
        auditReferences.append("https://audit.example/forged-signoff")

        let fresh = PrivacyNativeBridge.privacyCapabilities(
            fromArchive: privacyNoritoFrameWithPayload(0x42),
            bridgeAvailable: true
        )

        XCTAssertFalse(fresh.productionGate.missing.contains("tampered"))
        XCTAssertFalse(fresh.productionGate.requiredGates.contains("tampered"))
        XCTAssertFalse(
            fresh.productionGate.auditReferences.contains(
                "https://audit.example/forged-signoff"
            )
        )
        XCTAssertEqual(fresh.productionGate.requiredGates, PrivacyProductionGate.requiredGateKeys)
        XCTAssertEqual(fresh.productionGate.missing, PrivacyProductionGate.missingReasons)
        XCTAssertEqual(fresh.productionGate.auditReferences, [])
    }

    func testProductionReadyCapabilitiesRequireExactNativeGateEvidence() {
        let capabilities = PrivacyNativeBridge.privacyCapabilities(
            fromArchive: nativeCapabilitiesArchive(
                nativeCapability("confidential-transfer-v2", ready: true),
                nativeCapability("unshield", ready: true)
            ),
            bridgeAvailable: true
        )

        XCTAssertTrue(capabilities.productionReady)
        XCTAssertTrue(capabilities.productionGate.ready)
        XCTAssertTrue(capabilities.productionGate.realProving)
        XCTAssertTrue(capabilities.productionGate.externalAudit)
        XCTAssertEqual(capabilities.productionGate.missing, [])
        XCTAssertEqual(capabilities.productionGate.auditReferences.count, 19)
    }

    func testForgedProductionReadyCapabilityRowsFailClosed() {
        let cases: [(String, (inout NativeCapabilityFixture) -> Void)] = [
            ("empty required gates", { row in
                row.productionGate.requiredGates = []
            }),
            ("missing gate status", { row in
                row.productionGate.gates.removeLast()
            }),
            ("unpassed gate status", { row in
                row.productionGate.gates[0].passed = false
            }),
            ("nonempty missing reasons", { row in
                row.productionGate.missing = ["external audit omitted"]
            }),
            ("missing audit references", { row in
                row.productionGate.auditReferences = []
            }),
            ("single audit reference", { row in
                row.productionGate.auditReferences = ["chain_id:boi-privacy-4peer-chain"]
            }),
            ("duplicate audit reference", { row in
                row.productionGate.auditReferences = Self.productionAuditReferences()
                row.productionGate.auditReferences[18] = row.productionGate.auditReferences[17]
            }),
            ("reused audit hash", { row in
                row.productionGate.auditReferences = Self.productionAuditReferences()
                row.productionGate.auditReferences[14] =
                    "localnet_lifecycle_recursive_init_verify_hash:\(Self.productionHash(10))"
            }),
            ("bad audit hash", { row in
                row.productionGate.auditReferences = Self.productionAuditReferences()
                row.productionGate.auditReferences[2] =
                    "review_artifact_hash:sha256:not-a-hex-digest"
            }),
            ("uppercase audit signature", { row in
                row.productionGate.auditReferences = Self.productionAuditReferences()
                row.productionGate.auditReferences[3] =
                    "review_artifact_signature:ed25519:\(String(repeating: "B", count: 128))"
            }),
            ("mock localnet marker", { row in
                row.productionGate.auditReferences = Self.productionAuditReferences()
                row.productionGate.auditReferences[6] =
                    "localnet_run_id:mock-privacy-4peer-localnet-2026-06-13"
            }),
            ("planned entrypoint", { row in
                row.plannedEntrypoints = ["buildFuturePrivacyProofV2"]
            }),
            ("production ready mismatch", { row in
                row.productionReady = false
            })
        ]

        for (caseName, mutate) in cases {
            var row = nativeCapability("confidential-transfer-v2", ready: true)
            mutate(&row)
            let capabilities = PrivacyNativeBridge.privacyCapabilities(
                fromArchive: nativeCapabilitiesArchive(
                    row,
                    nativeCapability("unshield", ready: true)
                ),
                bridgeAvailable: true
            )

            assertFailClosedProductionGate(capabilities)
            XCTAssertFalse(capabilities.productionReady, caseName)
        }
    }

    func testRejectsEmptyRequestArchivesBeforeBridgeCall() {
        let helpers: [(String, (Data) throws -> Data)] = [
            ("build", PrivacyNativeBridge.buildProofV1),
            ("confidential transfer", PrivacyNativeBridge.buildConfidentialTransferProofV2),
            ("confidential unshield", PrivacyNativeBridge.buildConfidentialUnshieldProofV3),
            ("ZK-ACE authorization", PrivacyNativeBridge.buildZkAceAuthorizationProofV1),
            ("Jindo lattice proof", PrivacyNativeBridge.buildJindoLatticeProofV0),
            (
                "SIS-with-hints anonymous credential proof",
                PrivacyNativeBridge.buildSisHintsAnonymousCredentialProofV0
            ),
            (
                "silent-threshold credential showing proof",
                PrivacyNativeBridge.buildSilentThresholdCredentialShowingProofV0
            ),
            (
                "Vega credential predicate proof",
                PrivacyNativeBridge.buildVegaCredentialPredicateProofV0
            ),
            (
                "ZK-AMS admission batch proof",
                PrivacyNativeBridge.buildZkAmsAdmissionBatchProofV0
            ),
            ("zkAt policy proof", PrivacyNativeBridge.buildZkAtPolicyProofV1),
            (
                "Jindo polynomial commitment verify",
                PrivacyNativeBridge.verifyJindoPolynomialCommitmentV0
            ),
            (
                "SIS-with-hints anonymous credential verify",
                PrivacyNativeBridge.verifySisHintsAnonymousCredentialProofV0
            ),
            (
                "silent-threshold credential showing verify",
                PrivacyNativeBridge.verifySilentThresholdCredentialShowingProofV0
            ),
            (
                "Vega credential predicate verify",
                PrivacyNativeBridge.verifyVegaCredentialPredicateProofV0
            ),
            (
                "ZK-AMS admission batch verify",
                PrivacyNativeBridge.verifyZkAmsAdmissionBatchProofV0
            ),
            ("zkAt policy verify", PrivacyNativeBridge.verifyZkAtPolicyProofV1),
            ("verify", PrivacyNativeBridge.verifyProofV1)
        ]

        for (label, helper) in helpers {
            XCTAssertThrowsError(try helper(Data()), "helper \(label) should reject empty archives") { error in
                XCTAssertEqual(error as? PrivacyNativeBridgeError, .emptyRequestArchive)
            }
            XCTAssertThrowsError(
                try helper(privacyNoritoFrame(0x52)),
                "helper \(label) should reject zero-payload request archives"
            ) { error in
                XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
            }
        }
    }

    func testRejectsOversizedRequestArchivesBeforeBridgeCall() {
        let oversized = Data(
            repeating: 0x7F,
            count: PrivacyNativeBridge.privacyNativeArchiveMaxBytes + 1
        )

        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                requestArchive: oversized,
                bridgeAvailable: true,
                expectedSchemaByte: 0x42
            ) { _ in
                XCTFail("oversized request must not reach native dispatch")
                return Data([0x01])
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .oversizedRequestArchive)
        }
    }

    func testCapabilitiesNilNativeOutputIsRejected() {
        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                bridgeAvailable: true,
                expectedSchemaByte: 0x50
            ) {
                nil
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }
    }

    func testCapabilitiesEmptyNativeOutputIsRejected() {
        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                bridgeAvailable: true,
                expectedSchemaByte: 0x50
            ) {
                Data()
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }
    }

    func testCapabilitiesNativeErrorIsRejected() {
        enum LocalError: Error {
            case rejected
        }

        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                bridgeAvailable: true,
                expectedSchemaByte: 0x50
            ) {
                throw LocalError.rejected
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }
    }

    func testNativeErrorsAreSanitizedBeforeExposingRequestBytes() {
        let witness = "swift-sdk-private-witness-never-echo-a3de"
        let leakingError = NSError(
            domain: "native panic included \(witness)",
            code: 7,
            userInfo: [NSLocalizedDescriptionKey: "native panic included \(witness)"]
        )
        let requestArchive = privacyNoritoFrameWithPayload(0x52)

        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                bridgeAvailable: true,
                expectedSchemaByte: 0x50
            ) {
                throw leakingError
            }
        ) { error in
            assertSanitizedNativeError(error, witness: witness)
        }

        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                requestArchive: requestArchive,
                bridgeAvailable: true,
                expectedSchemaByte: 0x42
            ) { request in
                XCTAssertEqual(request, requestArchive)
                throw leakingError
            }
        ) { error in
            assertSanitizedNativeError(error, witness: witness)
        }
    }

    func testProofNativeOutputGuards() {
        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                requestArchive: privacyNoritoFrameWithPayload(0x52),
                bridgeAvailable: false,
                expectedSchemaByte: 0x42
            ) { _ in
                Data([0x02])
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .bridgeUnavailable)
        }

        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                requestArchive: privacyNoritoFrameWithPayload(0x52),
                bridgeAvailable: true,
                expectedSchemaByte: 0x42
            ) { _ in
                nil
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }

        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                requestArchive: privacyNoritoFrameWithPayload(0x52),
                bridgeAvailable: true,
                expectedSchemaByte: 0x42
            ) { _ in
                Data()
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }

        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                requestArchive: privacyNoritoFrameWithPayload(0x52),
                bridgeAvailable: true,
                expectedSchemaByte: 0x42
            ) { _ in
                privacyNoritoFrame(0x42)
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }
    }

    func testReturnsNonemptyNativeOutput() throws {
        let nativeOutput = privacyNoritoFrameWithFlags(0x42, flags: 0x26)
        let requestArchive = privacyNoritoFrameWithFlags(0x52, flags: 0x26)
        let archive = try PrivacyNativeBridge.call(
            requestArchive: requestArchive,
            bridgeAvailable: true,
            expectedSchemaByte: 0x42
        ) { request in
            XCTAssertEqual(request, requestArchive)
            return nativeOutput
        }
        XCTAssertEqual(archive, nativeOutput)
    }

    func testRejectsInvalidNoritoRequestArchivesBeforeBridgeCall() {
        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                requestArchive: privacyNoritoFrame(0x52),
                bridgeAvailable: true,
                expectedSchemaByte: 0x42
            ) { _ in
                XCTFail("empty-payload request must not reach native dispatch")
                return privacyNoritoFrameWithPayload(0x42)
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }

        let malformedArchives = [
            Data([0x01]),
            invalidPrivacyNoritoFrame(offset: 0, value: 0x58),
            invalidPrivacyNoritoFrame(offset: 4, value: 1),
            invalidPrivacyNoritoFrame(offset: 5, value: 1),
            invalidPrivacyNoritoFrame(offset: 22, value: 1),
            invalidPrivacyNoritoDeclaredPayloadLength(schemaByte: 0x52),
            invalidPrivacyNoritoOversizedPayloadLength(schemaByte: 0x52),
            invalidPrivacyNoritoFrame(offset: 39, value: 0x40),
            invalidPrivacyNoritoFrame(offset: 39, value: 0x20),
            invalidPrivacyNoritoWithNonzeroPadding(),
            invalidPrivacyNoritoWithExcessivePadding(),
            invalidPrivacyNoritoFrame(offset: 31, value: 1),
            invalidPrivacyNoritoPayloadTamper()
        ]

        for malformedArchive in malformedArchives {
            XCTAssertThrowsError(
                try PrivacyNativeBridge.call(
                    requestArchive: malformedArchive,
                    bridgeAvailable: true,
                    expectedSchemaByte: 0x42
                ) { _ in
                    XCTFail("invalid request must not reach native dispatch")
                    return privacyNoritoFrameWithPayload(0x42)
                }
            ) { error in
                XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
            }
        }
    }

    func testRejectsWrongSchemaRequestArchivesBeforeBridgeCall() {
        let forgedRequests = [
            privacyNoritoFrameWithPayload(0x50),
            privacyNoritoFrameWithPayload(0x42),
            privacyNoritoFrameWithPayload(0x56),
            privacyNoritoFrameWithSchemaOverride(0x52, offset: 6, value: 0x42),
            privacyNoritoFrameWithSchemaOverride(0x52, offset: 21, value: 0x56)
        ]

        for forgedRequest in forgedRequests {
            XCTAssertThrowsError(
                try PrivacyNativeBridge.call(
                    requestArchive: forgedRequest,
                    bridgeAvailable: true,
                    expectedSchemaByte: 0x42
                ) { _ in
                    XCTFail("wrong-schema request must not reach native dispatch")
                    return privacyNoritoFrameWithPayload(0x42)
                }
            ) { error in
                XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
            }
        }
    }

    func testRejectsInvalidNoritoNativeOutput() {
        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                bridgeAvailable: true,
                expectedSchemaByte: 0x50
            ) {
                Data([0x01])
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }

        for invalidArchive in [
            privacyNoritoFrame(0x50),
            privacyNoritoFrame(0x42),
            privacyNoritoFrame(0x56),
            invalidPrivacyNoritoFrame(offset: 0, value: 0x58),
            invalidPrivacyNoritoFrame(offset: 4, value: 1),
            invalidPrivacyNoritoFrame(offset: 5, value: 1),
            invalidPrivacyNoritoFrame(offset: 22, value: 1),
            invalidPrivacyNoritoDeclaredPayloadLength(schemaByte: 0x42),
            invalidPrivacyNoritoOversizedPayloadLength(schemaByte: 0x42),
            invalidPrivacyNoritoFrame(offset: 39, value: 0x40),
            invalidPrivacyNoritoFrame(offset: 39, value: 0x20),
            invalidPrivacyNoritoWithNonzeroPadding(),
            invalidPrivacyNoritoWithExcessivePadding(),
            invalidPrivacyNoritoFrame(offset: 31, value: 1),
            invalidPrivacyNoritoPayloadTamper()
        ] {
            XCTAssertThrowsError(
                try PrivacyNativeBridge.call(
                    bridgeAvailable: true,
                    expectedSchemaByte: 0x42
                ) {
                    invalidArchive
                }
            ) { error in
                XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
            }
        }
    }

    func testRejectsWrongOperationSchemaNativeOutputs() throws {
        for (expected, wrongSchemas) in [
            (UInt8(0x50), [UInt8(0x42), UInt8(0x56), UInt8(0x52)]),
            (UInt8(0x42), [UInt8(0x50), UInt8(0x56), UInt8(0x52)]),
            (UInt8(0x56), [UInt8(0x50), UInt8(0x42), UInt8(0x52)])
        ] {
            let accepted = try PrivacyNativeBridge.call(
                bridgeAvailable: true,
                expectedSchemaByte: expected
            ) {
                privacyNoritoFrameWithPayload(expected)
            }
            XCTAssertEqual(accepted, privacyNoritoFrameWithPayload(expected))

            for mixedSchema in [
                privacyNoritoFrameWithSchemaOverride(
                    expected,
                    offset: 6,
                    value: wrongSchemas[0]
                ),
                privacyNoritoFrameWithSchemaOverride(
                    expected,
                    offset: 21,
                    value: wrongSchemas[0]
                )
            ] {
                XCTAssertThrowsError(
                    try PrivacyNativeBridge.call(
                        bridgeAvailable: true,
                        expectedSchemaByte: expected
                    ) {
                        mixedSchema
                    }
                ) { error in
                    XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
                }
            }

            for wrongSchema in wrongSchemas {
                XCTAssertThrowsError(
                    try PrivacyNativeBridge.call(
                        bridgeAvailable: true,
                        expectedSchemaByte: expected
                    ) {
                        privacyNoritoFrameWithPayload(wrongSchema)
                    }
                ) { error in
                    XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
                }
            }
        }

        let requestArchive = privacyNoritoFrameWithPayload(0x52)
        XCTAssertThrowsError(
            try PrivacyNativeBridge.call(
                requestArchive: requestArchive,
                bridgeAvailable: true,
                expectedSchemaByte: 0x42
            ) { request in
                XCTAssertEqual(request, requestArchive)
                return privacyNoritoFrameWithPayload(0x56)
            }
        ) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }
    }

    func testTemporaryPrivacyRequestArchiveCopiesCallerData() throws {
        let requestArchive = privacyNoritoFrameWithPayload(0x52)
        var observed = [UInt8]()

        try NoritoNativeBridge.withTemporaryPrivacyRequestArchive(
            requestArchive: requestArchive
        ) { buffer in
            observed = Array(buffer)
        }

        XCTAssertEqual(observed, Array(requestArchive))
    }

    func testHostileTemporaryPrivacyRequestMutationCannotMutateCallerArchive() throws {
        let requestArchive = privacyNoritoFrameWithPayload(0x52)
        let originalArchive = requestArchive
        var observedBeforeMutation = [UInt8]()
        var observedAfterMutation = [UInt8]()
        var observedAfterClear: [UInt8]?

        try NoritoNativeBridge.withTemporaryPrivacyRequestArchive(
            requestArchive: requestArchive,
            didClearForTesting: { clearedArchive in
                observedAfterClear = clearedArchive
            }
        ) { buffer in
            observedBeforeMutation = Array(buffer)
            let request = UnsafeMutablePointer(mutating: buffer.baseAddress)
            request?[0] = 0x00
            request?[6] = 0x7F
            observedAfterMutation = Array(buffer)
        }

        XCTAssertEqual(observedBeforeMutation, Array(originalArchive))
        XCTAssertNotEqual(observedAfterMutation, Array(originalArchive))
        XCTAssertEqual(requestArchive, originalArchive)
        let clearedArchive = try XCTUnwrap(observedAfterClear)
        XCTAssertEqual(clearedArchive.count, originalArchive.count)
        XCTAssertTrue(clearedArchive.allSatisfy { $0 == 0 })
    }

    func testTemporaryPrivacyRequestArchiveClearsCopyWhenBodyThrows() throws {
        enum LocalError: Error, Equatable {
            case nativeFailure
        }

        let requestArchive = privacyNoritoFrameWithPayload(0x52)
        var observedBeforeThrow = [UInt8]()
        var observedAfterClear: [UInt8]?

        XCTAssertThrowsError(
            try NoritoNativeBridge.withTemporaryPrivacyRequestArchive(
                requestArchive: requestArchive,
                didClearForTesting: { clearedArchive in
                    observedAfterClear = clearedArchive
                }
            ) { buffer in
                observedBeforeThrow = Array(buffer)
                throw LocalError.nativeFailure
            }
        ) { error in
            XCTAssertEqual(error as? LocalError, .nativeFailure)
        }

        XCTAssertEqual(observedBeforeThrow, Array(requestArchive))
        let clearedArchive = try XCTUnwrap(observedAfterClear)
        XCTAssertEqual(clearedArchive.count, requestArchive.count)
        XCTAssertTrue(clearedArchive.allSatisfy { $0 == 0 })
        XCTAssertEqual(requestArchive, privacyNoritoFrameWithPayload(0x52))
    }

    func testTemporaryPrivacyRequestArchiveRejectsInvalidSizesBeforeCopy() {
        XCTAssertThrowsError(
            try NoritoNativeBridge.withTemporaryPrivacyRequestArchive(
                requestArchive: Data()
            ) { _ in
                XCTFail("empty request must not reach native dispatch")
            }
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .invalidPrivacyRequest)
        }

        let oversized = Data(
            repeating: 0x7F,
            count: PrivacyNativeBridge.privacyNativeArchiveMaxBytes + 1
        )
        XCTAssertThrowsError(
            try NoritoNativeBridge.withTemporaryPrivacyRequestArchive(
                requestArchive: oversized
            ) { _ in
                XCTFail("oversized request must not reach native dispatch")
            }
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .invalidPrivacyRequest)
        }

        XCTAssertThrowsError(
            try NoritoNativeBridge.withTemporaryPrivacyRequestArchive(
                requestArchive: Data([0x01])
            ) { _ in
                XCTFail("malformed request must not reach native dispatch")
            }
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .invalidPrivacyRequest)
        }

        XCTAssertThrowsError(
            try NoritoNativeBridge.withTemporaryPrivacyRequestArchive(
                requestArchive: privacyNoritoFrame(0x52)
            ) { _ in
                XCTFail("zero-payload request must not reach native dispatch")
            }
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .invalidPrivacyRequest)
        }
    }

    func testTemporaryPrivacyRequestArchiveRejectsWrongSchemaBeforeCopy() {
        for forgedRequest in [
            privacyNoritoFrameWithPayload(0x50),
            privacyNoritoFrameWithPayload(0x42),
            privacyNoritoFrameWithPayload(0x56)
        ] {
            XCTAssertThrowsError(
                try NoritoNativeBridge.withTemporaryPrivacyRequestArchive(
                    requestArchive: forgedRequest
                ) { _ in
                    XCTFail("wrong-schema request must not reach native dispatch")
                }
            ) { error in
                XCTAssertEqual(error as? NativeBridgeError, .invalidPrivacyRequest)
            }
        }
    }

    func testClearTemporaryPrivacyRequestArchiveZerosBuffer() {
        var requestArchive = [UInt8](Data([0x50, 0x01]) + Data("swift-sdk-witness-clear-5f2b".utf8))

        NoritoNativeBridge.clearTemporaryPrivacyRequestArchive(&requestArchive)

        XCTAssertTrue(requestArchive.allSatisfy { $0 == 0 })
    }

    func testReadPrivacyNativeOutputCopiesArchiveAndFreesPointer() throws {
        let bytes = [UInt8](privacyNoritoFrameWithPayload(0x50))
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: bytes.count)
        pointer.initialize(from: bytes, count: bytes.count)
        var freed = false
        defer {
            if !freed {
                pointer.deinitialize(count: bytes.count)
                pointer.deallocate()
            }
        }

        let archive = try NoritoNativeBridge.readPrivacyNativeOutput(
            pointer: pointer,
            length: CUnsignedLong(bytes.count),
            expectedSchemaByte: 0x50
        ) { freedPointer in
            XCTAssertEqual(freedPointer, Optional(pointer))
            assertPrivacyNativePointerZeroed(freedPointer, count: bytes.count)
            freedPointer?.deinitialize(count: bytes.count)
            freedPointer?.deallocate()
            freed = true
        }

        XCTAssertEqual(archive, Data(bytes))
        XCTAssertTrue(freed)
    }

    func testReadPrivacyNativeOutputCopiesBeforeFreeCallbackCanMutateBuffer() throws {
        let bytes = [UInt8](privacyNoritoFrameWithPayload(0x50))
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: bytes.count)
        pointer.initialize(from: bytes, count: bytes.count)
        var freed = false
        defer {
            if !freed {
                pointer.deinitialize(count: bytes.count)
                pointer.deallocate()
            }
        }

        let archive = try NoritoNativeBridge.readPrivacyNativeOutput(
            pointer: pointer,
            length: CUnsignedLong(bytes.count),
            expectedSchemaByte: 0x50
        ) { freedPointer in
            XCTAssertEqual(freedPointer, Optional(pointer))
            assertPrivacyNativePointerZeroed(freedPointer, count: bytes.count)
            freedPointer?.update(repeating: 0x7F, count: bytes.count)
            freedPointer?.deinitialize(count: bytes.count)
            freedPointer?.deallocate()
            freed = true
        }

        XCTAssertEqual(archive, Data(bytes))
        XCTAssertTrue(freed)
    }

    func testReadPrivacyNativeOutputRejectsInvalidArchiveAndFreesPointer() {
        let bytes: [UInt8] = [0x50, 0x01, 0x02]
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: bytes.count)
        pointer.initialize(from: bytes, count: bytes.count)
        var freed = false
        defer {
            if !freed {
                pointer.deinitialize(count: bytes.count)
                pointer.deallocate()
            }
        }

        XCTAssertThrowsError(
            try NoritoNativeBridge.readPrivacyNativeOutput(
                pointer: pointer,
                length: CUnsignedLong(bytes.count),
                expectedSchemaByte: 0x50
            ) { freedPointer in
                XCTAssertEqual(freedPointer, Optional(pointer))
                assertPrivacyNativePointerZeroed(freedPointer, count: bytes.count)
                freedPointer?.deinitialize(count: bytes.count)
                freedPointer?.deallocate()
                freed = true
            }
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .invalidPrivacyOutput)
        }
        XCTAssertTrue(freed)
    }

    func testReadPrivacyNativeOutputRejectsEmptyPayloadArchiveAndFreesPointer() {
        let bytes = [UInt8](privacyNoritoFrame(0x50))
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: bytes.count)
        pointer.initialize(from: bytes, count: bytes.count)
        var freed = false
        defer {
            if !freed {
                pointer.deinitialize(count: bytes.count)
                pointer.deallocate()
            }
        }

        XCTAssertThrowsError(
            try NoritoNativeBridge.readPrivacyNativeOutput(
                pointer: pointer,
                length: CUnsignedLong(bytes.count),
                expectedSchemaByte: 0x50
            ) { freedPointer in
                XCTAssertEqual(freedPointer, Optional(pointer))
                assertPrivacyNativePointerZeroed(freedPointer, count: bytes.count)
                freedPointer?.deinitialize(count: bytes.count)
                freedPointer?.deallocate()
                freed = true
            }
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .invalidPrivacyOutput)
        }
        XCTAssertTrue(freed)
    }

    func testReadPrivacyNativeOutputRejectsWrongOperationSchemaAndFreesPointer() {
        let bytes = [UInt8](privacyNoritoFrameWithPayload(0x56))
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: bytes.count)
        pointer.initialize(from: bytes, count: bytes.count)
        var freed = false
        defer {
            if !freed {
                pointer.deinitialize(count: bytes.count)
                pointer.deallocate()
            }
        }

        XCTAssertThrowsError(
            try NoritoNativeBridge.readPrivacyNativeOutput(
                pointer: pointer,
                length: CUnsignedLong(bytes.count),
                expectedSchemaByte: 0x42
            ) { freedPointer in
                XCTAssertEqual(freedPointer, Optional(pointer))
                freedPointer?.deinitialize(count: bytes.count)
                freedPointer?.deallocate()
                freed = true
            }
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .invalidPrivacyOutput)
        }
        XCTAssertTrue(freed)
    }

    func testReadPrivacyNativeOutputRejectsEmptyArchiveAndFreesPointer() {
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: 1)
        pointer.initialize(to: 0x50)
        var freed = false
        defer {
            if !freed {
                pointer.deinitialize(count: 1)
                pointer.deallocate()
            }
        }

        XCTAssertThrowsError(
            try NoritoNativeBridge.readPrivacyNativeOutput(
                pointer: pointer,
                length: 0,
                expectedSchemaByte: 0x50
            ) { freedPointer in
                XCTAssertEqual(freedPointer, Optional(pointer))
                freedPointer?.deinitialize(count: 1)
                freedPointer?.deallocate()
                freed = true
            }
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .invalidPrivacyOutput)
        }
        XCTAssertTrue(freed)
    }

    func testReadPrivacyNativeOutputRejectsOversizedArchiveAndFreesPointer() {
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: 1)
        pointer.initialize(to: 0x50)
        var freed = false
        defer {
            if !freed {
                pointer.deinitialize(count: 1)
                pointer.deallocate()
            }
        }

        XCTAssertThrowsError(
            try NoritoNativeBridge.readPrivacyNativeOutput(
                pointer: pointer,
                length: CUnsignedLong(PrivacyNativeBridge.privacyNativeArchiveMaxBytes + 1),
                expectedSchemaByte: 0x50
            ) { freedPointer in
                XCTAssertEqual(freedPointer, Optional(pointer))
                freedPointer?.deinitialize(count: 1)
                freedPointer?.deallocate()
                freed = true
            }
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .invalidPrivacyOutput)
        }
        XCTAssertTrue(freed)
    }

    func testLiveCapabilitiesArchiveWhenNativeBridgeIsAvailable() throws {
        guard PrivacyNativeBridge.isNativeAvailable else {
            throw XCTSkip("Privacy native bridge is unavailable.")
        }

        let archive = try PrivacyNativeBridge.capabilitiesV1()
        XCTAssertFalse(archive.isEmpty)
    }

    func testMalformedProofRequestsAreRejectedBeforeNativeBridgeDispatch() throws {
        let malformedArchive = Data([0x01, 0x02, 0x03])
        XCTAssertThrowsError(try PrivacyNativeBridge.buildProofV1(requestArchive: malformedArchive)) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }
        XCTAssertThrowsError(try PrivacyNativeBridge.verifyProofV1(requestArchive: malformedArchive)) { error in
            XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        }
    }

    private func assertFailClosedProductionGate(_ capabilities: PrivacyCapabilities) {
        XCTAssertFalse(capabilities.productionReady)
        XCTAssertEqual(capabilities.productionGate.version, PrivacyProductionGate.version)
        XCTAssertFalse(capabilities.productionGate.ready)
        XCTAssertFalse(capabilities.productionGate.realProving)
        XCTAssertFalse(capabilities.productionGate.realVerification)
        XCTAssertFalse(capabilities.productionGate.chainAdmission)
        XCTAssertFalse(capabilities.productionGate.sdkParity)
        XCTAssertFalse(capabilities.productionGate.walletState)
        XCTAssertFalse(capabilities.productionGate.witnessPrivacyChecks)
        XCTAssertFalse(capabilities.productionGate.deterministicTests)
        XCTAssertFalse(capabilities.productionGate.negativeAdversarialTests)
        XCTAssertFalse(capabilities.productionGate.replayNullifierTests)
        XCTAssertFalse(capabilities.productionGate.fuzzing)
        XCTAssertFalse(capabilities.productionGate.parserFuzzing)
        XCTAssertFalse(capabilities.productionGate.verifierFuzzing)
        XCTAssertFalse(capabilities.productionGate.performanceGates)
        XCTAssertFalse(capabilities.productionGate.externalAudit)
        XCTAssertEqual(capabilities.productionGate.auditReferences, [])
        XCTAssertEqual(capabilities.productionGate.requiredGates, PrivacyProductionGate.requiredGateKeys)
        XCTAssertEqual(capabilities.productionGate.missing, PrivacyProductionGate.missingReasons)
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "real proving engine is not registered"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "chain admission path is not enabled"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "witness privacy checks are incomplete"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "negative/adversarial tests are incomplete"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "replay/nullifier rejection tests are incomplete"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "parser fuzzing gate is incomplete"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "verifier fuzzing gate is incomplete"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "internal cryptographic review signoff is missing"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "implementation stage is not production-hardened"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "planned SDK entrypoints remain"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "dev fixture entrypoints are not production entrypoints"
            )
        )
        XCTAssertTrue(
            capabilities.productionGate.missing.contains(
                "Iroha production allowlist is not enabled for this audited row"
            )
        )
    }

    private func assertSanitizedNativeError(_ error: Error, witness: String) {
        XCTAssertEqual(error as? PrivacyNativeBridgeError, .nativeRejected)
        XCTAssertFalse(String(describing: error).contains(witness))
        if let localizedError = error as? LocalizedError,
           let description = localizedError.errorDescription {
            XCTAssertFalse(description.contains(witness))
        }
    }

    private struct NativeGateStatusFixture {
        var key: String
        var passed: Bool
    }

    private struct NativeProductionGateFixture {
        var version: String
        var ready: Bool
        var gates: [NativeGateStatusFixture]
        var requiredGates: [String]
        var missing: [String]
        var auditReferences: [String]
    }

    private struct NativeCapabilityFixture {
        var algorithmId: String
        var proofFamily: String
        var backendFamily: String
        var sdkEntrypoints: [String]
        var plannedEntrypoints: [String]
        var productionReady: Bool
        var productionGate: NativeProductionGateFixture
    }

    private func nativeCapabilitiesArchive(
        _ rows: NativeCapabilityFixture...
    ) -> Data {
        let flags = NoritoHeader.compactLen
        var payload = Data()
        appendUInt32LE(PrivacyNativeBridge.ffiVersionV1, to: &payload)
        payload.append(encodedNativeString(PrivacyProductionGate.version, flags: flags))
        payload.append(
            encodedNativeSequence(rows, flags: flags, encodeElement: encodedNativeCapability)
        )
        return privacyNoritoFrame(0x50, payload: payload, flags: flags)
    }

    private func nativeCapability(
        _ algorithmId: String,
        ready: Bool
    ) -> NativeCapabilityFixture {
        NativeCapabilityFixture(
            algorithmId: algorithmId,
            proofFamily: "halo2-ipa",
            backendFamily: "halo2-ipa",
            sdkEntrypoints: ["buildConfidentialTransferProofV2"],
            plannedEntrypoints: [],
            productionReady: ready,
            productionGate: NativeProductionGateFixture(
                version: PrivacyProductionGate.version,
                ready: ready,
                gates: PrivacyProductionGate.requiredGateKeys.map {
                    NativeGateStatusFixture(key: $0, passed: ready)
                },
                requiredGates: PrivacyProductionGate.requiredGateKeys,
                missing: ready ? [] : PrivacyProductionGate.missingReasons,
                auditReferences: ready ? Self.productionAuditReferences() : []
            )
        )
    }

    private func encodedNativeCapability(_ row: NativeCapabilityFixture) -> Data {
        let flags = NoritoHeader.compactLen
        var payload = Data()
        payload.append(encodedNativeString(row.algorithmId, flags: flags))
        payload.append(encodedNativeString(row.proofFamily, flags: flags))
        payload.append(encodedNativeString(row.backendFamily, flags: flags))
        payload.append(
            encodedNativeSequence(row.sdkEntrypoints, flags: flags) {
                encodedNativeString($0, flags: flags)
            }
        )
        payload.append(
            encodedNativeSequence(row.plannedEntrypoints, flags: flags) {
                encodedNativeString($0, flags: flags)
            }
        )
        payload.append(row.productionReady ? UInt8(1) : UInt8(0))
        payload.append(encodedNativeProductionGate(row.productionGate))
        return payload
    }

    private func encodedNativeProductionGate(_ gate: NativeProductionGateFixture) -> Data {
        let flags = NoritoHeader.compactLen
        var payload = Data()
        payload.append(encodedNativeString(gate.version, flags: flags))
        payload.append(gate.ready ? UInt8(1) : UInt8(0))
        payload.append(
            encodedNativeSequence(gate.gates, flags: flags, encodeElement: encodedNativeGateStatus)
        )
        payload.append(
            encodedNativeSequence(gate.requiredGates, flags: flags) {
                encodedNativeString($0, flags: flags)
            }
        )
        payload.append(
            encodedNativeSequence(gate.missing, flags: flags) {
                encodedNativeString($0, flags: flags)
            }
        )
        payload.append(
            encodedNativeSequence(gate.auditReferences, flags: flags) {
                encodedNativeString($0, flags: flags)
            }
        )
        return payload
    }

    private func encodedNativeGateStatus(_ status: NativeGateStatusFixture) -> Data {
        let flags = NoritoHeader.compactLen
        var payload = Data()
        payload.append(encodedNativeString(status.key, flags: flags))
        payload.append(status.passed ? UInt8(1) : UInt8(0))
        return payload
    }

    private func encodedNativeString(_ value: String, flags: UInt8) -> Data {
        let bytes = Data(value.utf8)
        var payload = Data()
        appendLength(UInt64(bytes.count), compact: (flags & NoritoHeader.compactLen) != 0, to: &payload)
        payload.append(bytes)
        return payload
    }

    private func encodedNativeSequence<T>(
        _ values: [T],
        flags: UInt8,
        encodeElement: (T) -> Data
    ) -> Data {
        var payload = Data()
        appendUInt64LE(UInt64(values.count), to: &payload)
        for value in values {
            let elementPayload = encodeElement(value)
            appendLength(
                UInt64(elementPayload.count),
                compact: (flags & NoritoHeader.compactLen) != 0,
                to: &payload
            )
            payload.append(elementPayload)
        }
        return payload
    }

    private func appendLength(_ value: UInt64, compact: Bool, to data: inout Data) {
        if compact {
            appendVarint(value, to: &data)
        } else {
            appendUInt64LE(value, to: &data)
        }
    }

    private func appendVarint(_ rawValue: UInt64, to data: inout Data) {
        var value = rawValue
        while value >= 0x80 {
            data.append(UInt8((value & 0x7F) | 0x80))
            value >>= 7
        }
        data.append(UInt8(value))
    }

    private func appendUInt32LE(_ value: UInt32, to data: inout Data) {
        data.append(contentsOf: withUnsafeBytes(of: value.littleEndian, Array.init))
    }

    private func appendUInt64LE(_ value: UInt64, to data: inout Data) {
        data.append(contentsOf: withUnsafeBytes(of: value.littleEndian, Array.init))
    }

    private static func productionAuditReferences() -> [String] {
        [
            "chain_id:boi-privacy-4peer-chain",
            "reviewer:security-reviewer",
            "review_artifact_hash:\(productionHash(1))",
            "review_artifact_signature:ed25519:\(String(repeating: "b", count: 128))",
            "fuzz_artifact_hash:\(productionHash(2))",
            "performance_artifact_hash:\(productionHash(3))",
            "localnet_run_id:boi-privacy-4peer-localnet-2026-06-13",
            "localnet_smoke_tx_hash:\(productionHash(4))",
            "localnet_replay_rejection_hash:\(productionHash(5))",
            "localnet_restart_replay_rejection_hash:\(productionHash(6))",
            "localnet_state_recovery_hash:\(productionHash(7))",
            "localnet_lifecycle_shield_tx_hash:\(productionHash(8))",
            "localnet_lifecycle_hop_proof_hash:\(productionHash(9))",
            "localnet_lifecycle_recursive_init_hash:\(productionHash(10))",
            "localnet_lifecycle_recursive_init_verify_hash:\(productionHash(11))",
            "localnet_lifecycle_recursive_append_hash:\(productionHash(12))",
            "localnet_lifecycle_recursive_append_verify_hash:\(productionHash(13))",
            "localnet_lifecycle_unshield_proof_hash:\(productionHash(14))",
            "localnet_lifecycle_redeem_tx_hash:\(productionHash(15))"
        ]
    }

    private static func productionHash(_ value: Int) -> String {
        let hex = String(value, radix: 16)
        return "sha256:\(String(repeating: "0", count: max(0, 64 - hex.count)))\(hex)"
    }

    private func privacyNoritoFrame(_ schemaByte: UInt8) -> Data {
        var frame = [UInt8](repeating: 0, count: 40)
        frame[0] = 0x4E
        frame[1] = 0x52
        frame[2] = 0x54
        frame[3] = 0x30
        for index in 6..<22 {
            frame[index] = schemaByte
        }
        return Data(frame)
    }

    private func privacyNoritoFrame(
        _ schemaByte: UInt8,
        payload: Data,
        flags: UInt8
    ) -> Data {
        let header = NoritoHeader(
            schema: [UInt8](repeating: schemaByte, count: 16),
            compression: .none,
            length: UInt64(payload.count),
            checksum: crc64ECMA(payload),
            flags: flags
        )
        var frame = Data()
        frame.append(header.encode())
        frame.append(payload)
        return frame
    }

    private func privacyNoritoFrameWithPayload(_ schemaByte: UInt8) -> Data {
        var frame = [UInt8](privacyNoritoFrame(schemaByte))
        frame.append(contentsOf: [0, 0, 0xA5, 0x5A, 0x11])
        frame[23] = 3
        let checksum: [UInt8] = [0xB9, 0xD3, 0xA8, 0x0C, 0xCD, 0x5D, 0x13, 0x24]
        for (index, byte) in checksum.enumerated() {
            frame[31 + index] = byte
        }
        return Data(frame)
    }

    private func privacyNoritoFrameWithPadding(
        _ schemaByte: UInt8,
        paddingLength: Int
    ) -> Data {
        var frame = [UInt8](privacyNoritoFrame(schemaByte))
        frame.append(contentsOf: [UInt8](repeating: 0, count: paddingLength))
        frame.append(contentsOf: [0xA5, 0x5A, 0x11])
        frame[23] = 3
        let checksum: [UInt8] = [0xB9, 0xD3, 0xA8, 0x0C, 0xCD, 0x5D, 0x13, 0x24]
        for (index, byte) in checksum.enumerated() {
            frame[31 + index] = byte
        }
        return Data(frame)
    }

    private func privacyNoritoFrameWithSchemaOverride(
        _ schemaByte: UInt8,
        offset: Int,
        value: UInt8
    ) -> Data {
        var frame = [UInt8](privacyNoritoFrameWithPayload(schemaByte))
        frame[offset] = value
        return Data(frame)
    }

    private func privacyNoritoFrameWithDeclaredPayloadLength(
        _ schemaByte: UInt8,
        payloadLength: UInt64
    ) -> Data {
        var frame = [UInt8](privacyNoritoFrameWithPayload(schemaByte))
        for index in 0..<8 {
            frame[23 + index] = UInt8(
                truncatingIfNeeded: payloadLength >> UInt64(index * 8)
            )
        }
        return Data(frame)
    }

    private func privacyNoritoFrameWithFlags(_ schemaByte: UInt8, flags: UInt8) -> Data {
        var frame = [UInt8](privacyNoritoFrameWithPayload(schemaByte))
        frame[39] = flags
        return Data(frame)
    }

    private func invalidPrivacyNoritoFrame(offset: Int, value: UInt8) -> Data {
        var frame = [UInt8](privacyNoritoFrame(0x50))
        frame[offset] = value
        return Data(frame)
    }

    private func invalidPrivacyNoritoDeclaredPayloadLength(
        schemaByte: UInt8 = 0x50
    ) -> Data {
        privacyNoritoFrameWithDeclaredPayloadLength(schemaByte, payloadLength: 6)
    }

    private func invalidPrivacyNoritoOversizedPayloadLength(
        schemaByte: UInt8 = 0x50
    ) -> Data {
        privacyNoritoFrameWithDeclaredPayloadLength(
            schemaByte,
            payloadLength: 0x8000_0000_0000_0000
        )
    }

    private func invalidPrivacyNoritoWithNonzeroPadding() -> Data {
        var frame = [UInt8](privacyNoritoFrame(0x50))
        frame.append(1)
        return Data(frame)
    }

    private func invalidPrivacyNoritoWithExcessivePadding() -> Data {
        privacyNoritoFrameWithPadding(0x50, paddingLength: 65)
    }

    private func invalidPrivacyNoritoPayloadTamper() -> Data {
        var frame = [UInt8](privacyNoritoFrameWithPayload(0x50))
        frame[44] ^= 0x7F
        return Data(frame)
    }

    private func withPrivacyOutputPointer(
        _ archive: Data,
        _ body: (UnsafeMutablePointer<UInt8>, CUnsignedLong) -> Void
    ) {
        let bytes = [UInt8](archive)
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: bytes.count)
        pointer.initialize(from: bytes, count: bytes.count)
        defer {
            pointer.deinitialize(count: bytes.count)
            pointer.deallocate()
        }
        body(pointer, CUnsignedLong(bytes.count))
    }

    private func assertPrivacyNativePointerZeroed(
        _ pointer: UnsafeMutablePointer<UInt8>?,
        count: Int
    ) {
        guard let pointer else {
            XCTFail("expected privacy native output pointer")
            return
        }
        let bytes = Array(UnsafeBufferPointer(start: pointer, count: count))
        XCTAssertTrue(bytes.allSatisfy { $0 == 0 })
    }
}

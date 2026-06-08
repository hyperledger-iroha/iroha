import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveCompactPaymentTokenProverTests: XCTestCase {
    func testRejectsEmptyRecordBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: Data(),
                    pallasOpenEnvelopesArchive: validKagemushaNoritoArchive()
                )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyRecordBundleArchive
            )
        }
    }

    func testRejectsEmptyPallasOpenEnvelopesArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validKagemushaNoritoArchive(),
                    pallasOpenEnvelopesArchive: Data()
                )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyPallasOpenEnvelopesArchive
            )
        }
    }

    func testRejectsMalformedInputArchivesBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: Data([0x01, 0x02]),
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native compact-token prover body must not run for malformed record bundles")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .invalidRecordBundleArchive
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: Data([0x01, 0x02]),
                    bridgeAvailable: false
                ) {
                    XCTFail("native compact-token prover body must not run for malformed Pallas openings")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .invalidPallasOpenEnvelopesArchive
            )
        }
    }

    func testRejectsOversizedInputArchivesBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        let oversizedArchive = Data(
            repeating: 0x7f,
            count: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1
        )
        let oversizedMessage =
            "must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes"
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: oversizedArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native recursive compact body must not run for oversized record bundles")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .oversizedRecordBundleArchive
            )
            XCTAssertTrue(error.localizedDescription.contains(oversizedMessage))
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: oversizedArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native recursive compact body must not run for oversized Pallas openings")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .oversizedPallasOpenEnvelopesArchive
            )
            XCTAssertTrue(error.localizedDescription.contains(oversizedMessage))
        }
    }

    func testRejectsEmptyPayloadInputArchivesBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        let emptyPayloadArchive = emptyPayloadKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: emptyPayloadArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native compact-token prover body must not run for empty record-bundle payloads")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyRecordBundlePayload
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: emptyPayloadArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native compact-token prover body must not run for empty Pallas payloads")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyPallasOpenEnvelopesPayload
            )
        }
    }

    func testRejectsEmptyNativeOutput() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: true
                ) {
                    Data()
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .proofRejected
            )
        }
    }

    func testRejectsMalformedNativeOutput() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: true
                ) {
                    Data([0x01])
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .invalidCompactTokenArchive
            )
        }
    }

    func testRejectsEmptyPayloadNativeOutput() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: true
                ) {
                    emptyPayloadKagemushaNoritoArchive()
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyCompactTokenPayload
            )
        }
    }

    func testReturnsValidNativeOutput() throws {
        let archive = validKagemushaNoritoArchive()
        let output = try KagemushaRecursiveCompactPaymentTokenProver
            .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                recordBundleArchive: archive,
                pallasOpenEnvelopesArchive: archive,
                bridgeAvailable: true
            ) {
                archive
            }

        XCTAssertEqual(output, archive)
    }

    func testProjectionRejectsMalformedBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: Data([0x01]),
                    bridgeAvailable: false
                ) {
                    XCTFail("native projection body must not run for malformed bundle archives")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .invalidBundleArchive
            )
        }
    }

    func testProjectionRejectsOversizedBundleArchiveBeforeBridgeCall() {
        let oversizedArchive = Data(
            repeating: 0x7f,
            count: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1
        )
        let oversizedMessage =
            "must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes"
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: oversizedArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native projection body must not run for oversized bundle archives")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .oversizedBundleArchive
            )
            XCTAssertTrue(error.localizedDescription.contains(oversizedMessage))
        }
    }

    func testProjectionRejectsEmptyPayloadBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: emptyPayloadKagemushaNoritoArchive(),
                    bridgeAvailable: false
                ) {
                    XCTFail("native projection body must not run for empty bundle payloads")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyBundlePayload
            )
        }
    }

    func testProjectionRequiresBridgeAfterInputValidation() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: false
                ) {
                    XCTFail("native projection body must not run when unavailable")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .bridgeUnavailable
            )
        }
    }

    func testProjectionRejectsMalformedNativeOutput() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    Data([0x01])
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .invalidCompactTokenArchive
            )
        }
    }

    func testProjectionReturnsValidNativeOutput() throws {
        let archive = validKagemushaNoritoArchive()
        let output = try KagemushaRecursiveCompactPaymentTokenProver
            .recursiveSpendCompactPaymentTokenFromBundle(
                bundleArchive: archive,
                bridgeAvailable: true
            ) {
                archive
            }

        XCTAssertEqual(output, archive)
    }

    func testProjectionVerifierRejectsMalformedVerifierRecordBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: Data([0x01]),
                    bridgeAvailable: true
                ) {
                    XCTFail("native projection verifier body must not run for malformed records")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .invalidVerifierRecordArchive
            )
        }
    }

    func testProjectionVerifierRejectsEmptyVerifierRecordBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: Data(),
                    bridgeAvailable: true
                ) {
                    XCTFail("native projection verifier body must not run for empty records")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyVerifierRecordArchive
            )
        }
    }

    func testProjectionVerifierRejectsOversizedVerifierRecordBeforeBridgeCall() {
        let oversizedArchive = Data(
            repeating: 0x7f,
            count: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: oversizedArchive,
                    bridgeAvailable: true
                ) {
                    XCTFail("native projection verifier body must not run for oversized records")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .oversizedVerifierRecordArchive
            )
            XCTAssertTrue(
                error.localizedDescription.contains(
                    "must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes"
                )
            )
        }
    }

    func testProjectionVerifierRejectsEmptyPayloadVerifierRecordBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: emptyPayloadKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    XCTFail("native projection verifier body must not run for empty record payloads")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyVerifierRecordPayload
            )
        }
    }

    func testProjectionVerifierRequiresNativeAvailabilityAfterInputValidation() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: false
                ) {
                    XCTFail("native projection verifier body must not run when unavailable")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .bridgeUnavailable
            )
        }
    }

    func testProjectionVerifierReturnsNativeBoolean() throws {
        XCTAssertTrue(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    true
                }
        )
        XCTAssertFalse(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    false
                }
        )
    }

    func testNilNativeOutputIsBridgeUnavailable() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: true
                ) {
                    nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .bridgeUnavailable
            )
        }
    }

    func testNativeRecursiveCompactUnavailableIsDistinctFromProofRejection() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    bridgeAvailable: true
                ) {
                    throw NativeBridgeError.kagemushaRecursiveCompactUnavailable
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .recursiveCompactUnavailable
            )
        }
    }

    func testVerifyRejectsEmptyCompactTokenArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: Data(),
                    bridgeAvailable: true
                ) {
                    true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyCompactTokenArchive
            )
        }
    }

    func testVerifyRejectsMalformedCompactTokenArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: Data([0x01]),
                    bridgeAvailable: true
                ) {
                    true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .invalidCompactTokenArchive
            )
        }
    }

    func testVerifyRejectsOversizedCompactTokenArchiveBeforeBridgeCall() {
        let oversizedArchive = Data(
            repeating: 0x7f,
            count: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: oversizedArchive,
                    bridgeAvailable: true
                ) {
                    XCTFail("native compact-token verifier body must not run for oversized compact tokens")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .oversizedCompactTokenArchive
            )
            XCTAssertTrue(
                error.localizedDescription.contains(
                    "must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes"
                )
            )
        }
    }

    func testVerifyRejectsEmptyPayloadCompactTokenArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: emptyPayloadKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyCompactTokenPayload
            )
        }
    }

    func testVerifyRequiresVerifierNativeAvailabilityAfterInputValidation() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: false
                ) {
                    XCTFail("native compact-token verifier body must not run when unavailable")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .bridgeUnavailable
            )
        }
    }

    func testVerifyReturnsNativeBoolean() throws {
        XCTAssertTrue(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    true
                }
        )
        XCTAssertFalse(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    false
                }
        )
    }

    func testNativeBridgeRejectsInvalidVerifierBooleanOutput() throws {
        XCTAssertFalse(
            try NoritoNativeBridge.normalizeKagemushaRecursiveCompactVerifierOutput(
                status: 0,
                valid: 0
            )
        )
        XCTAssertTrue(
            try NoritoNativeBridge.normalizeKagemushaRecursiveCompactVerifierOutput(
                status: 0,
                valid: 1
            )
        )
        XCTAssertThrowsError(
            try NoritoNativeBridge.normalizeKagemushaRecursiveCompactVerifierOutput(
                status: 0,
                valid: 2
            )
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .invalidKagemushaVerifierOutput)
        }
        XCTAssertThrowsError(
            try NoritoNativeBridge.normalizeKagemushaRecursiveCompactVerifierOutput(
                status: -311,
                valid: 0
            )
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .kagemushaProve)
        }
        XCTAssertThrowsError(
            try NoritoNativeBridge.normalizeKagemushaRecursiveCompactVerifierOutput(
                status: -312,
                valid: 0
            )
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .kagemushaRecursiveCompactUnavailable)
        }
    }

    #if canImport(Darwin)
    func testNativeBridgeCopiesBoundedKagemushaOutputAndFreesNativePointer() throws {
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: 1)
        pointer.initialize(to: 0x4e)
        var didFree = false

        let data = try NoritoNativeBridge.copyKagemushaNativeArchiveOutput(
            pointer: pointer,
            length: 1
        ) { outputPointer in
            XCTAssertEqual(outputPointer, pointer)
            outputPointer?.deinitialize(count: 1)
            outputPointer?.deallocate()
            didFree = true
        }

        XCTAssertEqual(data, Data([0x4e]))
        XCTAssertTrue(didFree)
    }

    func testNativeBridgeRejectsOversizedKagemushaOutputBeforeCopying() {
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: 1)
        pointer.initialize(to: 0x4e)
        var didFree = false

        XCTAssertThrowsError(
            try NoritoNativeBridge.copyKagemushaNativeArchiveOutput(
                pointer: pointer,
                length: CUnsignedLong(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1)
            ) { outputPointer in
                XCTAssertEqual(outputPointer, pointer)
                outputPointer?.deinitialize(count: 1)
                outputPointer?.deallocate()
                didFree = true
            }
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .kagemushaProve)
        }
        XCTAssertTrue(didFree)
    }
    #endif

    func testVerifyNilNativeResultIsBridgeUnavailable() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .bridgeUnavailable
            )
        }
    }

    func testVerifyNativeRejectionIsVerificationRejected() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    throw NativeBridgeError.kagemushaProve
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .verificationRejected
            )
        }
    }
}

private func validKagemushaNoritoArchive() -> Data {
    noritoEncode(
        typeName: "KagemushaRecursiveCompactPaymentTokenArchiveV1",
        payload: Data([0xa5, 0x5a, 0x11])
    )
}

private func emptyPayloadKagemushaNoritoArchive() -> Data {
    noritoEncode(
        typeName: "KagemushaRecursiveCompactPaymentTokenArchiveV1",
        payload: Data()
    )
}

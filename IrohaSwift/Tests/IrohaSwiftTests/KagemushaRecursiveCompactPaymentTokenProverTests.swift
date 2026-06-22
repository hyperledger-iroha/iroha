import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaRecursiveCompactPaymentTokenProverTests: XCTestCase {
    func testRejectsEmptyRecordBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: Data(),
                    pallasOpenEnvelopesArchive: validKagemushaNoritoArchive(),
                    recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive()
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
                    pallasOpenEnvelopesArchive: Data(),
                    recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive()
                )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyPallasOpenEnvelopesArchive
            )
        }
    }

    func testRejectsInvalidKeyArtifactsArchiveBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    recursiveCompactKeyArtifactsArchive: Data(),
                    bridgeAvailable: false
                ) {
                    XCTFail("native compact-token prover body must not run for empty key artifacts")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyKeyArtifactsArchive
            )
        }
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveCompactPaymentTokenProver
                    .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                        recordBundleArchive: validArchive,
                        pallasOpenEnvelopesArchive: validArchive,
                        recursiveCompactKeyArtifactsArchive: malformedArchive,
                        bridgeAvailable: false
                    ) {
                        XCTFail("native compact-token prover body must not run for malformed key artifacts")
                        return nil
                    },
                "key artifacts archive \(label) should be rejected before bridge call"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveCompactPaymentTokenProverError,
                    .invalidKeyArtifactsArchive
                )
            }
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    recursiveCompactKeyArtifactsArchive: emptyPayloadKagemushaNoritoArchive(),
                    bridgeAvailable: false
                ) {
                    XCTFail("native compact-token prover body must not run for empty key-artifact payloads")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyKeyArtifactsPayload
            )
        }
    }

    func testRejectsMalformedInputArchivesBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveCompactPaymentTokenProver
                    .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                        recordBundleArchive: malformedArchive,
                        pallasOpenEnvelopesArchive: validArchive,
                        recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
                        bridgeAvailable: false
                    ) {
                        XCTFail("native compact-token prover body must not run for malformed record bundles")
                        return nil
                    },
                "record bundle archive \(label) should be rejected before bridge call"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveCompactPaymentTokenProverError,
                    .invalidRecordBundleArchive
                )
            }
        }
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveCompactPaymentTokenProver
                    .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                        recordBundleArchive: validArchive,
                        pallasOpenEnvelopesArchive: malformedArchive,
                        recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
                        bridgeAvailable: false
                    ) {
                        XCTFail("native compact-token prover body must not run for malformed Pallas openings")
                        return nil
                    },
                "Pallas openings archive \(label) should be rejected before bridge call"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveCompactPaymentTokenProverError,
                    .invalidPallasOpenEnvelopesArchive
                )
            }
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
                    recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
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
                    recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
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
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    recursiveCompactKeyArtifactsArchive: oversizedArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native recursive compact body must not run for oversized key artifacts")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .oversizedKeyArtifactsArchive
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
                    recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
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
                    recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
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
                    recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
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
        for (label, nativeOutput) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveCompactPaymentTokenProver
                    .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                        recordBundleArchive: validArchive,
                        pallasOpenEnvelopesArchive: validArchive,
                        recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
                        bridgeAvailable: true
                    ) {
                        nativeOutput
                    },
                "native output \(label) should be rejected"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveCompactPaymentTokenProverError,
                    .invalidCompactTokenArchive
                )
            }
        }
    }

    func testRejectsEmptyPayloadNativeOutput() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
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
                recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
                bridgeAvailable: true
            ) {
                archive
            }

        XCTAssertEqual(output, archive)
    }

    func testProjectionRejectsEmptyBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: Data(),
                    bridgeAvailable: false
                ) {
                    XCTFail("native projection body must not run for empty bundle archives")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyBundleArchive
            )
        }
    }

    func testProjectionRejectsMalformedBundleArchiveBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveCompactPaymentTokenProver
                    .recursiveSpendCompactPaymentTokenFromBundle(
                        bundleArchive: malformedArchive,
                        bridgeAvailable: false
                    ) {
                        XCTFail("native projection body must not run for malformed bundle archives")
                        return nil
                    },
                "bundle archive \(label) should be rejected before bridge call"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveCompactPaymentTokenProverError,
                    .invalidBundleArchive
                )
            }
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
        let validArchive = validKagemushaNoritoArchive()
        for (label, nativeOutput) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveCompactPaymentTokenProver
                    .recursiveSpendCompactPaymentTokenFromBundle(
                        bundleArchive: validArchive,
                        bridgeAvailable: true
                    ) {
                        nativeOutput
                    },
                "projection native output \(label) should be rejected"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveCompactPaymentTokenProverError,
                    .invalidCompactTokenArchive
                )
            }
        }
    }

    func testProjectionNilNativeOutputIsBridgeUnavailable() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: validKagemushaNoritoArchive(),
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

    func testProjectionRejectsEmptyNativeOutput() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: validKagemushaNoritoArchive(),
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

    func testProjectionRejectsEmptyPayloadNativeOutput() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: validKagemushaNoritoArchive(),
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

    func testProjectionNativeRejectionIsProofRejected() {
        enum LocalError: Error {
            case rejected
        }

        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    throw NativeBridgeError.kagemushaProve
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .proofRejected
            )
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(
                    bundleArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    throw LocalError.rejected
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .proofRejected
            )
        }
    }

    private func malformedKagemushaNoritoArchives(_ validArchive: Data) -> [(String, Data)] {
        var compressed = validArchive
        compressed[22] = 0x01
        var unsupportedFlags = validArchive
        unsupportedFlags[39] = NoritoHeader.varintOffsets
        var invalidFieldBitset = validArchive
        invalidFieldBitset[39] = NoritoHeader.fieldBitset
        return [
            ("truncated", Data([0x01])),
            ("compressed", compressed),
            ("unsupported flags", unsupportedFlags),
            ("invalid field bitset", invalidFieldBitset),
            (
                "nonzero header padding",
                kagemushaNoritoFrameWithHeaderPadding(validArchive, padding: Data([0x7f]))
            ),
            (
                "excessive header padding",
                kagemushaNoritoFrameWithHeaderPadding(
                    validArchive,
                    padding: Data(repeating: 0, count: 65)
                )
            ),
        ]
    }

    private func kagemushaNoritoFrameWithHeaderPadding(_ archive: Data, padding: Data) -> Data {
        var padded = archive
        padded.insert(contentsOf: padding, at: NoritoHeader.encodedLength)
        return padded
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

    func testProjectionVerifierRejectsEmptyCompactTokenArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: Data(),
                    verifierRecordArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: false
                ) {
                    XCTFail("native projection verifier body must not run for empty compact tokens")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyCompactTokenArchive
            )
        }
    }

    func testProjectionVerifierRejectsMalformedCompactTokenArchiveBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveCompactPaymentTokenProver
                    .verifyRecursiveSpendCompactPaymentTokenProjection(
                        compactTokenArchive: malformedArchive,
                        verifierRecordArchive: validArchive,
                        bridgeAvailable: false
                    ) {
                        XCTFail("native projection verifier body must not run for malformed compact tokens")
                        return true
                    },
                "compact token archive \(label) should be rejected before bridge call"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveCompactPaymentTokenProverError,
                    .invalidCompactTokenArchive
                )
            }
        }
    }

    func testProjectionVerifierRejectsEmptyPayloadCompactTokenArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: emptyPayloadKagemushaNoritoArchive(),
                    verifierRecordArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: false
                ) {
                    XCTFail("native projection verifier body must not run for empty compact-token payloads")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyCompactTokenPayload
            )
        }
    }

    func testProjectionVerifierRejectsMalformedVerifierRecordBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveCompactPaymentTokenProver
                    .verifyRecursiveSpendCompactPaymentTokenProjection(
                        compactTokenArchive: validArchive,
                        verifierRecordArchive: malformedArchive,
                        bridgeAvailable: true
                    ) {
                        XCTFail("native projection verifier body must not run for malformed records")
                        return true
                    },
                "verifier record archive \(label) should be rejected before bridge call"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveCompactPaymentTokenProverError,
                    .invalidVerifierRecordArchive
                )
            }
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

    func testProjectionVerifierNilNativeResultIsBridgeUnavailable() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: validKagemushaNoritoArchive(),
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

    func testProjectionVerifierNativeRecursiveCompactUnavailableIsDistinctFromRejection() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: validKagemushaNoritoArchive(),
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

    func testProjectionVerifierNativeRejectionIsVerificationRejected() {
        enum LocalError: Error {
            case rejected
        }

        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: validKagemushaNoritoArchive(),
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
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    verifierRecordArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    throw LocalError.rejected
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .verificationRejected
            )
        }
    }

    func testNilNativeOutputIsBridgeUnavailable() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive: validArchive,
                    pallasOpenEnvelopesArchive: validArchive,
                    recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
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
                    recursiveCompactKeyArtifactsArchive: validRecursiveCompactKeyArtifactsArchive(),
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
                    recursiveCompactVerifierKeysArchive: validRecursiveCompactVerifierKeysArchive(),
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
        let validArchive = validKagemushaNoritoArchive()
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveCompactPaymentTokenProver
                    .verifyRecursiveCompactPaymentToken(
                        compactTokenArchive: malformedArchive,
                        recursiveCompactVerifierKeysArchive: validRecursiveCompactVerifierKeysArchive(),
                        bridgeAvailable: true
                    ) {
                        true
                    },
                "compact token archive \(label) should be rejected before bridge call"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveCompactPaymentTokenProverError,
                    .invalidCompactTokenArchive
                )
            }
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
                    recursiveCompactVerifierKeysArchive: validRecursiveCompactVerifierKeysArchive(),
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
                    recursiveCompactVerifierKeysArchive: validRecursiveCompactVerifierKeysArchive(),
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

    func testVerifyRejectsInvalidVerifierKeysArchiveBeforeBridgeCall() {
        let validArchive = validKagemushaNoritoArchive()
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: validArchive,
                    recursiveCompactVerifierKeysArchive: Data(),
                    bridgeAvailable: true
                ) {
                    XCTFail("native compact-token verifier body must not run for empty verifier keys")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyVerifierKeysArchive
            )
        }
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaRecursiveCompactPaymentTokenProver
                    .verifyRecursiveCompactPaymentToken(
                        compactTokenArchive: validArchive,
                        recursiveCompactVerifierKeysArchive: malformedArchive,
                        bridgeAvailable: true
                    ) {
                        XCTFail("native compact-token verifier body must not run for malformed verifier keys")
                        return true
                    },
                "verifier keys archive \(label) should be rejected before bridge call"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaRecursiveCompactPaymentTokenProverError,
                    .invalidVerifierKeysArchive
                )
            }
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: validArchive,
                    recursiveCompactVerifierKeysArchive: emptyPayloadKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    XCTFail("native compact-token verifier body must not run for empty verifier-key payloads")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .emptyVerifierKeysPayload
            )
        }
    }

    func testVerifyRejectsOversizedVerifierKeysArchiveBeforeBridgeCall() {
        let oversizedArchive = Data(
            repeating: 0x7f,
            count: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    recursiveCompactVerifierKeysArchive: oversizedArchive,
                    bridgeAvailable: true
                ) {
                    XCTFail("native compact-token verifier body must not run for oversized verifier keys")
                    return true
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveCompactPaymentTokenProverError,
                .oversizedVerifierKeysArchive
            )
            XCTAssertTrue(
                error.localizedDescription.contains(
                    "must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes"
                )
            )
        }
    }

    func testVerifyRequiresVerifierNativeAvailabilityAfterInputValidation() {
        XCTAssertThrowsError(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    recursiveCompactVerifierKeysArchive: validRecursiveCompactVerifierKeysArchive(),
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
                    recursiveCompactVerifierKeysArchive: validRecursiveCompactVerifierKeysArchive(),
                    bridgeAvailable: true
                ) {
                    true
                }
        )
        XCTAssertFalse(
            try KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveCompactPaymentToken(
                    compactTokenArchive: validKagemushaNoritoArchive(),
                    recursiveCompactVerifierKeysArchive: validRecursiveCompactVerifierKeysArchive(),
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
                    recursiveCompactVerifierKeysArchive: validRecursiveCompactVerifierKeysArchive(),
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
                    recursiveCompactVerifierKeysArchive: validRecursiveCompactVerifierKeysArchive(),
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

private func validRecursiveCompactKeyArtifactsArchive() -> Data {
    noritoEncode(
        typeName: "KagemushaRecursiveCompactKeyArtifactsV1",
        payload: Data([0xe1, 0x7a, 0x11])
    )
}

private func validRecursiveCompactVerifierKeysArchive() -> Data {
    noritoEncode(
        typeName: "KagemushaRecursiveCompactVerifierKeysV1",
        payload: Data([0xe2, 0x7b, 0x12])
    )
}

private func emptyPayloadKagemushaNoritoArchive() -> Data {
    noritoEncode(
        typeName: "KagemushaRecursiveCompactPaymentTokenArchiveV1",
        payload: Data()
    )
}

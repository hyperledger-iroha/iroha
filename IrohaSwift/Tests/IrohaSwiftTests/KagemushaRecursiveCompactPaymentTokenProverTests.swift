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

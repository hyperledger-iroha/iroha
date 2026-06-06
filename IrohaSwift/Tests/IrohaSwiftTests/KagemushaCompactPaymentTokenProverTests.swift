import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaCompactPaymentTokenProverTests: XCTestCase {
    func testRejectsEmptyRecordBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive: Data())
        ) { error in
            XCTAssertEqual(
                error as? KagemushaCompactPaymentTokenProverError,
                .emptyRecordBundleArchive
            )
        }
    }

    func testRejectsEmptyNativeOutput() {
        XCTAssertThrowsError(
            try KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(
                    recordBundleArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    Data()
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaCompactPaymentTokenProverError,
                .proofRejected
            )
        }
    }

    func testRejectsMalformedRecordBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(
                    recordBundleArchive: Data([0x01, 0x02]),
                    bridgeAvailable: false
                ) {
                    XCTFail("native compact-token prover body must not run for malformed record bundles")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaCompactPaymentTokenProverError,
                .invalidRecordBundleArchive
            )
        }
    }

    func testRejectsEmptyPayloadRecordBundleArchiveBeforeBridgeCall() {
        XCTAssertThrowsError(
            try KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(
                    recordBundleArchive: emptyPayloadKagemushaNoritoArchive(),
                    bridgeAvailable: false
                ) {
                    XCTFail("native compact-token prover body must not run for empty record-bundle payloads")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaCompactPaymentTokenProverError,
                .emptyRecordBundlePayload
            )
        }
    }

    func testRejectsMalformedNativeOutput() {
        XCTAssertThrowsError(
            try KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(
                    recordBundleArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    Data([0x01])
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaCompactPaymentTokenProverError,
                .invalidCompactTokenArchive
            )
        }
    }

    func testRejectsEmptyPayloadNativeOutput() {
        XCTAssertThrowsError(
            try KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(
                    recordBundleArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    emptyPayloadKagemushaNoritoArchive()
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaCompactPaymentTokenProverError,
                .emptyCompactTokenPayload
            )
        }
    }

    func testReturnsValidNativeOutput() throws {
        let archive = validKagemushaNoritoArchive()
        let output = try KagemushaCompactPaymentTokenProver
            .proveVerifiedCompactPaymentTokenWithRecords(
                recordBundleArchive: archive,
                bridgeAvailable: true
            ) {
                archive
            }

        XCTAssertEqual(output, archive)
    }

    func testNilNativeOutputIsBridgeUnavailable() {
        XCTAssertThrowsError(
            try KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(
                    recordBundleArchive: validKagemushaNoritoArchive(),
                    bridgeAvailable: true
                ) {
                    nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaCompactPaymentTokenProverError,
                .bridgeUnavailable
            )
        }
    }
}

private func validKagemushaNoritoArchive() -> Data {
    noritoEncode(
        typeName: "KagemushaCompactPaymentTokenArchiveV1",
        payload: Data([0xa5, 0x5a, 0x11])
    )
}

private func emptyPayloadKagemushaNoritoArchive() -> Data {
    noritoEncode(
        typeName: "KagemushaCompactPaymentTokenArchiveV1",
        payload: Data()
    )
}

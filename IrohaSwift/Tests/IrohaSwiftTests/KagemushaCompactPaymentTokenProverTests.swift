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
        let validArchive = validKagemushaNoritoArchive()
        for (label, malformedArchive) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaCompactPaymentTokenProver
                    .proveVerifiedCompactPaymentTokenWithRecords(
                        recordBundleArchive: malformedArchive,
                        bridgeAvailable: false
                    ) {
                        XCTFail("native compact-token prover body must not run for malformed record bundles")
                        return nil
                    },
                "record bundle \(label) should be rejected"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaCompactPaymentTokenProverError,
                    .invalidRecordBundleArchive
                )
            }
        }
    }

    func testRejectsOversizedRecordBundleArchiveBeforeBridgeCall() {
        let oversizedArchive = Data(
            repeating: 0x7f,
            count: KagemushaRecursiveSpendProver.nativeArchiveMaxBytes + 1
        )
        XCTAssertThrowsError(
            try KagemushaCompactPaymentTokenProver
                .proveVerifiedCompactPaymentTokenWithRecords(
                    recordBundleArchive: oversizedArchive,
                    bridgeAvailable: false
                ) {
                    XCTFail("native compact-token prover body must not run for oversized record bundles")
                    return nil
                }
        ) { error in
            XCTAssertEqual(
                error as? KagemushaCompactPaymentTokenProverError,
                .oversizedRecordBundleArchive
            )
            XCTAssertTrue(
                error.localizedDescription.contains(
                    "must not exceed \(KagemushaRecursiveSpendProver.nativeArchiveMaxBytes) bytes"
                )
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
        let validArchive = validKagemushaNoritoArchive()
        for (label, nativeOutput) in malformedKagemushaNoritoArchives(validArchive) {
            XCTAssertThrowsError(
                try KagemushaCompactPaymentTokenProver
                    .proveVerifiedCompactPaymentTokenWithRecords(
                        recordBundleArchive: validArchive,
                        bridgeAvailable: true
                    ) {
                        nativeOutput
                    },
                "native output \(label) should be rejected"
            ) { error in
                XCTAssertEqual(
                    error as? KagemushaCompactPaymentTokenProverError,
                    .invalidCompactTokenArchive
                )
            }
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

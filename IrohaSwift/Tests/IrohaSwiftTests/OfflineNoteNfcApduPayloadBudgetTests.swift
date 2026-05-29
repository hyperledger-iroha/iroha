import XCTest
@testable import IrohaSwift

final class OfflineNoteNfcApduPayloadBudgetTests: XCTestCase {
    func testNfcApduAcceptsWalletDeviceTransferBudget() throws {
        XCTAssertEqual(OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes, 64 * 1024)

        let payload = Data(repeating: 0xA5, count: OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes)
        let infoBytes = try OfflineNoteNfcApduProtocol.encodeInfo(
            kind: .paymentToken,
            payloadBytes: payload
        )
        let info = try XCTUnwrap(OfflineNoteNfcApduProtocol.decodeInfo(infoBytes))

        XCTAssertEqual(info.payloadLength, payload.count)
        XCTAssertTrue(OfflineNoteNfcApduProtocol.payloadDigestMatches(payload, expectedSha256: info.sha256))
    }

    func testNfcApduRejectsPayloadsAboveWalletDeviceTransferBudget() throws {
        let payload = Data(repeating: 0x5A, count: OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes + 1)

        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.encodeInfo(
                kind: .paymentToken,
                payloadBytes: payload
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidPayloadLength)
        }
    }
}

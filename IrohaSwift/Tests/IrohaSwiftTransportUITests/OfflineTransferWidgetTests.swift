import Foundation
import XCTest
import IrohaSwift
import IrohaSwiftMobileTransports
import IrohaSwiftTransferUI

final class OfflineTransferWidgetTests: XCTestCase {
    func testNfcConfigurationBuildsCustomSelectAidApdu() {
        let aid = Data([0xF0, 0x50, 0x4B, 0x45, 0x50, 0x4B, 0x52, 0x4E, 0x46, 0x43, 0x01])
        let configuration = IrohaOfflineNfcConfiguration(applicationIdentifier: aid)

        XCTAssertEqual(configuration.applicationIdentifierHex, "F0504B45504B524E464301")
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(configuration.selectAidAPDUData(), aid: aid),
            .select
        )
    }

    func testPreparedPaymentRetryClassificationKeepsOnlyTransientFailuresOpen() {
        XCTAssertTrue(IrohaOfflineNfcExchangeError.ackPending.shouldRetryPreparedPaymentTransfer)
        XCTAssertTrue(IrohaOfflineNfcExchangeError.nfcTimeout.shouldRetryPreparedPaymentTransfer)
        XCTAssertTrue(IrohaOfflineNfcExchangeError.peerRejected(statusWord: nil).shouldRetryPreparedPaymentTransfer)
        XCTAssertTrue(IrohaOfflineNfcExchangeError.peerRejected(statusWord: 0x6985).shouldRetryPreparedPaymentTransfer)

        XCTAssertFalse(IrohaOfflineNfcExchangeError.peerRejected(statusWord: 0x6A80).shouldRetryPreparedPaymentTransfer)
        XCTAssertFalse(IrohaOfflineNfcExchangeError.invalidPayload.shouldRetryPreparedPaymentTransfer)
        XCTAssertFalse(IrohaOfflineNfcExchangeError.checksumMismatch.shouldRetryPreparedPaymentTransfer)
    }

    func testNfcErrorTechnicalCodesDoNotExposePayloads() {
        XCTAssertEqual(
            IrohaOfflineNfcExchangeError.peerRejected(statusWord: 0x6985).technicalCode,
            "IrohaOfflineNfcExchangeError.peerRejected.6985"
        )
        XCTAssertEqual(
            IrohaOfflineNfcExchangeError.peerRejected(statusWord: nil).technicalCode,
            "IrohaOfflineNfcExchangeError.peerRejected.nil"
        )
    }

    func testFountainPayloadFramesKeepSmallPayloadSingleFrame() {
        let payload = "offline-small-payload"
        XCTAssertEqual(IrohaOfflineFountainPayloadFrames.frames(for: payload), [payload])
    }
}

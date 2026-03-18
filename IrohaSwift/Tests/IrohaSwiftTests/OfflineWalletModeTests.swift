import XCTest
@testable import IrohaSwift

final class OfflineWalletModeTests: XCTestCase {

    func testDefaultsExposeLedgerReconcilableMode() throws {
        let client = ToriiClient(baseURL: URL(string: "https://example.invalid")!)
        let wallet = try OfflineWallet(toriiClient: client)

        XCTAssertEqual(wallet.circulationMode, .ledgerReconcilable)
        XCTAssertTrue(wallet.requiresLedgerReconciliation)
        XCTAssertTrue(wallet.circulationModeNotice().details.contains("Torii"))
    }

    func testModeChangeInvokesHandler() throws {
        let expectation = expectation(description: "mode handler called")
        expectation.expectedFulfillmentCount = 2
        let client = ToriiClient(baseURL: URL(string: "https://example.invalid")!)
        var capturedMode: OfflineWalletCirculationMode?
        var capturedNotice: OfflineWalletCirculationNotice?
        let wallet = try OfflineWallet(
            toriiClient: client,
            circulationMode: .ledgerReconcilable,
            onCirculationModeChange: { mode, notice in
                capturedMode = mode
                capturedNotice = notice
                expectation.fulfill()
            })

        wallet.setCirculationMode(.offlineOnly)

        wait(for: [expectation], timeout: 1.0)
        XCTAssertEqual(wallet.circulationMode, .offlineOnly)
        XCTAssertFalse(wallet.requiresLedgerReconciliation)
        XCTAssertEqual(capturedMode, .offlineOnly)
        XCTAssertEqual(capturedNotice?.headline, OfflineWalletCirculationMode.offlineOnly.notice.headline)
    }
}

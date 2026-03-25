import XCTest
#if canImport(Combine)
import Combine
#endif
@testable import IrohaSwift

@available(iOS 15.0, macOS 12.0, *)
final class ConnectSessionBalanceTests: XCTestCase {
    private let encodedUsdAssetID =
        "5ywNgSPQ5KyuQh7SwaZmwMW4GTXu#6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"

    func testBalanceStreamFiltersAccount() async throws {
        let (session, tempURL) = makeSessionWithEvents()
        addTeardownBlock {
            try? FileManager.default.removeItem(at: tempURL)
        }

        let stream = session.balanceStream(accountID: "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn")
        var iterator = stream.makeAsyncIterator()
        let first = try await iterator.next()
        XCTAssertEqual(first?.accountID, "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn")
        XCTAssertEqual(first?.sequence, 1)
        XCTAssertEqual(first?.assets.first?.assetId, encodedUsdAssetID)
        XCTAssertEqual(first?.queueDiagnostics?.state, .healthy)

        let second = try await iterator.next()
        XCTAssertNil(second, "Non-matching account snapshots should be filtered")
    }

#if canImport(Combine)
    @MainActor
    func testBalancePublisherBridgesStream() throws {
        let expectation = expectation(description: "publisher completes")
        let (session, tempURL) = makeSessionWithEvents()
        addTeardownBlock {
            try? FileManager.default.removeItem(at: tempURL)
        }

        var received: [ConnectBalanceSnapshot] = []
        let cancellable = session
            .balancePublisher(accountID: "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn", scheduler: nil)
            .sink(receiveCompletion: { completion in
                if case .failure(let error) = completion {
                    XCTFail("Unexpected error \(error)")
                }
                expectation.fulfill()
            }, receiveValue: { snapshot in
                received.append(snapshot)
            })

        wait(for: [expectation], timeout: 1)
        cancellable.cancel()
        XCTAssertEqual(received.count, 1)
        XCTAssertEqual(received.first?.assets.first?.quantity, "10")
    }
#endif

    private func makeSessionWithEvents() -> (ConnectSession, URL) {
        let tempRoot = FileManager.default.temporaryDirectory
            .appendingPathComponent("connect-balances-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)

        let sessionID = Data(repeating: 0x44, count: 32)
        let diagnostics = ConnectSessionDiagnostics(sessionID: sessionID,
                                                    configuration: .init(rootDirectory: tempRoot))
        let tracker = ConnectQueueStateTracker(sessionID: sessionID,
                                               configuration: .init(rootDirectory: tempRoot))
        try? tracker.updateSnapshot { snapshot in
            snapshot.state = .healthy
            snapshot.appToWallet.depth = 2
            snapshot.walletToApp.depth = 1
        }

        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.local")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)

        let events = [
            ConnectEvent(sequence: 1,
                         direction: .walletToApp,
                         payload: .balanceSnapshot(ConnectBalanceSnapshot(accountID: "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
                                                                          assets: [ConnectBalanceAsset(assetId: encodedUsdAssetID,
                                                                                                       quantity: "10")],
                                                                          lastUpdatedMs: 1))),
            ConnectEvent(sequence: 2,
                         direction: .walletToApp,
                         payload: .balanceSnapshot(ConnectBalanceSnapshot(accountID: "6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9",
                                                                          assets: [ConnectBalanceAsset(assetId: encodedUsdAssetID,
                                                                                                       quantity: "20")],
                                                                          lastUpdatedMs: 2)))
        ]

        let builder: ConnectSession.ConnectEventStreamBuilder = { _, filter in
            AsyncThrowingStream { continuation in
                Task {
                    for event in events where filter.matches(event) {
                        continuation.yield(event)
                    }
                    continuation.finish()
                }
            }
        }

        let session = ConnectSession(sessionID: sessionID,
                                     client: client,
                                     diagnostics: diagnostics,
                                     eventStreamBuilder: builder)
        return (session, tempRoot)
    }
}

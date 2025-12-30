import XCTest
@testable import IrohaSwift

final class ConnectAsyncSequenceTests: XCTestCase {
    func testSequenceYieldsFrames() async throws {
        let sessionID = Data(repeating: 0x11, count: 32)
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCodecAvailable,
                      "NoritoBridge connect codec unavailable")
        _ = try ConnectCodec.encode(ConnectFrame(sessionID: sessionID,
                                                 direction: .walletToApp,
                                                 sequence: 0,
                                                 kind: .control(.ping(ConnectPing(nonce: 0)))))
        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: ConnectWebSocketFactory { _ in stub })
        await client.start()

        let sequence = ConnectFrameSequence(client: client)
        let expectation = expectation(description: "frames")
        expectation.expectedFulfillmentCount = 2

        Task {
            var iterator = sequence.makeAsyncIterator()
            let first = try await iterator.next()
            XCTAssertEqual(first?.sequence, 1)
            expectation.fulfill()
            let second = try await iterator.next()
            XCTAssertEqual(second?.sequence, 2)
            expectation.fulfill()
        }

        for seq in 1...2 {
            while stub.pendingReceives.isEmpty {
                try await Task.sleep(nanoseconds: 1_000_000)
            }
            let frame = ConnectFrame(sessionID: sessionID,
                                     direction: .walletToApp,
                                     sequence: UInt64(seq),
                                     kind: .control(.ping(ConnectPing(nonce: UInt64(seq)))))
            let data = try ConnectCodec.encode(frame)
            stub.emit(data)
        }

        await fulfillment(of: [expectation], timeout: 1)
    }
}

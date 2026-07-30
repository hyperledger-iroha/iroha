import XCTest
@testable import IrohaSwift

@available(iOS 15.0, macOS 12.0, *)
final class ConnectSessionEventStreamTests: XCTestCase {
    func testEventStreamYieldsCiphertextEnvelope() async throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isConnectCryptoAvailable,
            "NoritoBridge connect crypto unavailable"
        )
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isConnectCodecAvailable,
            "NoritoBridge connect codec unavailable"
        )

        let stub = StubWebSocketTask()
        let client = ConnectClient(url: URL(string: "wss://example.test")!,
                                   webSocketFactory: StubWebSocketFactory(task: stub).factory)
        await client.start()

        let sessionID = Data(repeating: 0x11, count: 32)
        let walletKey = Data(repeating: 0x22, count: 32)
        let keys = ConnectDirectionKeys(appToWallet: Data(repeating: 0x33, count: 32),
                                        walletToApp: walletKey)
        let session = ConnectSession(sessionID: sessionID,
                                     client: client,
                                     directionKeys: keys)

        let stream = session.eventStream()
        let expectation = expectation(description: "connect event")
        expectation.expectedFulfillmentCount = 1
        var receivedEvent: ConnectEvent?

        Task {
            var iterator = stream.makeAsyncIterator()
            receivedEvent = try? await iterator.next()
            expectation.fulfill()
        }

        let expectedSignature = try Self.validEd25519Signature(message: "connect event stream")
        let frameBytes = try makeEncryptedSignResultFrame(sequence: 9,
                                                          sessionID: sessionID,
                                                          key: walletKey,
                                                          signature: expectedSignature)
        stub.emit(frameBytes)

        await fulfillment(of: [expectation], timeout: 2)

        let event = try XCTUnwrap(receivedEvent)
        if case .signResultOk(let signature) = event.payload {
            XCTAssertEqual(event.sequence, 9)
            XCTAssertEqual(event.direction, .walletToApp)
            XCTAssertEqual(signature.signature, expectedSignature)
        } else {
            XCTFail("Expected sign result event")
        }
    }
}

@available(iOS 15.0, macOS 12.0, *)
private func makeEncryptedSignResultFrame(sequence: UInt64,
                                          sessionID: Data,
                                          key: Data,
                                          signature: Data) throws -> Data {
    try ConnectEnvelopeCodec.encryptSignResultOk(sequence: sequence,
                                                 signature: signature,
                                                 algorithm: "ed25519",
                                                 key: key,
                                                 sessionID: sessionID,
                                                 direction: .walletToApp)
}

@available(iOS 15.0, macOS 12.0, *)
private extension ConnectSessionEventStreamTests {
    static func validEd25519Signature(message: String) throws -> Data {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x63, count: 32))
        return try signingKey.sign(Data(message.utf8))
    }
}
